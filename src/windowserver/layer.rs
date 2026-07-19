use alloc::vec;
use alloc::vec::Vec;
use core::ptr;
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

    fn cubic_bezier(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;
        mt3 * p0 + 3.0 * mt2 * t * p1 + 3.0 * mt * t2 * p2 + t3 * p3
    }

    pub fn squircle_polygon(w: f32, h: f32, r: f32) -> alloc::vec::Vec<(f32, f32)> {
        let r = r.min(w / 2.0).min(h / 2.0);
        let lx = libm::fminf(w / 2.0, 1.528665 * r);
        let ly = libm::fminf(h / 2.0, 1.528665 * r);

        let cx3 = 0.63148 * r;
        let cx4 = 0.37282 * r;
        let cx5 = 0.16905 * r;
        let cx6 = 0.07491 * r;
        let cy3 = cx3;
        let cy4 = cx4;
        let cy5 = cx5;
        let cy6 = cx6;

        let d1x = 0.04 * r + 0.75697 * (lx - r);
        let d2x = 0.18 * r + 0.90847 * (lx - r);
        let d1y = 0.04 * r + 0.75697 * (ly - r);
        let d2y = 0.18 * r + 0.90847 * (ly - r);

        let mut pts = alloc::vec::Vec::new();
        let segs = 64;

        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(w, w, w, w - cx6, t),
                       Self::cubic_bezier(h / 2.0, h - ly + d1y, h - ly + d2y, h - cy3, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(w - cx6, w - cx5, w - cx4, w - cx3, t),
                       Self::cubic_bezier(h - cy3, h - cy4, h - cy5, h - cy6, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(w - cx3, w - lx + d2x, w - lx + d1x, w - lx, t),
                       Self::cubic_bezier(h - cy6, h, h, h, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(w - lx, lx, lx, lx, t), h));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(lx, lx - d1x, lx - d2x, cx3, t),
                       Self::cubic_bezier(h, h, h, h - cy6, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(cx3, cx4, cx5, cx6, t),
                       Self::cubic_bezier(h - cy6, h - cy5, h - cy4, h - cy3, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(cx6, 0.0, 0.0, 0.0, t),
                       Self::cubic_bezier(h - cy3, h - ly + d2y, h - ly + d1y, h - ly, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((0.0, Self::cubic_bezier(h - ly, ly, ly, ly, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(0.0, 0.0, 0.0, cx6, t),
                       Self::cubic_bezier(ly, ly - d1y, ly - d2y, cy3, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(cx6, cx5, cx4, cx3, t),
                       Self::cubic_bezier(cy3, cy4, cy5, cy6, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(cx3, lx - d2x, lx - d1x, lx, t),
                       Self::cubic_bezier(cy6, 0.0, 0.0, 0.0, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(lx, w - lx, w - lx, w - lx, t), 0.0));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(w - lx, w - lx + d1x, w - lx + d2x, w - cx3, t),
                       Self::cubic_bezier(0.0, 0.0, 0.0, cy6, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(w - cx3, w - cx4, w - cx5, w - cx6, t),
                       Self::cubic_bezier(cy6, cy5, cy4, cy3, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(w - cx6, w, w, w, t),
                       Self::cubic_bezier(cy3, ly - d2y, ly - d1y, ly, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((w, Self::cubic_bezier(ly, h - ly, h - ly, h - ly, t)));
        }
        pts
    }

    pub fn point_in_polygon(px: f32, py: f32, poly: &[(f32, f32)]) -> bool {
        let n = poly.len();
        if n < 3 { return false; }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = poly[i];
            let (xj, yj) = poly[j];
            if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    pub fn blend_alpha(bg: u32, fg: u32, alpha: f32) -> u32 {
        if alpha >= 1.0 { return fg; }
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
        let v = c.0;
        let y0 = y.min(self.height);
        let y1 = (y + h).min(self.height);
        let x0 = x.min(self.width);
        let x1 = (x + w).min(self.width);
        let stride = self.width;

        if r == 0 {
            for py in y0..y1 {
                self.buf[py * stride + x0..py * stride + x1].fill(v);
            }
            return;
        }

        let rf = r as f32;
        let poly = Self::squircle_polygon(w as f32, h as f32, rf);
        let x0f = x as f32;
        let y0f = y as f32;
        let r_f = r as f32;
        let w_f = w as f32;
        let h_f = h as f32;
        let lx = libm::fminf(w_f / 2.0, 1.528665 * rf);
        let ly = libm::fminf(h_f / 2.0, 1.528665 * rf);
        let mx = (lx * 0.7) as usize;
        let my = (ly * 0.7) as usize;
        let off = [0.25f32, 0.75f32];

        for py in y0..y1 {
            let row = py * stride;
            let base_y = py as f32 - y0f;

            let in_corner_row = base_y < r_f || base_y >= h_f - r_f;

            if !in_corner_row {
                self.buf[row + x0..row + x1].fill(v);
                continue;
            }

            let corner_x_end = (x + r).min(x1);
            let mid_x_start = (x + r).max(x0);
            let mid_x_end = (x + w - r).min(x1);
            let corner_x_start = (x + w - r).max(x0);

            if mid_x_end > mid_x_start {
                self.buf[row + mid_x_start..row + mid_x_end].fill(v);
            }

            if corner_x_end > x0 {
                let safe_l = (x + mx).min(corner_x_end).max(x0);
                if safe_l > x0 {
                    for px in x0..safe_l {
                        Self::pixel_aa(&mut self.buf[row + px], v, px as f32 - x0f, base_y, &poly, &off);
                    }
                }
                let inner_l_end = (x + r - mx).max(x + mx).min(corner_x_end);
                if inner_l_end > safe_l {
                    self.buf[row + safe_l..row + inner_l_end].fill(v);
                }
                if corner_x_end > inner_l_end {
                    for px in inner_l_end..corner_x_end {
                        Self::pixel_aa(&mut self.buf[row + px], v, px as f32 - x0f, base_y, &poly, &off);
                    }
                }
            }

            if corner_x_start > corner_x_end {
                let safe_r_start = (x + w - r + mx).max(corner_x_start).min(x1);
                if safe_r_start > corner_x_start {
                    for px in corner_x_start..safe_r_start {
                        Self::pixel_aa(&mut self.buf[row + px], v, px as f32 - x0f, base_y, &poly, &off);
                    }
                }
                let inner_r_end = (x + w - mx).min(x1).max(safe_r_start);
                if inner_r_end > safe_r_start {
                    self.buf[row + safe_r_start..row + inner_r_end].fill(v);
                }
                if x1 > inner_r_end {
                    for px in inner_r_end..x1 {
                        Self::pixel_aa(&mut self.buf[row + px], v, px as f32 - x0f, base_y, &poly, &off);
                    }
                }
            }
        }
    }

    fn pixel_aa(dst: &mut u32, fg: u32, px: f32, py: f32, poly: &[(f32, f32)], off: &[f32; 2]) {
        let mut hits = 0u32;
        for sy in 0..2 {
            for sx in 0..2 {
                if Self::point_in_polygon(px + off[sx], py + off[sy], poly) {
                    hits += 1;
                }
            }
        }
        if hits > 0 {
            *dst = Self::blend_alpha(*dst, fg, hits as f32 * 0.25);
        }
    }

    fn pixel_aa_batch(dst: &mut [u32], fg: u32, px: [f32; 4], py: f32, poly: &[(f32, f32)], off: &[f32; 2], count: usize) {
        if count < 4 {
            for i in 0..count {
                Self::pixel_aa(&mut dst[i], fg, px[i], py, poly, off);
            }
            return;
        }

        let n = poly.len();
        if n < 3 { return; }

        #[cfg(target_arch = "x86_64")]
        unsafe {
            use core::arch::x86_64::*;

            let o0 = _mm_set1_ps(off[0]);
            let o1 = _mm_set1_ps(off[1]);
            let pxv = _mm_set_ps(px[3], px[2], px[1], px[0]);
            let sample_x0 = _mm_add_ps(pxv, o0);
            let sample_x1 = _mm_add_ps(pxv, o1);

            let mut inside = _mm_setzero_si128();
            let mut j = n - 1;
            for i in 0..n {
                let (ax, ay) = poly[i];
                let (bx, by) = poly[j];
                if ((ay > py) != (by > py)) {
                    let ey = by - ay;
                    let inv_ey = if ey.abs() > 1e-10 { 1.0 / ey } else { 0.0 };
                    let x_int = _mm_set1_ps(ax + (py - ay) * (bx - ax) * inv_ey);
                    let cmp0 = _mm_castps_si128(_mm_cmplt_ps(sample_x0, x_int));
                    let cmp1 = _mm_castps_si128(_mm_cmplt_ps(sample_x1, x_int));
                    let bits = _mm_and_si128(_mm_or_si128(cmp0, cmp1), _mm_set1_epi32(1));
                    inside = _mm_xor_si128(inside, bits);
                }
                j = i;
            }

            let mut inside_arr = [0u32; 4];
            _mm_storeu_si128(inside_arr.as_mut_ptr() as *mut __m128i, inside);

            for p in 0..4 {
                if inside_arr[p] != 0 {
                    dst[p] = Self::blend_alpha(dst[p], fg, 0.25);
                }
            }
            return;
        }

        #[cfg(not(target_arch = "x86_64"))]
        for i in 0..count {
            Self::pixel_aa(&mut dst[i], fg, px[i], py, poly, off);
        }
    }

    pub fn blend_alpha(bg: u32, fg: u32, alpha: f32) -> u32 {
        if alpha >= 1.0 { return fg; }
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
            let poly = Self::squircle_polygon(w as f32, h as f32, rf);
            let off = [0.25f32, 0.75f32];
            let base_y = py as f32;
            let lx = libm::fminf(w as f32 / 2.0, 1.528665 * rf);
            let ly = libm::fminf(h as f32 / 2.0, 1.528665 * rf);
            let mx = (lx * 0.7) as usize;
            let my = (ly * 0.7) as usize;

            let left_end = r.min(end_x);
            let right_start = (w - r).min(end_x);

            let safe_l = mx.min(left_end);
            if safe_l > 0 {
                for px in 0..safe_l {
                    let sp = src.buf[src_row + px];
                    if sp == Color::TRANSPARENT.0 { continue; }
                    Self::pixel_aa(&mut self.buf[dst_row + px], sp, px as f32, base_y, &poly, &off);
                }
            }
            let inner_l_end = (r - mx).max(mx).min(left_end);
            if inner_l_end > safe_l {
                for px in safe_l..inner_l_end {
                    let sp = src.buf[src_row + px];
                    if sp != Color::TRANSPARENT.0 {
                        self.buf[dst_row + px] = sp;
                    }
                }
            }
            if left_end > inner_l_end {
                for px in inner_l_end..left_end {
                    let sp = src.buf[src_row + px];
                    if sp == Color::TRANSPARENT.0 { continue; }
                    Self::pixel_aa(&mut self.buf[dst_row + px], sp, px as f32, base_y, &poly, &off);
                }
            }

            for px in left_end..right_start {
                let sp = src.buf[src_row + px];
                if sp != Color::TRANSPARENT.0 {
                    self.buf[dst_row + px] = sp;
                }
            }

            if end_x > right_start {
                let safe_r = (right_start + mx).min(end_x);
                if safe_r > right_start {
                    for px in right_start..safe_r {
                        let sp = src.buf[src_row + px];
                        if sp == Color::TRANSPARENT.0 { continue; }
                        Self::pixel_aa(&mut self.buf[dst_row + px], sp, px as f32, base_y, &poly, &off);
                    }
                }
                let inner_r_end = (w - mx).min(end_x).max(safe_r);
                if inner_r_end > safe_r {
                    for px in safe_r..inner_r_end {
                        let sp = src.buf[src_row + px];
                        if sp != Color::TRANSPARENT.0 {
                            self.buf[dst_row + px] = sp;
                        }
                    }
                }
                if end_x > inner_r_end {
                    for px in inner_r_end..end_x {
                        let sp = src.buf[src_row + px];
                        if sp == Color::TRANSPARENT.0 { continue; }
                        Self::pixel_aa(&mut self.buf[dst_row + px], sp, px as f32, base_y, &poly, &off);
                    }
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
