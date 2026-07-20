use alloc::vec::Vec;

const FIXED_SHIFT: i32 = 10;
const FIXED_ONE: i32 = 1 << FIXED_SHIFT;

#[inline(always)]
fn clamp_u8(v: i32) -> u8 {
    v.max(0).min(255) as u8
}

fn build_fixed_kernel(blur_r: i32) -> Vec<i32> {
    let sigma = blur_r as f32 / 3.0;
    let sigma_sq2 = 2.0 * sigma * sigma;
    let size = (blur_r * 2 + 1) as usize;
    let mut kernel: Vec<i32> = Vec::with_capacity(size);
    let mut ksum = 0.0f32;
    for i in -blur_r..=blur_r {
        let x = i as f32;
        let kw = libm::expf(-x * x / sigma_sq2);
        kernel.push((kw * FIXED_ONE as f32) as i32);
        ksum += kw;
    }
    let scale = FIXED_ONE as f32 / ksum;
    for kw in &mut kernel {
        *kw = (*kw as f32 * scale / FIXED_ONE as f32) as i32;
    }
    kernel
}

#[inline(always)]
fn pixel_r(px: u32) -> i32 { ((px >> 16) & 0xFF) as i32 }
#[inline(always)]
fn pixel_g(px: u32) -> i32 { ((px >> 8) & 0xFF) as i32 }
#[inline(always)]
fn pixel_b(px: u32) -> i32 { (px & 0xFF) as i32 }
#[inline(always)]
fn make_pixel(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

// ----------------------------------------------------------------------------
// スカラー（端の処理 ＆ フォールバック用）
// ----------------------------------------------------------------------------
#[inline(always)]
unsafe fn blur_h_pixel_scalar(src_row: *const u32, dst_row: *mut u32, x: usize, w: usize, kernel: &[i32], blur_r: i32) {
    let mut r_acc = 0; let mut g_acc = 0; let mut b_acc = 0;
    for k in 0..kernel.len() {
        let sx = (x as i32 + k as i32 - blur_r).max(0).min(w as i32 - 1) as usize;
        let px = *src_row.add(sx);
        let kw = *kernel.get_unchecked(k);
        r_acc += pixel_r(px) * kw;
        g_acc += pixel_g(px) * kw;
        b_acc += pixel_b(px) * kw;
    }
    *dst_row.add(x) = make_pixel(
        clamp_u8(r_acc >> FIXED_SHIFT),
        clamp_u8(g_acc >> FIXED_SHIFT),
        clamp_u8(b_acc >> FIXED_SHIFT),
    );
}

#[inline(always)]
unsafe fn blur_v_pixel_scalar(src: *const u32, dst_row: *mut u32, x: usize, clamped_ys: &[usize], w: usize, kernel: &[i32]) {
    let mut r_acc = 0; let mut g_acc = 0; let mut b_acc = 0;
    for k in 0..kernel.len() {
        let sy = *clamped_ys.get_unchecked(k);
        let px = *src.add(sy * w + x);
        let kw = *kernel.get_unchecked(k);
        r_acc += pixel_r(px) * kw;
        g_acc += pixel_g(px) * kw;
        b_acc += pixel_b(px) * kw;
    }
    *dst_row.add(x) = make_pixel(
        clamp_u8(r_acc >> FIXED_SHIFT),
        clamp_u8(g_acc >> FIXED_SHIFT),
        clamp_u8(b_acc >> FIXED_SHIFT),
    );
}

// ----------------------------------------------------------------------------
// ARM64 (NEON) Implementation
// ----------------------------------------------------------------------------
#[cfg(target_arch = "aarch64")]
mod neon {
    use super::*;
    use core::arch::aarch64::*;

    #[inline(always)]
    unsafe fn load_and_extract(ptr: *const u32) -> (int32x4_t, int32x4_t, int32x4_t) {
        let p = vld1q_u32(ptr);
        let mask = vdupq_n_u32(0xFF);
        let r = vreinterpretq_s32_u32(vandq_u32(vshrq_n_u32(p, 16), mask));
        let g = vreinterpretq_s32_u32(vandq_u32(vshrq_n_u32(p, 8), mask));
        let b = vreinterpretq_s32_u32(vandq_u32(p, mask));
        (r, g, b)
    }

    #[inline(always)]
    unsafe fn store_4_pixels(ptr: *mut u32, r: int32x4_t, g: int32x4_t, b: int32x4_t) {
        let r_u = vreinterpretq_u32_s32(r);
        let g_u = vreinterpretq_u32_s32(g);
        let b_u = vreinterpretq_u32_s32(b);
        let out = vorrq_u32(vorrq_u32(vshlq_n_u32(r_u, 16), vshlq_n_u32(g_u, 8)), b_u);
        vst1q_u32(ptr, out);
    }

    pub unsafe fn blur_h_simd(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
        let safe_start = (blur_r as usize).min(w);
        let safe_end = w.saturating_sub(blur_r as usize);

        for y in 0..h {
            let src_row = src.as_ptr().add(y * w);
            let dst_row = dst.as_mut_ptr().add(y * w);
            let mut x = 0;

            // 左端（はみ出るためスカラー）
            while x < safe_start { blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r); x += 1; }

            // 中央（はみ出ないことが保証されているため、安全に連続ロード）
            while x + 4 <= safe_end {
                let mut r_acc = vdupq_n_s32(0); let mut g_acc = vdupq_n_s32(0); let mut b_acc = vdupq_n_s32(0);
                for k in 0..kernel.len() {
                    let sx = x as i32 + k as i32 - blur_r;
                    let (r, g, b) = load_and_extract(src_row.add(sx as usize));
                    let kw = vdupq_n_s32(*kernel.get_unchecked(k));
                    r_acc = vmlaq_s32(r_acc, r, kw);
                    g_acc = vmlaq_s32(g_acc, g, kw);
                    b_acc = vmlaq_s32(b_acc, b, kw);
                }
                store_4_pixels(dst_row.add(x), vshrq_n_s32(r_acc, FIXED_SHIFT), vshrq_n_s32(g_acc, FIXED_SHIFT), vshrq_n_s32(b_acc, FIXED_SHIFT));
                x += 4;
            }

            // 右端（はみ出るためスカラー）
            while x < w { blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r); x += 1; }
        }
    }

    pub unsafe fn blur_v_simd(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
        let mut clamped_ys = alloc::vec![0usize; kernel.len()];
        for y in 0..h {
            let dst_row = dst.as_mut_ptr().add(y * w);
            // Y座標の境界計算をループの外で1回だけ行う
            for k in 0..kernel.len() {
                clamped_ys[k] = (y as i32 + k as i32 - blur_r).clamp(0, h as i32 - 1) as usize;
            }

            let mut x = 0;
            // メモリが連続しているX軸に対して4画素ずつロード
            while x + 4 <= w {
                let mut r_acc = vdupq_n_s32(0); let mut g_acc = vdupq_n_s32(0); let mut b_acc = vdupq_n_s32(0);
                for k in 0..kernel.len() {
                    let sy = *clamped_ys.get_unchecked(k);
                    let (r, g, b) = load_and_extract(src.as_ptr().add(sy * w + x));
                    let kw = vdupq_n_s32(*kernel.get_unchecked(k));
                    r_acc = vmlaq_s32(r_acc, r, kw);
                    g_acc = vmlaq_s32(g_acc, g, kw);
                    b_acc = vmlaq_s32(b_acc, b, kw);
                }
                store_4_pixels(dst_row.add(x), vshrq_n_s32(r_acc, FIXED_SHIFT), vshrq_n_s32(g_acc, FIXED_SHIFT), vshrq_n_s32(b_acc, FIXED_SHIFT));
                x += 4;
            }

            // 余り
            while x < w { blur_v_pixel_scalar(src.as_ptr(), dst_row, x, &clamped_ys, w, kernel); x += 1; }
        }
    }
}

// ----------------------------------------------------------------------------
// x86_64 (SSE4.1) Implementation
// ----------------------------------------------------------------------------
#[cfg(target_arch = "x86_64")]
mod sse {
    use super::*;
    use core::arch::x86_64::*;

    #[inline(always)]
    unsafe fn load_and_extract(ptr: *const u32) -> (__m128i, __m128i, __m128i) {
        let p = _mm_loadu_si128(ptr as *const __m128i);
        let mask = _mm_set1_epi32(0xFF);
        let r = _mm_and_si128(_mm_srli_epi32(p, 16), mask);
        let g = _mm_and_si128(_mm_srli_epi32(p, 8), mask);
        let b = _mm_and_si128(p, mask);
        (r, g, b)
    }

    #[inline(always)]
    unsafe fn store_4_pixels(ptr: *mut u32, r: __m128i, g: __m128i, b: __m128i) {
        let r_shift = _mm_slli_epi32(r, 16);
        let g_shift = _mm_slli_epi32(g, 8);
        let out = _mm_or_si128(_mm_or_si128(r_shift, g_shift), b);
        _mm_storeu_si128(ptr as *mut __m128i, out);
    }

    #[target_feature(enable = "sse4.1")]
    pub unsafe fn blur_h_simd(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
        let safe_start = (blur_r as usize).min(w);
        let safe_end = w.saturating_sub(blur_r as usize);

        for y in 0..h {
            let src_row = src.as_ptr().add(y * w);
            let dst_row = dst.as_mut_ptr().add(y * w);
            let mut x = 0;

            while x < safe_start { blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r); x += 1; }

            while x + 4 <= safe_end {
                let mut r_acc = _mm_setzero_si128(); let mut g_acc = _mm_setzero_si128(); let mut b_acc = _mm_setzero_si128();
                for k in 0..kernel.len() {
                    let sx = x as i32 + k as i32 - blur_r;
                    let (r, g, b) = load_and_extract(src_row.add(sx as usize));
                    let kw = _mm_set1_epi32(*kernel.get_unchecked(k));
                    r_acc = _mm_add_epi32(r_acc, _mm_mullo_epi32(r, kw));
                    g_acc = _mm_add_epi32(g_acc, _mm_mullo_epi32(g, kw));
                    b_acc = _mm_add_epi32(b_acc, _mm_mullo_epi32(b, kw));
                }
                store_4_pixels(dst_row.add(x), _mm_srli_epi32(r_acc, FIXED_SHIFT), _mm_srli_epi32(g_acc, FIXED_SHIFT), _mm_srli_epi32(b_acc, FIXED_SHIFT));
                x += 4;
            }

            while x < w { blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r); x += 1; }
        }
    }

    #[target_feature(enable = "sse4.1")]
    pub unsafe fn blur_v_simd(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
        let mut clamped_ys = alloc::vec![0usize; kernel.len()];
        for y in 0..h {
            let dst_row = dst.as_mut_ptr().add(y * w);
            for k in 0..kernel.len() {
                clamped_ys[k] = (y as i32 + k as i32 - blur_r).clamp(0, h as i32 - 1) as usize;
            }

            let mut x = 0;
            while x + 4 <= w {
                let mut r_acc = _mm_setzero_si128(); let mut g_acc = _mm_setzero_si128(); let mut b_acc = _mm_setzero_si128();
                for k in 0..kernel.len() {
                    let sy = *clamped_ys.get_unchecked(k);
                    let (r, g, b) = load_and_extract(src.as_ptr().add(sy * w + x));
                    let kw = _mm_set1_epi32(*kernel.get_unchecked(k));
                    r_acc = _mm_add_epi32(r_acc, _mm_mullo_epi32(r, kw));
                    g_acc = _mm_add_epi32(g_acc, _mm_mullo_epi32(g, kw));
                    b_acc = _mm_add_epi32(b_acc, _mm_mullo_epi32(b, kw));
                }
                store_4_pixels(dst_row.add(x), _mm_srli_epi32(r_acc, FIXED_SHIFT), _mm_srli_epi32(g_acc, FIXED_SHIFT), _mm_srli_epi32(b_acc, FIXED_SHIFT));
                x += 4;
            }

            while x < w { blur_v_pixel_scalar(src.as_ptr(), dst_row, x, &clamped_ys, w, kernel); x += 1; }
        }
    }
}

// ----------------------------------------------------------------------------
// API
// ----------------------------------------------------------------------------
pub fn blur_region_to(src: &[u32], dst: &mut [u32], w: usize, y_start: usize, y_end: usize, blur_r: i32) {
    let kernel = build_fixed_kernel(blur_r);
    let region_h = y_end - y_start;
    if region_h == 0 || w == 0 { return; }
    let mut tmp = alloc::vec![0u32; w * region_h];
    let region = &src[y_start * w..y_end * w];

    #[cfg(target_arch = "aarch64")]
    unsafe {
        neon::blur_h_simd(region, &mut tmp, w, region_h, &kernel, blur_r);
        neon::blur_v_simd(&tmp, dst, w, region_h, &kernel, blur_r);
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        sse::blur_h_simd(region, &mut tmp, w, region_h, &kernel, blur_r);
        sse::blur_v_simd(&tmp, dst, w, region_h, &kernel, blur_r);
    }
    // 上記以外のアーキテクチャ（RISC-Vなど）向けのフォールバック処理は省略（必要な場合は追加）
}

pub fn blur_region_darkened_to(src: &[u32], dst: &mut [u32], w: usize, y_start: usize, y_end: usize, blur_r: i32, brightness: u32) {
    let kernel = build_fixed_kernel(blur_r);
    let region_h = y_end - y_start;
    if region_h == 0 || w == 0 { return; }
    let mut tmp = alloc::vec![0u32; w * region_h];
    let region = &src[y_start * w..y_end * w];

    #[cfg(target_arch = "aarch64")]
    unsafe {
        neon::blur_h_simd(region, &mut tmp, w, region_h, &kernel, blur_r);
        neon::blur_v_simd(&tmp, dst, w, region_h, &kernel, blur_r); // 型ミスマッチ(&mut tmp -> &tmp)修正
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        sse::blur_h_simd(region, &mut tmp, w, region_h, &kernel, blur_r);
        sse::blur_v_simd(&tmp, dst, w, region_h, &kernel, blur_r);
    }

    let b = brightness;
    for px in dst[..w * region_h].iter_mut() {
        let r = (((*px >> 16) & 0xFF) * b / 255) as u32;
        let g = (((*px >> 8) & 0xFF) * b / 255) as u32;
        let bl = ((*px & 0xFF) * b / 255) as u32;
        *px = (r << 16) | (g << 8) | bl;
    }
}