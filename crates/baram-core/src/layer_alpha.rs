impl LayerSystem {
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
