use core::ptr;
use uefi::boot::{self, ScopedProtocol};
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::Status;
use crate::color::Color;

#[inline]
unsafe fn copy_swap_rb(src: *const u32, dst: *mut u32, len: usize) {
    #[cfg(target_arch = "x86_64")]
    {
        use core::arch::x86_64::*;
        let keep = _mm_set1_epi32(0xff00_ff00u32 as i32);
        let red = _mm_set1_epi32(0x00ff_0000);
        let blue = _mm_set1_epi32(0x0000_00ff);
        let mut i = 0usize;
        while i + 4 <= len {
            let p = _mm_loadu_si128(src.add(i) as *const __m128i);
            let out = _mm_or_si128(
                _mm_and_si128(p, keep),
                _mm_or_si128(
                    _mm_srli_epi32(_mm_and_si128(p, red), 16),
                    _mm_slli_epi32(_mm_and_si128(p, blue), 16),
                ),
            );
            _mm_storeu_si128(dst.add(i) as *mut __m128i, out);
            i += 4;
        }
        for i in i..len {
            let p = *src.add(i);
            *dst.add(i) = (p & 0xff00_ff00)
                | ((p & 0x00ff_0000) >> 16)
                | ((p & 0x0000_00ff) << 16);
        }
        return;
    }

    #[cfg(target_arch = "aarch64")]
    {
        use core::arch::aarch64::*;
        let keep = vdupq_n_u32(0xff00_ff00);
        let red = vdupq_n_u32(0x00ff_0000);
        let blue = vdupq_n_u32(0x0000_00ff);
        let mut i = 0usize;
        while i + 4 <= len {
            let p = vld1q_u32(src.add(i));
            let out = vorrq_u32(
                vandq_u32(p, keep),
                vorrq_u32(
                    vshrq_n_u32(vandq_u32(p, red), 16),
                    vshlq_n_u32(vandq_u32(p, blue), 16),
                ),
            );
            vst1q_u32(dst.add(i), out);
            i += 4;
        }
        for i in i..len {
            let p = *src.add(i);
            *dst.add(i) = (p & 0xff00_ff00)
                | ((p & 0x00ff_0000) >> 16)
                | ((p & 0x0000_00ff) << 16);
        }
        return;
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    for i in 0..len {
        let p = *src.add(i);
        *dst.add(i) = (p & 0xff00_ff00)
            | ((p & 0x00ff_0000) >> 16)
            | ((p & 0x0000_00ff) << 16);
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct FramebufferInfo {
    pub base: usize,
    pub size: usize,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub pixel_format: PixelFormat,
}

pub struct Screen {
    info: FramebufferInfo,
    fb_ptr: *mut u8,
    _gop: ScopedProtocol<GraphicsOutput>,
}

unsafe impl Send for Screen {}
unsafe impl Sync for Screen {}

impl Screen {
    pub fn take() -> Result<Screen, Status> {
        let handle = boot::get_handle_for_protocol::<GraphicsOutput>()
            .map_err(|_| Status::UNSUPPORTED)?;
        let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(handle)
            .map_err(|_| Status::UNSUPPORTED)?;

        // The compositor is tuned for a 720p working set.  Picking the
        // firmware's largest mode (often 4K) multiplies every software blend
        // and framebuffer write by up to 9x with no UI benefit.
        const TARGET_W: usize = 1280;
        const TARGET_H: usize = 720;
        let mut best_score = usize::MAX;
        let mut best_mode: Option<uefi::proto::console::gop::Mode> = None;
        for mode in gop.modes() {
            let (w, h) = mode.info().resolution();
            let area_delta = w.abs_diff(TARGET_W)
                .saturating_mul(TARGET_H)
                .saturating_add(h.abs_diff(TARGET_H).saturating_mul(TARGET_W));
            let aspect_delta = w.saturating_mul(TARGET_H)
                .abs_diff(h.saturating_mul(TARGET_W));
            let undersized_penalty = if w < TARGET_W || h < TARGET_H {
                usize::MAX / 4
            } else {
                0
            };
            let score = undersized_penalty
                .saturating_add(area_delta)
                .saturating_add(aspect_delta.saturating_mul(4));
            if score < best_score {
                best_score = score;
                best_mode = Some(mode);
            }
        }
        if let Some(mode) = best_mode {
            let _ = gop.set_mode(&mode);
        }

        let info = gop.current_mode_info();
        let (w, h) = info.resolution();
        let stride = info.stride();
        let pf = info.pixel_format();

        let (fb_base, fb_size) = {
            let mut fb = gop.frame_buffer();
            (fb.as_mut_ptr() as usize, fb.size())
        };

        Ok(Screen {
            info: FramebufferInfo {
                base: fb_base,
                size: fb_size,
                width: w,
                height: h,
                stride,
                pixel_format: pf,
            },
            fb_ptr: fb_base as *mut u8,
            _gop: gop,
        })
    }

    pub fn width(&self) -> usize { self.info.width }
    pub fn height(&self) -> usize { self.info.height }
    #[allow(dead_code)]
    pub fn info(&self) -> FramebufferInfo { self.info }

    pub fn clear(&mut self, c: Color) {
        self.fill_rect(0, 0, self.info.width, self.info.height, c);
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: Color) {
        let x0 = x.min(self.info.width);
        let y0 = y.min(self.info.height);
        let x1 = (x + w).min(self.info.width);
        let y1 = (y + h).min(self.info.height);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for yy in y0..y1 {
            self.fill_line(x0, yy, x1, c);
        }
    }

    fn fill_line(&mut self, x0: usize, y: usize, x1: usize, c: Color) {
        let pf = self.info.pixel_format;
        let stride = self.info.stride;
        let base = self.fb_ptr;
        let n = x1.saturating_sub(x0);
        if n == 0 { return; }
        let v = match pf {
            PixelFormat::Rgb => ((c.b() as u32) << 16) | ((c.g() as u32) << 8) | (c.r() as u32),
            PixelFormat::Bgr => ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | (c.b() as u32),
            PixelFormat::Bitmask => c.0,
            _ => c.0,
        };
        let off = (y * stride + x0) * 4;
        const CHUNK: usize = 64;
        let mut remaining = n;
        let mut offset = 0usize;
        while remaining > 0 {
            let chunk = remaining.min(CHUNK);
            let tmp = [v; CHUNK];
            unsafe {
                ptr::copy_nonoverlapping(tmp.as_ptr(), base.add(off + offset * 4) as *mut u32, chunk);
            }
            offset += chunk;
            remaining -= chunk;
        }
    }

    pub fn read_pixel(&self, x: usize, y: usize) -> Color {
        if x >= self.info.width || y >= self.info.height {
            return Color::BLACK;
        }
        let stride = self.info.stride;
        let base = self.fb_ptr;
        let off = (y * stride + x) * 4;
        let v = unsafe { ptr::read_volatile(base.add(off) as *const u32) };
        match self.info.pixel_format {
            PixelFormat::Rgb => Color::rgb((v & 0xFF) as u8,
                                           ((v >>  8) & 0xFF) as u8,
                                           ((v >> 16) & 0xFF) as u8),
            PixelFormat::Bgr => Color::rgb(((v >> 16) & 0xFF) as u8,
                                           ((v >>  8) & 0xFF) as u8,
                                           (v & 0xFF) as u8),
            _ => Color(v),
        }
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, c: Color) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }
        let pf = self.info.pixel_format;
        let stride = self.info.stride;
        let base = self.fb_ptr;
        let v = match pf {
            PixelFormat::Rgb => ((c.b() as u32) << 16) | ((c.g() as u32) << 8) | (c.r() as u32),
            PixelFormat::Bgr => ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | (c.b() as u32),
            PixelFormat::Bitmask => c.0,
            _ => c.0,
        };
        let off = (y * stride + x) * 4;
        unsafe {
            ptr::write_volatile(base.add(off) as *mut u32, v);
        }
    }

    pub fn flush_layer_row(&mut self, y: usize, row: &[u32]) {
        self.flush_layer_row_range(y, 0, row);
    }

    pub fn flush_layer_row_range(&mut self, y: usize, x_offset: usize, row: &[u32]) {
        if y >= self.info.height || x_offset >= self.info.width { return; }
        let pf = self.info.pixel_format;
        let stride = self.info.stride;
        let base = self.fb_ptr;
        let n = row.len().min(self.info.width.saturating_sub(x_offset));
        let off_base = (y * stride + x_offset) * 4;
        match pf {
            PixelFormat::Rgb => {
                unsafe {
                    copy_swap_rb(
                        row.as_ptr(),
                        base.add(off_base) as *mut u32,
                        n,
                    );
                }
            }
            _ => {
                unsafe {
                    ptr::copy_nonoverlapping(row.as_ptr(), base.add(off_base) as *mut u32, n);
                }
            }
        }
    }

    pub fn rect_outline(&mut self, x: usize, y: usize, w: usize, h: usize, c: Color) {
        if w == 0 || h == 0 { return; }
        self.fill_rect(x, y, w, 1, c);
        self.fill_rect(x, y + h - 1, w, 1, c);
        self.fill_rect(x, y, 1, h, c);
        self.fill_rect(x + w - 1, y, 1, h, c);
    }
}
