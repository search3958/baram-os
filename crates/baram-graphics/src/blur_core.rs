const FIXED_SHIFT: i32 = 10;
const FIXED_ONE: i32 = 1 << FIXED_SHIFT;
const RECIP_SHIFT: i32 = 16;

#[cfg(target_arch = "x86_64")]
#[inline]
fn avx2_available() -> bool {
    use core::arch::x86_64::{__cpuid, __cpuid_count, _xgetbv};
    use core::sync::atomic::{AtomicU8, Ordering};

    // 0 = unknown, 1 = unavailable, 2 = available. CPU and XCR0 state are
    // invariant after boot, so avoid serializing CPUID on every UI blur.
    static CACHED: AtomicU8 = AtomicU8::new(0);
    match CACHED.load(Ordering::Relaxed) {
        1 => return false,
        2 => return true,
        _ => {}
    }

    let available = unsafe {
        let leaf1 = __cpuid(1);
        const OSXSAVE: u32 = 1 << 27;
        const AVX: u32 = 1 << 28;
        if leaf1.ecx & (AVX | OSXSAVE) != (AVX | OSXSAVE) || (_xgetbv(0) & 0x6) != 0x6 {
            false
        } else {
            __cpuid_count(7, 0).ebx & (1 << 5) != 0
        }
    };
    CACHED.store(if available { 2 } else { 1 }, Ordering::Relaxed);
    available
}

#[inline(always)]
fn clamp_u8(v: i32) -> u8 {
    v.max(0).min(255) as u8
}

#[inline(always)]
fn pixel_r(px: u32) -> i32 {
    ((px >> 16) & 0xFF) as i32
}
#[inline(always)]
fn pixel_g(px: u32) -> i32 {
    ((px >> 8) & 0xFF) as i32
}
#[inline(always)]
fn pixel_b(px: u32) -> i32 {
    (px & 0xFF) as i32
}
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
unsafe fn blur_h_pixel_scalar(
    src_row: *const u32,
    dst_row: *mut u32,
    x: usize,
    w: usize,
    kernel: &[i32],
    blur_r: i32,
) {
    let mut r_acc = 0;
    let mut g_acc = 0;
    let mut b_acc = 0;
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
unsafe fn blur_v_pixel_scalar(
    src: *const u32,
    dst_row: *mut u32,
    x: usize,
    w: usize,
    h: usize,
    y: usize,
    kernel: &[i32],
    blur_r: i32,
) {
    let mut r_acc = 0;
    let mut g_acc = 0;
    let mut b_acc = 0;
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

        let mut r_sum = 0i32;
        let mut g_sum = 0i32;
        let mut b_sum = 0i32;
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
        let mut r_sum = 0i32;
        let mut g_sum = 0i32;
        let mut b_sum = 0i32;
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
unsafe fn box_blur_v_single_col(
    src: *const u32,
    dst: *mut u32,
    w: usize,
    h: usize,
    x: usize,
    r: i32,
) {
    let count = 2 * r + 1;
    let recip = box_recip(count);

    let mut r_sum = 0i32;
    let mut g_sum = 0i32;
    let mut b_sum = 0i32;
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

    let mut r_sum = 0i32;
    let mut g_sum = 0i32;
    let mut b_sum = 0i32;
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
        box_blur_v_single_col(
            pass.src,
            pass.dst,
            pass.width,
            pass.height,
            column,
            pass.radius,
        );
    }
}

fn parallel_box_blur_h(src: &[u32], dst: &mut [u32], w: usize, h: usize, r: i32) {
    let pass = HorizontalBoxPass {
        src: src.as_ptr(),
        dst: dst.as_mut_ptr(),
        width: w,
        radius: r,
    };
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

fn box_blur_1pass_scalar(
    src: &[u32],
    dst: &mut [u32],
    tmp: &mut [u32],
    w: usize,
    h: usize,
    r: i32,
) {
    parallel_box_blur_h(src, tmp, w, h, r);
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
    #[cfg(target_arch = "x86_64")]
    if avx2_available() {
        parallel_box_blur_h(src, tmp, w, h, r);
        unsafe {
            box_avx2::box_blur_v_simd8(tmp, dst, w, h, r);
        }
        parallel_box_blur_h(dst, tmp, w, h, r);
        unsafe {
            box_avx2::box_blur_v_simd8(tmp, dst, w, h, r);
        }
        return;
    }
    box_blur_2pass_scalar(src, dst, tmp, w, h, r);
}

fn box_blur_2pass(src: &[u32], dst: &mut [u32], w: usize, h: usize, blur_r: i32) {
    let mut tmp = alloc::vec![0u32; w * h];
    box_blur_2pass_with_scratch(src, dst, &mut tmp, w, h, blur_r);
}

/// Blur a small region using the normal Gaussian path for low radii, but only
/// one horizontal/vertical box-blur sweep for larger radii. This is intended
/// for the title-bar's progressive blur steps; the regular region blur keeps
/// its two box sweeps for wallpaper-quality output.
pub fn blur_region_to_single_box(
    src: &[u32],
    dst: &mut [u32],
    w: usize,
    y_start: usize,
    y_end: usize,
    blur_r: i32,
) {
    let region_h = y_end.saturating_sub(y_start);
    let len = w.saturating_mul(region_h);
    if len == 0 || src.len() < y_end.saturating_mul(w) || dst.len() < len {
        return;
    }
    let region = &src[y_start * w..y_end * w];
    if blur_r < 10 {
        gaussian_convolution(region, &mut dst[..len], w, region_h, blur_r);
        return;
    }

    let mut scratch = alloc::vec![0u32; len];
    let radius = (blur_r / 2).max(1);
    #[cfg(target_arch = "x86_64")]
    if avx2_available() {
        parallel_box_blur_h(region, &mut scratch, w, region_h, radius);
        unsafe {
            box_avx2::box_blur_v_simd8(&scratch, &mut dst[..len], w, region_h, radius);
        }
        return;
    }
    box_blur_1pass_scalar(region, &mut dst[..len], &mut scratch, w, region_h, radius);
}

// ----------------------------------------------------------------------------
// x86_64 (AVX2 - ガウシアン畳み込み: 8画素同時 + キャッシュブロッキング)
// ----------------------------------------------------------------------------

