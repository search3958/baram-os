#[cfg(target_arch = "aarch64")]
mod neon {
    use super::*;
    use core::arch::aarch64::*;

    #[inline(always)]
    unsafe fn load4(ptr: *const u32) -> (int32x4_t, int32x4_t, int32x4_t) {
        let p = vld1q_u32(ptr);
        let mask = vdupq_n_u32(0xFF);
        (
            vreinterpretq_s32_u32(vandq_u32(vshrq_n_u32(p, 16), mask)),
            vreinterpretq_s32_u32(vandq_u32(vshrq_n_u32(p, 8), mask)),
            vreinterpretq_s32_u32(vandq_u32(p, mask)),
        )
    }

    #[inline(always)]
    unsafe fn store4(ptr: *mut u32, r: int32x4_t, g: int32x4_t, b: int32x4_t) {
        let out = vorrq_u32(
            vorrq_u32(
                vshlq_n_u32(vreinterpretq_u32_s32(r), 16),
                vshlq_n_u32(vreinterpretq_u32_s32(g), 8),
            ),
            vreinterpretq_u32_s32(b),
        );
        vst1q_u32(ptr, out);
    }

    pub unsafe fn blur_h_simd(
        src: &[u32],
        dst: &mut [u32],
        w: usize,
        h: usize,
        kernel: &[i32],
        blur_r: i32,
    ) {
        let safe_start = (blur_r as usize).min(w);
        let safe_end = w.saturating_sub(blur_r as usize);

        for y in 0..h {
            let src_row = src.as_ptr().add(y * w);
            let dst_row = dst.as_mut_ptr().add(y * w);
            let mut x = 0;

            while x < safe_start {
                blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r);
                x += 1;
            }

            while x + 4 <= safe_end {
                let mut r_acc = vdupq_n_s32(0);
                let mut g_acc = vdupq_n_s32(0);
                let mut b_acc = vdupq_n_s32(0);
                for k in 0..kernel.len() {
                    let sx = x as i32 + k as i32 - blur_r;
                    let (r, g, b) = load4(src_row.add(sx as usize));
                    let kw = vdupq_n_s32(*kernel.get_unchecked(k));
                    r_acc = vmlaq_s32(r_acc, r, kw);
                    g_acc = vmlaq_s32(g_acc, g, kw);
                    b_acc = vmlaq_s32(b_acc, b, kw);
                }
                store4(
                    dst_row.add(x),
                    vshrq_n_s32(r_acc, FIXED_SHIFT),
                    vshrq_n_s32(g_acc, FIXED_SHIFT),
                    vshrq_n_s32(b_acc, FIXED_SHIFT),
                );
                x += 4;
            }

            while x < w {
                blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r);
                x += 1;
            }
        }
    }

    pub unsafe fn blur_v_simd(
        src: &[u32],
        dst: &mut [u32],
        w: usize,
        h: usize,
        kernel: &[i32],
        blur_r: i32,
    ) {
        for y in 0..h {
            let dst_row = dst.as_mut_ptr().add(y * w);
            let mut x = 0;

            while x + 4 <= w {
                let mut r_acc = vdupq_n_s32(0);
                let mut g_acc = vdupq_n_s32(0);
                let mut b_acc = vdupq_n_s32(0);
                for k in 0..kernel.len() {
                    let sy = (y as i32 + k as i32 - blur_r).clamp(0, h as i32 - 1) as usize;
                    let (r, g, b) = load4(src.as_ptr().add(sy * w + x));
                    let kw = vdupq_n_s32(*kernel.get_unchecked(k));
                    r_acc = vmlaq_s32(r_acc, r, kw);
                    g_acc = vmlaq_s32(g_acc, g, kw);
                    b_acc = vmlaq_s32(b_acc, b, kw);
                }
                store4(
                    dst_row.add(x),
                    vshrq_n_s32(r_acc, FIXED_SHIFT),
                    vshrq_n_s32(g_acc, FIXED_SHIFT),
                    vshrq_n_s32(b_acc, FIXED_SHIFT),
                );
                x += 4;
            }

            while x < w {
                blur_v_pixel_scalar(src.as_ptr(), dst_row, x, w, h, y, kernel, blur_r);
                x += 1;
            }
        }
    }
}

// ----------------------------------------------------------------------------
// API
// ----------------------------------------------------------------------------

