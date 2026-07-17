use core::fmt::Write;
use crate::pexpert::gop::Screen;

const MAX_LINES: usize = 64;
const MAX_MSG_LEN: usize = 120;

static mut LOG_BUF: [[u8; MAX_MSG_LEN]; MAX_LINES] = [[0u8; MAX_MSG_LEN]; MAX_LINES];
static mut LOG_COUNT: usize = 0;
static mut LOG_HEAD: usize = 0;

static mut SCREEN_READY: bool = false;
static mut FB_BASE: usize = 0;
static mut FB_W: usize = 0;
static mut FB_H: usize = 0;
static mut FB_STRIDE: usize = 0;

pub unsafe fn init_screen(screen: &Screen) {
    let info = screen.info();
    FB_BASE = info.base;
    FB_W = info.width;
    FB_H = info.height;
    FB_STRIDE = info.stride;
    SCREEN_READY = true;
    flush();
}

pub fn log(msg: &str) {
    unsafe {
        let idx = (LOG_HEAD + LOG_COUNT) % MAX_LINES;
        let len = msg.len().min(MAX_MSG_LEN - 1);
        let dst = &mut LOG_BUF[idx][..len];
        let src = msg.as_bytes();
        dst.copy_from_slice(&src[..len]);
        LOG_BUF[idx][len] = 0;

        if LOG_COUNT < MAX_LINES {
            LOG_COUNT += 1;
        } else {
            LOG_HEAD = (LOG_HEAD + 1) % MAX_LINES;
        }

        if SCREEN_READY {
            draw_last_line(idx);
        }
    }
}

pub fn log_fmt(args: core::fmt::Arguments) {
    struct FmtWriter {
        buf: [u8; MAX_MSG_LEN],
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
    let mut w = FmtWriter { buf: [0u8; MAX_MSG_LEN], pos: 0 };
    let _ = write!(w, "{}", args);
    let s = core::str::from_utf8(&w.buf[..w.pos]).unwrap_or("?");
    log(s);
}

unsafe fn flush() {
    if !SCREEN_READY || FB_BASE == 0 { return; }

    // Dark background
    let bg = 0x00000000u32;
    let v = if cfg!(target_endian = "little") { bg } else { bg.swap_bytes() };
    for y in 0..FB_H {
        for x in 0..FB_W {
            let off = (y * FB_STRIDE + x) * 4;
            core::ptr::write_volatile((FB_BASE + off) as *mut u32, v);
        }
    }

    // Draw all buffered lines
    let line_h = 16usize;
    let margin = 10usize;
    let mut y = margin;
    for i in 0..LOG_COUNT {
        let idx = (LOG_HEAD + i) % MAX_LINES;
        let len = LOG_BUF[idx].iter().position(|&b| b == 0).unwrap_or(MAX_MSG_LEN);
        if len == 0 { continue; }
        if let Ok(s) = core::str::from_utf8(&LOG_BUF[idx][..len]) {
            draw_text_bitmap(margin, y, s);
            y += line_h;
            if y + line_h > FB_H { break; }
        }
    }
}

unsafe fn draw_last_line(idx: usize) {
    if !SCREEN_READY || FB_BASE == 0 { return; }

    let line_h = 16usize;
    let margin = 10usize;

    // Count lines visible on screen
    let max_lines = (FB_H - margin) / line_h;
    let visible_idx = if LOG_COUNT <= max_lines {
        idx
    } else {
        let offset = LOG_COUNT - max_lines;
        (LOG_HEAD + offset) % MAX_LINES
    };

    let screen_line = if LOG_COUNT <= max_lines {
        LOG_COUNT - 1
    } else {
        max_lines - 1
    };

    let y = margin + screen_line * line_h;

    // Clear line area
    let bg = 0x00000000u32;
    let v = if cfg!(target_endian = "little") { bg } else { bg.swap_bytes() };
    for py in y..(y + line_h).min(FB_H) {
        for x in 0..FB_W {
            let off = (py * FB_STRIDE + x) * 4;
            core::ptr::write_volatile((FB_BASE + off) as *mut u32, v);
        }
    }

    let len = LOG_BUF[visible_idx].iter().position(|&b| b == 0).unwrap_or(MAX_MSG_LEN);
    if len > 0 {
        if let Ok(s) = core::str::from_utf8(&LOG_BUF[visible_idx][..len]) {
            draw_text_bitmap(margin, y, s);
        }
    }
}

fn draw_text_bitmap(mut x: usize, y: usize, s: &str) {
    use crate::libkern::font::{self, GLYPH_H, GLYPH_W};
    let fg = 0x00FFFFFFu32; // white in BGRA
    for &b in s.as_bytes() {
        if x + GLYPH_W > unsafe { FB_W } { break; }
        if b >= 0x20 && b <= 0x7E {
            let glyph = font::glyph(b);
            for row in 0..GLYPH_H {
                let py = y + row;
                if py >= unsafe { FB_H } { break; }
                let bits = glyph[row];
                for col in 0..GLYPH_W {
                    if (bits >> (7 - col)) & 1 == 1 {
                        let px = x + col;
                        let off = (py * unsafe { FB_STRIDE } + px) * 4;
                        unsafe {
                            core::ptr::write_volatile((FB_BASE + off) as *mut u32, fg);
                        }
                    }
                }
            }
        }
        x += GLYPH_W;
    }
}
