impl LayerSystem {
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
                let aa_bounds = Self::aa_row_bounds(poly, base_y);

                let edge_l = span_l.saturating_sub(1);
                if shape_l >= shape_x && edge_l < end_x {
                    let sp = src.buf[src_row + edge_l];
                    if sp != Color::TRANSPARENT.0 {
                        Self::pixel_aa(
                            &mut self.buf[dst_row + edge_l],
                            sp,
                            (shape_x + edge_l) as f32,
                            &aa_bounds,
                            true,
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
                            &aa_bounds,
                            false,
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

}

