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

fn gaussian_convolution_scalar_with_buffers(
    src: &[u32],
    dst: &mut [u32],
    tmp: &mut [u32],
    w: usize,
    h: usize,
    kernel: &[i32],
    blur_r: i32,
) {
    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            unsafe {
                blur_h_pixel_scalar(
                    src.as_ptr().add(row),
                    tmp.as_mut_ptr().add(row),
                    x,
                    w,
                    &kernel,
                    blur_r,
                );
            }
        }
    }
    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            unsafe {
                blur_v_pixel_scalar(
                    tmp.as_ptr(),
                    dst.as_mut_ptr().add(row),
                    x,
                    w,
                    h,
                    y,
                    &kernel,
                    blur_r,
                );
            }
        }
    }
}

fn gaussian_convolution_with_scratch(
    src: &[u32],
    dst: &mut [u32],
    tmp: &mut [u32],
    w: usize,
    h: usize,
    blur_r: i32,
) {
    let kernel_size = (blur_r * 2 + 1) as usize;
    let mut kernel = alloc::vec![0i32; kernel_size];
    build_fixed_kernel_buffer(blur_r, &mut kernel);

    #[cfg(target_arch = "x86_64")]
    {
        if avx2_available() {
            unsafe {
                avx2::blur_h_simd(src, tmp, w, h, &kernel, blur_r);
                avx2::blur_v_simd(tmp, dst, w, h, &kernel, blur_r);
            }
        } else {
            gaussian_convolution_scalar_with_buffers(src, dst, tmp, w, h, &kernel, blur_r);
        }
        return;
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        neon::blur_h_simd(src, tmp, w, h, &kernel, blur_r);
        neon::blur_v_simd(tmp, dst, w, h, &kernel, blur_r);
        return;
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    gaussian_convolution_scalar_with_buffers(src, dst, tmp, w, h, &kernel, blur_r);
}

fn gaussian_convolution(src: &[u32], dst: &mut [u32], w: usize, h: usize, blur_r: i32) {
    let mut tmp = alloc::vec![0u32; w * h];
    gaussian_convolution_with_scratch(src, dst, &mut tmp, w, h, blur_r);
}

pub fn blur_region_to(
    src: &[u32],
    dst: &mut [u32],
    w: usize,
    y_start: usize,
    y_end: usize,
    blur_r: i32,
) {
    let region_h = y_end - y_start;
    if region_h == 0 || w == 0 {
        return;
    }
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
    if len == 0 || src.len() < y_end.saturating_mul(w) || dst.len() < len || scratch.len() < len {
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
        gaussian_convolution_with_scratch(
            region,
            &mut dst[..len],
            &mut scratch[..len],
            w,
            region_h,
            blur_r,
        );
    }
}

pub fn blur_region_darkened_to(
    src: &[u32],
    dst: &mut [u32],
    w: usize,
    y_start: usize,
    y_end: usize,
    blur_r: i32,
    brightness: u32,
) {
    let region_h = y_end - y_start;
    if region_h == 0 || w == 0 {
        return;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_gaussian_preserves_every_pixel_of_a_flat_image() {
        let width = 7;
        let height = 5;
        let color = 0x0064_96c8;
        let src = alloc::vec![color; width * height];
        let mut dst = alloc::vec![0; src.len()];

        let mut scratch = alloc::vec![0; src.len()];
        let mut kernel = alloc::vec![0; 9];
        build_fixed_kernel_buffer(4, &mut kernel);
        gaussian_convolution_scalar_with_buffers(
            &src,
            &mut dst,
            &mut scratch,
            width,
            height,
            &kernel,
            4,
        );

        assert!(dst.iter().all(|pixel| *pixel == color));
    }
}

