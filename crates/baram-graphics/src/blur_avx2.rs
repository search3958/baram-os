#[cfg(target_arch = "x86_64")]
mod avx2 {
    use super::*;
    use core::arch::x86_64::*;

    const BLOCK_W: usize = 32;

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn load8(ptr: *const u32) -> (__m256i, __m256i, __m256i) {
        let p = _mm256_loadu_si256(ptr as *const __m256i);
        let mask = _mm256_set1_epi32(0xFF);
        (
            _mm256_and_si256(_mm256_srli_epi32(p, 16), mask),
            _mm256_and_si256(_mm256_srli_epi32(p, 8), mask),
            _mm256_and_si256(p, mask),
        )
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn store8(ptr: *mut u32, r: __m256i, g: __m256i, b: __m256i) {
        let out = _mm256_or_si256(
            _mm256_or_si256(_mm256_slli_epi32(r, 16), _mm256_slli_epi32(g, 8)),
            b,
        );
        _mm256_storeu_si256(ptr as *mut __m256i, out);
    }

    #[inline(always)]
    unsafe fn load4(ptr: *const u32) -> (__m128i, __m128i, __m128i) {
        let p = _mm_loadu_si128(ptr as *const __m128i);
        let mask = _mm_set1_epi32(0xFF);
        (
            _mm_and_si128(_mm_srli_epi32(p, 16), mask),
            _mm_and_si128(_mm_srli_epi32(p, 8), mask),
            _mm_and_si128(p, mask),
        )
    }

    #[inline(always)]
    unsafe fn store4(ptr: *mut u32, r: __m128i, g: __m128i, b: __m128i) {
        let out = _mm_or_si128(_mm_or_si128(_mm_slli_epi32(r, 16), _mm_slli_epi32(g, 8)), b);
        _mm_storeu_si128(ptr as *mut __m128i, out);
    }

    #[target_feature(enable = "avx2")]
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

            while x + 8 <= safe_end {
                let mut r_acc = _mm256_setzero_si256();
                let mut g_acc = _mm256_setzero_si256();
                let mut b_acc = _mm256_setzero_si256();
                for k in 0..kernel.len() {
                    let sx = x as i32 + k as i32 - blur_r;
                    let (r, g, b) = load8(src_row.add(sx as usize));
                    let kw = _mm256_set1_epi32(*kernel.get_unchecked(k));
                    r_acc = _mm256_add_epi32(r_acc, _mm256_mullo_epi32(r, kw));
                    g_acc = _mm256_add_epi32(g_acc, _mm256_mullo_epi32(g, kw));
                    b_acc = _mm256_add_epi32(b_acc, _mm256_mullo_epi32(b, kw));
                }
                store8(
                    dst_row.add(x),
                    _mm256_srli_epi32(r_acc, FIXED_SHIFT),
                    _mm256_srli_epi32(g_acc, FIXED_SHIFT),
                    _mm256_srli_epi32(b_acc, FIXED_SHIFT),
                );
                x += 8;
            }

            while x + 4 <= safe_end {
                let mut r_acc = _mm_setzero_si128();
                let mut g_acc = _mm_setzero_si128();
                let mut b_acc = _mm_setzero_si128();
                for k in 0..kernel.len() {
                    let sx = x as i32 + k as i32 - blur_r;
                    let (r, g, b) = load4(src_row.add(sx as usize));
                    let kw = _mm_set1_epi32(*kernel.get_unchecked(k));
                    r_acc = _mm_add_epi32(r_acc, _mm_mullo_epi32(r, kw));
                    g_acc = _mm_add_epi32(g_acc, _mm_mullo_epi32(g, kw));
                    b_acc = _mm_add_epi32(b_acc, _mm_mullo_epi32(b, kw));
                }
                store4(
                    dst_row.add(x),
                    _mm_srli_epi32(r_acc, FIXED_SHIFT),
                    _mm_srli_epi32(g_acc, FIXED_SHIFT),
                    _mm_srli_epi32(b_acc, FIXED_SHIFT),
                );
                x += 4;
            }

            while x < w {
                blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r);
                x += 1;
            }
        }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn blur_v_simd(
        src: &[u32],
        dst: &mut [u32],
        w: usize,
        h: usize,
        kernel: &[i32],
        blur_r: i32,
    ) {
        for x_block in (0..w).step_by(BLOCK_W) {
            let x_end = (x_block + BLOCK_W).min(w);

            for y in 0..h {
                let dst_row = dst.as_mut_ptr().add(y * w);
                let mut x = x_block;

                while x + 8 <= x_end {
                    let mut r_acc = _mm256_setzero_si256();
                    let mut g_acc = _mm256_setzero_si256();
                    let mut b_acc = _mm256_setzero_si256();
                    for k in 0..kernel.len() {
                        let sy = (y as i32 + k as i32 - blur_r).clamp(0, h as i32 - 1) as usize;
                        let (r, g, b) = load8(src.as_ptr().add(sy * w + x));
                        let kw = _mm256_set1_epi32(*kernel.get_unchecked(k));
                        r_acc = _mm256_add_epi32(r_acc, _mm256_mullo_epi32(r, kw));
                        g_acc = _mm256_add_epi32(g_acc, _mm256_mullo_epi32(g, kw));
                        b_acc = _mm256_add_epi32(b_acc, _mm256_mullo_epi32(b, kw));
                    }
                    store8(
                        dst_row.add(x),
                        _mm256_srli_epi32(r_acc, FIXED_SHIFT),
                        _mm256_srli_epi32(g_acc, FIXED_SHIFT),
                        _mm256_srli_epi32(b_acc, FIXED_SHIFT),
                    );
                    x += 8;
                }

                while x + 4 <= x_end {
                    let mut r_acc = _mm_setzero_si128();
                    let mut g_acc = _mm_setzero_si128();
                    let mut b_acc = _mm_setzero_si128();
                    for k in 0..kernel.len() {
                        let sy = (y as i32 + k as i32 - blur_r).clamp(0, h as i32 - 1) as usize;
                        let (r, g, b) = load4(src.as_ptr().add(sy * w + x));
                        let kw = _mm_set1_epi32(*kernel.get_unchecked(k));
                        r_acc = _mm_add_epi32(r_acc, _mm_mullo_epi32(r, kw));
                        g_acc = _mm_add_epi32(g_acc, _mm_mullo_epi32(g, kw));
                        b_acc = _mm_add_epi32(b_acc, _mm_mullo_epi32(b, kw));
                    }
                    store4(
                        dst_row.add(x),
                        _mm_srli_epi32(r_acc, FIXED_SHIFT),
                        _mm_srli_epi32(g_acc, FIXED_SHIFT),
                        _mm_srli_epi32(b_acc, FIXED_SHIFT),
                    );
                    x += 4;
                }

                while x < x_end {
                    blur_v_pixel_scalar(src.as_ptr(), dst_row, x, w, h, y, kernel, blur_r);
                    x += 1;
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------
// x86_64 (AVX2 - ボックスブラー専用: 水平パス=gather / 垂直パス=連続load)
// ----------------------------------------------------------------------------

