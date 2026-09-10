pub fn blit_cached(layer: &mut LayerSystem, pixels: &[u8], w: usize, h: usize, ox: i32, oy: i32) {
    blit_cached_alpha(layer, pixels, w, h, ox, oy, 255);
}

pub fn blit_cached_alpha(
    layer: &mut LayerSystem,
    pixels: &[u8],
    w: usize,
    h: usize,
    ox: i32,
    oy: i32,
    alpha_scale: u32,
) {
    let lw = layer.width();
    let lh = layer.height();
    let buf = layer.buf_mut();
    let stride = w * 4;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use core::arch::aarch64::*;
        let _v_alpha_scale = vdupq_n_s32(alpha_scale as i32);
        let _v_255 = vdupq_n_s32(255);
        let _v_255_inv = vdupq_n_s32(0);

        for sy in 0..h {
            let dst_y = oy as usize + sy;
            if dst_y >= lh {
                break;
            }
            let row = sy * stride;
            let dst_row = dst_y * lw;
            let dst_x0 = ox.max(0) as usize;
            let sx0 = dst_x0.saturating_sub(ox as usize);

            let mut sx = sx0;
            while sx + 4 <= w {
                let dst_x = ox as usize + sx;
                if dst_x + 4 > lw {
                    break;
                }

                let off0 = row + sx * 4;
                let a0 = pixels[off0 + 3] as i32;
                let a1 = pixels[off0 + 4 + 3] as i32;
                let a2 = pixels[off0 + 8 + 3] as i32;
                let a3 = pixels[off0 + 12 + 3] as i32;

                let a_scaled = vld1q_s32(
                    [
                        a0 * alpha_scale as i32 / 255,
                        a1 * alpha_scale as i32 / 255,
                        a2 * alpha_scale as i32 / 255,
                        a3 * alpha_scale as i32 / 255,
                    ]
                    .as_ptr(),
                );

                let all_zero = vmaxvq_s32(a_scaled) == 0;
                if all_zero {
                    sx += 4;
                    continue;
                }

                let bg_ptr = buf[dst_row + dst_x..].as_mut_ptr();
                let bg0 = *bg_ptr.add(0);
                let bg1 = *bg_ptr.add(1);
                let bg2 = *bg_ptr.add(2);
                let bg3 = *bg_ptr.add(3);

                let inv0 = 255 - vgetq_lane_s32(a_scaled, 0);
                let inv1 = 255 - vgetq_lane_s32(a_scaled, 1);
                let inv2 = 255 - vgetq_lane_s32(a_scaled, 2);
                let inv3 = 255 - vgetq_lane_s32(a_scaled, 3);

                let out = [
                    blend_pixel_u32(
                        bg0,
                        pixels[off0],
                        pixels[off0 + 1],
                        pixels[off0 + 2],
                        vgetq_lane_s32(a_scaled, 0) as u32,
                        inv0 as u32,
                    ),
                    blend_pixel_u32(
                        bg1,
                        pixels[off0 + 4],
                        pixels[off0 + 5],
                        pixels[off0 + 6],
                        vgetq_lane_s32(a_scaled, 1) as u32,
                        inv1 as u32,
                    ),
                    blend_pixel_u32(
                        bg2,
                        pixels[off0 + 8],
                        pixels[off0 + 9],
                        pixels[off0 + 10],
                        vgetq_lane_s32(a_scaled, 2) as u32,
                        inv2 as u32,
                    ),
                    blend_pixel_u32(
                        bg3,
                        pixels[off0 + 12],
                        pixels[off0 + 13],
                        pixels[off0 + 14],
                        vgetq_lane_s32(a_scaled, 3) as u32,
                        inv3 as u32,
                    ),
                ];
                vst1q_u32(bg_ptr, vld1q_u32(out.as_ptr()));
                sx += 4;
            }

            for sx in sx..w {
                let dst_x = ox as usize + sx;
                if dst_x >= lw {
                    break;
                }
                let off = row + sx * 4;
                let a = (pixels[off + 3] as u32 * alpha_scale / 255) as u32;
                if a == 0 {
                    continue;
                }
                let dst = &mut buf[dst_row + dst_x];
                if a == 255 {
                    *dst = Color::rgb(pixels[off], pixels[off + 1], pixels[off + 2]).0;
                } else {
                    let cr = pixels[off] as u32;
                    let cg = pixels[off + 1] as u32;
                    let cb = pixels[off + 2] as u32;
                    let bg = Color(*dst);
                    let inv = 255 - a;
                    let r = (cr * a + bg.r() as u32 * inv) / 255;
                    let g = (cg * a + bg.g() as u32 * inv) / 255;
                    let b = (cb * a + bg.b() as u32 * inv) / 255;
                    *dst = Color::rgb(r as u8, g as u8, b as u8).0;
                }
            }
        }
        return;
    }

    #[cfg(not(target_arch = "aarch64"))]
    for sy in 0..h {
        let dst_y = oy as usize + sy;
        if dst_y >= lh {
            break;
        }
        let row = sy * stride;
        let dst_row = dst_y * lw;
        let dst_x0 = ox.max(0) as usize;
        let sx0 = dst_x0.saturating_sub(ox as usize);
        for sx in sx0..w {
            let dst_x = ox as usize + sx;
            if dst_x >= lw {
                break;
            }
            let off = row + sx * 4;
            let a = (pixels[off + 3] as u32 * alpha_scale / 255) as u32;
            if a == 0 {
                continue;
            }
            let dst = &mut buf[dst_row + dst_x];
            if a == 255 {
                *dst = Color::rgb(pixels[off], pixels[off + 1], pixels[off + 2]).0;
            } else {
                let cr = pixels[off] as u32;
                let cg = pixels[off + 1] as u32;
                let cb = pixels[off + 2] as u32;
                let bg = Color(*dst);
                let inv = 255 - a;
                let r = (cr * a + bg.r() as u32 * inv) / 255;
                let g = (cg * a + bg.g() as u32 * inv) / 255;
                let b = (cb * a + bg.b() as u32 * inv) / 255;
                *dst = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }
    }
}

pub fn blit_cached_scaled(
    layer: &mut LayerSystem,
    pixels: &[u8],
    w: usize,
    h: usize,
    ox: i32,
    oy: i32,
    scale: i32,
) {
    if scale <= 1 {
        blit_cached(layer, pixels, w, h, ox, oy);
        return;
    }
    let lw = layer.width();
    let lh = layer.height();
    let buf = layer.buf_mut();
    let src_stride = w * 4;
    let dst_w = w * scale as usize;
    let dst_h = h * scale as usize;
    for sy in 0..dst_h {
        let dst_y = oy as usize + sy;
        if dst_y >= lh {
            break;
        }
        let src_y = sy / scale as usize;
        if src_y >= h {
            break;
        }
        let src_row = src_y * src_stride;
        let dst_row = dst_y * lw;
        for sx in 0..dst_w {
            let dst_x = ox as usize + sx;
            if dst_x >= lw {
                break;
            }
            let src_x = sx / scale as usize;
            if src_x >= w {
                break;
            }
            let off = src_row + src_x * 4;
            let a = pixels[off + 3] as u32;
            if a == 0 {
                continue;
            }
            let dst = &mut buf[dst_row + dst_x];
            if a == 255 {
                *dst = Color::rgb(pixels[off], pixels[off + 1], pixels[off + 2]).0;
            } else {
                let cr = pixels[off] as u32;
                let cg = pixels[off + 1] as u32;
                let cb = pixels[off + 2] as u32;
                let bg = Color(*dst);
                let inv = 255 - a;
                let r = (cr * a + bg.r() as u32 * inv) / 255;
                let g = (cg * a + bg.g() as u32 * inv) / 255;
                let b = (cb * a + bg.b() as u32 * inv) / 255;
                *dst = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }
    }
}

pub fn blit_shadow(layer: &mut LayerSystem, pixels: &[u8], w: usize, h: usize, ox: i32, oy: i32) {
    let lw = layer.width();
    let lh = layer.height();
    let buf = layer.buf_mut();
    let stride = w * 4;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use core::arch::aarch64::*;
        for sy in 0..h {
            let dst_y = oy as usize + sy;
            if dst_y >= lh {
                break;
            }
            let row = sy * stride;
            let dst_row = dst_y * lw;
            let dst_x0 = ox.max(0) as usize;
            let sx0 = dst_x0.saturating_sub(ox as usize);

            let mut sx = sx0;
            while sx + 4 <= w {
                let dst_x = ox as usize + sx;
                if dst_x + 4 > lw {
                    break;
                }

                let off0 = row + sx * 4;
                let a0 = pixels[off0 + 3] as i32;
                let a1 = pixels[off0 + 4 + 3] as i32;
                let a2 = pixels[off0 + 8 + 3] as i32;
                let a3 = pixels[off0 + 12 + 3] as i32;

                if a0 == 0 && a1 == 0 && a2 == 0 && a3 == 0 {
                    sx += 4;
                    continue;
                }

                let bg_ptr = buf[dst_row + dst_x..].as_mut_ptr();
                let bg0 = *bg_ptr.add(0);
                let bg1 = *bg_ptr.add(1);
                let bg2 = *bg_ptr.add(2);
                let bg3 = *bg_ptr.add(3);

                let inv0 = (255 - a0) as u32;
                let inv1 = (255 - a1) as u32;
                let inv2 = (255 - a2) as u32;
                let inv3 = (255 - a3) as u32;

                let out = [
                    ((bg0 >> 16 & 0xFF) * inv0 / 255) << 16
                        | ((bg0 >> 8 & 0xFF) * inv0 / 255) << 8
                        | (bg0 & 0xFF) * inv0 / 255,
                    ((bg1 >> 16 & 0xFF) * inv1 / 255) << 16
                        | ((bg1 >> 8 & 0xFF) * inv1 / 255) << 8
                        | (bg1 & 0xFF) * inv1 / 255,
                    ((bg2 >> 16 & 0xFF) * inv2 / 255) << 16
                        | ((bg2 >> 8 & 0xFF) * inv2 / 255) << 8
                        | (bg2 & 0xFF) * inv2 / 255,
                    ((bg3 >> 16 & 0xFF) * inv3 / 255) << 16
                        | ((bg3 >> 8 & 0xFF) * inv3 / 255) << 8
                        | (bg3 & 0xFF) * inv3 / 255,
                ];
                vst1q_u32(bg_ptr, vld1q_u32(out.as_ptr()));
                sx += 4;
            }

            for sx in sx..w {
                let dst_x = ox as usize + sx;
                if dst_x >= lw {
                    break;
                }
                let a = pixels[row + sx * 4 + 3] as u32;
                if a == 0 {
                    continue;
                }
                let inv = 255 - a;
                let bg = Color(buf[dst_row + dst_x]);
                let r = (bg.r() as u32 * inv) / 255;
                let g = (bg.g() as u32 * inv) / 255;
                let b = (bg.b() as u32 * inv) / 255;
                buf[dst_row + dst_x] = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }
        return;
    }

    #[cfg(not(target_arch = "aarch64"))]
    for sy in 0..h {
        let dst_y = oy as usize + sy;
        if dst_y >= lh {
            break;
        }
        let row = sy * stride;
        let dst_row = dst_y * lw;
        let dst_x0 = ox.max(0) as usize;
        let sx0 = dst_x0.saturating_sub(ox as usize);
        for sx in sx0..w {
            let dst_x = ox as usize + sx;
            if dst_x >= lw {
                break;
            }
            let a = pixels[row + sx * 4 + 3] as u32;
            if a == 0 {
                continue;
            }
            let inv = 255 - a;
            let bg = Color(buf[dst_row + dst_x]);
            let r = (bg.r() as u32 * inv) / 255;
            let g = (bg.g() as u32 * inv) / 255;
            let b = (bg.b() as u32 * inv) / 255;
            buf[dst_row + dst_x] = Color::rgb(r as u8, g as u8, b as u8).0;
        }
    }
}

static mut CURSOR_SHADOW_CACHE: Option<(usize, usize, Vec<u8>)> = None;

pub fn draw_svg_shadow(
    layer: &mut LayerSystem,
    svg: &str,
    ox: i32,
    oy: i32,
    target_w: f32,
    target_h: f32,
    blur_r: i32,
    _offset_y: i32,
) {
    let tw = target_w as usize;
    let th = target_h as usize;
    if tw == 0 || th == 0 {
        return;
    }

    unsafe {
        if let Some((cw, ch, ref cached)) = CURSOR_SHADOW_CACHE {
            if cw == tw && ch == th {
                let pad = blur_r as usize;
                let pw = tw + pad * 2;
                let ph = th + pad * 2;
                blit_shadow(layer, cached, pw, ph, ox - pad as i32, oy - pad as i32);
                return;
            }
        }
    }

    let svg_buf = rasterize_svg_to_buffer(svg, tw, th);

    let mut silhouette: Vec<f32> = alloc::vec![0.0; tw * th];
    for i in 0..tw * th {
        if svg_buf[i * 4 + 3] > 0 {
            silhouette[i] = 1.0;
        }
    }

    let pad = blur_r as usize;
    let pw = tw + pad * 2;
    let ph = th + pad * 2;
    let mut padded: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..th {
        for x in 0..tw {
            padded[(y + pad) * pw + (x + pad)] = silhouette[y * tw + x];
        }
    }

    let sigma = blur_r as f32 / 3.0;
    let mut kernel: Vec<f32> = alloc::vec![0.0; (blur_r * 2 + 1) as usize];
    let mut k_sum = 0.0f32;
    for i in 0..=blur_r * 2 {
        let x = (i - blur_r) as f32;
        let w = libm::expf(-x * x / (2.0 * sigma * sigma));
        kernel[i as usize] = w;
        k_sum += w;
    }
    for k in kernel.iter_mut() {
        *k /= k_sum;
    }

    let mut tmp: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..ph {
        for x in 0..pw {
            let mut sum = 0.0f32;
            for dx in -blur_r..=blur_r {
                let sx = x as i32 + dx;
                if sx >= 0 && sx < pw as i32 {
                    sum += padded[y * pw + sx as usize] * kernel[(dx + blur_r) as usize];
                }
            }
            tmp[y * pw + x] = sum;
        }
    }

    let mut result: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..ph {
        for x in 0..pw {
            let mut sum = 0.0f32;
            for dy in -blur_r..=blur_r {
                let sy = y as i32 + dy;
                if sy >= 0 && sy < ph as i32 {
                    sum += tmp[sy as usize * pw + x] * kernel[(dy + blur_r) as usize];
                }
            }
            result[y * pw + x] = sum;
        }
    }

    let mut shadow: Vec<u8> = alloc::vec![0u8; pw * ph * 4];
    for i in 0..pw * ph {
        let a = (result[i] * 120.0).min(255.0) as u8;
        shadow[i * 4] = 0;
        shadow[i * 4 + 1] = 0;
        shadow[i * 4 + 2] = 0;
        shadow[i * 4 + 3] = a;
    }

    blit_shadow(layer, &shadow, pw, ph, ox - pad as i32, oy - pad as i32);

    unsafe {
        CURSOR_SHADOW_CACHE = Some((tw, th, shadow));
    }
}


