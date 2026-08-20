impl LayerSystem {
    pub fn squircle_polygon(w: f32, h: f32, r: f32) -> alloc::vec::Vec<(f32, f32)> {
        Self::compute_squircle_polygon(w, h, r)
    }

    fn cached_squircle(w: f32, h: f32, r: f32) -> &'static alloc::vec::Vec<(f32, f32)> {
        use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
        static CACHED: AtomicBool = AtomicBool::new(false);
        static CW: AtomicU32 = AtomicU32::new(0);
        static CH: AtomicU32 = AtomicU32::new(0);
        static CR: AtomicU32 = AtomicU32::new(0);
        static mut POLY: alloc::vec::Vec<(f32, f32)> = alloc::vec::Vec::new();

        let wi = (w * 100.0) as u32;
        let hi = (h * 100.0) as u32;
        let ri = (r * 100.0) as u32;

        if CACHED.load(Ordering::Relaxed)
            && CW.load(Ordering::Relaxed) == wi
            && CH.load(Ordering::Relaxed) == hi
            && CR.load(Ordering::Relaxed) == ri
        {
            unsafe {
                return &POLY;
            }
        }

        let poly = Self::compute_squircle_polygon(w, h, r);
        unsafe {
            POLY = poly;
        }
        CW.store(wi, Ordering::Relaxed);
        CH.store(hi, Ordering::Relaxed);
        CR.store(ri, Ordering::Relaxed);
        CACHED.store(true, Ordering::Relaxed);
        unsafe { &POLY }
    }

    #[inline]
    pub fn point_in_polygon(px: f32, py: f32, poly: &[(f32, f32)]) -> bool {
        let n = poly.len();
        if n < 3 {
            return false;
        }
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

    #[inline]
    fn squircle_row_bounds(poly: &[(f32, f32)], py: f32) -> Option<(f32, f32)> {
        let n = poly.len();
        if n < 3 {
            return None;
        }
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut hits = 0usize;
        let mut j = n - 1;
        for i in 0..n {
            let (x0, y0) = poly[j];
            let (x1, y1) = poly[i];
            if (y0 > py) != (y1 > py) {
                let x = x0 + (py - y0) * (x1 - x0) / (y1 - y0);
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                hits += 1;
            }
            j = i;
        }
        if hits >= 2 {
            Some((min_x, max_x))
        } else {
            None
        }
    }

    #[inline]
    fn pixel_span_from_bounds(left: f32, right: f32, max_w: usize) -> (usize, usize) {
        // Only classify pixels as fully covered when the whole pixel lies
        // inside the shape. Center-based rounding makes partially covered
        // pixels opaque and is the reason the anti-aliasing looks washed out.
        let start = libm::ceilf(left).max(0.0) as usize;
        let end = libm::floorf(right).max(0.0) as usize;
        (start.min(max_w), end.min(max_w))
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

    pub fn fill_rounded_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        r: usize,
        c: Color,
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let r = r.min(w / 2).min(h / 2);
        if r == 0 {
            self.fill_rect(x, y, w, h, c);
            return;
        }
        let poly = Self::cached_squircle(w as f32, h as f32, r as f32);
        self.fill_rounded_rect_with_polygon(x, y, w, h, r, c, poly);
    }

    /// Fill a rounded rectangle with direct circular-corner coverage.
    ///
    /// This is intentionally separate from the cached squircle path. Xiao's
    /// 128x64 surface makes a low-resolution polygon edge very obvious, so
    /// its small controls use 8x8 subpixel samples against the exact rounded
    /// rectangle instead of a short polygon approximation.
    pub fn fill_rounded_rect_aa(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        r: usize,
        c: Color,
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let r = r.min(w / 2).min(h / 2);
        if r == 0 {
            self.fill_rect(x, y, w, h, c);
            return;
        }
        let Some((x0, y0, x1, y1)) =
            self.clipped_rect(x, y, x.saturating_add(w), y.saturating_add(h))
        else {
            return;
        };

        const SAMPLES: usize = 8;
        const SAMPLE_COUNT: f32 = (SAMPLES * SAMPLES) as f32;
        let radius = r as f32;
        let right_inner = w as f32 - radius;
        let bottom_inner = h as f32 - radius;
        let value = c.0;
        self.mark_dirty_rect(x0, y0, x1, y1);

        for py in y0..y1 {
            let row = py * self.width;
            let local_y = py.saturating_sub(y);
            for px in x0..x1 {
                let local_x = px.saturating_sub(x);

                // Pixels in either central strip cannot intersect a corner,
                // so keep the fast opaque fill for the bulk of the control.
                if (local_x >= r && local_x + 1 <= w - r)
                    || (local_y >= r && local_y + 1 <= h - r)
                {
                    self.buf[row + px] = value;
                    continue;
                }

                let mut hits = 0usize;
                for sy in 0..SAMPLES {
                    let sample_y = local_y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    let nearest_y = sample_y.clamp(radius, bottom_inner);
                    let dy = sample_y - nearest_y;
                    for sx in 0..SAMPLES {
                        let sample_x =
                            local_x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                        let nearest_x = sample_x.clamp(radius, right_inner);
                        let dx = sample_x - nearest_x;
                        if dx * dx + dy * dy <= radius * radius {
                            hits += 1;
                        }
                    }
                }

                if hits == SAMPLES * SAMPLES {
                    self.buf[row + px] = value;
                } else if hits != 0 {
                    self.buf[row + px] = Self::blend_alpha(
                        self.buf[row + px],
                        value,
                        hits as f32 / SAMPLE_COUNT,
                    );
                }
            }
        }
    }

    /// Fill a rounded rectangle using caller-owned geometry. This variant is
    /// allocation-free and can be used by independent AP rendering jobs.
    pub fn fill_rounded_rect_with_polygon(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        r: usize,
        c: Color,
        poly: &[(f32, f32)],
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let r = r.min(w / 2).min(h / 2);
        if r == 0 {
            self.fill_rect(x, y, w, h, c);
            return;
        }
        let v = c.0;
        let Some((x0, y0, x1, y1)) =
            self.clipped_rect(x, y, x.saturating_add(w), y.saturating_add(h))
        else {
            return;
        };
        let stride = self.width;

        self.mark_dirty_rect(x0, y0, x1, y1);
        let x0f = x as f32;
        let y0f = y as f32;
        let r_f = r as f32;
        let h_f = h as f32;

        for py in y0..y1 {
            let row = py * stride;
            let base_y = py as f32 - y0f;

            let in_corner_row = base_y < r_f || base_y >= h_f - r_f;

            if !in_corner_row {
                // The entire straight section is opaque. Keep this as one
                // contiguous fill so rounded controls do not pay for any
                // alpha or geometry work in their rectangular interior.
                self.buf[row + x0..row + x1].fill(v);
                continue;
            }

            if let Some((left, right)) = Self::squircle_row_bounds(poly, base_y + 0.5) {
                let (span_l, span_r) = Self::pixel_span_from_bounds(left, right, w);
                let fill_l = x.saturating_add(span_l).max(x0).min(x1);
                let fill_r = x.saturating_add(span_r).max(x0).min(x1);
                if fill_l < fill_r {
                    self.buf[row + fill_l..row + fill_r].fill(v);
                }
                let aa_bounds = Self::aa_row_bounds(poly, base_y);

                let edge_l = x + span_l.saturating_sub(1);
                if edge_l >= x0 && edge_l < x1 {
                    Self::pixel_aa(
                        &mut self.buf[row + edge_l],
                        v,
                        edge_l as f32 - x0f,
                        &aa_bounds,
                        true,
                    );
                }
                let edge_r = x + span_r;
                if edge_r >= x0 && edge_r < x1 {
                    Self::pixel_aa(
                        &mut self.buf[row + edge_r],
                        v,
                        edge_r as f32 - x0f,
                        &aa_bounds,
                        false,
                    );
                }
            }
        }
    }

    #[inline(always)]
    fn aa_row_bounds(poly: &[(f32, f32)], py: f32) -> [Option<(f32, f32)>; 4] {
        [
            Self::squircle_row_bounds(poly, py + 0.125),
            Self::squircle_row_bounds(poly, py + 0.375),
            Self::squircle_row_bounds(poly, py + 0.625),
            Self::squircle_row_bounds(poly, py + 0.875),
        ]
    }

    #[inline(always)]
    fn pixel_aa(
        dst: &mut u32,
        fg: u32,
        px: f32,
        bounds: &[Option<(f32, f32)>; 4],
        left_edge: bool,
    ) {
        let mut hits = 0u32;
        // Only the two edge pixels of each scanline use this path. Rather than
        // running a full point-in-polygon test for every 4x4 sample, calculate
        // the two scanline boundaries once per sub-row and test the relevant
        // side only. This preserves the AA coverage while removing most of
        // the divisions and polygon walks from rounded-rect rendering.
        for bounds in bounds.iter().flatten() {
            let (left, right) = *bounds;
            for sx in 0..4 {
                let sample_x = px + (sx as f32 + 0.5) * 0.25;
                let inside = if left_edge {
                    sample_x >= left
                } else {
                    sample_x <= right
                };
                if inside {
                    hits += 1;
                }
            }
        }
        if hits >= 16 {
            *dst = fg;
        } else if hits > 0 {
            *dst = Self::blend_alpha(*dst, fg, hits as f32 * (1.0 / 16.0));
        }
    }

    pub fn fill_circle(&mut self, cx: usize, cy: usize, r: usize, c: Color) {
        if r == 0 {
            return;
        }
        let rf = r as f32;
        let cr = c.r() as f32;
        let cg = c.g() as f32;
        let cb = c.b() as f32;
        let Some((x0, y0, x1, y1)) = self.clipped_rect(
            cx.saturating_sub(r),
            cy.saturating_sub(r),
            cx.saturating_add(r + 1),
            cy.saturating_add(r + 1),
        ) else {
            return;
        };
        self.mark_dirty_rect(x0, y0, x1, y1);
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

    pub fn rounded_rect_outline(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        r: usize,
        c: Color,
        fill: Color,
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let r = r.min(w / 2).min(h / 2);
        self.fill_rounded_rect(x, y, w, h, r, c);
        // Keep the outline one device pixel wide.  The native Warp3/Warp4
        // controls use a one-pixel border; a two-pixel inset makes buttons
        // and text fields look visibly heavier than their reference UI.
        let inner_r = r.saturating_sub(1);
        self.fill_rounded_rect(
            x + 1,
            y + 1,
            w.saturating_sub(2),
            h.saturating_sub(2),
            inner_r,
            fill,
        );
    }

    pub fn rect_outline(&mut self, x: usize, y: usize, w: usize, h: usize, c: Color) {
        if w == 0 || h == 0 {
            return;
        }
        self.fill_rect(x, y, w, 1, c);
        self.fill_rect(x, y + h - 1, w, 1, c);
        self.fill_rect(x, y, 1, h, c);
        self.fill_rect(x + w - 1, y, 1, h, c);
    }

    pub fn flush(&mut self, screen: &mut Screen) {
        let w = self.width;
        let h = self.height;
        if let Some((x0, y0, x1, y1)) = self.take_dirty() {
            let x0 = x0.min(w);
            let y0 = y0.min(h);
            let x1 = x1.min(w);
            let y1 = y1.min(h);
            if x1 > x0 && y1 > y0 {
                for y in y0..y1 {
                    let row = &self.buf[y * w + x0..y * w + x1];
                    screen.flush_layer_row_range(y, x0, row);
                }
            }
        }
        self.frame_count += 1;
    }

    pub fn flush_rect(&mut self, screen: &mut Screen, x0: usize, y0: usize, x1: usize, y1: usize) {
        let w = self.width;
        let y0 = y0.min(self.height);
        let y1 = y1.min(self.height);
        let x0 = x0.min(w);
        let x1 = x1.min(w);
        for y in y0..y1 {
            let row = &self.buf[y * w + x0..y * w + x1];
            screen.flush_layer_row_range(y, x0, row);
        }
        let _ = self.take_dirty();
        self.frame_count += 1;
    }

}
