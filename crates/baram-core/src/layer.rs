use crate::color::Color;
use crate::screen::Screen;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr;

#[inline(always)]
fn blend_u32(bg: u32, fg: u32, a: u32) -> u32 {
    if a == 0 {
        return bg;
    }
    if a >= 255 {
        return fg;
    }
    let inv = 255 - a;
    let r = (((fg >> 16) & 0xFF) * a + ((bg >> 16) & 0xFF) * inv) / 255;
    let g = (((fg >> 8) & 0xFF) * a + ((bg >> 8) & 0xFF) * inv) / 255;
    let b = ((fg & 0xFF) * a + (bg & 0xFF) * inv) / 255;
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn avx2_available() -> bool {
    use core::arch::x86_64::{__cpuid, __cpuid_count, _xgetbv};
    use core::sync::atomic::{AtomicU8, Ordering};

    // CPU/OS AVX state does not change while this UEFI image is running.
    static AVAILABLE: AtomicU8 = AtomicU8::new(0); // 0 unknown, 1 no, 2 yes
    match AVAILABLE.load(Ordering::Relaxed) {
        1 => return false,
        2 => return true,
        _ => {}
    }

    let available = unsafe {
        let leaf1 = __cpuid(1);
        const AVX: u32 = 1 << 28;
        const OSXSAVE: u32 = 1 << 27;
        if leaf1.ecx & (AVX | OSXSAVE) != (AVX | OSXSAVE) || (_xgetbv(0) & 0x6) != 0x6 {
            false
        } else {
            (__cpuid_count(7, 0).ebx & (1 << 5)) != 0
        }
    };
    AVAILABLE.store(if available { 2 } else { 1 }, Ordering::Relaxed);
    available
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn blend_alpha_avx2(src: *const u32, dst: *mut u32, len: usize) {
    use core::arch::x86_64::*;

    let zero = _mm256_setzero_si256();
    let full_alpha = _mm256_set1_epi32(255);
    let mut px = 0usize;

    while px + 8 <= len {
        let sp = _mm256_loadu_si256(src.add(px) as *const __m256i);
        let a = _mm256_srli_epi32(sp, 24);
        let zero_alpha = _mm256_cmpeq_epi32(a, zero);
        if _mm256_movemask_epi8(zero_alpha) == -1 {
            px += 8;
            continue;
        }

        let opaque_alpha = _mm256_cmpeq_epi32(a, full_alpha);
        if _mm256_movemask_epi8(opaque_alpha) == -1 {
            _mm256_storeu_si256(dst.add(px) as *mut __m256i, sp);
            px += 8;
            continue;
        }

        // Mixed-alpha blocks are relatively rare in the compositor. Keep the
        // exact scalar blend here: LLVM's x86 UEFI legalizer crashes on the
        // AVX2 32-bit multiply sequence during fat LTO. Fully transparent and
        // fully opaque blocks still take the 8-pixel AVX2 fast paths above.
        for lane in 0..8 {
            let pixel = *src.add(px + lane);
            let alpha = pixel >> 24;
            if alpha != 0 {
                let old = *dst.add(px + lane);
                *dst.add(px + lane) = blend_u32(old, pixel, alpha);
            }
        }
        px += 8;
    }

    for i in px..len {
        let sp = *src.add(i);
        let a = sp >> 24;
        if a != 0 {
            let old = *dst.add(i);
            *dst.add(i) = blend_u32(old, sp, a);
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn blend_global_alpha_avx2(src: *const u32, dst: *mut u32, len: usize, alpha: u8) {
    use core::arch::x86_64::*;

    let zero = _mm256_setzero_si256();
    let a = _mm256_set1_epi16(alpha as i16);
    let inv = _mm256_set1_epi16((255 - alpha as u16) as i16);
    let one = _mm256_set1_epi16(1);
    let mut px = 0usize;
    while px + 8 <= len {
        let sp = _mm256_loadu_si256(src.add(px) as *const __m256i);
        let dp = _mm256_loadu_si256(dst.add(px) as *const __m256i);
        let sl = _mm256_unpacklo_epi8(sp, zero);
        let sh = _mm256_unpackhi_epi8(sp, zero);
        let dl = _mm256_unpacklo_epi8(dp, zero);
        let dh = _mm256_unpackhi_epi8(dp, zero);
        let sum_l = _mm256_add_epi16(_mm256_mullo_epi16(sl, a), _mm256_mullo_epi16(dl, inv));
        let sum_h = _mm256_add_epi16(_mm256_mullo_epi16(sh, a), _mm256_mullo_epi16(dh, inv));
        let div_l = _mm256_srli_epi16(
            _mm256_add_epi16(_mm256_add_epi16(sum_l, one), _mm256_srli_epi16(sum_l, 8)),
            8,
        );
        let div_h = _mm256_srli_epi16(
            _mm256_add_epi16(_mm256_add_epi16(sum_h, one), _mm256_srli_epi16(sum_h, 8)),
            8,
        );
        let packed = _mm256_packus_epi16(div_l, div_h);
        _mm256_storeu_si256(dst.add(px) as *mut __m256i, packed);
        px += 8;
    }
    for i in px..len {
        *dst.add(i) = blend_u32(*dst.add(i), *src.add(i), alpha as u32);
    }
}

pub struct LayerSystem {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) buf: Vec<u32>,
    frame_count: u64,
    clip_stack: Vec<(usize, usize, usize, usize)>,
    clip: Option<(usize, usize, usize, usize)>,
    dirty: bool,
    dirty_x0: usize,
    dirty_y0: usize,
    dirty_x1: usize,
    dirty_y1: usize,
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
            buf: vec![Color::TRANSPARENT.0; w * h],
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
        let segs = 4;

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
        let start = libm::ceilf(left - 0.5).max(0.0) as usize;
        let end = (libm::floorf(right - 0.5) as i32 + 1).max(0) as usize;
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
        let off = [0.25f32, 0.75f32];

        for py in y0..y1 {
            let row = py * stride;
            let base_y = py as f32 - y0f;

            let in_corner_row = base_y < r_f || base_y >= h_f - r_f;

            if !in_corner_row {
                self.buf[row + x0..row + x1].fill(v);
                continue;
            }

            if let Some((left, right)) = Self::squircle_row_bounds(poly, base_y + 0.5) {
                let (span_l, span_r) = Self::pixel_span_from_bounds(left, right, w);
                let fill_l = (x + span_l).max(x0).min(x1);
                let fill_r = (x + span_r).max(x0).min(x1);
                if fill_r > fill_l {
                    self.buf[row + fill_l..row + fill_r].fill(v);
                }

                let edge_l = x + span_l.saturating_sub(1);
                if edge_l >= x0 && edge_l < x1 {
                    Self::pixel_aa(
                        &mut self.buf[row + edge_l],
                        v,
                        edge_l as f32 - x0f,
                        base_y,
                        poly,
                        &off,
                    );
                }
                let edge_r = x + span_r;
                if edge_r >= x0 && edge_r < x1 {
                    Self::pixel_aa(
                        &mut self.buf[row + edge_r],
                        v,
                        edge_r as f32 - x0f,
                        base_y,
                        poly,
                        &off,
                    );
                }
            }
        }
    }

    fn pixel_aa(dst: &mut u32, fg: u32, px: f32, py: f32, poly: &[(f32, f32)], _off: &[f32; 2]) {
        let mut hits = 0u32;
        for sy in 0..4 {
            for sx in 0..4 {
                let sample_x = px + (sx as f32 + 0.5) * 0.25;
                let sample_y = py + (sy as f32 + 0.5) * 0.25;
                if Self::point_in_polygon(sample_x, sample_y, poly) {
                    hits += 1;
                }
            }
        }
        if hits > 0 {
            *dst = Self::blend_alpha(*dst, fg, hits as f32 * (1.0 / 16.0));
        }
    }

    fn pixel_aa_batch(
        dst: &mut [u32],
        fg: u32,
        px: [f32; 4],
        py: f32,
        poly: &[(f32, f32)],
        off: &[f32; 2],
        count: usize,
    ) {
        if count < 4 {
            for i in 0..count {
                Self::pixel_aa(&mut dst[i], fg, px[i], py, poly, off);
            }
            return;
        }

        let n = poly.len();
        if n < 3 {
            return;
        }

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
        let inner_r = if r > 2 { r - 2 } else { 0 };
        self.fill_rounded_rect(
            x + 2,
            y + 2,
            w.saturating_sub(4),
            h.saturating_sub(4),
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

    pub fn composit_rounded(
        &mut self,
        src: &LayerSystem,
        dx: usize,
        dy: usize,
        sx: usize,
        sy: usize,
        w: usize,
        h: usize,
        r: usize,
    ) {
        let shape_w = w;
        let shape_h = h;
        let original_dx = dx;
        let original_dy = dy;
        let r = r.min(shape_w / 2).min(shape_h / 2);
        let rf = r as f32;
        let sw = src.width;
        let sh = src.height;
        let dw = self.width;
        let dh = self.height;
        let Some((dx, dy, sx, sy, w, h)) = self.clipped_blit(dx, dy, sx, sy, w, h, sw, sh) else {
            return;
        };
        let shape_x = dx - original_dx;
        let shape_y = dy - original_dy;
        self.mark_dirty_rect(dx, dy, dx + w, dy + h);

        if r == 0 {
            for py in 0..h {
                let src_y = sy + py;
                let dst_y = dy + py;
                if src_y >= sh || dst_y >= dh {
                    continue;
                }
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
        let corner_row_start = shape_h.saturating_sub(r);

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh {
                continue;
            }

            let src_row = src_y * sw + sx;
            let dst_row = dst_y * dw + dx;

            let shape_py = shape_y + py;
            let in_top_corner = shape_py < corner_end;
            let in_bot_corner = shape_py >= corner_row_start;

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

            let end_x = w;
            let poly = Self::cached_squircle(shape_w as f32, shape_h as f32, rf);
            let off = [0.25f32, 0.75f32];
            let base_y = shape_py as f32;
            if let Some((left, right)) = Self::squircle_row_bounds(poly, base_y + 0.5) {
                let (shape_l, shape_r) = Self::pixel_span_from_bounds(left, right, shape_w);
                let span_l = shape_l.saturating_sub(shape_x).min(end_x);
                let span_r = shape_r.saturating_sub(shape_x).min(end_x);

                if span_r > span_l {
                    unsafe {
                        ptr::copy_nonoverlapping(
                            src.buf.as_ptr().add(src_row + span_l),
                            self.buf.as_mut_ptr().add(dst_row + span_l),
                            span_r - span_l,
                        );
                    }
                }

                let edge_l = span_l.saturating_sub(1);
                if shape_l >= shape_x && edge_l < end_x {
                    let sp = src.buf[src_row + edge_l];
                    if sp != Color::TRANSPARENT.0 {
                        Self::pixel_aa(
                            &mut self.buf[dst_row + edge_l],
                            sp,
                            (shape_x + edge_l) as f32,
                            base_y,
                            poly,
                            &off,
                        );
                    }
                }
                let edge_r = span_r;
                if edge_r < end_x {
                    let sp = src.buf[src_row + edge_r];
                    if sp != Color::TRANSPARENT.0 {
                        Self::pixel_aa(
                            &mut self.buf[dst_row + edge_r],
                            sp,
                            (shape_x + edge_r) as f32,
                            base_y,
                            poly,
                            &off,
                        );
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

        let Some((dx, dy, sx, sy, w, h)) = self.clipped_blit(dx, dy, sx, sy, w, h, sw, sh) else {
            return;
        };
        self.mark_dirty_rect(dx, dy, dx + w, dy + h);

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh {
                continue;
            }

            let src_row = src_y * sw + sx;
            let dst_row = dst_y * dw + dx;
            let max_px = w;

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

        let Some((dx, dy, sx, sy, copy_w, h)) = self.clipped_blit(dx, dy, sx, sy, w, h, sw, sh)
        else {
            return;
        };
        self.mark_dirty_rect(dx, dy, dx + copy_w, dy + h);

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh {
                continue;
            }

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

    /// Fast path for a known-opaque source.  This deliberately skips the
    /// per-pixel transparency probe in `composit_rect`; the platform memcpy
    /// implementation can use its vector/SIMD copy path for every scanline.
    pub fn composit_rect_opaque(
        &mut self,
        src: &LayerSystem,
        dx: usize,
        dy: usize,
        sx: usize,
        sy: usize,
        w: usize,
        h: usize,
    ) {
        let Some((dx, dy, sx, sy, copy_w, copy_h)) =
            self.clipped_blit(dx, dy, sx, sy, w, h, src.width, src.height)
        else {
            return;
        };
        self.mark_dirty_rect(dx, dy, dx + copy_w, dy + copy_h);
        for row in 0..copy_h {
            unsafe {
                ptr::copy_nonoverlapping(
                    src.buf.as_ptr().add((sy + row) * src.width + sx),
                    self.buf.as_mut_ptr().add((dy + row) * self.width + dx),
                    copy_w,
                );
            }
        }
    }

    /// Composite an opaque layer with a fractional downward Y translation.
    /// Only transition tails use this path; normal scrolling remains on the
    /// memcpy fast path above.
    pub fn composit_rect_opaque_subpixel_y(
        &mut self,
        src: &LayerSystem,
        dx: usize,
        dy: usize,
        sx: usize,
        sy: usize,
        w: usize,
        h: usize,
        fraction_y: f32,
    ) {
        let fraction = fraction_y.clamp(0.0, 0.999);
        if fraction <= 0.0 {
            self.composit_rect_opaque(src, dx, dy, sx, sy, w, h);
            return;
        }
        if dx >= self.width || dy >= self.height || sx >= src.width || sy >= src.height {
            return;
        }
        let copy_w = w.min(src.width - sx).min(self.width - dx);
        let copy_h = h.min(src.height - sy);
        if copy_w == 0 || copy_h == 0 {
            return;
        }
        let fraction_255 = (fraction * 255.0) as u32;
        let inverse = 255 - fraction_255;
        let out_h = (copy_h + 1).min(self.height - dy);
        self.mark_dirty_rect(dx, dy, dx + copy_w, dy + out_h);
        for out_y in 0..out_h {
            let dst_row = (dy + out_y) * self.width + dx;
            if out_y == 0 {
                let src_row = sy * src.width + sx;
                for px in 0..copy_w {
                    let bg = self.buf[dst_row + px];
                    let fg = src.buf[src_row + px];
                    let r =
                        (((fg >> 16) & 0xff) * inverse + ((bg >> 16) & 0xff) * fraction_255) / 255;
                    let g =
                        (((fg >> 8) & 0xff) * inverse + ((bg >> 8) & 0xff) * fraction_255) / 255;
                    let b = ((fg & 0xff) * inverse + (bg & 0xff) * fraction_255) / 255;
                    self.buf[dst_row + px] = 0xff00_0000 | (r << 16) | (g << 8) | b;
                }
            } else if out_y < copy_h {
                let previous = (sy + out_y - 1) * src.width + sx;
                let current = (sy + out_y) * src.width + sx;
                for px in 0..copy_w {
                    let a = src.buf[previous + px];
                    let b = src.buf[current + px];
                    let r =
                        (((a >> 16) & 0xff) * fraction_255 + ((b >> 16) & 0xff) * inverse) / 255;
                    let g = (((a >> 8) & 0xff) * fraction_255 + ((b >> 8) & 0xff) * inverse) / 255;
                    let blue = ((a & 0xff) * fraction_255 + (b & 0xff) * inverse) / 255;
                    self.buf[dst_row + px] = 0xff00_0000 | (r << 16) | (g << 8) | blue;
                }
            } else {
                let src_row = (sy + copy_h - 1) * src.width + sx;
                for px in 0..copy_w {
                    let bg = self.buf[dst_row + px];
                    let fg = src.buf[src_row + px];
                    let r =
                        (((fg >> 16) & 0xff) * fraction_255 + ((bg >> 16) & 0xff) * inverse) / 255;
                    let g =
                        (((fg >> 8) & 0xff) * fraction_255 + ((bg >> 8) & 0xff) * inverse) / 255;
                    let b = ((fg & 0xff) * fraction_255 + (bg & 0xff) * inverse) / 255;
                    self.buf[dst_row + px] = 0xff00_0000 | (r << 16) | (g << 8) | b;
                }
            }
        }
    }

    /// Composite an opaque RGB source with one opacity for the whole layer.
    /// The hot x86 path processes eight pixels per AVX2 vector; AArch64 uses
    /// four-lane NEON channel arithmetic.
    pub fn composit_rect_global_alpha(
        &mut self,
        src: &[u32],
        src_width: usize,
        src_height: usize,
        dx: usize,
        dy: usize,
        alpha: u8,
    ) {
        if alpha == 0 || src.len() < src_width.saturating_mul(src_height) {
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
        let sx = x0 - dx;
        let copy_w = x1 - x0;
        for y in y0..y1 {
            let sy = y - dy;
            let src_row = sy * src_width + sx;
            let dst_row = y * self.width + x0;
            if alpha == 255 {
                self.buf[dst_row..dst_row + copy_w]
                    .copy_from_slice(&src[src_row..src_row + copy_w]);
                continue;
            }

            #[cfg(target_arch = "x86_64")]
            if avx2_available() {
                unsafe {
                    blend_global_alpha_avx2(
                        src.as_ptr().add(src_row),
                        self.buf.as_mut_ptr().add(dst_row),
                        copy_w,
                        alpha,
                    );
                }
                continue;
            }

            #[cfg(target_arch = "aarch64")]
            unsafe {
                use core::arch::aarch64::*;
                let va = vdupq_n_u32(alpha as u32);
                let vi = vdupq_n_u32(255 - alpha as u32);
                let mask = vdupq_n_u32(0xff);
                let mut px = 0usize;
                while px + 4 <= copy_w {
                    let sp = vld1q_u32(src.as_ptr().add(src_row + px));
                    let dp = vld1q_u32(self.buf.as_ptr().add(dst_row + px));
                    let sr = vandq_u32(vshrq_n_u32(sp, 16), mask);
                    let sg = vandq_u32(vshrq_n_u32(sp, 8), mask);
                    let sb = vandq_u32(sp, mask);
                    let dr = vandq_u32(vshrq_n_u32(dp, 16), mask);
                    let dg = vandq_u32(vshrq_n_u32(dp, 8), mask);
                    let db = vandq_u32(dp, mask);
                    #[inline(always)]
                    unsafe fn div255(v: uint32x4_t) -> uint32x4_t {
                        vshrq_n_u32(
                            vaddq_u32(vaddq_u32(v, vdupq_n_u32(1)), vshrq_n_u32(v, 8)),
                            8,
                        )
                    }
                    let r = div255(vaddq_u32(vmulq_u32(sr, va), vmulq_u32(dr, vi)));
                    let g = div255(vaddq_u32(vmulq_u32(sg, va), vmulq_u32(dg, vi)));
                    let b = div255(vaddq_u32(vmulq_u32(sb, va), vmulq_u32(db, vi)));
                    vst1q_u32(
                        self.buf.as_mut_ptr().add(dst_row + px),
                        vorrq_u32(
                            vdupq_n_u32(0xff00_0000),
                            vorrq_u32(vshlq_n_u32(r, 16), vorrq_u32(vshlq_n_u32(g, 8), b)),
                        ),
                    );
                    px += 4;
                }
                for px in px..copy_w {
                    self.buf[dst_row + px] =
                        blend_u32(self.buf[dst_row + px], src[src_row + px], alpha as u32);
                }
                continue;
            }

            #[allow(unreachable_code)]
            for px in 0..copy_w {
                self.buf[dst_row + px] =
                    blend_u32(self.buf[dst_row + px], src[src_row + px], alpha as u32);
            }
        }
    }

    pub fn composit_rect_alpha(
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

        let Some((dx, dy, sx, sy, w, h)) = self.clipped_blit(dx, dy, sx, sy, w, h, sw, sh) else {
            return;
        };
        self.mark_dirty_rect(dx, dy, dx + w, dy + h);

        #[cfg(target_arch = "x86_64")]
        if avx2_available() {
            for py in 0..h {
                unsafe {
                    blend_alpha_avx2(
                        src.buf.as_ptr().add((sy + py) * sw + sx),
                        self.buf.as_mut_ptr().add((dy + py) * dw + dx),
                        w,
                    );
                }
            }
            return;
        }

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh {
                continue;
            }

            let src_row = src_y * sw + sx;
            let dst_row = dst_y * dw + dx;
            let max_px = w;

            #[cfg(target_arch = "aarch64")]
            unsafe {
                use core::arch::aarch64::*;
                let mut px = 0usize;
                while px + 4 <= max_px {
                    let sp = vld1q_u32(src.buf.as_ptr().add(src_row + px));
                    let a = vshrq_n_u32(sp, 24);
                    let a0 = vgetq_lane_u32(a, 0);
                    let a1 = vgetq_lane_u32(a, 1);
                    let a2 = vgetq_lane_u32(a, 2);
                    let a3 = vgetq_lane_u32(a, 3);
                    if a0 == 0 && a1 == 0 && a2 == 0 && a3 == 0 {
                        px += 4;
                        continue;
                    }
                    if a0 == 255 && a1 == 255 && a2 == 255 && a3 == 255 {
                        vst1q_u32(self.buf.as_mut_ptr().add(dst_row + px), sp);
                        px += 4;
                        continue;
                    }

                    let dp = vld1q_u32(self.buf.as_ptr().add(dst_row + px));
                    let inv = vsubq_u32(vdupq_n_u32(255), a);
                    let mask = vdupq_n_u32(0xff);
                    let sr = vandq_u32(vshrq_n_u32(sp, 16), mask);
                    let sg = vandq_u32(vshrq_n_u32(sp, 8), mask);
                    let sb = vandq_u32(sp, mask);
                    let dr = vandq_u32(vshrq_n_u32(dp, 16), mask);
                    let dg = vandq_u32(vshrq_n_u32(dp, 8), mask);
                    let db = vandq_u32(dp, mask);

                    #[inline(always)]
                    unsafe fn div255(v: uint32x4_t) -> uint32x4_t {
                        vshrq_n_u32(
                            vaddq_u32(vaddq_u32(v, vdupq_n_u32(1)), vshrq_n_u32(v, 8)),
                            8,
                        )
                    }
                    let r = div255(vaddq_u32(vmulq_u32(sr, a), vmulq_u32(dr, inv)));
                    let g = div255(vaddq_u32(vmulq_u32(sg, a), vmulq_u32(dg, inv)));
                    let b = div255(vaddq_u32(vmulq_u32(sb, a), vmulq_u32(db, inv)));
                    let blended = vorrq_u32(
                        vdupq_n_u32(0xff00_0000),
                        vorrq_u32(vshlq_n_u32(r, 16), vorrq_u32(vshlq_n_u32(g, 8), b)),
                    );
                    let out = vbslq_u32(vceqq_u32(a, vdupq_n_u32(0)), dp, blended);
                    vst1q_u32(self.buf.as_mut_ptr().add(dst_row + px), out);
                    px += 4;
                }
                for px in px..max_px {
                    let sp = src.buf[src_row + px];
                    let src_a = ((sp >> 24) & 0xFF) as u32;
                    if src_a == 0 {
                        continue;
                    }
                    if src_a >= 255 {
                        self.buf[dst_row + px] = sp;
                    } else {
                        let inv = 255 - src_a;
                        let sr = (sp >> 16) & 0xFF;
                        let sg = (sp >> 8) & 0xFF;
                        let sb = sp & 0xFF;
                        let dp = self.buf[dst_row + px];
                        let dr = (dp >> 16) & 0xFF;
                        let dg = (dp >> 8) & 0xFF;
                        let db = dp & 0xFF;
                        let r = (sr * src_a + dr * inv) / 255;
                        let g = (sg * src_a + dg * inv) / 255;
                        let b = (sb * src_a + db * inv) / 255;
                        self.buf[dst_row + px] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
                    }
                }
            }

            #[cfg(not(target_arch = "aarch64"))]
            for px in 0..max_px {
                let sp = src.buf[src_row + px];
                let src_a = ((sp >> 24) & 0xFF) as u32;
                if src_a == 0 {
                    continue;
                }
                if src_a >= 255 {
                    self.buf[dst_row + px] = sp;
                } else {
                    let inv = 255 - src_a;
                    let sr = (sp >> 16) & 0xFF;
                    let sg = (sp >> 8) & 0xFF;
                    let sb = sp & 0xFF;
                    let dp = self.buf[dst_row + px];
                    let dr = (dp >> 16) & 0xFF;
                    let dg = (dp >> 8) & 0xFF;
                    let db = dp & 0xFF;
                    let r = (sr * src_a + dr * inv) / 255;
                    let g = (sg * src_a + dg * inv) / 255;
                    let b = (sb * src_a + db * inv) / 255;
                    self.buf[dst_row + px] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
                }
            }
        }
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
}
