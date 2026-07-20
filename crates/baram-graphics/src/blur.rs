use alloc::vec::Vec;
use baram_core::Color;

const FIXED_SHIFT: i32 = 10;
const FIXED_ONE: i32 = 1 << FIXED_SHIFT;

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

#[cfg(target_arch = "aarch64")]
mod neon {
    use super::*;
    use core::arch::aarch64::*;

    #[inline(always)]
    unsafe fn store_4_pixels(ptr: *mut u32, r: int32x4_t, g: int32x4_t, b: int32x4_t) {
        let r_shifted = vreinterpretq_u32_s32(vshlq_n_s32(r, 16));
        let g_shifted = vreinterpretq_u32_s32(vshlq_n_s32(g, 8));
        let out = vorrq_u32(vorrq_u32(r_shifted, g_shifted), vreinterpretq_u32_s32(b));
        vst1q_u32(ptr, out);
    }

    #[inline(always)]
    unsafe fn extract_channels(
        p0: u32, p1: u32, p2: u32, p3: u32,
    ) -> (int32x4_t, int32x4_t, int32x4_t) {
        let r_arr = [pixel_r(p0), pixel_r(p1), pixel_r(p2), pixel_r(p3)];
        let g_arr = [pixel_g(p0), pixel_g(p1), pixel_g(p2), pixel_g(p3)];
        let b_arr = [pixel_b(p0), pixel_b(p1), pixel_b(p2), pixel_b(p3)];
        let r = vld1q_s32(r_arr.as_ptr());
        let g = vld1q_s32(g_arr.as_ptr());
        let b = vld1q_s32(b_arr.as_ptr());
        (r, g, b)
    }

    pub unsafe fn blur_h_neon(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
        let klen = kernel.len();
        let w_i32 = w as i32;
        let x_base = vdupq_n_s32(0);
        let x_max = vdupq_n_s32(w_i32 - 1);

        for y in 0..h {
            let src_row = src.as_ptr().add(y * w);
            let dst_row = dst.as_mut_ptr().add(y * w);
            let mut x = 0usize;

            while x + 4 <= w {
                let mut r_acc = vdupq_n_s32(0);
                let mut g_acc = vdupq_n_s32(0);
                let mut b_acc = vdupq_n_s32(0);
                let indices = [0, 1, 2, 3];
                let x_vec = vaddq_s32(vdupq_n_s32(x as i32), vld1q_s32(indices.as_ptr()));

                let mut k = 0;
                while k + 2 <= klen {
                    let off0 = k as i32 - blur_r;
                    let off1 = (k + 1) as i32 - blur_r;
                    let sx0 = vmaxq_s32(x_base, vminq_s32(vaddq_s32(x_vec, vdupq_n_s32(off0)), x_max));
                    let sx1 = vmaxq_s32(x_base, vminq_s32(vaddq_s32(x_vec, vdupq_n_s32(off1)), x_max));

                    let (r0, g0, b0) = extract_channels(
                        *src_row.add(vgetq_lane_s32(sx0, 0) as usize),
                        *src_row.add(vgetq_lane_s32(sx0, 1) as usize),
                        *src_row.add(vgetq_lane_s32(sx0, 2) as usize),
                        *src_row.add(vgetq_lane_s32(sx0, 3) as usize),
                    );
                    let (r1, g1, b1) = extract_channels(
                        *src_row.add(vgetq_lane_s32(sx1, 0) as usize),
                        *src_row.add(vgetq_lane_s32(sx1, 1) as usize),
                        *src_row.add(vgetq_lane_s32(sx1, 2) as usize),
                        *src_row.add(vgetq_lane_s32(sx1, 3) as usize),
                    );

                    let kw0 = vdupq_n_s32(kernel[k]);
                    let kw1 = vdupq_n_s32(kernel[k + 1]);
                    r_acc = vmlaq_s32(vmlaq_s32(r_acc, r0, kw0), r1, kw1);
                    g_acc = vmlaq_s32(vmlaq_s32(g_acc, g0, kw0), g1, kw1);
                    b_acc = vmlaq_s32(vmlaq_s32(b_acc, b0, kw0), b1, kw1);
                    k += 2;
                }
                for kk in k..klen {
                    let off = kk as i32 - blur_r;
                    let sx = vmaxq_s32(x_base, vminq_s32(vaddq_s32(x_vec, vdupq_n_s32(off)), x_max));
                    let (r0, g0, b0) = extract_channels(
                        *src_row.add(vgetq_lane_s32(sx, 0) as usize),
                        *src_row.add(vgetq_lane_s32(sx, 1) as usize),
                        *src_row.add(vgetq_lane_s32(sx, 2) as usize),
                        *src_row.add(vgetq_lane_s32(sx, 3) as usize),
                    );
                    let kw = vdupq_n_s32(kernel[kk]);
                    r_acc = vmlaq_s32(r_acc, r0, kw);
                    g_acc = vmlaq_s32(g_acc, g0, kw);
                    b_acc = vmlaq_s32(b_acc, b0, kw);
                }

                store_4_pixels(
                    dst_row.add(x),
                    vshrq_n_s32(r_acc, FIXED_SHIFT),
                    vshrq_n_s32(g_acc, FIXED_SHIFT),
                    vshrq_n_s32(b_acc, FIXED_SHIFT),
                );
                x += 4;
            }

            while x < w {
                let (mut r_acc, mut g_acc, mut b_acc) = (0i32, 0i32, 0i32);
                let mut kk = 0;
                while kk + 2 <= klen {
                    let sx0 = (x as i32 + kk as i32 - blur_r).max(0).min(w_i32 - 1) as usize;
                    let sx1 = (x as i32 + (kk + 1) as i32 - blur_r).max(0).min(w_i32 - 1) as usize;
                    let p0 = *src_row.add(sx0);
                    let p1 = *src_row.add(sx1);
                    r_acc += pixel_r(p0) * kernel[kk] + pixel_r(p1) * kernel[kk + 1];
                    g_acc += pixel_g(p0) * kernel[kk] + pixel_g(p1) * kernel[kk + 1];
                    b_acc += pixel_b(p0) * kernel[kk] + pixel_b(p1) * kernel[kk + 1];
                    kk += 2;
                }
                for kk2 in kk..klen {
                    let sx = (x as i32 + kk2 as i32 - blur_r).max(0).min(w_i32 - 1) as usize;
                    let px = *src_row.add(sx);
                    let kw = kernel[kk2];
                    r_acc += pixel_r(px) * kw;
                    g_acc += pixel_g(px) * kw;
                    b_acc += pixel_b(px) * kw;
                }
                *dst_row.add(x) = make_pixel(
                    clamp_u8(r_acc >> FIXED_SHIFT),
                    clamp_u8(g_acc >> FIXED_SHIFT),
                    clamp_u8(b_acc >> FIXED_SHIFT),
                );
                x += 1;
            }
        }
    }

    pub unsafe fn blur_v_neon(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
        let klen = kernel.len();
        let h_i32 = h as i32;
        let y_base = vdupq_n_s32(0);
        let y_max = vdupq_n_s32(h_i32 - 1);
        let src_ptr = src.as_ptr();

        for y in 0..h {
            let dst_row = dst.as_mut_ptr().add(y * w);
            let mut x = 0usize;

            while x + 4 <= w {
                let mut r_acc = vdupq_n_s32(0);
                let mut g_acc = vdupq_n_s32(0);
                let mut b_acc = vdupq_n_s32(0);
                let y_vec = vdupq_n_s32(y as i32);

                let mut k = 0;
                while k + 2 <= klen {
                    let off0 = k as i32 - blur_r;
                    let off1 = (k + 1) as i32 - blur_r;
                    let sy0 = vmaxq_s32(y_base, vminq_s32(vaddq_s32(y_vec, vdupq_n_s32(off0)), y_max));
                    let sy1 = vmaxq_s32(y_base, vminq_s32(vaddq_s32(y_vec, vdupq_n_s32(off1)), y_max));

                    let (r0, g0, b0) = extract_channels(
                        *src_ptr.add(vgetq_lane_s32(sy0, 0) as usize * w + x),
                        *src_ptr.add(vgetq_lane_s32(sy0, 1) as usize * w + x + 1),
                        *src_ptr.add(vgetq_lane_s32(sy0, 2) as usize * w + x + 2),
                        *src_ptr.add(vgetq_lane_s32(sy0, 3) as usize * w + x + 3),
                    );
                    let (r1, g1, b1) = extract_channels(
                        *src_ptr.add(vgetq_lane_s32(sy1, 0) as usize * w + x),
                        *src_ptr.add(vgetq_lane_s32(sy1, 1) as usize * w + x + 1),
                        *src_ptr.add(vgetq_lane_s32(sy1, 2) as usize * w + x + 2),
                        *src_ptr.add(vgetq_lane_s32(sy1, 3) as usize * w + x + 3),
                    );

                    let kw0 = vdupq_n_s32(kernel[k]);
                    let kw1 = vdupq_n_s32(kernel[k + 1]);
                    r_acc = vmlaq_s32(vmlaq_s32(r_acc, r0, kw0), r1, kw1);
                    g_acc = vmlaq_s32(vmlaq_s32(g_acc, g0, kw0), g1, kw1);
                    b_acc = vmlaq_s32(vmlaq_s32(b_acc, b0, kw0), b1, kw1);
                    k += 2;
                }
                for kk in k..klen {
                    let off = kk as i32 - blur_r;
                    let sy = vmaxq_s32(y_base, vminq_s32(vaddq_s32(y_vec, vdupq_n_s32(off)), y_max));
                    let (r0, g0, b0) = extract_channels(
                        *src_ptr.add(vgetq_lane_s32(sy, 0) as usize * w + x),
                        *src_ptr.add(vgetq_lane_s32(sy, 1) as usize * w + x + 1),
                        *src_ptr.add(vgetq_lane_s32(sy, 2) as usize * w + x + 2),
                        *src_ptr.add(vgetq_lane_s32(sy, 3) as usize * w + x + 3),
                    );
                    let kw = vdupq_n_s32(kernel[kk]);
                    r_acc = vmlaq_s32(r_acc, r0, kw);
                    g_acc = vmlaq_s32(g_acc, g0, kw);
                    b_acc = vmlaq_s32(b_acc, b0, kw);
                }

                store_4_pixels(
                    dst_row.add(x),
                    vshrq_n_s32(r_acc, FIXED_SHIFT),
                    vshrq_n_s32(g_acc, FIXED_SHIFT),
                    vshrq_n_s32(b_acc, FIXED_SHIFT),
                );
                x += 4;
            }

            while x < w {
                let (mut r_acc, mut g_acc, mut b_acc) = (0i32, 0i32, 0i32);
                let mut kk = 0;
                while kk + 2 <= klen {
                    let sy0 = (y as i32 + kk as i32 - blur_r).max(0).min(h_i32 - 1) as usize;
                    let sy1 = (y as i32 + (kk + 1) as i32 - blur_r).max(0).min(h_i32 - 1) as usize;
                    let p0 = *src_ptr.add(sy0 * w + x);
                    let p1 = *src_ptr.add(sy1 * w + x);
                    r_acc += pixel_r(p0) * kernel[kk] + pixel_r(p1) * kernel[kk + 1];
                    g_acc += pixel_g(p0) * kernel[kk] + pixel_g(p1) * kernel[kk + 1];
                    b_acc += pixel_b(p0) * kernel[kk] + pixel_b(p1) * kernel[kk + 1];
                    kk += 2;
                }
                for kk2 in kk..klen {
                    let sy = (y as i32 + kk2 as i32 - blur_r).max(0).min(h_i32 - 1) as usize;
                    let px = *src_ptr.add(sy * w + x);
                    let kw = kernel[kk2];
                    r_acc += pixel_r(px) * kw;
                    g_acc += pixel_g(px) * kw;
                    b_acc += pixel_b(px) * kw;
                }
                *dst_row.add(x) = make_pixel(
                    clamp_u8(r_acc >> FIXED_SHIFT),
                    clamp_u8(g_acc >> FIXED_SHIFT),
                    clamp_u8(b_acc >> FIXED_SHIFT),
                );
                x += 1;
            }
        }
    }
}

fn blur_h_scalar(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
    let klen = kernel.len();
    for y in 0..h {
        let src_row = y * w;
        let dst_row = y * w;
        for x in 0..w {
            let (mut r_acc, mut g_acc, mut b_acc) = (0i32, 0i32, 0i32);
            for k in 0..klen {
                let sx = (x as i32 + k as i32 - blur_r).max(0).min(w as i32 - 1) as usize;
                let px = src[src_row + sx];
                let kw = kernel[k];
                r_acc += pixel_r(px) * kw;
                g_acc += pixel_g(px) * kw;
                b_acc += pixel_b(px) * kw;
            }
            dst[dst_row + x] = make_pixel(
                clamp_u8(r_acc >> FIXED_SHIFT),
                clamp_u8(g_acc >> FIXED_SHIFT),
                clamp_u8(b_acc >> FIXED_SHIFT),
            );
        }
    }
}

fn blur_v_scalar(src: &[u32], dst: &mut [u32], w: usize, h: usize, kernel: &[i32], blur_r: i32) {
    let klen = kernel.len();
    for y in 0..h {
        let dst_row = y * w;
        for x in 0..w {
            let (mut r_acc, mut g_acc, mut b_acc) = (0i32, 0i32, 0i32);
            for k in 0..klen {
                let sy = (y as i32 + k as i32 - blur_r).max(0).min(h as i32 - 1) as usize;
                let px = src[sy * w + x];
                let kw = kernel[k];
                r_acc += pixel_r(px) * kw;
                g_acc += pixel_g(px) * kw;
                b_acc += pixel_b(px) * kw;
            }
            dst[dst_row + x] = make_pixel(
                clamp_u8(r_acc >> FIXED_SHIFT),
                clamp_u8(g_acc >> FIXED_SHIFT),
                clamp_u8(b_acc >> FIXED_SHIFT),
            );
        }
    }
}

pub fn blur_region_to(src: &[u32], dst: &mut [u32], w: usize, y_start: usize, y_end: usize, blur_r: i32) {
    let kernel = build_fixed_kernel(blur_r);
    let region_h = y_end - y_start;
    if region_h == 0 || w == 0 {
        return;
    }
    let mut tmp = alloc::vec![0u32; w * region_h];
    let region: Vec<u32> = src[y_start * w..y_end * w].to_vec();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        neon::blur_h_neon(&region, &mut tmp, w, region_h, &kernel, blur_r);
        neon::blur_v_neon(&tmp, dst, w, region_h, &kernel, blur_r);
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        blur_h_scalar(&region, &mut tmp, w, region_h, &kernel, blur_r);
        blur_v_scalar(&tmp, dst, w, region_h, &kernel, blur_r);
    }
}

pub fn blur_region_darkened_to(src: &[u32], dst: &mut [u32], w: usize, y_start: usize, y_end: usize, blur_r: i32, brightness: u32) {
    let kernel = build_fixed_kernel(blur_r);
    let region_h = y_end - y_start;
    if region_h == 0 || w == 0 {
        return;
    }
    let mut tmp = alloc::vec![0u32; w * region_h];
    let region: Vec<u32> = src[y_start * w..y_end * w].to_vec();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        neon::blur_h_neon(&region, &mut tmp, w, region_h, &kernel, blur_r);
        neon::blur_v_neon(&mut tmp, dst, w, region_h, &kernel, blur_r);
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        blur_h_scalar(&region, &mut tmp, w, region_h, &kernel, blur_r);
        blur_v_scalar(&mut tmp, dst, w, region_h, &kernel, blur_r);
    }

    let b = brightness;
    for px in dst[..w * region_h].iter_mut() {
        let r = (((*px >> 16) & 0xFF) * b / 255) as u32;
        let g = (((*px >> 8) & 0xFF) * b / 255) as u32;
        let bl = ((*px & 0xFF) * b / 255) as u32;
        *px = (r << 16) | (g << 8) | bl;
    }
}
