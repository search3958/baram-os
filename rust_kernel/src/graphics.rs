use core::ffi::c_void;

use crate::runtime::{rt_atan2f, rt_free, rt_malloc};

unsafe extern "C" {
}

pub fn lerp_color(c1: u32, c2: u32, t: f32) -> u32 {
    let r1 = ((c1 >> 16) & 0xff) as f32;
    let g1 = ((c1 >> 8) & 0xff) as f32;
    let b1 = (c1 & 0xff) as f32;
    let a1 = ((c1 >> 24) & 0xff) as f32;
    let r2 = ((c2 >> 16) & 0xff) as f32;
    let g2 = ((c2 >> 8) & 0xff) as f32;
    let b2 = (c2 & 0xff) as f32;
    let a2 = ((c2 >> 24) & 0xff) as f32;

    let r = (r1 + (r2 - r1) * t) as u32;
    let g = (g1 + (g2 - g1) * t) as u32;
    let b = (b1 + (b2 - b1) * t) as u32;
    let a = (a1 + (a2 - a1) * t) as u32;

    (a << 24) | (r << 16) | (g << 8) | b
}

pub fn blend_colors(bg: u32, fg: u32, alpha: u8) -> u32 {
    if alpha == 0 {
        return bg;
    }
    if alpha == 255 {
        return fg | 0xff00_0000;
    }

    let inv_alpha = (255u32).saturating_sub(alpha as u32);
    let rb_bg = bg & 0x00ff_00ff;
    let g_bg = (bg >> 8) & 0xff;
    let rb_fg = fg & 0x00ff_00ff;
    let g_fg = (fg >> 8) & 0xff;

    let rb_out = ((rb_fg * alpha as u32) + (rb_bg * inv_alpha)) >> 8;
    let g_out = ((g_fg * alpha as u32) + (g_bg * inv_alpha)) >> 8;

    0xff00_0000 | (rb_out & 0x00ff_00ff) | ((g_out & 0xff) << 8)
}

pub unsafe fn apply_conic_gradient(
    data: *mut u8,
    w: i32,
    h: i32,
    rx: i32,
    ry: i32,
    rw: i32,
    rh: i32,
    c1: u32,
    c2: u32,
) {
    if data.is_null() || w <= 0 || h <= 0 || rw <= 0 || rh <= 0 {
        return;
    }

    let cx = rx as f32 + rw as f32 / 2.0;
    let cy = ry as f32 + rh as f32 / 2.0;
    const PI: f32 = 3.14159265;

    let mut y = ry;
    while y < ry + rh {
        if y >= 0 && y < h {
            let mut x = rx;
            while x < rx + rw {
                if x >= 0 && x < w {
                    let mask_idx = ((y * w + x) * 4 + 3) as isize;
                    let mask = *data.offset(mask_idx);
                    if mask != 0 {
                        let dx = x as f32 - cx;
                        let dy = y as f32 - cy;
                        let angle = rt_atan2f(dy, dx);
                        let t = (angle + PI) / (2.0 * PI);
                        let color = lerp_color(c1, c2, t);
                        let idx = ((y * w + x) * 4) as isize;
                        *data.offset(idx) = ((color >> 16) & 0xff) as u8;
                        *data.offset(idx + 1) = ((color >> 8) & 0xff) as u8;
                        *data.offset(idx + 2) = (color & 0xff) as u8;
                        *data.offset(idx + 3) =
                            ((((color >> 24) & 0xff) as u32 * mask as u32) / 255) as u8;
                    }
                }
                x += 1;
            }
        }
        y += 1;
    }
}

pub unsafe fn box_blur_alpha(data: *mut u8, w: i32, h: i32, radius: i32) {
    if data.is_null() || w <= 0 || h <= 0 || radius <= 0 {
        return;
    }

    let tmp = rt_malloc((w as usize) * (h as usize)) as *mut u8;
    if tmp.is_null() {
        return;
    }

    let mut pass = 0;
    while pass < 3 {
        let mut y = 0;
        while y < h {
          let mut x = 0;
          while x < w {
                let mut sum = 0i32;
                let mut count = 0i32;
                let mut dx = -radius;
                while dx <= radius {
                    let nx = x + dx;
                    if nx >= 0 && nx < w {
                        let alpha = *data.offset(((y * w + nx) * 4 + 3) as isize) as i32;
                        sum += alpha;
                        count += 1;
                    }
                    dx += 1;
                }
                *tmp.offset((y * w + x) as isize) = (sum / count) as u8;
                x += 1;
            }
            y += 1;
        }

        let mut x = 0;
        while x < w {
            let mut y = 0;
            while y < h {
                let mut sum = 0i32;
                let mut count = 0i32;
                let mut dy = -radius;
                while dy <= radius {
                    let ny = y + dy;
                    if ny >= 0 && ny < h {
                        sum += *tmp.offset((ny * w + x) as isize) as i32;
                        count += 1;
                    }
                    dy += 1;
                }
                *data.offset(((y * w + x) * 4 + 3) as isize) = (sum / count) as u8;
                y += 1;
            }
            x += 1;
        }

        pass += 1;
    }

    rt_free(tmp as *mut c_void);
}
