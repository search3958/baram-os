use crate::font;
use crate::bdf_font;
use crate::ttf_font;
use crate::ttf_font_hud;
use baram_core::{Color, LayerSystem};

pub trait LayerFontExt {
    fn put_char(&mut self, x: usize, y: usize, ch: char, fg: Color);
    fn put_str(&mut self, x: usize, y: usize, s: &str, fg: Color);
    fn put_str_hud(&mut self, x: usize, y: usize, s: &str, fg: Color);
}

impl LayerFontExt for LayerSystem {
    fn put_char(&mut self, x: usize, y: usize, ch: char, fg: Color) {
        if bdf_font::is_available() {
            let w = self.width();
            let (clip_x0, clip_y0, clip_x1, clip_y1) = self.clip_bounds();
            let buf = self.buf_mut();
            if bdf_font::with_glyph(ch, |data, gw, gh, _advance, y_off| {
                for row in 0..gh {
                    let py = y as i32 + y_off + row;
                    if py < clip_y0 as i32 || py >= clip_y1 as i32 { continue; }
                    for col in 0..gw {
                        let px = x as i32 + col;
                        if px >= clip_x0 as i32 && px < clip_x1 as i32 && data[(row * gw + col) as usize] != 0 {
                            buf[py as usize * w + px as usize] = fg.0;
                        }
                    }
                }
                true
            }) { return; }
        }
        if ttf_font::is_available() && ch as u32 >= 0x20 {
            let baseline = y as i32 + ttf_font::ascent();
            let w = self.width();
            let (clip_x0, clip_y0, clip_x1, clip_y1) = self.clip_bounds();
            let buf = self.buf_mut();
            let drawn = ttf_font::with_glyph(ch, |data, gw, gh, _advance, y_off| {
                if gw <= 0 || gh <= 0 {
                    return false;
                }
                for row in 0..gh {
                    let py = baseline + y_off + row;
                    if py < clip_y0 as i32 || py >= clip_y1 as i32 {
                        continue;
                    }
                    for col in 0..gw {
                        let px = x as i32 + col;
                        if px < clip_x0 as i32 || px >= clip_x1 as i32 {
                            continue;
                        }
                        let alpha = data[(row * gw + col) as usize];
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
                            buf[idx] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
                        }
                    }
                }
                true
            });
            if drawn {
                return;
            }
        }
        if (ch as u32) < 0x20 || (ch as u32) > 0x7E {
            return;
        }
        let glyph = font::glyph(ch as u8);
        let w = self.width();
        let (clip_x0, clip_y0, clip_x1, clip_y1) = self.clip_bounds();
        let buf = self.buf_mut();
        for row in 0..font::GLYPH_H {
            let bits = glyph[row];
            let py = y + row;
            if py >= clip_y1 {
                break;
            }
            if py < clip_y0 {
                continue;
            }
            for col in 0..font::GLYPH_W {
                if (bits >> (7 - col)) & 1 == 1 {
                    let px = x + col;
                    if px >= clip_x0 && px < clip_x1 {
                        buf[py * w + px] = fg.0;
                    }
                }
            }
        }
    }

    fn put_str(&mut self, mut x: usize, y: usize, s: &str, fg: Color) {
        if bdf_font::is_available() {
            for ch in s.chars() {
                self.put_char(x, y, ch, fg);
                x += bdf_font::advance(ch).max(1) as usize;
            }
            return;
        }
        if ttf_font::is_available() {
            for ch in s.chars() {
                let mut advance = 0;
                let drawn = ttf_font::with_glyph(ch, |_data, w, h, glyph_advance, _y_off| {
                    advance = glyph_advance;
                    w > 0 && h > 0
                });
                if drawn {
                    self.put_char(x, y, ch, fg);
                    x += advance.max(0) as usize;
                } else if (ch as u32) < 0x80 {
                    self.put_char(x, y, ch, fg);
                    x += font::GLYPH_W;
                }
            }
            return;
        }
        for &b in s.as_bytes() {
            if b >= 0x80 {
                break;
            }
            self.put_char(x, y, b as char, fg);
            x += font::GLYPH_W;
        }
    }

    fn put_str_hud(&mut self, mut x: usize, y: usize, s: &str, fg: Color) {
        if bdf_font::is_available() {
            self.put_str(x, y, s, fg);
            return;
        }
        if ttf_font_hud::is_available() {
            for ch in s.chars() {
                let glyph = ttf_font_hud::glyph(ch);
                if glyph.w > 0 && glyph.h > 0 {
                    let baseline = y as i32 + ttf_font_hud::ascent();
                    let w = self.width();
                    let buf = self.buf_mut();
                    for row in 0..glyph.h {
                        let py = baseline + glyph.y_off + row;
                        if py < 0 || py >= buf.len() as i32 / w as i32 {
                            continue;
                        }
                        for col in 0..glyph.w {
                            let px = x as i32 + col;
                            if px < 0 || px >= w as i32 {
                                continue;
                            }
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
