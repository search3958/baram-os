#![no_std]

mod foundation;
mod graphics;
mod kernel;
mod warp;
mod windows;

use core::ffi::{c_char, c_uchar};
use core::panic::PanicInfo;
use warp::RustWindowConfig;

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
