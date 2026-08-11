use core::fmt::Write;
use core::ptr;

use baram_core::Color;
use uefi::boot;
use uefi::proto::console::serial::Serial;

static mut FB_BASE: usize = 0;
static mut FB_W: usize = 0;
static mut FB_H: usize = 0;
static mut FB_STRIDE: usize = 0;
static mut FB_PF_RGB: bool = true;

pub unsafe fn init_from_screen(screen: &baram_core::Screen) {
    let info = screen.info();
    FB_BASE = info.base;
    FB_W = info.width;
    FB_H = info.height;
    FB_STRIDE = info.stride;
    FB_PF_RGB = matches!(info.pixel_format, uefi::proto::console::gop::PixelFormat::Rgb);
}

unsafe fn put_pixel(x: usize, y: usize, c: Color) {
    if FB_BASE == 0 || x >= FB_W || y >= FB_H {
        return;
    }
    let v = if FB_PF_RGB {
        ((c.b() as u32) << 16) | ((c.g() as u32) << 8) | (c.r() as u32)
    } else {
        ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | (c.b() as u32)
    };
    let off = (y * FB_STRIDE + x) * 4;
    ptr::write_volatile((FB_BASE + off) as *mut u32, v);
}

unsafe fn fill_screen(c: Color) {
    if FB_BASE == 0 {
        return;
    }
    let v = if FB_PF_RGB {
        ((c.b() as u32) << 16) | ((c.g() as u32) << 8) | (c.r() as u32)
    } else {
        ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | (c.b() as u32)
    };
    for y in 0..FB_H {
        for x in 0..FB_W {
            let off = (y * FB_STRIDE + x) * 4;
            ptr::write_volatile((FB_BASE + off) as *mut u32, v);
        }
    }
}

fn draw_ttf(mut x: usize, y: usize, s: &str, fg: Color) {
    draw_ttf_scaled(x, y, s, fg, 14.0);
}

fn blend_pixel(x: usize, y: usize, fg: Color, alpha: u8) {
    if alpha == 0 {
        return;
    }
    if alpha == 255 {
        unsafe { put_pixel(x, y, fg); }
        return;
    }
    let a = alpha as u32;
    let inv = 255 - a;
    let bg = unsafe {
        if FB_BASE == 0 || x >= FB_W || y >= FB_H {
            return;
        }
        let off = (y * FB_STRIDE + x) * 4;
        let v = ptr::read_volatile((FB_BASE + off) as *const u32);
        if FB_PF_RGB {
            Color::rgb(((v >> 16) & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8)
        } else {
            Color::rgb((v & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, ((v >> 16) & 0xFF) as u8)
        }
    };
    let r = ((fg.r() as u32 * a + bg.r() as u32 * inv) / 255) as u8;
    let g = ((fg.g() as u32 * a + bg.g() as u32 * inv) / 255) as u8;
    let b = ((fg.b() as u32 * a + bg.b() as u32 * inv) / 255) as u8;
    unsafe { put_pixel(x, y, Color::rgb(r, g, b)); }
}

fn draw_ttf_scaled(mut x: usize, y: usize, s: &str, fg: Color, pixel_size: f32) {
    if !baram_font::ttf_font::is_available() {
        for &b in s.as_bytes() {
            if b >= 0x20 && b <= 0x7E {
                draw_bitmap_char(x, y, b, fg);
            }
            x += baram_font::font::GLYPH_W;
        }
        return;
    }
    let asc = baram_font::ttf_font::ascent_at_size(pixel_size);
    for ch in s.chars() {
        let g = baram_font::ttf_font::glyph_at_size(ch, pixel_size);
        if g.w > 0 && g.h > 0 {
            let baseline = y as i32 + asc;
            for row in 0..g.h {
                let py = baseline + g.y_off + row;
                if py < 0 {
                    continue;
                }
                for col in 0..g.w {
                    let px = x as i32 + col;
                    if px < 0 {
                        continue;
                    }
                    let alpha = g.data[(row * g.w + col) as usize];
                    blend_pixel(px as usize, py as usize, fg, alpha);
                }
            }
            x += g.advance.max(0) as usize;
        } else {
            x += baram_font::font::GLYPH_W;
        }
    }
}

fn draw_bitmap_char(x: usize, y: usize, c: u8, fg: Color) {
    use baram_font::font::{self, GLYPH_H, GLYPH_W};
    let glyph = font::glyph(c);
    for row in 0..GLYPH_H {
        let bits = glyph[row];
        for col in 0..GLYPH_W {
            if (bits >> (7 - col)) & 1 == 1 {
                unsafe {
                    put_pixel(x + col, y + row, fg);
                }
            }
        }
    }
}

fn draw_info_icon(ox: usize, oy: usize, size: usize, fg: Color) {
    let s = size as f32;
    let cx = s / 2.0;
    let cy = s / 2.0;
    let outer_r = s * 10.0 / 22.0;
    let inner_r = s * 8.0 / 22.0;
    let ring_w = outer_r - inner_r;

    for py in 0..size {
        for px in 0..size {
            let x = px as f32 + 0.5;
            let y = py as f32 + 0.5;
            let dx = x - cx;
            let dy = y - cy;
            let dist = libm::sqrtf(dx * dx + dy * dy);

            let mut alpha: f32 = 0.0;

            if dist >= inner_r - 0.5 && dist <= outer_r + 0.5 {
                let a_outer = (outer_r + 0.5 - dist).max(0.0).min(1.0);
                let a_inner = (dist - inner_r + 0.5).max(0.0).min(1.0);
                alpha = a_outer.min(a_inner);
            }

            let bx = x / s * 22.0;
            let by = y / s * 22.0;

            let bar_cx = 11.0;
            let bar_hw = ring_w / s * 22.0 / 2.0;
            if bx >= bar_cx - bar_hw && bx <= bar_cx + bar_hw && by >= 5.5 && by <= 12.0 {
                alpha = 1.0;
            }

            let dot_cx = 11.0;
            let dot_cy = 16.0;
            let dot_hw = ring_w / s * 22.0 / 2.0;
            if bx >= dot_cx - dot_hw && bx <= dot_cx + dot_hw && by >= dot_cy - dot_hw && by <= dot_cy + dot_hw {
                alpha = 1.0;
            }

            if alpha > 0.0 {
                let a = (alpha * 255.0) as u8;
                blend_pixel(ox + px, oy + py, fg, a);
            }
        }
    }
}

struct FmtWriter {
    buf: [u8; 512],
    pos: usize,
}

fn serial_log(message: &str) {
    let Ok(handle) = boot::get_handle_for_protocol::<Serial>() else {
        return;
    };
    let Ok(mut serial) = boot::open_protocol_exclusive::<Serial>(handle) else {
        return;
    };
    let _ = serial.write(message.as_bytes());
}

impl Write for FmtWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            if self.pos < self.buf.len() {
                self.buf[self.pos] = b;
                self.pos += 1;
            }
        }
        Ok(())
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let mut serial = FmtWriter {
        buf: [0u8; 512],
        pos: 0,
    };
    let _ = write!(serial, "BARAM PANIC: {}\r\n", info.message());
    if let Some(location) = info.location() {
        let _ = write!(serial, "at {}:{}\r\n", location.file(), location.line());
    }
    if serial.pos > 0 {
        if let Ok(message) = core::str::from_utf8(&serial.buf[..serial.pos]) {
            serial_log(message);
        }
    }
    unsafe {
        fill_screen(Color::rgb(0, 0, 0));
    }

    let margin = 80usize;

    let white = Color::rgb(255, 255, 255);
    let gray = Color::rgb(0xAA, 0xAA, 0xAA);

    let icon_size = 56usize;
    draw_info_icon(margin, 80, icon_size, white);

    let title = "問題が発生したためシステムが停止しました";
    draw_ttf_scaled(margin, 80 + icon_size + 20, title, white, 42.0);

    let detail_y = 80 + icon_size + 20 + 60;

    let mut fw = FmtWriter {
        buf: [0u8; 512],
        pos: 0,
    };
    let _ = write!(fw, "{}", info.message());
    if fw.pos > 0 {
        if let Ok(msg) = core::str::from_utf8(&fw.buf[..fw.pos]) {
            draw_ttf_scaled(margin, detail_y, msg, gray, 18.0);
        }
    }

    if let Some(loc) = info.location() {
        let mut fw2 = FmtWriter {
            buf: [0u8; 512],
            pos: 0,
        };
        let _ = write!(fw2, "{}:{}", loc.file(), loc.line());
        if fw2.pos > 0 {
            if let Ok(loc_s) = core::str::from_utf8(&fw2.buf[..fw2.pos]) {
                draw_ttf_scaled(margin, detail_y + 40, loc_s, gray, 18.0);
            }
        }
    }

    loop {
        uefi::boot::stall(core::time::Duration::from_millis(100));
    }
}
