const FIXED_SHIFT: i32 = 10;
const FIXED_ONE: i32 = 1 << FIXED_SHIFT;
const RECIP_SHIFT: i32 = 16;

#[inline(always)]
fn clamp_u8(v: i32) -> u8 {
    v.max(0).min(255) as u8
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

#[inline(always)]
fn box_recip(count: i32) -> i32 {
    (((1i64 << RECIP_SHIFT) + (count as i64) / 2) / count as i64) as i32
}

#[inline(always)]
fn box_average(sum: i32, recip: i32) -> i32 {
    (sum * recip + (1 << (RECIP_SHIFT - 1))) >> RECIP_SHIFT
}

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
unsafe fn blur_v_pixel_scalar(src: *const u32, dst_row: *mut u32, x: usize, w: usize, h: usize, y: usize, kernel: &[i32], blur_r: i32) {
    let mut r_acc = 0; let mut g_acc = 0; let mut b_acc = 0;
    for k in 0..kernel.len() {
        let sy = (y as i32 + k as i32 - blur_r).clamp(0, h as i32 - 1) as usize;
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
// Kuskir法: 3パスボックスブラー (radius >= 10 で使用)
// ランニングサムで O(1)/px。カーネルサイズに依存しない。
// 初期窓を [-r, r] に正しく対称化し、除算は逆数乗算に置き換え済み。
// ----------------------------------------------------------------------------
#[inline(always)]
unsafe fn box_blur_h(src: *const u32, dst: *mut u32, w: usize, h: usize, r: i32) {
    let count = 2 * r + 1;
    let recip = box_recip(count);

    for y in 0..h {
        let sp = src.add(y * w);
        let dp = dst.add(y * w);

        let mut r_sum = 0i32; let mut g_sum = 0i32; let mut b_sum = 0i32;
        for i in -r..=r {
            let sx = i.clamp(0, w as i32 - 1) as usize;
            let px = *sp.add(sx);
            r_sum += pixel_r(px);
            g_sum += pixel_g(px);
            b_sum += pixel_b(px);
        }
        *dp = make_pixel(
            clamp_u8(box_average(r_sum, recip)),
            clamp_u8(box_average(g_sum, recip)),
            clamp_u8(box_average(b_sum, recip)),
        );

        for x in 1..w {
            let rem = *sp.add((x as i32 - r - 1).max(0) as usize);
            let add = *sp.add((x as i32 + r).min(w as i32 - 1) as usize);
            r_sum += pixel_r(add) - pixel_r(rem);
            g_sum += pixel_g(add) - pixel_g(rem);
            b_sum += pixel_b(add) - pixel_b(rem);

            *dp.add(x) = make_pixel(
                clamp_u8(box_average(r_sum, recip)),
                clamp_u8(box_average(g_sum, recip)),
                clamp_u8(box_average(b_sum, recip)),
            );
        }
    }
}

#[inline(always)]
unsafe fn box_blur_v(src: *const u32, dst: *mut u32, w: usize, h: usize, r: i32) {
    let count = 2 * r + 1;
    let recip = box_recip(count);

    for x in 0..w {
        let mut r_sum = 0i32; let mut g_sum = 0i32; let mut b_sum = 0i32;
        for i in -r..=r {
            let sy = i.clamp(0, h as i32 - 1) as usize;
            let px = *src.add(sy * w + x);
            r_sum += pixel_r(px);
            g_sum += pixel_g(px);
            b_sum += pixel_b(px);
        }
        *dst.add(x) = make_pixel(
            clamp_u8(box_average(r_sum, recip)),
            clamp_u8(box_average(g_sum, recip)),
            clamp_u8(box_average(b_sum, recip)),
        );

        for y in 1..h {
            let rem = *src.add((y as i32 - r - 1).max(0) as usize * w + x);
            let add = *src.add((y as i32 + r).min(h as i32 - 1) as usize * w + x);
            r_sum += pixel_r(add) - pixel_r(rem);
            g_sum += pixel_g(add) - pixel_g(rem);
            b_sum += pixel_b(add) - pixel_b(rem);

            *dst.add(y * w + x) = make_pixel(
                clamp_u8(box_average(r_sum, recip)),
                clamp_u8(box_average(g_sum, recip)),
                clamp_u8(box_average(b_sum, recip)),
            );
        }
    }
}

#[inline(always)]
unsafe fn box_blur_v_single_col(src: *const u32, dst: *mut u32, w: usize, h: usize, x: usize, r: i32) {
    let count = 2 * r + 1;
    let recip = box_recip(count);

    let mut r_sum = 0i32; let mut g_sum = 0i32; let mut b_sum = 0i32;
    for i in -r..=r {
        let sy = i.clamp(0, h as i32 - 1) as usize;
        let px = *src.add(sy * w + x);
        r_sum += pixel_r(px);
        g_sum += pixel_g(px);
        b_sum += pixel_b(px);
    }
    *dst.add(x) = make_pixel(
        clamp_u8(box_average(r_sum, recip)),
        clamp_u8(box_average(g_sum, recip)),
        clamp_u8(box_average(b_sum, recip)),
    );

    for y in 1..h {
        let rem = *src.add((y as i32 - r - 1).max(0) as usize * w + x);
        let add = *src.add((y as i32 + r).min(h as i32 - 1) as usize * w + x);
        r_sum += pixel_r(add) - pixel_r(rem);
        g_sum += pixel_g(add) - pixel_g(rem);
        b_sum += pixel_b(add) - pixel_b(rem);

        *dst.add(y * w + x) = make_pixel(
            clamp_u8(box_average(r_sum, recip)),
            clamp_u8(box_average(g_sum, recip)),
            clamp_u8(box_average(b_sum, recip)),
        );
    }
}

#[inline(always)]
unsafe fn box_blur_h_single_row(src: *const u32, dst: *mut u32, w: usize, r: i32) {
    let count = 2 * r + 1;
    let recip = box_recip(count);

    let mut r_sum = 0i32; let mut g_sum = 0i32; let mut b_sum = 0i32;
    for i in -r..=r {
        let sx = i.clamp(0, w as i32 - 1) as usize;
        let px = *src.add(sx);
        r_sum += pixel_r(px);
        g_sum += pixel_g(px);
        b_sum += pixel_b(px);
    }
    *dst = make_pixel(
        clamp_u8(box_average(r_sum, recip)),
        clamp_u8(box_average(g_sum, recip)),
        clamp_u8(box_average(b_sum, recip)),
    );

    for x in 1..w {
        let rem = *src.add((x as i32 - r - 1).max(0) as usize);
        let add = *src.add((x as i32 + r).min(w as i32 - 1) as usize);
        r_sum += pixel_r(add) - pixel_r(rem);
        g_sum += pixel_g(add) - pixel_g(rem);
        b_sum += pixel_b(add) - pixel_b(rem);

        *dst.add(x) = make_pixel(
            clamp_u8(box_average(r_sum, recip)),
            clamp_u8(box_average(g_sum, recip)),
            clamp_u8(box_average(b_sum, recip)),
        );
    }
}

struct HorizontalBoxPass {
    src: *const u32,
    dst: *mut u32,
    width: usize,
    radius: i32,
}

unsafe impl Sync for HorizontalBoxPass {}

fn run_horizontal_box_pass(pass: &HorizontalBoxPass, row: usize) {
    unsafe {
        box_blur_h_single_row(
            pass.src.add(row * pass.width),
            pass.dst.add(row * pass.width),
            pass.width,
            pass.radius,
        );
    }
}

struct VerticalBoxPass {
    src: *const u32,
    dst: *mut u32,
    width: usize,
    height: usize,
    radius: i32,
}

unsafe impl Sync for VerticalBoxPass {}

fn run_vertical_box_pass(pass: &VerticalBoxPass, column: usize) {
    unsafe {
        box_blur_v_single_col(pass.src, pass.dst, pass.width, pass.height, column, pass.radius);
    }
}

fn parallel_box_blur_h(src: &[u32], dst: &mut [u32], w: usize, h: usize, r: i32) {
    let pass = HorizontalBoxPass { src: src.as_ptr(), dst: dst.as_mut_ptr(), width: w, radius: r };
    baram_core::parallel::for_each(h, &pass, run_horizontal_box_pass);
}

fn parallel_box_blur_v(src: &[u32], dst: &mut [u32], w: usize, h: usize, r: i32) {
    let pass = VerticalBoxPass {
        src: src.as_ptr(),
        dst: dst.as_mut_ptr(),
        width: w,
        height: h,
        radius: r,
    };
    baram_core::parallel::for_each(w, &pass, run_vertical_box_pass);
}

fn box_blur_2pass_scalar(
    src: &[u32],
    dst: &mut [u32],
    tmp: &mut [u32],
    w: usize,
    h: usize,
    r: i32,
) {
    parallel_box_blur_h(src, tmp, w, h, r);
    parallel_box_blur_v(tmp, dst, w, h, r);
    parallel_box_blur_h(dst, tmp, w, h, r);
    parallel_box_blur_v(tmp, dst, w, h, r);
}

fn box_blur_2pass_with_scratch(
    src: &[u32],
    dst: &mut [u32],
    tmp: &mut [u32],
    w: usize,
    h: usize,
    blur_r: i32,
) {
    let r = (blur_r / 2).max(1);
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        unsafe {
            box_avx2::box_blur_h_simd8(src, tmp, w, h, r);
            box_avx2::box_blur_v_simd8(tmp, dst, w, h, r);
            box_avx2::box_blur_h_simd8(dst, tmp, w, h, r);
            box_avx2::box_blur_v_simd8(tmp, dst, w, h, r);
        }
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
    {
        box_blur_2pass_scalar(src, dst, tmp, w, h, r);
    }
}

fn box_blur_2pass(src: &[u32], dst: &mut [u32], w: usize, h: usize, blur_r: i32) {
    let mut tmp = alloc::vec![0u32; w * h];
    box_blur_2pass_with_scratch(src, dst, &mut tmp, w, h, blur_r);
}

// ----------------------------------------------------------------------------
// x86_64 (AVX2 - ガウシアン畳み込み: 8画素同時 + キャッシュブロッキング)
// ----------------------------------------------------------------------------
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
mod avx2 {
    use super::*;
    use core::arch::x86_64::*;

    const BLOCK_W: usize = 32;

    #[inline(always)]
    unsafe fn load8(ptr: *const u32) -> (__m256i, __m256i, __m256i) {
        let p = _mm256_loadu_si256(ptr as *const __m256i);
        let mask = _mm256_set1_epi32(0xFF);
        (_mm256_and_si256(_mm256_srli_epi32(p, 16), mask),
         _mm256_and_si256(_mm256_srli_epi32(p, 8), mask),
         _mm256_and_si256(p, mask))
    }

    #[inline(always)]
    unsafe fn store8(ptr: *mut u32, r: __m256i, g: __m256i, b: __m256i) {
        let out = _mm256_or_si256(_mm256_or_si256(_mm256_slli_epi32(r, 16), _mm256_slli_epi32(g, 8)), b);
        _mm256_storeu_si256(ptr as *mut __m256i, out);
    }

    #[inline(always)]
    unsafe fn load4(ptr: *const u32) -> (__m128i, __m128i, __m128i) {
        let p = _mm_loadu_si128(ptr as *const __m128i);
        let mask = _mm_set1_epi32(0xFF);
        (_mm_and_si128(_mm_srli_epi32(p, 16), mask),
         _mm_and_si128(_mm_srli_epi32(p, 8), mask),
         _mm_and_si128(p, mask))
    }

    #[inline(always)]
    unsafe fn store4(ptr: *mut u32, r: __m128i, g: __m128i, b: __m128i) {
        let out = _mm_or_si128(_mm_or_si128(_mm_slli_epi32(r, 16), _mm_slli_epi32(g, 8)), b);
        _mm_storeu_si128(ptr as *mut __m128i, out);
    }

    pub unsafe fn blur_h_simd(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
        let safe_start = (blur_r as usize).min(w);
        let safe_end = w.saturating_sub(blur_r as usize);

        for y in 0..h {
            let src_row = src.as_ptr().add(y * w);
            let dst_row = dst.as_mut_ptr().add(y * w);
            let mut x = 0;

            while x < safe_start { blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r); x += 1; }

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
                store8(dst_row.add(x), _mm256_srli_epi32(r_acc, FIXED_SHIFT), _mm256_srli_epi32(g_acc, FIXED_SHIFT), _mm256_srli_epi32(b_acc, FIXED_SHIFT));
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
                store4(dst_row.add(x), _mm_srli_epi32(r_acc, FIXED_SHIFT), _mm_srli_epi32(g_acc, FIXED_SHIFT), _mm_srli_epi32(b_acc, FIXED_SHIFT));
                x += 4;
            }

            while x < w { blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r); x += 1; }
        }
    }

    pub unsafe fn blur_v_simd(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
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
                    store8(dst_row.add(x), _mm256_srli_epi32(r_acc, FIXED_SHIFT), _mm256_srli_epi32(g_acc, FIXED_SHIFT), _mm256_srli_epi32(b_acc, FIXED_SHIFT));
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
                    store4(dst_row.add(x), _mm_srli_epi32(r_acc, FIXED_SHIFT), _mm_srli_epi32(g_acc, FIXED_SHIFT), _mm_srli_epi32(b_acc, FIXED_SHIFT));
                    x += 4;
                }

                while x < x_end { blur_v_pixel_scalar(src.as_ptr(), dst_row, x, w, h, y, kernel, blur_r); x += 1; }
            }
        }
    }
}

// ----------------------------------------------------------------------------
// x86_64 (AVX2 - ボックスブラー専用: 水平パス=gather / 垂直パス=連続load)
// ----------------------------------------------------------------------------
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
mod box_avx2 {
    use super::*;
    use core::arch::x86_64::*;

    #[inline(always)]
    unsafe fn extract_channels(p: __m256i) -> (__m256i, __m256i, __m256i) {
        let mask = _mm256_set1_epi32(0xFF);
        (_mm256_and_si256(_mm256_srli_epi32(p, 16), mask),
         _mm256_and_si256(_mm256_srli_epi32(p, 8), mask),
         _mm256_and_si256(p, mask))
    }

    #[inline(always)]
    unsafe fn pack_pixel(r: __m256i, g: __m256i, b: __m256i) -> __m256i {
        _mm256_or_si256(_mm256_or_si256(_mm256_slli_epi32(r, 16), _mm256_slli_epi32(g, 8)), b)
    }

    #[inline(always)]
    unsafe fn finalize(sum: __m256i, recip: __m256i) -> __m256i {
        let product = _mm256_mullo_epi32(sum, recip);
        let rounded =
            _mm256_add_epi32(product, _mm256_set1_epi32(1 << (RECIP_SHIFT - 1)));
        let v = _mm256_srai_epi32(rounded, RECIP_SHIFT);
        _mm256_max_epi32(_mm256_min_epi32(v, _mm256_set1_epi32(255)), _mm256_setzero_si256())
    }

    // 8行を同時に処理（行方向はgatherでストライドアクセス）
    pub unsafe fn box_blur_h_simd8(src: &[u32], dst: &mut [u32], w: usize, h: usize, r: i32) {
        let count = 2 * r + 1;
        let recip = _mm256_set1_epi32(box_recip(count));
        let wi = w as i32;

        let mut y = 0;
        while y + 8 <= h {
            let row_off = _mm256_setr_epi32(0, wi, 2*wi, 3*wi, 4*wi, 5*wi, 6*wi, 7*wi);
            let mut row_off_arr = [0i32; 8];
            _mm256_storeu_si256(row_off_arr.as_mut_ptr() as *mut __m256i, row_off);

            let base = src.as_ptr().add(y * w);
            let dbase = dst.as_mut_ptr().add(y * w);

            let mut r_sum = _mm256_setzero_si256();
            let mut g_sum = _mm256_setzero_si256();
            let mut b_sum = _mm256_setzero_si256();
            for i in -r..=r {
                let sx = i.clamp(0, w as i32 - 1);
                let idx = _mm256_add_epi32(row_off, _mm256_set1_epi32(sx));
                let p = _mm256_i32gather_epi32(base as *const i32, idx, 4);
                let (r8, g8, b8) = extract_channels(p);
                r_sum = _mm256_add_epi32(r_sum, r8);
                g_sum = _mm256_add_epi32(g_sum, g8);
                b_sum = _mm256_add_epi32(b_sum, b8);
            }
            write_lanes(dbase, &row_off_arr, 0,
                pack_pixel(finalize(r_sum, recip), finalize(g_sum, recip), finalize(b_sum, recip)));

            for x in 1..w {
                let rem_x = (x as i32 - r - 1).max(0);
                let add_x = (x as i32 + r).min(wi - 1);
                let rem_idx = _mm256_add_epi32(row_off, _mm256_set1_epi32(rem_x));
                let add_idx = _mm256_add_epi32(row_off, _mm256_set1_epi32(add_x));
                let rem_p = _mm256_i32gather_epi32(base as *const i32, rem_idx, 4);
                let add_p = _mm256_i32gather_epi32(base as *const i32, add_idx, 4);
                let (rr, rg, rb) = extract_channels(rem_p);
                let (ar, ag, ab) = extract_channels(add_p);
                r_sum = _mm256_add_epi32(_mm256_sub_epi32(r_sum, rr), ar);
                g_sum = _mm256_add_epi32(_mm256_sub_epi32(g_sum, rg), ag);
                b_sum = _mm256_add_epi32(_mm256_sub_epi32(b_sum, rb), ab);

                write_lanes(dbase, &row_off_arr, x,
                    pack_pixel(finalize(r_sum, recip), finalize(g_sum, recip), finalize(b_sum, recip)));
            }
            y += 8;
        }
        // 端数の行はスカラーで
        while y < h {
            box_blur_h_single_row(src.as_ptr().add(y * w), dst.as_mut_ptr().add(y * w), w, r);
            y += 1;
        }
    }

    #[inline(always)]
    unsafe fn write_lanes(dbase: *mut u32, row_off_arr: &[i32; 8], x: usize, packed: __m256i) {
        let mut buf = [0i32; 8];
        _mm256_storeu_si256(buf.as_mut_ptr() as *mut __m256i, packed);
        for lane in 0..8 {
            *dbase.add(row_off_arr[lane] as usize + x) = buf[lane] as u32;
        }
    }

    // 8列を同時に処理（列方向は連続メモリなので通常load/store）
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
            let out = pack_pixel(finalize(r_sum, recip), finalize(g_sum, recip), finalize(b_sum, recip));
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

                let out = pack_pixel(finalize(r_sum, recip), finalize(g_sum, recip), finalize(b_sum, recip));
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
#[cfg(target_arch = "aarch64")]
mod neon {
    use super::*;
    use core::arch::aarch64::*;

    #[inline(always)]
    unsafe fn load4(ptr: *const u32) -> (int32x4_t, int32x4_t, int32x4_t) {
        let p = vld1q_u32(ptr);
        let mask = vdupq_n_u32(0xFF);
        (vreinterpretq_s32_u32(vandq_u32(vshrq_n_u32(p, 16), mask)),
         vreinterpretq_s32_u32(vandq_u32(vshrq_n_u32(p, 8), mask)),
         vreinterpretq_s32_u32(vandq_u32(p, mask)))
    }

    #[inline(always)]
    unsafe fn store4(ptr: *mut u32, r: int32x4_t, g: int32x4_t, b: int32x4_t) {
        let out = vorrq_u32(vorrq_u32(vshlq_n_u32(vreinterpretq_u32_s32(r), 16), vshlq_n_u32(vreinterpretq_u32_s32(g), 8)), vreinterpretq_u32_s32(b));
        vst1q_u32(ptr, out);
    }

    pub unsafe fn blur_h_simd(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
        let safe_start = (blur_r as usize).min(w);
        let safe_end = w.saturating_sub(blur_r as usize);

        for y in 0..h {
            let src_row = src.as_ptr().add(y * w);
            let dst_row = dst.as_mut_ptr().add(y * w);
            let mut x = 0;

            while x < safe_start { blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r); x += 1; }

            while x + 4 <= safe_end {
                let mut r_acc = vdupq_n_s32(0); let mut g_acc = vdupq_n_s32(0); let mut b_acc = vdupq_n_s32(0);
                for k in 0..kernel.len() {
                    let sx = x as i32 + k as i32 - blur_r;
                    let (r, g, b) = load4(src_row.add(sx as usize));
                    let kw = vdupq_n_s32(*kernel.get_unchecked(k));
                    r_acc = vmlaq_s32(r_acc, r, kw);
                    g_acc = vmlaq_s32(g_acc, g, kw);
                    b_acc = vmlaq_s32(b_acc, b, kw);
                }
                store4(dst_row.add(x), vshrq_n_s32(r_acc, FIXED_SHIFT), vshrq_n_s32(g_acc, FIXED_SHIFT), vshrq_n_s32(b_acc, FIXED_SHIFT));
                x += 4;
            }

            while x < w { blur_h_pixel_scalar(src_row, dst_row, x, w, kernel, blur_r); x += 1; }
        }
    }

    pub unsafe fn blur_v_simd(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
        for y in 0..h {
            let dst_row = dst.as_mut_ptr().add(y * w);
            let mut x = 0;

            while x + 4 <= w {
                let mut r_acc = vdupq_n_s32(0); let mut g_acc = vdupq_n_s32(0); let mut b_acc = vdupq_n_s32(0);
                for k in 0..kernel.len() {
                    let sy = (y as i32 + k as i32 - blur_r).clamp(0, h as i32 - 1) as usize;
                    let (r, g, b) = load4(src.as_ptr().add(sy * w + x));
                    let kw = vdupq_n_s32(*kernel.get_unchecked(k));
                    r_acc = vmlaq_s32(r_acc, r, kw);
                    g_acc = vmlaq_s32(g_acc, g, kw);
                    b_acc = vmlaq_s32(b_acc, b, kw);
                }
                store4(dst_row.add(x), vshrq_n_s32(r_acc, FIXED_SHIFT), vshrq_n_s32(g_acc, FIXED_SHIFT), vshrq_n_s32(b_acc, FIXED_SHIFT));
                x += 4;
            }

            while x < w { blur_v_pixel_scalar(src.as_ptr(), dst_row, x, w, h, y, kernel, blur_r); x += 1; }
        }
    }
}

// ----------------------------------------------------------------------------
// API
// ----------------------------------------------------------------------------
pub fn build_fixed_kernel_buffer(blur_r: i32, out_kernel: &mut [i32]) {
    let sigma = blur_r as f32 / 3.0;
    let sigma_sq2 = 2.0 * sigma * sigma;
    let mut ksum = 0.0f32;
    for i in -blur_r..=blur_r {
        let x = i as f32;
        let kw = libm::expf(-x * x / sigma_sq2);
        out_kernel[(i + blur_r) as usize] = (kw * FIXED_ONE as f32) as i32;
        ksum += kw;
    }
    let scale = FIXED_ONE as f32 / ksum;
    for kw in out_kernel[0..=(blur_r * 2) as usize].iter_mut() {
        *kw = (*kw as f32 * scale / FIXED_ONE as f32) as i32;
    }
    // Integer truncation must not reduce the total weight: a normalized blur
    // keeps a flat-color image at exactly the same brightness on every pass.
    let normalized_sum: i32 = out_kernel[0..=(blur_r * 2) as usize].iter().sum();
    out_kernel[blur_r as usize] += FIXED_ONE - normalized_sum;
}

fn gaussian_convolution_scalar(src: &[u32], dst: &mut [u32], w: usize, h: usize, blur_r: i32) {
    let kernel_size = (blur_r * 2 + 1) as usize;
    let mut kernel = alloc::vec![0i32; kernel_size];
    build_fixed_kernel_buffer(blur_r, &mut kernel);
    let mut tmp = alloc::vec![0u32; w * h];
    let mut x = 0;
    while x + 4 <= w {
        unsafe { blur_h_pixel_scalar(src.as_ptr().add(x), tmp.as_mut_ptr().add(x), x, w, &kernel, blur_r); }
        x += 4;
    }
    while x < w {
        unsafe { blur_h_pixel_scalar(src.as_ptr().add(x), tmp.as_mut_ptr().add(x), x, w, &kernel, blur_r); }
        x += 1;
    }
    for y in 0..h {
        let mut cx = 0;
        while cx + 4 <= w {
            unsafe { blur_v_pixel_scalar(tmp.as_ptr(), dst.as_mut_ptr().add(y * w + cx), cx, w, h, y, &kernel, blur_r); }
            cx += 4;
        }
        while cx < w {
            unsafe { blur_v_pixel_scalar(tmp.as_ptr(), dst.as_mut_ptr().add(y * w + cx), cx, w, h, y, &kernel, blur_r); }
            cx += 1;
        }
    }
}

fn gaussian_convolution(src: &[u32], dst: &mut [u32], w: usize, h: usize, blur_r: i32) {
    let kernel_size = (blur_r * 2 + 1) as usize;
    let mut kernel = alloc::vec![0i32; kernel_size];
    build_fixed_kernel_buffer(blur_r, &mut kernel);
    let mut tmp = alloc::vec![0u32; w * h];

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    unsafe {
        avx2::blur_h_simd(src, &mut tmp, w, h, &kernel, blur_r);
        avx2::blur_v_simd(&tmp, dst, w, h, &kernel, blur_r);
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        neon::blur_h_simd(src, &mut tmp, w, h, &kernel, blur_r);
        neon::blur_v_simd(&tmp, dst, w, h, &kernel, blur_r);
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        target_arch = "aarch64"
    )))]
    {
        gaussian_convolution_scalar(src, dst, w, h, blur_r);
    }
}

pub fn blur_region_to(src: &[u32], dst: &mut [u32], w: usize, y_start: usize, y_end: usize, blur_r: i32) {
    let region_h = y_end - y_start;
    if region_h == 0 || w == 0 { return; }
    let region = &src[y_start * w..y_end * w];

    if blur_r >= 10 {
        box_blur_2pass(region, dst, w, region_h, blur_r);
    } else {
        gaussian_convolution(region, dst, w, region_h, blur_r);
    }
}

pub fn blur_region_to_with_scratch(
    src: &[u32],
    dst: &mut [u32],
    scratch: &mut [u32],
    w: usize,
    y_start: usize,
    y_end: usize,
    blur_r: i32,
) {
    let region_h = y_end.saturating_sub(y_start);
    let len = w.saturating_mul(region_h);
    if len == 0 || src.len() < y_end.saturating_mul(w)
        || dst.len() < len || scratch.len() < len
    {
        return;
    }
    let region = &src[y_start * w..y_end * w];
    if blur_r >= 10 {
        box_blur_2pass_with_scratch(
            region,
            &mut dst[..len],
            &mut scratch[..len],
            w,
            region_h,
            blur_r,
        );
    } else {
        gaussian_convolution(region, &mut dst[..len], w, region_h, blur_r);
    }
}

pub fn blur_region_darkened_to(src: &[u32], dst: &mut [u32], w: usize, y_start: usize, y_end: usize, blur_r: i32, brightness: u32) {
    let region_h = y_end - y_start;
    if region_h == 0 || w == 0 { return; }
    let region = &src[y_start * w..y_end * w];

    if blur_r >= 10 {
        box_blur_2pass(region, dst, w, region_h, blur_r);
    } else {
        gaussian_convolution(region, dst, w, region_h, blur_r);
    }

    let b = brightness;
    for px in dst[..w * region_h].iter_mut() {
        let r = (((*px >> 16) & 0xFF) * b / 255) as u32;
        let g = (((*px >> 8) & 0xFF) * b / 255) as u32;
        let bl = ((*px & 0xFF) * b / 255) as u32;
        *px = (r << 16) | (g << 8) | bl;
    }
}
