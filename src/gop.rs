//! Wrapper around the UEFI Graphics Output Protocol.
//!
//! This module owns the framebuffer information we need to draw pixels
//! directly to the screen, plus a few convenience primitives (filled
//! rectangles, bitmapped text).  We deliberately avoid depending on
//! helpers for printing so the output is fully under our control.

use core::ptr;
use uefi::boot::{self, ScopedProtocol};
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::Status;


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color(0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }
    pub const fn r(self) -> u8 { ((self.0 >> 16) & 0xFF) as u8 }
    pub const fn g(self) -> u8 { ((self.0 >>  8) & 0xFF) as u8 }
    pub const fn b(self) -> u8 { ((self.0 >>  0) & 0xFF) as u8 }

    pub const BLACK:   Color = Color::rgb(0x00, 0x00, 0x00);
    pub const BG:      Color = Color::rgb(0xF0, 0xF0, 0xF0);
    pub const PANEL:   Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const ACCENT:  Color = Color::rgb(0x00, 0x78, 0xD7);
    pub const TEXT:    Color = Color::rgb(0x1A, 0x1A, 0x1A);
    pub const MUTED:   Color = Color::rgb(0x66, 0x66, 0x66);
    pub const GOOD:    Color = Color::rgb(0x10, 0x7C, 0x10);
    #[allow(dead_code)]
    pub const WARN:    Color = Color::rgb(0xD8, 0x3B, 0x01);
    pub const CURSOR:  Color = Color::rgb(0x1A, 0x1A, 0x1A);
    pub const BORDER:  Color = Color::rgb(0xD0, 0xD0, 0xD0);
    pub const TASKBAR: Color = Color::rgb(0xF0, 0xF0, 0xF0);
    pub const WIN_BG:  Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const WIN_INACTIVE: Color = Color::rgb(0xE8, 0xE8, 0xE8);
    pub const CARD_BG: Color = Color::rgb(0xF5, 0xF5, 0xF5);
    pub const SHADOW:  Color = Color::rgb(0xA0, 0xA0, 0xA0);
    pub const TRANSPARENT: Color = Color(0x0000_0000);
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

        
        let mut best_area: usize = 0;
        let mut best_mode: Option<uefi::proto::console::gop::Mode> = None;
        for mode in gop.modes() {
            let (w, h) = mode.info().resolution();
            let area = w * h;
            if area > best_area {
                best_area = area;
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
        let v = match pf {
            PixelFormat::Rgb => ((c.b() as u32) << 16) | ((c.g() as u32) << 8) | (c.r() as u32),
            PixelFormat::Bgr => ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | (c.b() as u32),
            PixelFormat::Bitmask => c.0,
            _ => c.0,
        };
        for x in x0..x1 {
            let off = (y * stride + x) * 4;
            unsafe {
                ptr::write_volatile(base.add(off) as *mut u32, v);
            }
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
        if y >= self.info.height { return; }
        let pf = self.info.pixel_format;
        let stride = self.info.stride;
        let base = self.fb_ptr;
        let n = row.len().min(self.info.width);
        for x in 0..n {
            let c = Color(row[x]);
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
    }

    pub fn flush_layer_row_range(&mut self, y: usize, x_offset: usize, row: &[u32]) {
        if y >= self.info.height { return; }
        let pf = self.info.pixel_format;
        let stride = self.info.stride;
        let base = self.fb_ptr;
        let n = row.len().min(self.info.width.saturating_sub(x_offset));
        let off_base = (y * stride + x_offset) * 4;
        match pf {
            PixelFormat::Bgr => {
                for x in 0..n {
                    let v = row[x];
                    unsafe {
                        ptr::write_volatile(base.add(off_base + x * 4) as *mut u32, v);
                    }
                }
            }
            PixelFormat::Rgb => {
                for x in 0..n {
                    let c = Color(row[x]);
                    let v = ((c.b() as u32) << 16) | ((c.g() as u32) << 8) | (c.r() as u32);
                    unsafe {
                        ptr::write_volatile(base.add(off_base + x * 4) as *mut u32, v);
                    }
                }
            }
            _ => {
                for x in 0..n {
                    let v = row[x];
                    unsafe {
                        ptr::write_volatile(base.add(off_base + x * 4) as *mut u32, v);
                    }
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
