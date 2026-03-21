#![no_std]

mod foundation;
mod graphics;
mod kernel;
mod runtime;
mod state;
mod warp;
mod windows;

use core::ffi::{c_char, c_uchar};
use core::panic::PanicInfo;
use kernel::RustGlobalVar;
use warp::RustWindowConfig;
use windows::{RustLayer, RustWindow};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub unsafe extern "C" fn rust_foundation_octal_to_int(s: *const c_char, len: i32) -> u32 {
    foundation::octal_to_int(s as *const u8, len)
}

#[no_mangle]
pub unsafe extern "C" fn rust_foundation_tar_find_file(
    tar_data: *const c_char,
    tar_size: usize,
    filename: *const c_char,
    out_size: *mut u32,
) -> *const c_char {
    foundation::tar_find_file(tar_data as *const u8, tar_size, filename, out_size) as *const c_char
}

#[no_mangle]
pub extern "C" fn rust_graphics_lerp_color(c1: u32, c2: u32, t: f32) -> u32 {
    graphics::lerp_color(c1, c2, t)
}

#[no_mangle]
pub unsafe extern "C" fn rust_graphics_apply_conic_gradient(
    data: *mut c_uchar,
    w: i32,
    h: i32,
    rx: i32,
    ry: i32,
    rw: i32,
    rh: i32,
    c1: u32,
    c2: u32,
) {
    graphics::apply_conic_gradient(data, w, h, rx, ry, rw, rh, c1, c2);
}

#[no_mangle]
pub extern "C" fn rust_graphics_blend_colors(bg: u32, fg: u32, alpha: u8) -> u32 {
    graphics::blend_colors(bg, fg, alpha)
}

#[no_mangle]
pub unsafe extern "C" fn rust_graphics_box_blur_alpha(
    data: *mut c_uchar,
    w: i32,
    h: i32,
    radius: i32,
) {
    graphics::box_blur_alpha(data, w, h, radius);
}

#[no_mangle]
pub extern "C" fn rust_windows_should_bake_inactive(
    is_active: i32,
    buffer_h: i32,
    window_h: i32,
    scroll_y: f32,
    target_scroll_y: f32,
) -> i32 {
    windows::should_bake_inactive(is_active, buffer_h, window_h, scroll_y, target_scroll_y)
}

#[no_mangle]
pub extern "C" fn rust_windows_compute_content_src_y(
    dy: i32,
    scroll_y: f32,
    scale: f32,
    buffer_h: i32,
) -> i32 {
    windows::compute_content_src_y(dy, scroll_y, scale, buffer_h)
}

#[no_mangle]
pub unsafe extern "C" fn rust_windows_clear_cached_buffers(win: *mut RustWindow) {
    windows::clear_cached_buffers(win);
}

#[no_mangle]
pub unsafe extern "C" fn rust_windows_bake_window(win: *mut RustWindow) {
    windows::bake_window(win);
}

#[no_mangle]
pub unsafe extern "C" fn rust_windows_draw_single_window(layer: *mut RustLayer, win: *mut RustWindow) {
    windows::draw_single_window(layer, win);
}

#[no_mangle]
pub unsafe extern "C" fn rust_windows_redraw_window(win: *mut RustWindow, is_active: i32) {
    windows::redraw_window(win, is_active);
}

#[no_mangle]
pub unsafe extern "C" fn rust_windows_update_window_caches(win: *mut RustWindow, is_active: i32) {
    windows::update_window_caches(win, is_active);
}

#[no_mangle]
pub unsafe extern "C" fn rust_warp_parse_baram_config(
    code: *const c_char,
    screen_width: i32,
    cfg: *mut RustWindowConfig,
) {
    warp::parse_baram_config(code, screen_width, cfg);
}

#[no_mangle]
pub unsafe extern "C" fn rust_kernel_append_uint(p: *mut c_char, v: u32) -> *mut c_char {
    kernel::append_uint(p as *mut u8, v) as *mut c_char
}

#[no_mangle]
pub unsafe extern "C" fn rust_kernel_find_global(
    key: *const c_char,
    vars: *const RustGlobalVar,
    count: i32,
) -> *const c_char {
    kernel::find_global(key, vars, count)
}

#[no_mangle]
pub unsafe extern "C" fn rust_kernel_upsert_global(
    key: *const c_char,
    val: *const c_char,
    vars: *mut RustGlobalVar,
    max_vars: i32,
    count: *mut i32,
    append_mode: i32,
) -> i32 {
    kernel::upsert_global(key, val, vars, max_vars, count, append_mode)
}

#[no_mangle]
pub unsafe extern "C" fn rust_kernel_sync_all_window_themes(
    windows: *mut RustWindow,
    count: i32,
    last_is_dark: *mut i32,
    system_dark: i32,
) -> i32 {
    kernel::sync_all_window_themes(windows, count, last_is_dark, system_dark)
}

#[no_mangle]
pub unsafe extern "C" fn kmain(magic: u32, mbi: *mut core::ffi::c_void) -> ! {
    kernel::run_kmain(magic, mbi)
}

#[no_mangle]
pub unsafe extern "C" fn rust_kernel_parse_os_settings(buf: *const c_char) {
    kernel::parse_os_settings(buf);
}

#[no_mangle]
pub unsafe extern "C" fn rust_kernel_handle_terminal_command(cmd: *const c_char) {
    kernel::handle_terminal_command(cmd);
}
