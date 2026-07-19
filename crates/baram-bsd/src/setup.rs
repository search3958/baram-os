use baram_core::{Color, LayerSystem, Screen};
use baram_font::ttf_font;
use baram_font::ttf_font_hud;
use baram_font::font::{self, GLYPH_H, GLYPH_W};
use crate::vfs;
use crate::config;
use baram_graphics::blur;

const SETUP_DONE_PATH: &str = "apps/.setup_done";
const WALLPAPER_BYTES: &[u8] = include_bytes!("../../../src/data/wallpaper/baram.png");
const CARD_H: usize = 320;
const BLUR_RADIUS: i32 = 30;

static mut CACHED_BLURRED: Option<alloc::vec::Vec<u32>> = None;
static mut CACHED_W: usize = 0;
static mut CACHED_H: usize = 0;

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

pub struct Button {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub label: &'static str,
    pub primary: bool,
}

pub struct SetupWizard {
    pub screen: SetupScreen,
    pub key_detected: bool,
    pub detected_raw_key: u8,
    pub anim_tick: u32,
    pub skipped: bool,
    pub buttons: alloc::vec::Vec<Button>,
    pub hover_btn: Option<usize>,
    dirty: bool,
    cached_frame: Option<alloc::vec::Vec<u32>>,
    cached_w: usize,
    cached_h: usize,
}

impl SetupWizard {
    pub fn new() -> Self {
        Self {
            screen: SetupScreen::Welcome,
            key_detected: false,
            detected_raw_key: 0,
            anim_tick: 0,
            skipped: false,
            buttons: alloc::vec::Vec::new(),
            hover_btn: None,
            dirty: true,
            cached_frame: None,
            cached_w: 0,
            cached_h: 0,
        }
    }

    pub fn hit_test(&self, mx: i32, my: i32) -> Option<usize> {
        for (i, btn) in self.buttons.iter().enumerate() {
            if mx >= btn.x as i32 && mx < (btn.x + btn.w) as i32
                && my >= btn.y as i32 && my < (btn.y + btn.h) as i32
            {
                return Some(i);
            }
        }
        None
    }

    pub fn on_click(&mut self, mx: i32, my: i32) {
        if let Some(idx) = self.hit_test(mx, my) {
            let label = self.buttons[idx].label;
            match self.screen {
                SetupScreen::Welcome => {
                    if label == "続行" {
                        self.screen = SetupScreen::Keyboard;
                        self.dirty = true;
                    } else if label == "スキップ" {
                        crate::shift_key::save_shift_key(0);
                        mark_setup_done();
                        self.skipped = true;
                        self.screen = SetupScreen::Done;
                        self.dirty = true;
                    }
                }
                SetupScreen::Keyboard => {
                    if label == "完了" && self.key_detected {
                        crate::shift_key::save_shift_key(self.detected_raw_key);
                        self.screen = SetupScreen::Done;
                        mark_setup_done();
                        self.dirty = true;
                    }
                    if label == "スキップ" {
                        crate::shift_key::save_shift_key(0);
                        mark_setup_done();
                        self.skipped = true;
                        self.screen = SetupScreen::Done;
                        self.dirty = true;
                    }
                }
                SetupScreen::Done => {}
            }
        }
    }

    pub fn on_hover(&mut self, mx: i32, my: i32) {
        let new_hover = self.hit_test(mx, my);
        if new_hover != self.hover_btn {
            self.hover_btn = new_hover;
            self.dirty = true;
        }
    }

    pub fn on_key(&mut self, ev: &baram_core::KeyEvent) {
        let is_esc = ev.raw_key == 0x29 || ev.scancode == 0x17;
        if is_esc && self.screen != SetupScreen::Done {
            crate::shift_key::save_shift_key(0);
            mark_setup_done();
            self.skipped = true;
            self.screen = SetupScreen::Done;
            self.dirty = true;
            return;
        }
        let is_enter = ev.printable == Some(b'\n') || ev.raw_key == 0x28 || ev.raw_key == 0x58 || ev.scancode == 0x1C;
        match self.screen {
            SetupScreen::Welcome => {
                if is_enter {
                    self.screen = SetupScreen::Keyboard;
                    self.dirty = true;
                }
            }
            SetupScreen::Keyboard => {
                if is_enter {
                    if self.key_detected {
                        crate::shift_key::save_shift_key(self.detected_raw_key);
                        self.screen = SetupScreen::Done;
                        mark_setup_done();
                        self.dirty = true;
                    }
                } else if ev.raw_key != 0 {
                    self.detected_raw_key = ev.raw_key;
                    self.key_detected = true;
                    self.dirty = true;
                }
            }
            SetupScreen::Done => {}
        }
        self.anim_tick = self.anim_tick.wrapping_add(1);
    }

    pub fn render(&mut self, buf: &mut [u32], w: usize, h: usize) {
        if !self.dirty {
            if let Some(ref cached) = self.cached_frame {
                if self.cached_w == w && self.cached_h == h && cached.len() == w * h {
                    buf.copy_from_slice(cached);
                    return;
                }
            }
        }
        self.buttons.clear();
        draw_wallpaper(buf, w, h);

        match self.screen {
            SetupScreen::Welcome => self.render_welcome(buf, w, h),
            SetupScreen::Keyboard => self.render_keyboard(buf, w, h),
            SetupScreen::Done => self.render_done(buf, w, h),
        }

        if self.cached_w != w || self.cached_h != h || self.cached_frame.as_ref().map_or(true, |f| f.len() != w * h) {
            self.cached_frame = Some(alloc::vec![0u32; w * h]);
            self.cached_w = w;
            self.cached_h = h;
        }
        if let Some(ref mut cached) = self.cached_frame {
            cached.copy_from_slice(buf);
        }
        self.dirty = false;
    }

    fn render_welcome(&mut self, buf: &mut [u32], w: usize, h: usize) {
        let card_w = 480;
        let card_h = CARD_H;
        let card_x = w / 2 - card_w / 2;
        let card_y = h / 2 - card_h / 2;
        draw_card(buf, w, h, card_x, card_y, card_w, card_h);

        let tx = card_x + 32;

        draw_str_hud_left(buf, w, tx, card_y + 60, "Hello", Color::TEXT, 2.0);
        draw_str_left(buf, w, tx, card_y + 120, "Baram OSへようこそ。", Color::MUTED, 1.0);
        draw_str_left(buf, w, tx, card_y + 140, "Enterキーで開始します。", Color::MUTED, 01.0);

        let btn_y = card_y + card_h - 70;
        let continue_btn_w = 140;
        let skip_btn_w = 100;
        let gap = 16;
        let continue_x = card_x + card_w - continue_btn_w - 32;
        let skip_x = continue_x - skip_btn_w - gap;

        self.buttons.push(Button { x: skip_x, y: btn_y, w: skip_btn_w, h: 40, label: "スキップ", primary: false });
        self.buttons.push(Button { x: continue_x, y: btn_y, w: continue_btn_w, h: 40, label: "続行", primary: true });

        draw_button(buf, w, skip_x, btn_y, skip_btn_w, 40, "スキップ", false, self.hover_btn == Some(0));
        draw_button(buf, w, continue_x, btn_y, continue_btn_w, 40, "続行", true, self.hover_btn == Some(1));
    }

    fn render_keyboard(&mut self, buf: &mut [u32], w: usize, h: usize) {
        let card_w = 480;
        let card_h = CARD_H;
        let card_x = w / 2 - card_w / 2;
        let card_y = h / 2 - card_h / 2;
        draw_card(buf, w, h, card_x, card_y, card_w, card_h);

        let tx = card_x + 32;

        let btn_y = card_y + card_h - 70;
        let skip_btn_w = 100;
        let continue_btn_w = 140;
        let gap = 16;
        let skip_x = card_x + card_w - skip_btn_w - 32;
        let continue_x = skip_x - continue_btn_w - gap;

        if self.key_detected {
            draw_str_left(buf, w, tx, card_y + 100, "完了", Color::TEXT, 1.5);
            draw_str_left(buf, w, tx, card_y + 160, "EnterキーかEscでセットアップを終了します", Color::MUTED, 1.0);

            let btn_x = card_x + card_w - continue_btn_w - 32;
            self.buttons.push(Button { x: btn_x, y: btn_y, w: continue_btn_w, h: 40, label: "完了", primary: true });
            draw_button(buf, w, btn_x, btn_y, continue_btn_w, 40, "完了", true, self.hover_btn == Some(0));
        } else {
            draw_str_left(buf, w, tx, card_y + 100, "キーボード設定", Color::TEXT, 1.5);
            draw_str_left(buf, w, tx, card_y + 160, "Shift にしたいキーを押してください", Color::MUTED, 1.0);
            draw_str_left(buf, w, tx, card_y + 220, "待機中...", Color::MUTED, 1.0);

            self.buttons.push(Button { x: skip_x, y: btn_y, w: skip_btn_w, h: 40, label: "スキップ", primary: false });
            draw_button(buf, w, skip_x, btn_y, skip_btn_w, 40, "スキップ", false, self.hover_btn == Some(0));
        }
    }

    fn render_done(&mut self, buf: &mut [u32], w: usize, h: usize) {
        let card_w = 480;
        let card_h = CARD_H;
        let card_x = w / 2 - card_w / 2;
        let card_y = h / 2 - card_h / 2;
        draw_card(buf, w, h, card_x, card_y, card_w, card_h);

        let tx = card_x + 32;

        draw_str_left(buf, w, tx, card_y + 100, "セットアップ完了", Color::TEXT, 1.5);
        draw_str_left(buf, w, tx, card_y + 160, "Baram OS を使い始めましょう", Color::MUTED, 1.0);
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

fn draw_str_left(buf: &mut [u32], screen_w: usize, x0: usize, cy: usize, text: &str, color: Color, scale: f32) {
    let screen_h = buf.len() / screen_w;

    if !ttf_font::is_available() {
        let mut x = x0;
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
    let mut x = x0 as i32;
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

fn draw_str_hud_left(buf: &mut [u32], screen_w: usize, x0: usize, cy: usize, text: &str, color: Color, scale: f32) {
    let screen_h = buf.len() / screen_w;

    if !ttf_font_hud::is_available() {
        draw_str_left(buf, screen_w, x0, cy, text, color, scale);
        return;
    }

    let pixel_size = 16.0 * scale;
    let ascent = ttf_font_hud::ascent_at_size(pixel_size);
    let mut x = x0 as i32;
    let baseline = cy as i32 + ascent;

    for ch in text.chars() {
        let g = ttf_font_hud::glyph_at_size(ch, pixel_size);
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

fn draw_wallpaper(buf: &mut [u32], screen_w: usize, screen_h: usize) {
    unsafe {
        if CACHED_W == screen_w && CACHED_H == screen_h {
            if let Some(ref cached) = CACHED_BLURRED {
                buf.copy_from_slice(cached);
                return;
            }
        }
    }

    let mut raw = alloc::vec![0u32; screen_w * screen_h];
    if let Ok((header, pixels)) = png_decoder::decode(WALLPAPER_BYTES) {
        let img_w = header.width as usize;
        let img_h = header.height as usize;
        let scale = if screen_w * img_h > screen_h * img_w {
            screen_w as f64 / img_w as f64
        } else {
            screen_h as f64 / img_h as f64
        };
        let src_w = (screen_w as f64 / scale) as usize;
        let src_h = (screen_h as f64 / scale) as usize;
        let src_x = (img_w.saturating_sub(src_w)) / 2;
        let src_y = (img_h.saturating_sub(src_h)) / 2;
        for y in 0..screen_h {
            let sy = (y * src_h / screen_h).min(src_h.saturating_sub(1)) + src_y;
            let src_row = sy * img_w;
            let dst_row = y * screen_w;
            for x in 0..screen_w {
                let sx = (x * src_w / screen_w).min(src_w.saturating_sub(1)) + src_x;
                let px = pixels[src_row + sx];
                raw[dst_row + x] = Color::rgb(px[0], px[1], px[2]).0;
            }
        }
    } else {
        for pixel in raw.iter_mut() {
            *pixel = Color::BG.0;
        }
    }

    blur::blur_region_to(&raw, buf, screen_w, 0, screen_h, BLUR_RADIUS);

    unsafe {
        CACHED_BLURRED = Some(buf.to_vec());
        CACHED_W = screen_w;
        CACHED_H = screen_h;
    }
}

fn draw_card(buf: &mut [u32], screen_w: usize, screen_h: usize, x: usize, y: usize, w: usize, h: usize) {
    draw_shadow(buf, screen_w, screen_h, x, y, w, h, 20);
    draw_card_body(buf, screen_w, screen_h, x, y, w, h);
}

fn draw_shadow(buf: &mut [u32], screen_w: usize, screen_h: usize, win_x: usize, win_y: usize, win_w: usize, win_h: usize, radius: usize) {
    let blur_r: i32 = 30;
    let r = radius as f32;
    let ww = win_w as i32;
    let wh = win_h as i32;

    let sx = (win_x as i32 - blur_r).max(0).min(screen_w as i32) as usize;
    let sy = (win_y as i32 - blur_r).max(0).min(screen_h as i32) as usize;
    let ex = (win_x as i32 + ww + blur_r).max(0).min(screen_w as i32) as usize;
    let ey = (win_y as i32 + wh + blur_r).max(0).min(screen_h as i32) as usize;
    if ex <= sx || ey <= sy { return; }

    let blur_r_f = blur_r as f32;
    let hww = ww as f32 / 2.0;
    let hwh = wh as f32 / 2.0;
    let center_x = win_x as f32 + hww;
    let center_y = win_y as f32 + hwh;
    let box_w = hww - r;
    let box_h = hwh - r;

    for py in sy..ey {
        let py_f = py as f32 + 0.5;
        let row_start = py * screen_w;
        for px in sx..ex {
            let px_f = px as f32 + 0.5;

            let qx = (px_f - center_x).abs();
            let qy = (py_f - center_y).abs();

            let dx = qx - box_w;
            let dy = qy - box_h;

            let dist = if dx > 0.0 && dy > 0.0 {
                libm::sqrtf(dx * dx + dy * dy)
            } else {
                dx.max(dy)
            };

            let edge_dist = dist - r;

            if edge_dist <= 0.0 || edge_dist >= blur_r_f {
                continue;
            }

            let t = (blur_r_f - edge_dist) / blur_r_f;
            let alpha_f = t * t * 0.175;
            let a = (alpha_f * 255.0) as u32;
            if a == 0 { continue; }

            let inv = 255 - a;
            let idx = row_start + px;
            let bg = Color(buf[idx]);
            let cr = (bg.r() as u32 * inv) / 255;
            let cg = (bg.g() as u32 * inv) / 255;
            let cb = (bg.b() as u32 * inv) / 255;
            buf[idx] = Color::rgb(cr as u8, cg as u8, cb as u8).0;
        }
    }
}

fn draw_card_body(buf: &mut [u32], screen_w: usize, screen_h: usize, x: usize, y: usize, w: usize, h: usize) {
    let radius = 20usize;
    let r = radius.min(w / 2).min(h / 2);
    let rf = r as f32;

    let x0 = x.min(screen_w);
    let y0 = y.min(screen_h);
    let x1 = (x + w).min(screen_w);
    let y1 = (y + h).min(screen_h);

    for py in y0..y1 {
        let row = py * screen_w;
        if r == 0 {
            buf[row + x0..row + x1].fill(Color::PANEL.0);
            continue;
        }
        let corner_top = py < y + r;
        let corner_bot = py >= y + h.saturating_sub(r);
        if !corner_top && !corner_bot {
            buf[row + x0..row + x1].fill(Color::PANEL.0);
            continue;
        }
        for px in x0..x1 {
            let in_corner = (px < x + r && corner_top)
                || (px >= x + w.saturating_sub(r) && corner_top)
                || (px < x + r && corner_bot)
                || (px >= x + w.saturating_sub(r) && corner_bot);
            if !in_corner {
                buf[row + px] = Color::PANEL.0;
                continue;
            }

            let cx_f = if px < x + r { x + r } else { x + w - r } as f32;
            let cy_f = if corner_top { y + r } else { y + h - r } as f32;
            let dx = px as f32 + 0.5 - cx_f;
            let dy = py as f32 + 0.5 - cy_f;
            let dist_sq = dx * dx + dy * dy;
            let alpha = if dist_sq < (rf - 0.5) * (rf - 0.5) {
                1.0
            } else if dist_sq > (rf + 0.5) * (rf + 0.5) {
                0.0
            } else {
                let dist = libm::sqrtf(dist_sq);
                (rf + 0.5 - dist).clamp(0.0, 1.0)
            };

            if alpha >= 1.0 {
                buf[row + px] = Color::PANEL.0;
            } else if alpha > 0.0 {
                let bg = buf[row + px];
                let br = ((bg >> 16) & 0xFF) as f32;
                let bg2 = ((bg >> 8) & 0xFF) as f32;
                let bb = (bg & 0xFF) as f32;
                let panel_r = Color::PANEL.r() as f32;
                let panel_g = Color::PANEL.g() as f32;
                let panel_b = Color::PANEL.b() as f32;
                let r2 = (panel_r * alpha + br * (1.0 - alpha)) as u32;
                let g = (panel_g * alpha + bg2 * (1.0 - alpha)) as u32;
                let b = (panel_b * alpha + bb * (1.0 - alpha)) as u32;
                buf[row + px] = Color::rgb(r2 as u8, g as u8, b as u8).0;
            }
        }
    }
}

fn draw_button(buf: &mut [u32], screen_w: usize, x: usize, y: usize, w: usize, h: usize, label: &str, primary: bool, hover: bool) {
    let radius = config::get_usize("ui-theme/button/corner", 20);
    let bg = if primary {
        if hover {
            config::get_color("ui-theme/color/btn_primary_hover", Color::BTN_PRIMARY_HOVER)
        } else {
            config::get_color("ui-theme/color/btn_primary", Color::BTN_PRIMARY)
        }
    } else {
        if hover {
            config::get_color("ui-theme/color/btn_tonal_hover", Color::BTN_TONAL_HOVER)
        } else {
            config::get_color("ui-theme/color/btn_tonal", Color::BTN_TONAL)
        }
    };
    let text_color = if primary { config::get_color("ui-theme/color/btn_text", Color::BTN_TEXT) } else { Color::TEXT };

    draw_rounded_rect(buf, screen_w, x, y, w, h, radius, bg);

    let text_color = if primary { config::get_color("ui-theme/color/btn_text", Color::BTN_TEXT) } else { Color::TEXT };
    draw_str_centered(buf, screen_w, x + w / 2, y + h / 2 - 8, label, text_color, 0.9);
}

fn draw_rounded_rect(buf: &mut [u32], screen_w: usize, x: usize, y: usize, w: usize, h: usize, radius: usize, color: Color) {
    let r = radius.min(w / 2).min(h / 2);
    let screen_h = buf.len() / screen_w;

    let x0 = x.min(screen_w);
    let y0 = y.min(screen_h);
    let x1 = (x + w).min(screen_w);
    let y1 = (y + h).min(screen_h);

    if r == 0 {
        for py in y0..y1 {
            buf[py * screen_w + x0..py * screen_w + x1].fill(color.0);
        }
        return;
    }

    let rf = r as f32;
    let poly = LayerSystem::squircle_polygon(w as f32, h as f32, rf);
    let x0f = x as f32;
    let y0f = y as f32;
    let off = [0.25f32, 0.75f32];
    let r_f = r as f32;
    let h_f = h as f32;

    for py in y0..y1 {
        let row = py * screen_w;
        let base_y = py as f32 - y0f;
        let in_corner_row = base_y < r_f || base_y >= h_f - r_f;

        if !in_corner_row {
            buf[row + x0..row + x1].fill(color.0);
            continue;
        }

        let corner_x_end = (x + r).min(x1);
        let mid_x_start = (x + r).max(x0);
        let mid_x_end = (x + w - r).min(x1);
        let corner_x_start = (x + w - r).max(x0);

        if mid_x_end > mid_x_start {
            buf[row + mid_x_start..row + mid_x_end].fill(color.0);
        }

        for px in x0..corner_x_end {
            let base_x = px as f32 - x0f;
            let mut hits = 0u32;
            for sy in 0..2 {
                for sx in 0..2 {
                    if LayerSystem::point_in_polygon(base_x + off[sx], base_y + off[sy], &poly) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                buf[row + px] = LayerSystem::blend_alpha(buf[row + px], color.0, hits as f32 * 0.25);
            }
        }

        if corner_x_start > corner_x_end {
            for px in corner_x_start..x1 {
                let base_x = px as f32 - x0f;
                let mut hits = 0u32;
                for sy in 0..2 {
                    for sx in 0..2 {
                        if LayerSystem::point_in_polygon(base_x + off[sx], base_y + off[sy], &poly) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    buf[row + px] = LayerSystem::blend_alpha(buf[row + px], color.0, hits as f32 * 0.25);
                }
            }
        }
    }
}
