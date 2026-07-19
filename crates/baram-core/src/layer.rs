use alloc::vec;
use alloc::vec::Vec;
use core::ptr;
use crate::color::Color;
use crate::screen::Screen;

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

    pub fn corner_sdf_alpha(px: usize, py: usize, x: usize, y: usize, w: usize, h: usize, rf: f32) -> f32 {
        let r = rf as usize;
        let in_corner = (px < x + r && py < y + r)
            || (px >= x + w.saturating_sub(r) && py < y + r)
            || (px < x + r && py >= y + h.saturating_sub(r))
            || (px >= x + w.saturating_sub(r) && py >= y + h.saturating_sub(r));
        if !in_corner {
            return 1.0;
        }
        let cx_f = if px < x + r { x + r } else { x + w - r } as f32;
        let cy_f = if py < y + r { y + r } else { y + h - r } as f32;
        let dx = px as f32 + 0.5 - cx_f;
        let dy = py as f32 + 0.5 - cy_f;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq < (rf - 0.5) * (rf - 0.5) {
            1.0
        } else if dist_sq > (rf + 0.5) * (rf + 0.5) {
            0.0
        } else {
            let dist = libm::sqrtf(dist_sq);
            (rf + 0.5 - dist).clamp(0.0, 1.0)
        }
    }

    pub fn blend_alpha(bg: u32, fg: u32, alpha: f32) -> u32 {
        if alpha >= 1.0 {
            return fg;
        }
        let cr = ((fg >> 16) & 0xFF) as f32;
        let cg = ((fg >> 8) & 0xFF) as f32;
        let cb = (fg & 0xFF) as f32;
        let br = ((bg >> 16) & 0xFF) as f32;
        let bg2 = ((bg >> 8) & 0xFF) as f32;
        let bb = (bg & 0xFF) as f32;
        let r = (cr * alpha + br * (1.0 - alpha)) as u32;
        let g = (cg * alpha + bg2 * (1.0 - alpha)) as u32;
        let b = (cb * alpha + bb * (1.0 - alpha)) as u32;
        Color::rgb(r as u8, g as u8, b as u8).0
    }

    pub fn fill_rounded_rect(&mut self, x: usize, y: usize, w: usize, h: usize, r: usize, c: Color) {
        if w == 0 || h == 0 { return; }
        let r = r.min(w / 2).min(h / 2);
        let rf = r as f32;
        let v = c.0;
        let y0 = y.min(self.height);
        let y1 = (y + h).min(self.height);
        let x0 = x.min(self.width);
        let x1 = (x + w).min(self.width);
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
                let alpha = Self::corner_sdf_alpha(px, py, x, y, w, h, rf);
                if alpha <= 0.0 { continue; }
                self.buf[row + px] = Self::blend_alpha(self.buf[row + px], v, alpha);
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
        let v = c.0;
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
                    self.put_pixel(px, py, Color(Self::blend_alpha(bg, v, alpha)));
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

                let mut all_opaque = true;
                for px in 0..copy_w {
                    if src.buf[src_row_start + px] == Color::TRANSPARENT.0 {
                        all_opaque = false;
                        break;
                    }
                }

                if all_opaque {
                    unsafe {
                        ptr::copy_nonoverlapping(
                            src.buf.as_ptr().add(src_row_start),
                            self.buf.as_mut_ptr().add(dst_row_start),
                            copy_w,
                        );
                    }
                } else {
                    for px in 0..copy_w {
                        let sp = src.buf[src_row_start + px];
                        if sp != Color::TRANSPARENT.0 {
                            self.buf[dst_row_start + px] = sp;
                        }
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

                let mut all_opaque = true;
                for px in 0..copy_w {
                    if src.buf[src_row + px] == Color::TRANSPARENT.0 {
                        all_opaque = false;
                        break;
                    }
                }

                if all_opaque {
                    unsafe {
                        ptr::copy_nonoverlapping(
                            src.buf.as_ptr().add(src_row),
                            self.buf.as_mut_ptr().add(dst_row),
                            copy_w,
                        );
                    }
                } else {
                    for px in 0..copy_w {
                        let sp = src.buf[src_row + px];
                        if sp != Color::TRANSPARENT.0 {
                            self.buf[dst_row + px] = sp;
                        }
                    }
                }
                continue;
            }

            let end_x = w.min(sw - sx).min(dw - dx);
            for px in 0..end_x {
                let sp = src.buf[src_row + px];
                if sp == Color::TRANSPARENT.0 { continue; }

                let alpha = Self::corner_sdf_alpha(px, py, 0, 0, w, h, rf);
                if alpha <= 0.0 { continue; }

                self.buf[dst_row + px] = Self::blend_alpha(self.buf[dst_row + px], sp, alpha);
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

        let copy_w = w.min(sw.saturating_sub(sx)).min(dw.saturating_sub(dx));
        if copy_w == 0 { return; }

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh { continue; }

            let src_row_start = src_y * sw + sx;
            let dst_row_start = dst_y * dw + dx;

            let mut all_opaque = true;
            for px in 0..copy_w {
                if src.buf[src_row_start + px] == Color::TRANSPARENT.0 {
                    all_opaque = false;
                    break;
                }
            }

            if all_opaque {
                unsafe {
                    ptr::copy_nonoverlapping(
                        src.buf.as_ptr().add(src_row_start),
                        self.buf.as_mut_ptr().add(dst_row_start),
                        copy_w,
                    );
                }
            } else {
                for px in 0..copy_w {
                    let sp = src.buf[src_row_start + px];
                    if sp != Color::TRANSPARENT.0 {
                        self.buf[dst_row_start + px] = sp;
                    }
                }
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
