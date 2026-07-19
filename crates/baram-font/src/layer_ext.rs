use baram_core::{Color, LayerSystem};
use crate::ttf_font;
use crate::ttf_font_hud;
use crate::font;

pub trait LayerFontExt {
    fn put_char(&mut self, x: usize, y: usize, ch: char, fg: Color);
    fn put_str(&mut self, x: usize, y: usize, s: &str, fg: Color);
    fn put_str_hud(&mut self, x: usize, y: usize, s: &str, fg: Color);
}

impl LayerFontExt for LayerSystem {
    fn put_char(&mut self, x: usize, y: usize, ch: char, fg: Color) {
        if ttf_font::is_available() && ch as u32 >= 0x20 {
            let glyph = ttf_font::glyph(ch);
            if glyph.w > 0 && glyph.h > 0 {
                let baseline = y as i32 + ttf_font::ascent();
                let w = self.width();
                let buf = self.buf_mut();
                for row in 0..glyph.h {
                    let py = baseline + glyph.y_off + row;
                    if py < 0 || py >= buf.len() as i32 / w as i32 { continue; }
                    for col in 0..glyph.w {
                        let px = x as i32 + col;
                        if px < 0 || px >= w as i32 { continue; }
                        let alpha = glyph.data[(row * glyph.w + col) as usize];
                        if alpha > 0 {
                            let a = alpha as u32;
                            let idx = py as usize * w + px as usize;
                            let bg = Color(buf[idx]);
                            let br = (bg.0 >> 16) & 0xFF;
                            let bg2 = (bg.0 >> 8) & 0xFF;
                            let bb = bg.0 & 0xFF;
                            let fr = (fg.0 >> 16) & 0xFF;
                            let fg2 = (fg.0 >> 8) & 0xFF;
                            let fb = fg.0 & 0xFF;
                            let r = (fr * a + br * (255 - a)) / 255;
                            let g = (fg2 * a + bg2 * (255 - a)) / 255;
                            let b = (fb * a + bb * (255 - a)) / 255;
                            buf[idx] = (r << 16) | (g << 8) | b;
                        }
                    }
                }
                return;
            }
        }
        if (ch as u32) < 0x20 || (ch as u32) > 0x7E { return; }
        let glyph = font::glyph(ch as u8);
        let w = self.width();
        let buf = self.buf_mut();
        for row in 0..font::GLYPH_H {
            let bits = glyph[row];
            let py = y + row;
            if py >= buf.len() / w { break; }
            for col in 0..font::GLYPH_W {
                if (bits >> (7 - col)) & 1 == 1 {
                    let px = x + col;
                    if px < w {
                        buf[py * w + px] = fg.0;
                    }
                }
            }
        }
    }

    fn put_str(&mut self, mut x: usize, y: usize, s: &str, fg: Color) {
        if ttf_font::is_available() {
            for ch in s.chars() {
                let glyph = ttf_font::glyph(ch);
                if glyph.w > 0 && glyph.h > 0 {
                    self.put_char(x, y, ch, fg);
                    x += glyph.advance.max(0) as usize;
                } else if (ch as u32) < 0x80 {
                    self.put_char(x, y, ch, fg);
                    x += font::GLYPH_W;
                }
            }
            return;
        }
        for &b in s.as_bytes() {
            if b >= 0x80 { break; }
            self.put_char(x, y, b as char, fg);
            x += font::GLYPH_W;
        }
    }

    fn put_str_hud(&mut self, mut x: usize, y: usize, s: &str, fg: Color) {
        if ttf_font_hud::is_available() {
            for ch in s.chars() {
                let glyph = ttf_font_hud::glyph(ch);
                if glyph.w > 0 && glyph.h > 0 {
                    let baseline = y as i32 + ttf_font_hud::ascent();
                    let w = self.width();
                    let buf = self.buf_mut();
                    for row in 0..glyph.h {
                        let py = baseline + glyph.y_off + row;
                        if py < 0 || py >= buf.len() as i32 / w as i32 { continue; }
                        for col in 0..glyph.w {
                            let px = x as i32 + col;
                            if px < 0 || px >= w as i32 { continue; }
                            let alpha = glyph.data[(row * glyph.w + col) as usize];
                            if alpha > 0 {
                                let a = alpha as u32;
                                let idx = py as usize * w + px as usize;
                                let bg = Color(buf[idx]);
                                let br = (bg.0 >> 16) & 0xFF;
                                let bg2 = (bg.0 >> 8) & 0xFF;
                                let bb = bg.0 & 0xFF;
                                let fr = (fg.0 >> 16) & 0xFF;
                                let fg2 = (fg.0 >> 8) & 0xFF;
                                let fb = fg.0 & 0xFF;
                                let r = (fr * a + br * (255 - a)) / 255;
                                let g = (fg2 * a + bg2 * (255 - a)) / 255;
                                let b = (fb * a + bb * (255 - a)) / 255;
                                buf[idx] = (r << 16) | (g << 8) | b;
                            }
                        }
                    }
                    x += glyph.advance.max(0) as usize;
                } else if (ch as u32) < 0x80 {
                    self.put_char(x, y, ch, fg);
                    x += font::GLYPH_W;
                }
            }
            return;
        }
        self.put_str(x, y, s, fg);
    }
}
