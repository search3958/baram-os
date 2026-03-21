use core::ffi::{c_char, c_void};

use crate::graphics::blend_colors;
use crate::runtime::{rt_free, rt_malloc};
use crate::state::{get_global_ptr, set_hud_status};

unsafe extern "C" {
    fn sqrtf(v: f32) -> f32;
    fn fabsf(v: f32) -> f32;
    fn layer_draw_string(layer: *mut RustLayer, x: i32, y: i32, s: *const c_char, color: u32, bg_color: u32);
    fn warp_context_update(ctx: *mut c_void, width: i32, height: i32);
    fn warp_context_get_svg(ctx: *mut c_void) -> *const c_char;
    fn warp_context_draw_texts(ctx: *mut c_void, layer: *mut RustLayer, off_x: i32, off_y: i32, scale: f32);
    fn warp_context_is_dirty(ctx: *mut c_void) -> i32;
    fn warp_context_clear_dirty(ctx: *mut c_void);
    fn warp_context_get_header_info(ctx: *mut c_void, out_text: *mut c_char, max_len: i32, out_action_count: *mut i32) -> i32;
    fn warp_context_get_header_action_info(ctx: *mut c_void, action_index: i32, out_text: *mut c_char, max_len: i32);
    fn warp1_context_update(ctx: *mut c_void, width: i32, height: i32);
    fn warp1_context_get_svg(ctx: *mut c_void) -> *const c_char;
    fn warp1_context_draw_texts(ctx: *mut c_void, layer: *mut RustLayer, off_x: i32, off_y: i32, scale: f32);
    fn warp1_context_is_dirty(ctx: *mut c_void) -> i32;
    fn warp1_context_clear_dirty(ctx: *mut c_void);
    fn warp1_context_get_header_info(ctx: *mut c_void, out_text: *mut c_char, max_len: i32, out_action_count: *mut i32) -> i32;
    fn warp1_context_get_header_action_info(ctx: *mut c_void, action_index: i32, out_text: *mut c_char, max_len: i32);
    fn kernel_svg_parse(svg: *const c_char) -> *mut c_void;
    fn kernel_svg_delete(image: *mut c_void);
    fn kernel_svg_height(image: *mut c_void) -> i32;
    fn kernel_svg_rasterize(image: *mut c_void, dst: *mut u8, w: i32, h: i32, scale: f32);
}

unsafe fn get_w1_global(key: *const c_char) -> *const c_char {
    get_global_ptr(key)
}

unsafe fn kernel_set_hud_status(status: *const c_char) {
    set_hud_status(status);
}

unsafe fn layer_draw_ttf(layer: *mut RustLayer, px: i32, py: i32, s: *const c_char, _font_size: f32, color: u32) {
    layer_draw_string(layer, px, py, s, color, 0);
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
        rt_free(win.shadow_cache as *mut c_void);
        win.shadow_cache = core::ptr::null_mut();
    }
    if !win.frame_cache.is_null() {
        rt_free(win.frame_cache as *mut c_void);
        win.frame_cache = core::ptr::null_mut();
    }
    if !win.window_mask.is_null() {
        rt_free(win.window_mask as *mut c_void);
        win.window_mask = core::ptr::null_mut();
    }
    if !win.rgba_buffer.is_null() {
        rt_free(win.rgba_buffer as *mut c_void);
        win.rgba_buffer = core::ptr::null_mut();
    }
    if !win.raster_cache.is_null() {
        rt_free(win.raster_cache as *mut c_void);
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
        rt_free(win.unified_buffer as *mut c_void);
    }
    win.unified_buffer = rt_malloc((sw as usize) * (sh as usize) * 4usize) as *mut u32;
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

fn c_str_eq(ptr: *const c_char, s: &[u8]) -> bool {
    if ptr.is_null() {
        return false;
    }
    unsafe {
        let bytes = core::ffi::CStr::from_ptr(ptr).to_bytes();
        bytes == s
    }
}

fn c_ptr_eq(a: *const c_char, b: *const c_char) -> bool {
    if a.is_null() || b.is_null() {
        return false;
    }
    unsafe { core::ffi::CStr::from_ptr(a).to_bytes() == core::ffi::CStr::from_ptr(b).to_bytes() }
}

unsafe fn c_buf_len(ptr: *const c_char) -> i32 {
    if ptr.is_null() {
        return 0;
    }
    core::ffi::CStr::from_ptr(ptr).to_bytes().len() as i32
}

pub unsafe fn update_window_caches(win: *mut RustWindow, is_active: i32) {
    if win.is_null() {
        return;
    }
    let win = &mut *win;
    let mut scale = win.render_scale;
    if scale <= 0.0 {
        scale = 1.0;
    }

    let title_h = if win.no_decoration != 0 { 0 } else { 40 };
    let shadow_size = if win.no_decoration != 0 { 0 } else { 48 };
    let win_r = 30.0f32;

    let full_sw = win.w + shadow_size * 2;
    let full_sh = win.h + title_h + shadow_size * 2;
    let mut sw = ((full_sw as f32) * scale) as i32;
    let mut sh = ((full_sh as f32) * scale) as i32;
    if sw < 1 { sw = 1; }
    if sh < 1 { sh = 1; }

    if win.shadow_cache.is_null() || win.shadow_cache_w != sw || win.shadow_cache_h != sh {
        if !win.shadow_cache.is_null() {
            rt_free(win.shadow_cache as *mut c_void);
        }
        win.shadow_cache = rt_malloc((sw as usize) * (sh as usize)) as *mut u8;
        win.shadow_cache_w = sw;
        win.shadow_cache_h = sh;

        let win_w_f = win.w as f32;
        let win_h_f = (win.h + title_h) as f32;
        let half_sw = full_sw as f32 / 2.0;
        let half_sh = full_sh as f32 / 2.0;

        let mut y = 0i32;
        while y < sh {
            let fy = y as f32 / scale;
            let mut x = 0i32;
            while x < sw {
                let fx = x as f32 / scale;
                let qx = fabsf(fx - half_sw) - (win_w_f / 2.0 - win_r);
                let qy = fabsf(fy - half_sh) - (win_h_f / 2.0 - win_r);
                let mx = if qx > 0.0 { qx } else { 0.0 };
                let my = if qy > 0.0 { qy } else { 0.0 };
                let mut inner = if qx > qy { qx } else { qy };
                if inner > 0.0 { inner = 0.0; }
                let dist = sqrtf(mx * mx + my * my) + inner - win_r;

                let mut alpha = 0u8;
                if dist <= 0.0 {
                    alpha = 64;
                } else if dist < shadow_size as f32 {
                    let d_ratio = dist / shadow_size as f32;
                    alpha = (64.0 * (1.0 - d_ratio) * (1.0 - d_ratio)) as u8;
                }
                *win.shadow_cache.add((y * sw + x) as usize) = alpha;
                x += 1;
            }
            y += 1;
        }
    }

    let full_fw = win.w;
    let full_fh = title_h;
    let mut fw = ((full_fw as f32) * scale) as i32;
    let mut fh = ((full_fh as f32) * scale) as i32;
    if fw < 1 && full_fw > 0 { fw = 1; }
    if fh < 1 && full_fh > 0 { fh = 1; }

    if win.frame_cache.is_null() || win.frame_cache_w != fw || win.frame_cache_h != fh {
        if !win.frame_cache.is_null() {
            rt_free(win.frame_cache as *mut c_void);
        }
        win.frame_cache = rt_malloc((fw as usize) * (fh as usize) * 4usize) as *mut u32;
        win.frame_cache_w = fw;
        win.frame_cache_h = fh;
    }

    let dark_key = b"~~main/dark\0";
    let is_dark = c_str_eq(get_w1_global(dark_key.as_ptr() as *const c_char), b"true");
    let theme = if is_dark {
        if is_active != 0 { 0xff1e1e1e } else { 0xff333333 }
    } else {
        if is_active != 0 { 0xfff5f5f5 } else { 0xffe0e0e0 }
    };
    let mut i = 0i32;
    while i < fw * fh {
        *win.frame_cache.add(i as usize) = theme;
        i += 1;
    }

    if fh > 0 {
        let mut frame_l = RustLayer {
            buffer: win.frame_cache,
            x: 0,
            y: 0,
            width: fw,
            height: fh,
            transparent: 0,
            active: 0,
            dynamic: 0,
        };

        let mut header_text = [0 as c_char; 128];
        let mut action_count = 0i32;
        let has_header = if win.is_warp1 != 0 {
            if !win.warp1_ctx.is_null() {
                warp1_context_get_header_info(win.warp1_ctx, header_text.as_mut_ptr(), 128, &mut action_count)
            } else { 0 }
        } else if !win.warp_ctx.is_null() {
            warp_context_get_header_info(win.warp_ctx, header_text.as_mut_ptr(), 128, &mut action_count)
        } else { 0 };

        if has_header != 0 {
            layer_draw_ttf(&mut frame_l, (70.0 * scale) as i32, (12.0 * scale) as i32, header_text.as_ptr(), 16.0 * scale, if is_dark { 0xffeeeeee } else { 0xff333333 });
            let mut ax = win.w - 12;
            let mut j = 0i32;
            while j < action_count {
                let mut act_text = [0 as c_char; 64];
                if win.is_warp1 != 0 {
                    warp1_context_get_header_action_info(win.warp1_ctx, j, act_text.as_mut_ptr(), 64);
                } else {
                    warp_context_get_header_action_info(win.warp_ctx, j, act_text.as_mut_ptr(), 64);
                }
                let text_w = c_buf_len(act_text.as_ptr()) * 9;
                let btn_w = text_w + 24;
                let btn_h = 26;
                ax -= btn_w;
                let bx = (ax as f32 * scale) as i32;
                let by = (7.0 * scale) as i32;
                let bw = (btn_w as f32 * scale) as i32;
                let bh = (btn_h as f32 * scale) as i32;
                let mut dy = 0i32;
                while dy < bh {
                    let mut dx = 0i32;
                    while dx < bw {
                        *frame_l.buffer.add(((by + dy) * fw + (bx + dx)) as usize) = if is_dark { 0xff444444 } else { 0xffffffff };
                        dx += 1;
                    }
                    dy += 1;
                }
                layer_draw_ttf(&mut frame_l, bx + (12.0 * scale) as i32, by + (7.0 * scale) as i32, act_text.as_ptr(), 14.0 * scale, if is_dark { 0xffeeeeee } else { 0xff000000 });
                ax -= 10;
                j += 1;
            }
        } else {
            layer_draw_ttf(&mut frame_l, (70.0 * scale) as i32, (12.0 * scale) as i32, win.title.as_ptr(), 16.0 * scale, if is_dark { 0xffeeeeee } else { 0xff333333 });
        }

        let colors = [0xffff2836u32, 0xff2ecc46u32];
        let centers_x = [20i32, 44i32];
        let btn_r = 7.0f32;
        let btn_y = 20i32;
        let mut k = 0usize;
        while k < 2 {
            let cx = centers_x[k] as f32 * scale;
            let cy = btn_y as f32 * scale;
            let cr = btn_r * scale;
            let i_r = cr as i32 + 2;
            let mut dy = -i_r;
            while dy <= i_r {
                let mut dx = -i_r;
                while dx <= i_r {
                    let px = cx as i32 + dx;
                    let py = cy as i32 + dy;
                    if px >= 0 && px < fw && py >= 0 && py < fh {
                        let dist = sqrtf((dx * dx + dy * dy) as f32);
                        let mut alpha_f = 0.5 - (dist - cr);
                        if alpha_f < 0.0 { alpha_f = 0.0; }
                        else if alpha_f > 1.0 { alpha_f = 1.0; }
                        if alpha_f > 0.0 {
                            let dst = frame_l.buffer.add((py * fw + px) as usize);
                            *dst = blend_colors(*dst, colors[k], (alpha_f * 255.0) as u8);
                        }
                    }
                    dx += 1;
                }
                dy += 1;
            }
            k += 1;
        }
    }

    let full_mw = win.w;
    let full_mh = win.h + title_h;
    let mut mw = ((full_mw as f32) * scale) as i32;
    let mut mh = ((full_mh as f32) * scale) as i32;
    if mw < 1 && full_mw > 0 { mw = 1; }
    if mh < 1 && full_mh > 0 { mh = 1; }

    if win.window_mask.is_null() || win.buffer_w != mw || win.shadow_cache_h != sh {
        if !win.window_mask.is_null() {
            rt_free(win.window_mask as *mut c_void);
        }
        win.window_mask = rt_malloc((mw as usize) * (mh as usize)) as *mut u8;
        let rw = full_mw as f32;
        let rh = full_mh as f32;
        let r = 32.0f32;
        let mut y = 0i32;
        while y < mh {
            let fy = y as f32 / scale + 0.5;
            let mut x = 0i32;
            while x < mw {
                let fx = x as f32 / scale + 0.5;
                let dx = fabsf(fx - rw / 2.0) - (rw / 2.0 - r);
                let dy = fabsf(fy - rh / 2.0) - (rh / 2.0 - r);
                let dist = if dx > 0.0 && dy > 0.0 {
                    sqrtf(sqrtf(dx * dx * dx * dx + dy * dy * dy * dy)) - r
                } else {
                    (if dx > dy { dx } else { dy }) - r
                };
                let mut alpha_f = 0.5 - dist;
                if alpha_f < 0.0 { alpha_f = 0.0; }
                else if alpha_f > 1.0 { alpha_f = 1.0; }
                *win.window_mask.add((y * mw + x) as usize) = (alpha_f * 255.0) as u8;
                x += 1;
            }
            y += 1;
        }
    }
}

pub unsafe fn redraw_window(win: *mut RustWindow, is_active: i32) {
    if win.is_null() {
        return;
    }
    let win = &mut *win;
    if win.warp_ctx.is_null() && win.warp1_ctx.is_null() {
        return;
    }

    let target_scale = 1.0f32;
    if win.render_scale != target_scale {
        win.is_dirty = 1;
        win.render_scale = target_scale;
        update_window_caches(win, is_active);
    }

    if is_active != 0 && !win.unified_buffer.is_null() {
        rt_free(win.unified_buffer as *mut c_void);
        win.unified_buffer = core::ptr::null_mut();
        win.unified_w = 0;
        win.unified_h = 0;
    }

    let needs_update = if win.is_warp1 != 0 {
        warp1_context_is_dirty(win.warp1_ctx) != 0 || win.rgba_buffer.is_null()
    } else {
        warp_context_is_dirty(win.warp_ctx) != 0 || win.rgba_buffer.is_null()
    };

    if needs_update {
        kernel_set_hud_status(b"EngineUpdate\0".as_ptr() as *const c_char);
        if win.is_warp1 != 0 {
            warp1_context_update(win.warp1_ctx, win.w, win.h);
            warp1_context_clear_dirty(win.warp1_ctx);
        } else {
            warp_context_update(win.warp_ctx, win.w, win.h);
            warp_context_clear_dirty(win.warp_ctx);
        }
    } else {
        kernel_set_hud_status(b"Cached\0".as_ptr() as *const c_char);
    }

    kernel_set_hud_status(b"SVGGen\0".as_ptr() as *const c_char);
    let svg = if win.is_warp1 != 0 {
        warp1_context_get_svg(win.warp1_ctx)
    } else {
        warp_context_get_svg(win.warp_ctx)
    };
    if svg.is_null() {
        return;
    }

    let svg_changed = win.last_svg_str.is_null() || !c_ptr_eq(win.last_svg_str, svg);
    if svg_changed {
        kernel_set_hud_status(b"NSVGParse\0".as_ptr() as *const c_char);
        let img = kernel_svg_parse(svg);
        if img.is_null() {
            kernel_set_hud_status(b"ParseErr\0".as_ptr() as *const c_char);
            return;
        }

        if !win.svg_image_cache.is_null() {
            kernel_svg_delete(win.svg_image_cache);
        }
        win.svg_image_cache = img;

        if !win.last_svg_str.is_null() {
            rt_free(win.last_svg_str as *mut c_void);
        }
        let svg_bytes = core::ffi::CStr::from_ptr(svg).to_bytes_with_nul();
        win.last_svg_str = rt_malloc(svg_bytes.len()) as *mut c_char;
        if !win.last_svg_str.is_null() {
            core::ptr::copy_nonoverlapping(
                svg_bytes.as_ptr(),
                win.last_svg_str as *mut u8,
                svg_bytes.len(),
            );
        }
    }

    if win.svg_image_cache.is_null() {
        kernel_set_hud_status(b"ParseMiss\0".as_ptr() as *const c_char);
        return;
    }

    let mut content_h = kernel_svg_height(win.svg_image_cache);
    if content_h < win.h {
        content_h = win.h;
    }

    let mut scaled_w = (win.w as f32 * target_scale) as i32;
    let mut scaled_h = (content_h as f32 * target_scale) as i32;
    if scaled_w < 1 {
        scaled_w = 1;
    }
    if scaled_h < 1 {
        scaled_h = 1;
    }

    let raster_size_changed =
        win.raster_cache.is_null() || win.raster_cache_w != scaled_w || win.raster_cache_h != scaled_h;
    if raster_size_changed {
        if !win.raster_cache.is_null() {
            rt_free(win.raster_cache as *mut c_void);
        }
        win.raster_cache = rt_malloc((scaled_w as usize) * (scaled_h as usize) * 4usize) as *mut u32;
        win.raster_cache_w = scaled_w;
        win.raster_cache_h = scaled_h;
    }

    if !win.raster_cache.is_null() && (svg_changed || raster_size_changed) {
        kernel_set_hud_status(b"ClearCache\0".as_ptr() as *const c_char);
        let mut i = 0i32;
        while i < scaled_w * scaled_h {
            *win.raster_cache.add(i as usize) = 0xffff_ffff;
            i += 1;
        }

        kernel_set_hud_status(b"NSVGRast\0".as_ptr() as *const c_char);
        kernel_svg_rasterize(
            win.svg_image_cache,
            win.raster_cache as *mut u8,
            scaled_w,
            scaled_h,
            target_scale,
        );

        kernel_set_hud_status(b"RBSwap\0".as_ptr() as *const c_char);
        let mut p = win.raster_cache as *mut u8;
        let mut n = 0i32;
        while n < scaled_w * scaled_h {
            let r = *p;
            let b = *p.add(2);
            *p = b;
            *p.add(2) = r;
            p = p.add(4);
            n += 1;
        }
    }

    if !win.raster_cache.is_null() {
        if win.rgba_buffer.is_null() || win.buffer_w != win.raster_cache_w || win.buffer_h != win.raster_cache_h {
            if !win.rgba_buffer.is_null() {
                rt_free(win.rgba_buffer as *mut c_void);
            }
            win.rgba_buffer =
                rt_malloc((win.raster_cache_w as usize) * (win.raster_cache_h as usize) * 4usize) as *mut u8;
            win.buffer_w = win.raster_cache_w;
            win.buffer_h = win.raster_cache_h;
            win.is_dirty = 1;
        }

        if !win.rgba_buffer.is_null() && (svg_changed || win.is_dirty != 0) {
            core::ptr::copy_nonoverlapping(
                win.raster_cache as *const u8,
                win.rgba_buffer,
                (win.buffer_w as usize) * (win.buffer_h as usize) * 4usize,
            );

            kernel_set_hud_status(b"TxtDraw\0".as_ptr() as *const c_char);
            let mut temp_layer = RustLayer {
                buffer: win.rgba_buffer as *mut u32,
                x: 0,
                y: 0,
                width: win.buffer_w,
                height: win.buffer_h,
                transparent: 0,
                active: 0,
                dynamic: 0,
            };
            if win.is_warp1 != 0 {
                warp1_context_draw_texts(win.warp1_ctx, &mut temp_layer, 0, 0, win.render_scale);
            } else {
                warp_context_draw_texts(win.warp_ctx, &mut temp_layer, 0, 0, win.render_scale);
            }
        }
    }

    if should_bake_inactive(is_active, win.buffer_h, win.h, win.scroll_y, win.target_scroll_y) != 0 {
        bake_window(win);
    }

    win.is_dirty = 0;
    win.is_calculating = 0;
    kernel_set_hud_status(b"Idle\0".as_ptr() as *const c_char);
}
