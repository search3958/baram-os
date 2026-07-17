use alloc::vec;
use alloc::vec::Vec;
use crate::pexpert::gop::{Color, Screen};

pub struct LayerSystem {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) buf: Vec<u32>,
    frame_count: u64,
    clip_stack: Vec<(usize, usize, usize, usize)>,
    clip: Option<(usize, usize, usize, usize)>,
}

impl LayerSystem {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            width: w,
            height: h,
            buf: vec![Color::BG.0; w * h],
            frame_count: 0,
            clip_stack: Vec::new(),
            clip: None,
        }
    }

    pub fn new_transparent(w: usize, h: usize) -> Self {
        Self {
            width: w,
            height: h,
            buf: vec![Color::TRANSPARENT.0; w * h],
            frame_count: 0,
            clip_stack: Vec::new(),
            clip: None,
        }
    }

    pub fn push_clip(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        let x0 = x0.min(self.width);
        let y0 = y0.min(self.height);
        let x1 = x1.min(self.width);
        let y1 = y1.min(self.height);
        
        if let Some(cur) = self.clip {
            self.clip_stack.push(cur);
            
            self.clip = Some((
                x0.max(cur.0),
                y0.max(cur.1),
                x1.min(cur.2),
                y1.min(cur.3),
            ));
        } else {
            self.clip = Some((x0, y0, x1, y1));
        }
    }

    pub fn pop_clip(&mut self) {
        if let Some(prev) = self.clip_stack.pop() {
            self.clip = Some(prev);
        } else {
            self.clip = None;
        }
    }

    #[inline]
    fn clip_test(&self, x: usize, y: usize) -> bool {
        if let Some((cx0, cy0, cx1, cy1)) = self.clip {
            x >= cx0 && x < cx1 && y >= cy0 && y < cy1
        } else {
            true
        }
    }

    pub fn clear(&mut self, c: Color) {
        self.buf.fill(c.0);
    }

    #[inline]
    pub fn put_pixel(&mut self, x: usize, y: usize, c: Color) {
        if x < self.width && y < self.height && self.clip_test(x, y) {
            self.buf[y * self.width + x] = c.0;
        }
    }

    #[allow(dead_code)]
    pub fn get_pixel(&self, x: usize, y: usize) -> Color {
        if x < self.width && y < self.height {
            Color(self.buf[y * self.width + x])
        } else {
            Color::BLACK
        }
    }

    #[inline]
    pub fn buf_mut(&mut self) -> &mut [u32] {
        &mut self.buf
    }

    #[inline]
    pub fn buf_ref(&self) -> &[u32] {
        &self.buf
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: Color) {
        let v = c.0;
        let stride = self.width;
        if let Some((cx0, cy0, cx1, cy1)) = self.clip {
            let x0 = x.max(cx0).min(stride);
            let y0 = y.max(cy0).min(self.height);
            let x1 = (x + w).min(cx1).min(stride);
            let y1 = (y + h).min(cy1).min(self.height);
            if x0 >= x1 || y0 >= y1 { return; }
            for yy in y0..y1 {
                self.buf[yy * stride + x0..yy * stride + x1].fill(v);
            }
        } else {
            let x0 = x.min(stride);
            let y0 = y.min(self.height);
            let x1 = (x + w).min(stride);
            let y1 = (y + h).min(self.height);
            if x0 >= x1 || y0 >= y1 { return; }
            for yy in y0..y1 {
                self.buf[yy * stride + x0..yy * stride + x1].fill(v);
            }
        }
    }

    pub fn fill_rounded_rect(&mut self, x: usize, y: usize, w: usize, h: usize, r: usize, c: Color) {
        if w == 0 || h == 0 { return; }
        let r = r.min(w / 2).min(h / 2);
        let rf = r as f32;
        let cr = c.r() as f32;
        let cg = c.g() as f32;
        let cb = c.b() as f32;
        let y0 = y.min(self.height);
        let y1 = (y + h).min(self.height);
        let x0 = x.min(self.width);
        let x1 = (x + w).min(self.width);
        let v = c.0;
        let stride = self.width;

        for py in y0..y1 {
            let row = py * stride;
            if r == 0 {
                self.buf[row + x0..row + x1].fill(v);
                continue;
            }
            let corner_top = py < y + r;
            let corner_bot = py >= y + h.saturating_sub(r);
            if !corner_top && !corner_bot {
                self.buf[row + x0..row + x1].fill(v);
                continue;
            }
            for px in x0..x1 {
                let in_corner = (px < x + r && corner_top)
                    || (px >= x + w.saturating_sub(r) && corner_top)
                    || (px < x + r && corner_bot)
                    || (px >= x + w.saturating_sub(r) && corner_bot);
                if !in_corner {
                    self.buf[row + px] = v;
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

                if alpha > 0.0 {
                    if alpha >= 1.0 {
                        self.buf[row + px] = v;
                    } else {
                        let bg = self.buf[row + px];
                        let br = ((bg >> 16) & 0xFF) as f32;
                        let bg2 = ((bg >> 8) & 0xFF) as f32;
                        let bb = (bg & 0xFF) as f32;
                        let r2 = (cr * alpha + br * (1.0 - alpha)) as u32;
                        let g = (cg * alpha + bg2 * (1.0 - alpha)) as u32;
                        let b = (cb * alpha + bb * (1.0 - alpha)) as u32;
                        self.buf[row + px] = Color::rgb(r2 as u8, g as u8, b as u8).0;
                    }
                }
            }
        }
    }

    pub fn fill_circle(&mut self, cx: usize, cy: usize, r: usize, c: Color) {
        if r == 0 { return; }
        let rf = r as f32;
        let cr = c.r() as f32;
        let cg = c.g() as f32;
        let cb = c.b() as f32;
        let x0 = cx.saturating_sub(r).min(self.width);
        let y0 = cy.saturating_sub(r).min(self.height);
        let x1 = (cx + r + 1).min(self.width);
        let y1 = (cy + r + 1).min(self.height);
        for py in y0..y1 {
            let row = py * self.width;
            for px in x0..x1 {
                let dx = px as f32 + 0.5 - cx as f32;
                let dy = py as f32 + 0.5 - cy as f32;
                let dist_sq = dx * dx + dy * dy;
                let alpha = if dist_sq < (rf - 0.5) * (rf - 0.5) {
                    1.0
                } else if dist_sq > (rf + 0.5) * (rf + 0.5) {
                    continue;
                } else {
                    let dist = libm::sqrtf(dist_sq);
                    (rf + 0.5 - dist).clamp(0.0, 1.0)
                };
                if alpha >= 1.0 {
                    self.buf[row + px] = c.0;
                } else {
                    let bg = self.buf[row + px];
                    let br = ((bg >> 16) & 0xFF) as f32;
                    let bg2 = ((bg >> 8) & 0xFF) as f32;
                    let bb = (bg & 0xFF) as f32;
                    let r2 = (cr * alpha + br * (1.0 - alpha)) as u32;
                    let g = (cg * alpha + bg2 * (1.0 - alpha)) as u32;
                    let b = (cb * alpha + bb * (1.0 - alpha)) as u32;
                    self.buf[row + px] = Color::rgb(r2 as u8, g as u8, b as u8).0;
                }
            }
        }
    }

    pub fn rounded_rect_outline(&mut self, x: usize, y: usize, w: usize, h: usize, r: usize, c: Color) {
        if w == 0 || h == 0 { return; }
        let r = r.min(w / 2).min(h / 2);
        let rf = r as f32;
        let cr = c.r() as f32;
        let cg = c.g() as f32;
        let cb = c.b() as f32;
        let y0 = y.min(self.height);
        let y1 = (y + h).min(self.height);
        let x0 = x.min(self.width);
        let x1 = (x + w).min(self.width);

        for py in y0..y1 {
            for px in x0..x1 {
                let on_edge = px == x || px == x + w - 1 || py == y || py == y + h - 1;
                if !on_edge { continue; }

                let dist_to_edge = if px < x + r && py < y + r {
                    let cx_f = (x + r) as f32;
                    let cy_f = (y + r) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else if px >= x + w.saturating_sub(r) && py < y + r && r > 0 {
                    let cx_f = (x + w - r) as f32;
                    let cy_f = (y + r) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else if px < x + r && py >= y + h.saturating_sub(r) && r > 0 {
                    let cx_f = (x + r) as f32;
                    let cy_f = (y + h - r) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else if px >= x + w.saturating_sub(r) && py >= y + h.saturating_sub(r) && r > 0 {
                    let cx_f = (x + w - r) as f32;
                    let cy_f = (y + h - r) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else {
                    self.put_pixel(px, py, c);
                    continue;
                };

                let alpha = if dist_to_edge < -0.5 {
                    0.0
                } else if dist_to_edge > 0.5 {
                    0.0
                } else {
                    (0.5 - dist_to_edge.abs()).clamp(0.0, 1.0)
                };

                if alpha > 0.0 {
                    let bg = self.buf[py * self.width + px];
                    let br = ((bg >> 16) & 0xFF) as f32;
                    let bg2 = ((bg >> 8) & 0xFF) as f32;
                    let bb = (bg & 0xFF) as f32;
                    let r2 = (cr * alpha + br * (1.0 - alpha)) as u32;
                    let g = (cg * alpha + bg2 * (1.0 - alpha)) as u32;
                    let b = (cb * alpha + bb * (1.0 - alpha)) as u32;
                    self.put_pixel(px, py, Color::rgb(r2 as u8, g as u8, b as u8));
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

    pub fn put_char(&mut self, x: usize, y: usize, ch: char, fg: Color) {
        if crate::libkern::ttf_font::is_available() && ch as u32 >= 0x20 {
            let glyph = crate::libkern::ttf_font::glyph(ch);
            if glyph.w > 0 && glyph.h > 0 {
                let baseline = y as i32 + crate::libkern::ttf_font::ascent();
                for row in 0..glyph.h {
                    let py = baseline + glyph.y_off + row;
                    if py < 0 || py >= self.height as i32 { continue; }
                    for col in 0..glyph.w {
                        let px = x as i32 + col;
                        if px < 0 || px >= self.width as i32 { continue; }
                        if !self.clip_test(px as usize, py as usize) { continue; }
                        let alpha = glyph.data[(row * glyph.w + col) as usize];
                        if alpha > 0 {
                            let a = alpha as u32;
                            let bg = self.buf[py as usize * self.width + px as usize];
                            let br = (bg >> 16) & 0xFF;
                            let bg2 = (bg >> 8) & 0xFF;
                            let bb = bg & 0xFF;
                            let fr = (fg.0 >> 16) & 0xFF;
                            let fg2 = (fg.0 >> 8) & 0xFF;
                            let fb = fg.0 & 0xFF;
                            let r = (fr * a + br * (255 - a)) / 255;
                            let g = (fg2 * a + bg2 * (255 - a)) / 255;
                            let b = (fb * a + bb * (255 - a)) / 255;
                            self.buf[py as usize * self.width + px as usize] = (r << 16) | (g << 8) | b;
                        }
                    }
                }
                return;
            }
        }
        if (ch as u32) < 0x20 || (ch as u32) > 0x7E { return; }
        use crate::libkern::font::{self, GLYPH_W, GLYPH_H};
        let glyph = font::glyph(ch as u8);
        for row in 0..GLYPH_H {
            let bits = glyph[row];
            let py = y + row;
            if py >= self.height { break; }
            for col in 0..GLYPH_W {
                if (bits >> (7 - col)) & 1 == 1 {
                    let px = x + col;
                    if px < self.width && self.clip_test(px, py) {
                        self.buf[py * self.width + px] = fg.0;
                    }
                }
            }
        }
    }

    pub fn put_str(&mut self, mut x: usize, y: usize, s: &str, fg: Color) {
        if crate::libkern::ttf_font::is_available() {
            for ch in s.chars() {
                let glyph = crate::libkern::ttf_font::glyph(ch);
                if glyph.w > 0 && glyph.h > 0 {
                    self.put_char(x, y, ch, fg);
                    x += glyph.advance.max(0) as usize;
                } else if (ch as u32) < 0x80 {
                    self.put_char(x, y, ch, fg);
                    x += crate::libkern::font::GLYPH_W;
                }
            }
            return;
        }
        use crate::libkern::font::GLYPH_W;
        for &b in s.as_bytes() {
            if b >= 0x80 { break; }
            self.put_char(x, y, b as char, fg);
            x += GLYPH_W;
        }
    }

    pub fn put_str_hud(&mut self, mut x: usize, y: usize, s: &str, fg: Color) {
        if crate::libkern::ttf_font_hud::is_available() {
            for ch in s.chars() {
                let glyph = crate::libkern::ttf_font_hud::glyph(ch);
                if glyph.w > 0 && glyph.h > 0 {
                    let baseline = y as i32 + crate::libkern::ttf_font_hud::ascent();
                    for row in 0..glyph.h {
                        let py = baseline + glyph.y_off + row;
                        if py < 0 || py >= self.height as i32 { continue; }
                        for col in 0..glyph.w {
                            let px = x as i32 + col;
                            if px < 0 || px >= self.width as i32 { continue; }
                            if !self.clip_test(px as usize, py as usize) { continue; }
                            let alpha = glyph.data[(row * glyph.w + col) as usize];
                            if alpha > 0 {
                                let a = alpha as u32;
                                let bg = self.buf[py as usize * self.width + px as usize];
                                let br = (bg >> 16) & 0xFF;
                                let bg2 = (bg >> 8) & 0xFF;
                                let bb = bg & 0xFF;
                                let fr = (fg.0 >> 16) & 0xFF;
                                let fg2 = (fg.0 >> 8) & 0xFF;
                                let fb = fg.0 & 0xFF;
                                let r = (fr * a + br * (255 - a)) / 255;
                                let g = (fg2 * a + bg2 * (255 - a)) / 255;
                                let b = (fb * a + bb * (255 - a)) / 255;
                                self.buf[py as usize * self.width + px as usize] = (r << 16) | (g << 8) | b;
                            }
                        }
                    }
                    x += glyph.advance.max(0) as usize;
                } else if (ch as u32) < 0x80 {
                    self.put_char(x, y, ch, fg);
                    x += crate::libkern::font::GLYPH_W;
                }
            }
            return;
        }
        self.put_str(x, y, s, fg);
    }

    pub fn flush(&mut self, screen: &mut Screen) {
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            let row = &self.buf[y * w..(y + 1) * w];
            screen.flush_layer_row(y, row);
        }
        self.frame_count += 1;
    }

    pub fn flush_rect(&self, screen: &mut Screen, x0: usize, y0: usize, x1: usize, y1: usize) {
        let w = self.width;
        let y0 = y0.min(self.height);
        let y1 = y1.min(self.height);
        let x0 = x0.min(w);
        let x1 = x1.min(w);
        for y in y0..y1 {
            let row = &self.buf[y * w + x0..y * w + x1];
            screen.flush_layer_row_range(y, x0, row);
        }
    }

    pub fn composit_rounded(
        &mut self,
        src: &LayerSystem,
        dx: usize, dy: usize,
        sx: usize, sy: usize,
        w: usize, h: usize,
        r: usize,
    ) {
        let r = r.min(w / 2).min(h / 2);
        let rf = r as f32;
        let sw = src.width;
        let sh = src.height;
        let dw = self.width;
        let dh = self.height;

        if r == 0 {
            for py in 0..h {
                let src_y = sy + py;
                let dst_y = dy + py;
                if src_y >= sh || dst_y >= dh { continue; }
                let src_row_start = src_y * sw + sx;
                let dst_row_start = dst_y * dw + dx;
                let copy_w = w.min(sw - sx).min(dw - dx);
                for px in 0..copy_w {
                    let sp = src.buf[src_row_start + px];
                    if sp != Color::TRANSPARENT.0 {
                        self.buf[dst_row_start + px] = sp;
                    }
                }
            }
            return;
        }

        let corner_end = r;
        let corner_row_start = h.saturating_sub(r);

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh { continue; }

            let src_row = src_y * sw + sx;
            let dst_row = dst_y * dw + dx;

            let in_top_corner = py < corner_end;
            let in_bot_corner = py >= corner_row_start;

            if !in_top_corner && !in_bot_corner {
                let copy_w = w.min(sw - sx).min(dw - dx);
                for px in 0..copy_w {
                    let sp = src.buf[src_row + px];
                    if sp != Color::TRANSPARENT.0 {
                        self.buf[dst_row + px] = sp;
                    }
                }
                continue;
            }

            let end_x = w.min(sw - sx).min(dw - dx);
            for px in 0..end_x {
                let src_pixel = Color(src.buf[src_row + px]);

                let alpha = {
                    let in_corner = (px < r && py < r)
                        || (px >= w.saturating_sub(r) && py < r)
                        || (px < r && py >= h.saturating_sub(r))
                        || (px >= w.saturating_sub(r) && py >= h.saturating_sub(r));
                    if !in_corner {
                        1.0
                    } else {
                        let cx_f = if px < r { r } else { w - r } as f32;
                        let cy_f = if py < r { r } else { h - r } as f32;
                        let dx_f = px as f32 + 0.5 - cx_f;
                        let dy_f = py as f32 + 0.5 - cy_f;
                        let dist_sq = dx_f * dx_f + dy_f * dy_f;
                        if dist_sq < (rf - 0.5) * (rf - 0.5) {
                            1.0
                        } else if dist_sq > (rf + 0.5) * (rf + 0.5) {
                            0.0
                        } else {
                            let dist = libm::sqrtf(dist_sq);
                            (rf + 0.5 - dist).clamp(0.0, 1.0)
                        }
                    }
                };

                if alpha <= 0.0 { continue; }
                if src_pixel.0 == Color::TRANSPARENT.0 { continue; }

                if alpha >= 1.0 {
                    self.buf[dst_row + px] = src_pixel.0;
                } else {
                    let dst_idx = dst_row + px;
                    let dst_pixel = Color(self.buf[dst_idx]);
                    let sr = src_pixel.r() as f32;
                    let sg = src_pixel.g() as f32;
                    let sb = src_pixel.b() as f32;
                    let dr = dst_pixel.r() as f32;
                    let dg = dst_pixel.g() as f32;
                    let db = dst_pixel.b() as f32;
                    let out_r = (sr * alpha + dr * (1.0 - alpha)) as u32;
                    let out_g = (sg * alpha + dg * (1.0 - alpha)) as u32;
                    let out_b = (sb * alpha + db * (1.0 - alpha)) as u32;
                    self.buf[dst_idx] = Color::rgb(out_r as u8, out_g as u8, out_b as u8).0;
                }
            }
        }
    }

    pub fn composit_shadow_alpha(
        &mut self,
        src: &LayerSystem,
        dx: usize,
        dy: usize,
        sx: usize,
        sy: usize,
        w: usize,
        h: usize,
    ) {
        let sw = src.width;
        let sh = src.height;
        let dw = self.width;
        let dh = self.height;

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh {
                continue;
            }

            let src_row = src_y * sw + sx;
            let dst_row = dst_y * dw + dx;
            let max_px = w.min(sw.saturating_sub(sx)).min(dw.saturating_sub(dx));

            for px in 0..max_px {
                let a = src.buf[src_row + px] & 0xFF;
                if a == 0 {
                    continue;
                }
                let inv = 255 - a;
                let idx = dst_row + px;
                let bg = self.buf[idx];
                let br = (bg >> 16) & 0xFF;
                let bg2 = (bg >> 8) & 0xFF;
                let bb = bg & 0xFF;
                let r = (br * inv) / 255;
                let g = (bg2 * inv) / 255;
                let b = (bb * inv) / 255;
                self.buf[idx] = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }
    }

    pub fn composit_rect(
        &mut self,
        src: &LayerSystem,
        dx: usize, dy: usize,
        sx: usize, sy: usize,
        w: usize, h: usize,
    ) {
        let sw = src.width;
        let sh = src.height;
        let dw = self.width;
        let dh = self.height;

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh { continue; }

            for px in 0..w {
                let src_x = sx + px;
                let dst_x = dx + px;
                if src_x >= sw || dst_x >= dw { continue; }

                let src_pixel = Color(src.buf[src_y * sw + src_x]);
                if src_pixel.0 == Color::TRANSPARENT.0 { continue; }

                self.buf[dst_y * dw + dst_x] = src_pixel.0;
            }
        }
    }

    pub fn composit_rect_alpha(
        &mut self,
        src: &LayerSystem,
        dx: usize, dy: usize,
        sx: usize, sy: usize,
        w: usize, h: usize,
    ) {
        let sw = src.width;
        let sh = src.height;
        let dw = self.width;
        let dh = self.height;

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh { continue; }

            for px in 0..w {
                let src_x = sx + px;
                let dst_x = dx + px;
                if src_x >= sw || dst_x >= dw { continue; }

                let sp = src.buf[src_y * sw + src_x];
                let src_a = ((sp >> 24) & 0xFF) as u32;
                if src_a == 0 { continue; }
                if src_a >= 255 {
                    self.buf[dst_y * dw + dst_x] = sp;
                } else {
                    let inv = 255 - src_a;
                    let sr = (sp >> 16) & 0xFF;
                    let sg = (sp >> 8) & 0xFF;
                    let sb = sp & 0xFF;
                    let dp = self.buf[dst_y * dw + dst_x];
                    let dr = (dp >> 16) & 0xFF;
                    let dg = (dp >> 8) & 0xFF;
                    let db = dp & 0xFF;
                    let r = (sr * src_a + dr * inv) / 255;
                    let g = (sg * src_a + dg * inv) / 255;
                    let b = (sb * src_a + db * inv) / 255;
                    self.buf[dst_y * dw + dst_x] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
                }
            }
        }
    }

    pub fn frame_count(&self) -> u64 { self.frame_count }
    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }
}
