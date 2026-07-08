use core::fmt::Write;
use core::ptr;

use crate::gop::Color;

static mut FB_BASE: usize = 0;
static mut FB_W: usize = 0;
static mut FB_H: usize = 0;
static mut FB_STRIDE: usize = 0;
static mut FB_PF_RGB: bool = true;

pub unsafe fn init_from_screen(screen: &crate::gop::Screen) {
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
    if !crate::ttf_font::is_available() {
        for &b in s.as_bytes() {
            if b >= 0x20 && b <= 0x7E {
                draw_bitmap_char(x, y, b, fg);
            }
            x += crate::font::GLYPH_W;
        }
        return;
    }
    let asc = crate::ttf_font::ascent();
    for ch in s.chars() {
        let g = crate::ttf_font::glyph(ch);
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
                    if alpha > 0 {
                        unsafe {
                            put_pixel(px as usize, py as usize, fg);
                        }
                    }
                }
            }
            x += g.advance.max(0) as usize;
        } else {
            x += crate::font::GLYPH_W;
        }
    }
}

fn draw_bitmap_char(x: usize, y: usize, c: u8, fg: Color) {
    use crate::font::{self, GLYPH_H, GLYPH_W};
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

struct FmtWriter {
    buf: [u8; 512],
    pos: usize,
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
    unsafe {
        fill_screen(Color::rgb(200, 0, 0));
    }

    let w = unsafe { FB_W };

    let title = "例外によりシステムが停止しました";

    let title_w = if crate::ttf_font::is_available() {
        let mut tw = 0i32;
        for ch in title.chars() {
            let g = crate::ttf_font::glyph(ch);
            tw += g.advance.max(0);
        }
        tw as usize
    } else {
        title.len() * crate::font::GLYPH_W
    };

    let margin = 40usize;
    let tx = w.saturating_sub(title_w + margin);
    let ty = 40usize;
    draw_ttf(tx, ty, title, Color::BLACK);

    let mut fw = FmtWriter {
        buf: [0u8; 512],
        pos: 0,
    };
    let _ = write!(fw, "{}", info.message());
    if fw.pos > 0 {
        if let Ok(msg) = core::str::from_utf8(&fw.buf[..fw.pos]) {
            draw_ttf(tx, ty + 50, msg, Color::BLACK);
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
                draw_ttf(tx, ty + 90, loc_s, Color::BLACK);
            }
        }
    }

    loop {
        uefi::boot::stall(core::time::Duration::from_secs(1));
    }
}
