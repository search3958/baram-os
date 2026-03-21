use core::ffi::{c_char, c_void};

use crate::graphics::blend_colors;

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}

#[repr(C)]
pub struct RustLayer {
    pub buffer: *mut u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub transparent: u32,
    pub active: i32,
    pub dynamic: i32,
}

#[repr(C)]
pub struct RustWindow {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub old_x: i32,
    pub old_y: i32,
    pub old_w: i32,
    pub old_h: i32,
    pub is_maximized: i32,
    pub title: [c_char; 64],
    pub warp_ctx: *mut c_void,
    pub warp1_ctx: *mut c_void,
    pub is_warp1: i32,
    pub rgba_buffer: *mut u8,
    pub buffer_w: i32,
    pub buffer_h: i32,
    pub is_dirty: i32,
    pub is_dragging: i32,
    pub is_resizing: i32,
    pub is_movable: i32,
    pub is_resizing_enabled: i32,
    pub is_always_full_res: i32,
    pub is_sticky: i32,
    pub background_color: u32,
    pub force_dark: i32,
    pub resize_w: i32,
    pub resize_h: i32,
    pub fade_alpha: f32,
    pub is_calculating: i32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub target_scroll_x: f32,
    pub target_scroll_y: f32,
    pub no_decoration: i32,
    pub is_menubar: i32,
    pub shadow_cache: *mut u8,
    pub shadow_cache_w: i32,
    pub shadow_cache_h: i32,
    pub frame_cache: *mut u32,
    pub frame_cache_w: i32,
    pub frame_cache_h: i32,
    pub window_mask: *mut u8,
    pub last_svg_str: *mut c_char,
    pub svg_image_cache: *mut c_void,
    pub raster_cache: *mut u32,
    pub raster_cache_w: i32,
    pub raster_cache_h: i32,
    pub render_scale: f32,
    pub dynamic_file_ptr: *mut c_void,
    pub unified_buffer: *mut u32,
    pub unified_w: i32,
    pub unified_h: i32,
}

pub fn should_bake_inactive(
    is_active: i32,
    buffer_h: i32,
    window_h: i32,
    scroll_y: f32,
    target_scroll_y: f32,
) -> i32 {
    if is_active != 0 {
        return 0;
    }
    if buffer_h > window_h {
        return 0;
    }
    if scroll_y != 0.0 || target_scroll_y != 0.0 {
        return 0;
    }
    1
}

pub fn compute_content_src_y(dy: i32, scroll_y: f32, scale: f32, buffer_h: i32) -> i32 {
    let src_y = ((dy as f32 - scroll_y) * scale) as i32;
    if src_y < 0 || src_y >= buffer_h {
        -1
    } else {
        src_y
    }
}

pub unsafe fn clear_cached_buffers(win: *mut RustWindow) {
    if win.is_null() {
        return;
    }
    let win = &mut *win;
    if !win.shadow_cache.is_null() {
        free(win.shadow_cache as *mut c_void);
        win.shadow_cache = core::ptr::null_mut();
    }
    if !win.frame_cache.is_null() {
        free(win.frame_cache as *mut c_void);
        win.frame_cache = core::ptr::null_mut();
    }
    if !win.window_mask.is_null() {
        free(win.window_mask as *mut c_void);
        win.window_mask = core::ptr::null_mut();
    }
    if !win.rgba_buffer.is_null() {
        free(win.rgba_buffer as *mut c_void);
        win.rgba_buffer = core::ptr::null_mut();
    }
    if !win.raster_cache.is_null() {
        free(win.raster_cache as *mut c_void);
        win.raster_cache = core::ptr::null_mut();
    }
    win.buffer_w = 0;
    win.buffer_h = 0;
    win.raster_cache_w = 0;
    win.raster_cache_h = 0;
}

pub unsafe fn bake_window(win: *mut RustWindow) {
    if win.is_null() {
        return;
    }
    let win = &mut *win;
    if win.shadow_cache.is_null() || win.frame_cache.is_null() || win.rgba_buffer.is_null() || win.window_mask.is_null() {
        return;
    }

    let scale = win.render_scale;
    let title_h = if win.no_decoration != 0 { 0 } else { 40 };
    let shadow_size = if win.no_decoration != 0 { 0 } else { 48 };
    let full_sw = win.w + shadow_size * 2;
    let full_sh = win.h + title_h + shadow_size * 2;
    let mut sw = ((full_sw as f32) * scale) as i32;
    let mut sh = ((full_sh as f32) * scale) as i32;
    if sw < 1 { sw = 1; }
    if sh < 1 { sh = 1; }

    if !win.unified_buffer.is_null() {
        free(win.unified_buffer as *mut c_void);
    }
    win.unified_buffer = malloc((sw as usize) * (sh as usize) * 4usize) as *mut u32;
    if win.unified_buffer.is_null() {
        win.unified_w = 0;
        win.unified_h = 0;
        return;
    }
    win.unified_w = sw;
    win.unified_h = sh;

    let mut i = 0i32;
    while i < sw * sh {
        *win.unified_buffer.add(i as usize) = 0;
        i += 1;
    }

    let shadow_off_y = (8.0f32 * scale) as i32;
    let mut y = 0i32;
    while y < win.shadow_cache_h {
        let py = y + shadow_off_y;
        if py >= sh { break; }
        let mut x = 0i32;
        while x < win.shadow_cache_w {
            let alpha = *win.shadow_cache.add((y * win.shadow_cache_w + x) as usize);
            if alpha > 0 {
                *win.unified_buffer.add((py * sw + x) as usize) = (alpha as u32) << 24;
            }
            x += 1;
        }
        y += 1;
    }

    let frame_x = ((shadow_size as f32) * scale) as i32;
    let frame_y = ((shadow_size as f32) * scale) as i32;
    let mut mw = ((win.w as f32) * scale) as i32;
    if mw < 1 && win.w > 0 { mw = 1; }

    let mut dy = 0i32;
    while dy < win.frame_cache_h {
        let py = frame_y + dy;
        let src_line = win.frame_cache.add((dy * win.frame_cache_w) as usize);
        let mask_line = win.window_mask.add((dy * mw) as usize);
        let mut dx = 0i32;
        while dx < win.frame_cache_w {
            let px = frame_x + dx;
            let color = *src_line.add(dx as usize);
            let alpha = if win.is_maximized != 0 { 255 } else { *mask_line.add(dx as usize) };
            let dst = win.unified_buffer.add((py * sw + px) as usize);
            *dst = blend_colors(*dst, color, alpha);
            dx += 1;
        }
        dy += 1;
    }

    let content_y = frame_y + ((title_h as f32) * scale) as i32;
    let mh = (((win.h + title_h) as f32) * scale) as i32;
    let mut dy2 = 0i32;
    while dy2 < win.buffer_h {
        let py = content_y + dy2;
        if py >= sh { break; }
        let src_line = (win.rgba_buffer as *mut u32).add((dy2 * win.buffer_w) as usize);
        let mut mask_y = ((title_h as f32) * scale) as i32 + dy2;
        if mask_y >= mh { mask_y = mh - 1; }
        let mask_line = win.window_mask.add((mask_y * mw) as usize);
        let mut dx = 0i32;
        while dx < win.buffer_w {
            let px = frame_x + dx;
            if px >= sw { break; }
            let color = *src_line.add(dx as usize);
            let alpha = if win.is_maximized != 0 || win.no_decoration != 0 { 255 } else { *mask_line.add(dx as usize) };
            let dst = win.unified_buffer.add((py * sw + px) as usize);
            *dst = blend_colors(*dst, color, alpha);
            dx += 1;
        }
        dy2 += 1;
    }

    clear_cached_buffers(win);
}

pub unsafe fn draw_single_window(layer: *mut RustLayer, win: *mut RustWindow) {
    if layer.is_null() || win.is_null() {
        return;
    }
    let layer = &mut *layer;
    let win = &mut *win;

    if !win.unified_buffer.is_null() && win.unified_w > 0 && win.unified_h > 0 {
        let title_h = if win.no_decoration != 0 { 0 } else { 40 };
        let shadow_size = if win.no_decoration != 0 { 0 } else { 48 };
        let start_x = win.x - shadow_size;
        let start_y = win.y - title_h - shadow_size;
        let x0 = if start_x < 0 { -start_x } else { 0 };
        let y0 = if start_y < 0 { -start_y } else { 0 };
        let x1 = if start_x + win.unified_w > layer.width { layer.width - start_x } else { win.unified_w };
        let y1 = if start_y + win.unified_h > layer.height { layer.height - start_y } else { win.unified_h };

        let mut dy = y0;
        while dy < y1 {
            let py = start_y + dy;
            let dst_line = layer.buffer.add((py * layer.width) as usize);
            let src_line = win.unified_buffer.add((dy * win.unified_w) as usize);
            let mut dx = x0;
            while dx < x1 {
                let px = start_x + dx;
                let color = *src_line.add(dx as usize);
                let alpha = (color >> 24) as u8;
                if alpha != 0 {
                    let dst = dst_line.add(px as usize);
                    *dst = blend_colors(*dst, color, alpha);
                }
                dx += 1;
            }
            dy += 1;
        }
        return;
    }

    if win.rgba_buffer.is_null() || !(win.no_decoration != 0 || (!win.shadow_cache.is_null() && !win.frame_cache.is_null())) {
        return;
    }

    let title_h = if win.no_decoration != 0 { 0 } else { 40 };
    let shadow_size = if win.no_decoration != 0 { 0 } else { 48 };
    let scale = win.render_scale;

    if win.is_maximized == 0 && win.no_decoration == 0 && !win.shadow_cache.is_null() {
        let sx_start = win.x - shadow_size;
        let sy_start = win.y - title_h - shadow_size + 8;
        let y0 = if sy_start < 0 { -sy_start } else { 0 };
        let y1 = if sy_start + (win.h + title_h + shadow_size * 2) > layer.height {
            layer.height - sy_start
        } else {
            win.h + title_h + shadow_size * 2
        };
        let x0 = if sx_start < 0 { -sx_start } else { 0 };
        let x1 = if sx_start + (win.w + shadow_size * 2) > layer.width {
            layer.width - sx_start
        } else {
            win.w + shadow_size * 2
        };

        let mut dy = y0;
        while dy < y1 {
            let py = sy_start + dy;
            let dst_line = layer.buffer.add((py * layer.width) as usize);
            let mut scaled_dy = ((dy as f32) * scale) as i32;
            if scaled_dy >= win.shadow_cache_h { scaled_dy = win.shadow_cache_h - 1; }
            let src_mask = win.shadow_cache.add((scaled_dy * win.shadow_cache_w) as usize);
            let mut dx = x0;
            while dx < x1 {
                let mut scaled_dx = ((dx as f32) * scale) as i32;
                if scaled_dx >= win.shadow_cache_w { scaled_dx = win.shadow_cache_w - 1; }
                let alpha = *src_mask.add(scaled_dx as usize);
                if alpha != 0 {
                    let dst = dst_line.add((sx_start + dx) as usize);
                    *dst = blend_colors(*dst, 0, alpha);
                }
                dx += 1;
            }
            dy += 1;
        }
    }

    if win.no_decoration == 0 && !win.frame_cache.is_null() {
        let ty0 = if win.y - title_h < 0 { -(win.y - title_h) } else { 0 };
        let ty1 = if win.y < layer.height { title_h } else { layer.height - (win.y - title_h) };
        let mut mw = ((win.w as f32) * scale) as i32;
        if mw < 1 && win.w > 0 { mw = 1; }

        let mut dy = ty0;
        while dy < ty1 {
            let py = win.y - title_h + dy;
            let mut scaled_dy = ((dy as f32) * scale) as i32;
            if scaled_dy >= win.frame_cache_h { scaled_dy = win.frame_cache_h - 1; }
            let dst_line = layer.buffer.add((py * layer.width) as usize);
            let src_line = win.frame_cache.add((scaled_dy * win.frame_cache_w) as usize);
            let mask_line = win.window_mask.add((scaled_dy * mw) as usize);
            let mut dx = 0i32;
            while dx < win.w {
                let px = win.x + dx;
                if px >= 0 && px < layer.width {
                    let mut scaled_dx = ((dx as f32) * scale) as i32;
                    if scaled_dx >= win.frame_cache_w { scaled_dx = win.frame_cache_w - 1; }
                    let alpha = if win.is_maximized != 0 { 255 } else { *mask_line.add(scaled_dx as usize) };
                    let dst = dst_line.add(px as usize);
                    *dst = blend_colors(*dst, *src_line.add(scaled_dx as usize), alpha);
                }
                dx += 1;
            }
            dy += 1;
        }
    }

    let cy0 = if win.y < 0 { -win.y } else { 0 };
    let cy1 = if win.y + win.h > layer.height { layer.height - win.y } else { win.h };
    let mut mw = ((win.w as f32) * scale) as i32;
    if mw < 1 && win.w > 0 { mw = 1; }
    let mh = (((win.h + title_h) as f32) * scale) as i32;

    let mut dy = cy0;
    while dy < cy1 {
        let py = win.y + dy;
        let dst_line = layer.buffer.add((py * layer.width) as usize);
        let src_y = compute_content_src_y(dy, win.scroll_y, scale, win.buffer_h);
        if src_y >= 0 {
            let src_content_line = (win.rgba_buffer as *mut u32).add((src_y * win.buffer_w) as usize);
            let mut scaled_mask_y = (((dy + title_h) as f32) * scale) as i32;
            if scaled_mask_y >= mh { scaled_mask_y = mh - 1; }
            let mask_line = win.window_mask.add((scaled_mask_y * mw) as usize);
            let fade_alpha_u8 = (win.fade_alpha * 255.0f32) as u8;

            let mut dx = 0i32;
            while dx < win.w {
                let px = win.x + dx;
                if px >= 0 && px < layer.width {
                    let mut src_x = ((dx as f32) * scale) as i32;
                    if src_x >= win.buffer_w { src_x = win.buffer_w - 1; }
                    let mut color = *src_content_line.add(src_x as usize);
                    let content_a = (color >> 24) as u8;
                    color = blend_colors(win.background_color, color, content_a);
                    if fade_alpha_u8 > 0 {
                        color = blend_colors(color, 0xffff_ffff, fade_alpha_u8);
                    }
                    let mut final_alpha = (color >> 24) as u8;
                    if win.is_maximized == 0 && win.no_decoration == 0 {
                        let mut scaled_dx = ((dx as f32) * scale) as i32;
                        if scaled_dx >= mw { scaled_dx = mw - 1; }
                        let mask_a = *mask_line.add(scaled_dx as usize);
                        final_alpha = (((final_alpha as u32) * (mask_a as u32)) / 255) as u8;
                    }
                    let dst = dst_line.add(px as usize);
                    *dst = blend_colors(*dst, color, final_alpha);
                }
                dx += 1;
            }
        }
        dy += 1;
    }
}
