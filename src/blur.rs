use alloc::vec::Vec;
use crate::gop::Color;

const FIXED_SHIFT: i32 = 10;
const FIXED_ONE: i32 = 1 << FIXED_SHIFT;

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

fn clamp_u8(v: i32) -> u8 {
    v.max(0).min(255) as u8
}

pub fn blur_region_to(src: &[u32], dst: &mut [u32], w: usize, y_start: usize, y_end: usize, blur_r: i32) {
    let kernel = build_fixed_kernel(blur_r);
    let region_h = y_end - y_start;
    let mut tmp = alloc::vec![0u32; w * region_h];

    for y in 0..region_h {
        let src_row = (y + y_start) * w;
        let tmp_row = y * w;
        for x in 0..w {
            let mut r_acc = 0i32;
            let mut g_acc = 0i32;
            let mut b_acc = 0i32;
            let x_i = x as i32;
            for k in 0..kernel.len() {
                let sx = (x_i + k as i32 - blur_r).max(0).min(w as i32 - 1) as usize;
                let px = src[src_row + sx];
                let kw = kernel[k];
                r_acc += ((px >> 16) & 0xFF) as i32 * kw;
                g_acc += ((px >> 8) & 0xFF) as i32 * kw;
                b_acc += (px & 0xFF) as i32 * kw;
            }
            tmp[tmp_row + x] = Color::rgb(
                clamp_u8(r_acc >> FIXED_SHIFT),
                clamp_u8(g_acc >> FIXED_SHIFT),
                clamp_u8(b_acc >> FIXED_SHIFT),
            ).0;
        }
    }

    for y in 0..region_h {
        for x in 0..w {
            let mut r_acc = 0i32;
            let mut g_acc = 0i32;
            let mut b_acc = 0i32;
            for k in 0..kernel.len() {
                let sy = (y as i32 + k as i32 - blur_r).max(0).min(region_h as i32 - 1) as usize;
                let px = tmp[sy * w + x];
                let kw = kernel[k];
                r_acc += ((px >> 16) & 0xFF) as i32 * kw;
                g_acc += ((px >> 8) & 0xFF) as i32 * kw;
                b_acc += (px & 0xFF) as i32 * kw;
            }
            dst[y * w + x] = Color::rgb(
                clamp_u8(r_acc >> FIXED_SHIFT),
                clamp_u8(g_acc >> FIXED_SHIFT),
                clamp_u8(b_acc >> FIXED_SHIFT),
            ).0;
        }
    }
}

pub fn blur_region_darkened_to(src: &[u32], dst: &mut [u32], w: usize, y_start: usize, y_end: usize, blur_r: i32, brightness: u32) {
    let kernel = build_fixed_kernel(blur_r);
    let region_h = y_end - y_start;
    let mut tmp = alloc::vec![0u32; w * region_h];

    for y in 0..region_h {
        let src_row = (y + y_start) * w;
        let tmp_row = y * w;
        for x in 0..w {
            let mut r_acc = 0i32;
            let mut g_acc = 0i32;
            let mut b_acc = 0i32;
            let x_i = x as i32;
            for k in 0..kernel.len() {
                let sx = (x_i + k as i32 - blur_r).max(0).min(w as i32 - 1) as usize;
                let px = src[src_row + sx];
                let kw = kernel[k];
                r_acc += ((px >> 16) & 0xFF) as i32 * kw;
                g_acc += ((px >> 8) & 0xFF) as i32 * kw;
                b_acc += (px & 0xFF) as i32 * kw;
            }
            let b = brightness as i32;
            tmp[tmp_row + x] = Color::rgb(
                clamp_u8((r_acc >> FIXED_SHIFT) * b / 255),
                clamp_u8((g_acc >> FIXED_SHIFT) * b / 255),
                clamp_u8((b_acc >> FIXED_SHIFT) * b / 255),
            ).0;
        }
    }

    for y in 0..region_h {
        let dst_row = (y + y_start) * w;
        for x in 0..w {
            let mut r_acc = 0i32;
            let mut g_acc = 0i32;
            let mut b_acc = 0i32;
            for k in 0..kernel.len() {
                let sy = (y as i32 + k as i32 - blur_r).max(0).min(region_h as i32 - 1) as usize;
                let px = tmp[sy * w + x];
                let kw = kernel[k];
                r_acc += ((px >> 16) & 0xFF) as i32 * kw;
                g_acc += ((px >> 8) & 0xFF) as i32 * kw;
                b_acc += (px & 0xFF) as i32 * kw;
            }
            let b = brightness as i32;
            dst[dst_row + x] = Color::rgb(
                clamp_u8((r_acc >> FIXED_SHIFT) * b / 255),
                clamp_u8((g_acc >> FIXED_SHIFT) * b / 255),
                clamp_u8((b_acc >> FIXED_SHIFT) * b / 255),
            ).0;
        }
    }
}
