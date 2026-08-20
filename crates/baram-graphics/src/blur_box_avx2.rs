#[cfg(target_arch = "x86_64")]
mod box_avx2 {
    use super::*;
    use core::arch::x86_64::*;

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn extract_channels(p: __m256i) -> (__m256i, __m256i, __m256i) {
        let mask = _mm256_set1_epi32(0xFF);
        (
            _mm256_and_si256(_mm256_srli_epi32(p, 16), mask),
            _mm256_and_si256(_mm256_srli_epi32(p, 8), mask),
            _mm256_and_si256(p, mask),
        )
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn pack_pixel(r: __m256i, g: __m256i, b: __m256i) -> __m256i {
        _mm256_or_si256(
            _mm256_or_si256(_mm256_slli_epi32(r, 16), _mm256_slli_epi32(g, 8)),
            b,
        )
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn finalize(sum: __m256i, recip: __m256i) -> __m256i {
        let product = _mm256_mullo_epi32(sum, recip);
        let rounded = _mm256_add_epi32(product, _mm256_set1_epi32(1 << (RECIP_SHIFT - 1)));
        let v = _mm256_srai_epi32(rounded, RECIP_SHIFT);
        _mm256_max_epi32(
            _mm256_min_epi32(v, _mm256_set1_epi32(255)),
            _mm256_setzero_si256(),
        )
    }

    // 8列を同時に処理（列方向は連続メモリなので通常load/store）
    #[target_feature(enable = "avx2")]
    pub unsafe fn box_blur_v_simd8(src: &[u32], dst: &mut [u32], w: usize, h: usize, r: i32) {
        let count = 2 * r + 1;
        let recip = _mm256_set1_epi32(box_recip(count));

        let mut x = 0;
        while x + 8 <= w {
            let mut r_sum = _mm256_setzero_si256();
            let mut g_sum = _mm256_setzero_si256();
            let mut b_sum = _mm256_setzero_si256();
            for i in -r..=r {
                let sy = i.clamp(0, h as i32 - 1) as usize;
                let p = _mm256_loadu_si256(src.as_ptr().add(sy * w + x) as *const __m256i);
                let (r8, g8, b8) = extract_channels(p);
                r_sum = _mm256_add_epi32(r_sum, r8);
                g_sum = _mm256_add_epi32(g_sum, g8);
                b_sum = _mm256_add_epi32(b_sum, b8);
            }
            let out = pack_pixel(
                finalize(r_sum, recip),
                finalize(g_sum, recip),
                finalize(b_sum, recip),
            );
            _mm256_storeu_si256(dst.as_mut_ptr().add(x) as *mut __m256i, out);

            for y in 1..h {
                let rem_y = (y as i32 - r - 1).max(0) as usize;
                let add_y = (y as i32 + r).min(h as i32 - 1) as usize;
                let rem_p = _mm256_loadu_si256(src.as_ptr().add(rem_y * w + x) as *const __m256i);
                let add_p = _mm256_loadu_si256(src.as_ptr().add(add_y * w + x) as *const __m256i);
                let (rr, rg, rb) = extract_channels(rem_p);
                let (ar, ag, ab) = extract_channels(add_p);
                r_sum = _mm256_add_epi32(_mm256_sub_epi32(r_sum, rr), ar);
                g_sum = _mm256_add_epi32(_mm256_sub_epi32(g_sum, rg), ag);
                b_sum = _mm256_add_epi32(_mm256_sub_epi32(b_sum, rb), ab);

                let out = pack_pixel(
                    finalize(r_sum, recip),
                    finalize(g_sum, recip),
                    finalize(b_sum, recip),
                );
                _mm256_storeu_si256(dst.as_mut_ptr().add(y * w + x) as *mut __m256i, out);
            }
            x += 8;
        }
        // 端数の列はスカラーで
        while x < w {
            box_blur_v_single_col(src.as_ptr(), dst.as_mut_ptr(), w, h, x, r);
            x += 1;
        }
    }
}

// ----------------------------------------------------------------------------
// ARM64 (NEON - 4画素同時)
// ----------------------------------------------------------------------------

