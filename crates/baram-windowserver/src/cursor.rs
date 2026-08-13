use alloc::vec::Vec;
use baram_bsd::config;
use baram_core::LayerSystem;
use baram_graphics::svg;

pub const CURSOR_SVG: &str = include_str!("../../../data/mouse.svg");
pub const CURSOR_SVG_SIZE: &str = include_str!("../../../data/mouse_size.svg");
pub const CURSOR_BOX_W: usize = 15;
pub const CURSOR_BOX_H: usize = 19;
pub const CURSOR_BOX_SIZE_W: usize = 19;
pub const CURSOR_BOX_SIZE_H: usize = 19;

pub struct CursorBitmap {
    pub pixels: Vec<u8>,
    pub shadow: Vec<u8>,
    pub w: usize,
    pub h: usize,
    pub shadow_w: usize,
    pub shadow_h: usize,
}

pub static mut CURSOR_NORMAL: Option<CursorBitmap> = None;
pub static mut CURSOR_RESIZE: Option<CursorBitmap> = None;
pub static mut CURSOR_SIZE_CACHE: [(Option<CursorBitmap>, Option<CursorBitmap>); 51] = [
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
    (None, None),
];

pub fn get_or_prerender_cursor(
    svg: &str,
    size: f32,
    blur_r: i32,
    is_resize: bool,
) -> &'static CursorBitmap {
    let idx = ((size * 10.0) as usize).min(50);
    unsafe {
        let cache = &mut CURSOR_SIZE_CACHE[idx];
        let slot = if is_resize {
            &mut cache.1
        } else {
            &mut cache.0
        };
        if slot.is_none() {
            let base_w = if is_resize {
                config::get_usize("ui-theme/cursor/size_w", 19)
            } else {
                config::get_usize("ui-theme/cursor/w", 15)
            };
            let base_h = if is_resize {
                config::get_usize("ui-theme/cursor/size_h", 19)
            } else {
                config::get_usize("ui-theme/cursor/h", 19)
            };
            let s10 = (size * 10.0) as i32;
            let w = (base_w as i32 * s10 / 10) as usize;
            let h = (base_h as i32 * s10 / 10) as usize;
            *slot = Some(prerender_cursor(svg, w, h, blur_r));
        }
        slot.as_ref().unwrap()
    }
}

pub fn prerender_cursor(svg: &str, w: usize, h: usize, blur_r: i32) -> CursorBitmap {
    let svg_buf = svg::rasterize_svg_to_buffer(svg, w, h);

    let mut silhouette: Vec<f32> = alloc::vec![0.0; w * h];
    for i in 0..w * h {
        if svg_buf[i * 4 + 3] > 0 {
            silhouette[i] = 1.0;
        }
    }

    let pad = blur_r as usize;
    let pw = w + pad * 2;
    let ph = h + pad * 2;
    let mut padded: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..h {
        for x in 0..w {
            padded[(y + pad) * pw + (x + pad)] = silhouette[y * w + x];
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

    CursorBitmap {
        pixels: svg_buf,
        shadow,
        w,
        h,
        shadow_w: pw,
        shadow_h: ph,
    }
}

pub fn draw_cursor_into_layer(
    layer: &mut LayerSystem,
    cx: i32,
    cy: i32,
    resizing: bool,
    pointer_size: f32,
) {
    let blur_r = config::get_i32("ui-theme/cursor/shadow_blur", 12);
    let shadow_x = config::get_i32("ui-theme/cursor/shadow_x", 3);
    let shadow_y = config::get_i32("ui-theme/cursor/shadow_y", 4);
    let pad = blur_r as i32;
    let bitmap = get_or_prerender_cursor(
        if resizing {
            CURSOR_SVG_SIZE
        } else {
            CURSOR_SVG
        },
        pointer_size,
        blur_r,
        resizing,
    );
    svg::blit_shadow(
        layer,
        &bitmap.shadow,
        bitmap.shadow_w,
        bitmap.shadow_h,
        cx + shadow_x - pad,
        cy + shadow_y - pad,
    );
    svg::blit_cached(layer, &bitmap.pixels, bitmap.w, bitmap.h, cx, cy);
}
