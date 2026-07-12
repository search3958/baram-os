use crate::gop::{Color, Screen};
use crate::ttf_font;
use crate::font::{self, GLYPH_H, GLYPH_W};
use crate::vfs;

const SETUP_DONE_PATH: &str = "apps/.setup_done";

pub fn is_setup_done() -> bool {
    !vfs::read_file(SETUP_DONE_PATH).is_empty()
}

pub fn mark_setup_done() {
    vfs::write_file(SETUP_DONE_PATH, b"done");
}

#[derive(Clone, Copy, PartialEq)]
pub enum SetupScreen {
    Welcome,
    Keyboard,
    Done,
}

pub struct SetupWizard {
    pub screen: SetupScreen,
    pub key_detected: bool,
    pub detected_raw_key: u8,
    pub anim_tick: u32,
}

impl SetupWizard {
    pub fn new() -> Self {
        Self {
            screen: SetupScreen::Welcome,
            key_detected: false,
            detected_raw_key: 0,
            anim_tick: 0,
        }
    }

    pub fn on_key(&mut self, ev: &crate::keyboard::KeyEvent) {
        match self.screen {
            SetupScreen::Welcome => {
                if ev.printable == Some(b'\n') || ev.printable == Some(b' ') || ev.scancode == 0x5C {
                    self.screen = SetupScreen::Keyboard;
                }
            }
            SetupScreen::Keyboard => {
                if ev.printable == Some(b'\n') {
                    if self.key_detected {
                        crate::keyboard::save_shift_key(self.detected_raw_key);
                        self.screen = SetupScreen::Done;
                        mark_setup_done();
                    }
                } else if ev.raw_key != 0 {
                    self.detected_raw_key = ev.raw_key;
                    self.key_detected = true;
                }
            }
            SetupScreen::Done => {}
        }
        self.anim_tick = self.anim_tick.wrapping_add(1);
    }

    pub fn render(&self, buf: &mut [u32], w: usize, h: usize) {
        for pixel in buf.iter_mut() {
            *pixel = Color::BG.0;
        }

        match self.screen {
            SetupScreen::Welcome => self.render_welcome(buf, w, h),
            SetupScreen::Keyboard => self.render_keyboard(buf, w, h),
            SetupScreen::Done => self.render_done(buf, w, h),
        }
    }

    fn render_welcome(&self, buf: &mut [u32], w: usize, h: usize) {
        let title = "Baram OS";
        let sub = "初回セットアップへようこそ";
        let hint = "Enter キーを押して开始";

        let cx = w / 2;
        let cy = h / 2;

        draw_str_centered(buf, w, cx, cy - 60, title, Color::TEXT, 2.0);
        draw_str_centered(buf, w, cx, cy, sub, Color::MUTED, 1.0);
        draw_str_centered(buf, w, cx, cy + 60, hint, Color::MUTED, 0.8);
    }

    fn render_keyboard(&self, buf: &mut [u32], w: usize, h: usize) {
        let title = "キーボード設定";
        let instruction = "Shift にしたいキーを押してください";
        let status = if self.key_detected {
            "キーを検出しました！"
        } else {
            "待機中..."
        };
        let hint = if self.key_detected {
            "Enter キーで完了"
        } else {
            ""
        };

        let cx = w / 2;
        let cy = h / 2;

        draw_str_centered(buf, w, cx, cy - 80, title, Color::TEXT, 1.5);
        draw_str_centered(buf, w, cx, cy - 20, instruction, Color::MUTED, 1.0);

        let status_color = if self.key_detected { Color::ACCENT } else { Color::MUTED };
        draw_str_centered(buf, w, cx, cy + 40, status, status_color, 1.0);

        if !hint.is_empty() {
            draw_str_centered(buf, w, cx, cy + 100, hint, Color::MUTED, 0.8);
        }

        let dot_y = cy as i32 + 160;
        let dot_r = 8i32;
        let dot_color = if self.key_detected { Color::ACCENT } else { Color::MUTED };
        fill_circle(buf, w, h, cx as i32, dot_y, dot_r, dot_color.0);
    }

    fn render_done(&self, buf: &mut [u32], w: usize, h: usize) {
        let title = "セットアップ完了";
        let sub = "Baram OS を使い始めましょう";

        let cx = w / 2;
        let cy = h / 2;

        draw_str_centered(buf, w, cx, cy - 40, title, Color::TEXT, 1.5);
        draw_str_centered(buf, w, cx, cy + 20, sub, Color::MUTED, 1.0);
    }
}

fn draw_str_centered(buf: &mut [u32], screen_w: usize, cx: usize, cy: usize, text: &str, color: Color, scale: f32) {
    let screen_h = buf.len() / screen_w;

    if !ttf_font::is_available() {
        let mut x = cx.saturating_sub(text.len() * 4);
        for &b in text.as_bytes() {
            if b >= 0x80 { break; }
            let glyph = font::glyph(b);
            let gw = (GLYPH_W as f32 * scale) as usize;
            let gh = (GLYPH_H as f32 * scale) as usize;
            for row in 0..gh {
                let src_row = (row as f32 / scale) as usize;
                if src_row >= GLYPH_H { continue; }
                let bits = glyph[src_row];
                for col in 0..gw {
                    let src_col = (col as f32 / scale) as usize;
                    if src_col >= GLYPH_W { continue; }
                    if (bits >> (7 - src_col)) & 1 == 1 {
                        let px = x + col;
                        let py = cy + row;
                        if px < screen_w && py < screen_h {
                            buf[py * screen_w + px] = color.0;
                        }
                    }
                }
            }
            x += gw;
        }
        return;
    }

    let ascent = (ttf_font::ascent() as f32 * scale) as i32;

    let mut total_w = 0i32;
    for ch in text.chars() {
        let g = ttf_font::glyph_at_size(ch, 14.0 * scale);
        total_w += if g.w > 0 { g.advance } else { (8.0 * scale) as i32 };
    }

    let mut x = cx as i32 - total_w / 2;
    let baseline = cy as i32 + ascent;

    for ch in text.chars() {
        let g = ttf_font::glyph_at_size(ch, 14.0 * scale);
        if g.w <= 0 {
            x += (8.0 * scale) as i32;
            continue;
        }
        let adv = g.advance;
        for row in 0..g.h {
            let py = baseline + g.y_off + row;
            if py < 0 || py >= screen_h as i32 { continue; }
            for col in 0..g.w {
                let alpha = g.data[(row * g.w + col) as usize];
                if alpha == 0 { continue; }
                let px = x + col;
                if px < 0 || px >= screen_w as i32 { continue; }
                let idx = py as usize * screen_w + px as usize;
                if idx < buf.len() {
                    let a = alpha as u32;
                    let bg = Color(buf[idx]);
                    let r = (color.r() as u32 * a + bg.r() as u32 * (255 - a)) / 255;
                    let g2 = (color.g() as u32 * a + bg.g() as u32 * (255 - a)) / 255;
                    let b = (color.b() as u32 * a + bg.b() as u32 * (255 - a)) / 255;
                    buf[idx] = Color::rgb(r as u8, g2 as u8, b as u8).0;
                }
            }
        }
        x += adv;
    }
}

fn fill_circle(buf: &mut [u32], screen_w: usize, screen_h: usize, cx: i32, cy: i32, r: i32, color: u32) {
    let r2 = r * r;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r2 {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && (px as usize) < screen_w && py >= 0 && (py as usize) < screen_h {
                    buf[py as usize * screen_w + px as usize] = color;
                }
            }
        }
    }
}
