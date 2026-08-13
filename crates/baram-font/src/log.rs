use core::fmt::Write;

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

pub unsafe fn init_screen(screen: &baram_core::Screen) {
    let info = screen.info();
    FB_BASE = info.base;
    FB_W = info.width;
    FB_H = info.height;
    FB_STRIDE = info.stride;
    SCREEN_READY = true;
}

pub fn log(msg: &str) {
    unsafe {
        let len = msg.len().min(MAX_MSG_LEN - 1);
        let slot = (LOG_HEAD + LOG_COUNT) % MAX_LINES;
        if LOG_COUNT < MAX_LINES {
            LOG_COUNT += 1;
        } else {
            LOG_HEAD = (LOG_HEAD + 1) % MAX_LINES;
        }
        let buf = &mut LOG_BUF[slot];
        buf[..len].copy_from_slice(msg.as_bytes());
        buf[len] = 0;

        if SCREEN_READY {
            draw_log();
        }
    }
}

pub fn log_fmt(args: core::fmt::Arguments) {
    let mut s = alloc::string::String::new();
    let _ = write!(s, "{}", args);
    log(&s);
}

fn draw_log() {
    unsafe {
        if !SCREEN_READY || FB_W == 0 || FB_H == 0 {
            return;
        }

        let line_h = 14;
        let max_lines = (FB_H / line_h).min(LOG_COUNT);
        let start_idx = if LOG_COUNT < MAX_LINES { 0 } else { LOG_HEAD };

        for i in 0..max_lines {
            let idx = (start_idx + i) % MAX_LINES;
            let msg = core::str::from_utf8(&LOG_BUF[idx]).unwrap_or("");
            let y = i * line_h;
            draw_text(msg, 8, y, 0xFFFFFF);
        }
    }
}

fn draw_text(s: &str, x: usize, y: usize, color: u32) {
    unsafe {
        let stride = FB_STRIDE;
        let base = FB_BASE as *mut u32;
        for (i, &b) in s.as_bytes().iter().enumerate() {
            let px = x + i * 8;
            if px + 8 > FB_W || y + 14 > FB_H {
                break;
            }

            let glyph = crate::font::glyph(b);
            for row in 0..14 {
                let bits = if row < 16 { glyph[row] } else { 0 };
                for col in 0..8 {
                    if (bits >> (7 - col)) & 1 == 1 {
                        let dst_y = y + row;
                        let dst_x = px + col;
                        if dst_y < FB_H && dst_x < FB_W {
                            let off = dst_y * stride + dst_x;
                            core::ptr::write_volatile(base.add(off), color);
                        }
                    }
                }
            }
        }
    }
}

pub fn log_line_str(s: &str) {
    log(s);
    log("\n");
}
