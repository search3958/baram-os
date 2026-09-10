impl LayerSystem {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            width: w,
            height: h,
            buf: LayerBuffer::Owned(vec![Color::BG.0; w * h]),
            frame_count: 0,
            clip_stack: Vec::new(),
            clip: None,
            dirty: true,
            dirty_x0: 0,
            dirty_y0: 0,
            dirty_x1: w,
            dirty_y1: h,
        }
    }

    pub fn new_transparent(w: usize, h: usize) -> Self {
        Self {
            width: w,
            height: h,
            buf: LayerBuffer::Owned(vec![Color::TRANSPARENT.0; w * h]),
            frame_count: 0,
            clip_stack: Vec::new(),
            clip: None,
            dirty: true,
            dirty_x0: 0,
            dirty_y0: 0,
            dirty_x1: w,
            dirty_y1: h,
        }
    }

    /// Use the logical pixel buffer owned by Screen. This is intended for
    /// single-task fullscreen applications; normal compositors continue to
    /// use independent layers through `new` and `new_transparent`.
    pub fn new_screen_backed(screen: &mut Screen) -> Self {
        let w = screen.width();
        let h = screen.height();
        let (ptr, len) = screen.layer_buffer_ptr();
        Self {
            width: w,
            height: h,
            buf: LayerBuffer::Borrowed { ptr, len },
            frame_count: 0,
            clip_stack: Vec::new(),
            clip: None,
            dirty: true,
            dirty_x0: 0,
            dirty_y0: 0,
            dirty_x1: w,
            dirty_y1: h,
        }
    }

    pub fn push_clip(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        let x0 = x0.min(self.width);
        let y0 = y0.min(self.height);
        let x1 = x1.min(self.width);
        let y1 = y1.min(self.height);

        if let Some(cur) = self.clip {
            self.clip_stack.push(cur);
            self.clip = Some((x0.max(cur.0), y0.max(cur.1), x1.min(cur.2), y1.min(cur.3)));
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

    /// Current drawable bounds, or the complete layer when no clip is active.
    /// Direct pixel writers such as font rasterizers must honor this too.
    pub fn clip_bounds(&self) -> (usize, usize, usize, usize) {
        self.clip.unwrap_or((0, 0, self.width, self.height))
    }

    #[inline]
    fn mark_dirty_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        if !self.dirty {
            self.dirty = true;
            self.dirty_x0 = x0;
            self.dirty_y0 = y0;
            self.dirty_x1 = x1;
            self.dirty_y1 = y1;
        } else {
            if x0 < self.dirty_x0 {
                self.dirty_x0 = x0;
            }
            if y0 < self.dirty_y0 {
                self.dirty_y0 = y0;
            }
            if x1 > self.dirty_x1 {
                self.dirty_x1 = x1;
            }
            if y1 > self.dirty_y1 {
                self.dirty_y1 = y1;
            }
        }
    }

    pub fn mark_all_dirty(&mut self) {
        self.dirty = true;
        self.dirty_x0 = 0;
        self.dirty_y0 = 0;
        self.dirty_x1 = self.width;
        self.dirty_y1 = self.height;
    }

    pub fn take_dirty(&mut self) -> Option<(usize, usize, usize, usize)> {
        if self.dirty {
            let r = (self.dirty_x0, self.dirty_y0, self.dirty_x1, self.dirty_y1);
            self.dirty = false;
            self.dirty_x0 = self.width;
            self.dirty_y0 = self.height;
            self.dirty_x1 = 0;
            self.dirty_y1 = 0;
            Some(r)
        } else {
            None
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

    #[inline]
    fn clipped_rect(
        &self,
        x0: usize,
        y0: usize,
        x1: usize,
        y1: usize,
    ) -> Option<(usize, usize, usize, usize)> {
        let mut r = (
            x0.min(self.width),
            y0.min(self.height),
            x1.min(self.width),
            y1.min(self.height),
        );
        if let Some((cx0, cy0, cx1, cy1)) = self.clip {
            r.0 = r.0.max(cx0);
            r.1 = r.1.max(cy0);
            r.2 = r.2.min(cx1);
            r.3 = r.3.min(cy1);
        }
        if r.0 < r.2 && r.1 < r.3 {
            Some(r)
        } else {
            None
        }
    }

    #[inline]
    fn clipped_blit(
        &self,
        dx: usize,
        dy: usize,
        sx: usize,
        sy: usize,
        w: usize,
        h: usize,
        src_w: usize,
        src_h: usize,
    ) -> Option<(usize, usize, usize, usize, usize, usize)> {
        let (x0, y0, mut x1, mut y1) =
            self.clipped_rect(dx, dy, dx.saturating_add(w), dy.saturating_add(h))?;
        let nsx = sx.saturating_add(x0 - dx);
        let nsy = sy.saturating_add(y0 - dy);
        if nsx >= src_w || nsy >= src_h {
            return None;
        }
        x1 = x1.min(x0.saturating_add(src_w - nsx));
        y1 = y1.min(y0.saturating_add(src_h - nsy));
        if x0 >= x1 || y0 >= y1 {
            None
        } else {
            Some((x0, y0, nsx, nsy, x1 - x0, y1 - y0))
        }
    }

    /// Copy a screen-sized backing buffer, honoring the active damage clip.
    pub fn copy_from_screen_buffer(&mut self, src: &[u32]) {
        if src.len() < self.width * self.height {
            return;
        }
        let (x0, y0, x1, y1) = self.clip.unwrap_or((0, 0, self.width, self.height));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        self.mark_dirty_rect(x0, y0, x1, y1);
        if x0 == 0 && x1 == self.width {
            self.buf[y0 * self.width..y1 * self.width]
                .copy_from_slice(&src[y0 * self.width..y1 * self.width]);
        } else {
            for y in y0..y1 {
                let start = y * self.width + x0;
                let end = y * self.width + x1;
                self.buf[start..end].copy_from_slice(&src[start..end]);
            }
        }
    }

    /// Copy a rectangular source buffer at `(dx, dy)`, intersecting it with
    /// the current clip so unchanged rows never cross the memory bus.
    pub fn copy_rect_buffer(
        &mut self,
        src: &[u32],
        src_width: usize,
        src_height: usize,
        dx: usize,
        dy: usize,
    ) {
        if src_width == 0 || src_height == 0 || src.len() < src_width * src_height {
            return;
        }
        let Some((x0, y0, x1, y1)) = self.clipped_rect(
            dx,
            dy,
            dx.saturating_add(src_width),
            dy.saturating_add(src_height),
        ) else {
            return;
        };
        self.mark_dirty_rect(x0, y0, x1, y1);
        let sx0 = x0 - dx;
        for y in y0..y1 {
            let sy = y - dy;
            let src_start = sy * src_width + sx0;
            let len = x1 - x0;
            let dst_start = y * self.width + x0;
            self.buf[dst_start..dst_start + len].copy_from_slice(&src[src_start..src_start + len]);
        }
    }

    pub fn clear(&mut self, c: Color) {
        self.buf.fill(c.0);
        self.mark_all_dirty();
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
            if x0 >= x1 || y0 >= y1 {
                return;
            }
            self.mark_dirty_rect(x0, y0, x1, y1);
            for yy in y0..y1 {
                self.buf[yy * stride + x0..yy * stride + x1].fill(v);
            }
        } else {
            let x0 = x.min(stride);
            let y0 = y.min(self.height);
            let x1 = (x + w).min(stride);
            let y1 = (y + h).min(self.height);
            if x0 >= x1 || y0 >= y1 {
                return;
            }
            self.mark_dirty_rect(x0, y0, x1, y1);
            for yy in y0..y1 {
                self.buf[yy * stride + x0..yy * stride + x1].fill(v);
            }
        }
    }

    /// Fill a rectangle whose layout position may be outside the layer.
    /// Keeping the signed origin is important for scrolling: clamping the
    /// origin before drawing turns an off-screen item into a sticky item at
    /// the top-left corner.
    pub fn fill_rect_signed(&mut self, x: i32, y: i32, w: usize, h: usize, c: Color) {
        if w == 0 || h == 0 {
            return;
        }
        let right = x.saturating_add(w.min(i32::MAX as usize) as i32);
        let bottom = y.saturating_add(h.min(i32::MAX as usize) as i32);
        let x0 = x.max(0).min(self.width as i32) as usize;
        let y0 = y.max(0).min(self.height as i32) as usize;
        let x1 = right.max(0).min(self.width as i32) as usize;
        let y1 = bottom.max(0).min(self.height as i32) as usize;
        if x0 < x1 && y0 < y1 {
            self.fill_rect(x0, y0, x1 - x0, y1 - y0, c);
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

    fn compute_squircle_polygon(w: f32, h: f32, r: f32) -> alloc::vec::Vec<(f32, f32)> {
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
        // Three segments per cubic keep the squircle silhouette while
        // reducing point-in-polygon work for every anti-aliased edge pixel.
        let segs = 3;

        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(w, w, w, w - cx6, t),
                Self::cubic_bezier(h / 2.0, h - ly + d1y, h - ly + d2y, h - cy3, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(w - cx6, w - cx5, w - cx4, w - cx3, t),
                Self::cubic_bezier(h - cy3, h - cy4, h - cy5, h - cy6, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(w - cx3, w - lx + d2x, w - lx + d1x, w - lx, t),
                Self::cubic_bezier(h - cy6, h, h, h, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(w - lx, lx, lx, lx, t), h));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(lx, lx - d1x, lx - d2x, cx3, t),
                Self::cubic_bezier(h, h, h, h - cy6, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(cx3, cx4, cx5, cx6, t),
                Self::cubic_bezier(h - cy6, h - cy5, h - cy4, h - cy3, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(cx6, 0.0, 0.0, 0.0, t),
                Self::cubic_bezier(h - cy3, h - ly + d2y, h - ly + d1y, h - ly, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((0.0, Self::cubic_bezier(h - ly, ly, ly, ly, t)));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(0.0, 0.0, 0.0, cx6, t),
                Self::cubic_bezier(ly, ly - d1y, ly - d2y, cy3, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(cx6, cx5, cx4, cx3, t),
                Self::cubic_bezier(cy3, cy4, cy5, cy6, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(cx3, lx - d2x, lx - d1x, lx, t),
                Self::cubic_bezier(cy6, 0.0, 0.0, 0.0, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((Self::cubic_bezier(lx, w - lx, w - lx, w - lx, t), 0.0));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(w - lx, w - lx + d1x, w - lx + d2x, w - cx3, t),
                Self::cubic_bezier(0.0, 0.0, 0.0, cy6, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(w - cx3, w - cx4, w - cx5, w - cx6, t),
                Self::cubic_bezier(cy6, cy5, cy4, cy3, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((
                Self::cubic_bezier(w - cx6, w, w, w, t),
                Self::cubic_bezier(cy3, ly - d2y, ly - d1y, ly, t),
            ));
        }
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            pts.push((w, Self::cubic_bezier(ly, h - ly, h - ly, h - ly, t)));
        }
        pts
    }

}
