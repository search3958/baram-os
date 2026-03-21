#![no_std]

mod foundation;
mod graphics;
mod kernel;
mod runtime;
mod state;
mod warp;
mod windows;

use core::ffi::{c_char, c_uchar, c_int, c_void};
use core::panic::PanicInfo;
use kernel::RustGlobalVar;
use warp::RustWindowConfig;
use windows::{RustLayer, RustWindow};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// Foundation functions
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

// Graphics functions
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

// Windows functions
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

// Warp functions
#[no_mangle]
pub unsafe extern "C" fn rust_warp_parse_baram_config(
    code: *const c_char,
    screen_width: i32,
    cfg: *mut RustWindowConfig,
) {
    warp::parse_baram_config(code, screen_width, cfg);
}

// Kernel functions
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
pub unsafe extern "C" fn rust_kernel_parse_os_settings(buf: *const c_char) {
    kernel::parse_os_settings(buf);
}

#[no_mangle]
pub unsafe extern "C" fn rust_kernel_handle_terminal_command(cmd: *const c_char) {
    kernel::handle_terminal_command(cmd);
}

// Runtime functions (C interop)
#[no_mangle]
pub unsafe extern "C" fn rt_malloc(size: usize) -> *mut c_void {
    runtime::rt_malloc(size)
}

#[no_mangle]
pub unsafe extern "C" fn rt_free(ptr: *mut c_void) {
    runtime::rt_free(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn rt_realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    runtime::rt_realloc(ptr, size)
}

#[no_mangle]
pub unsafe extern "C" fn rt_memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    runtime::rt_memcpy(dest, src, n)
}

#[no_mangle]
pub unsafe extern "C" fn rt_memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    runtime::rt_memset(s, c, n)
}

#[no_mangle]
pub unsafe extern "C" fn rt_strlen(s: *const c_char) -> usize {
    runtime::rt_strlen(s)
}

#[no_mangle]
pub unsafe extern "C" fn rt_strcmp(a: *const c_char, b: *const c_char) -> c_int {
    runtime::rt_strcmp(a, b)
}

#[no_mangle]
pub unsafe extern "C" fn rt_strncmp(a: *const c_char, b: *const c_char, n: usize) -> c_int {
    runtime::rt_strncmp(a, b, n)
}

#[no_mangle]
pub unsafe extern "C" fn rt_strcasecmp(a: *const c_char, b: *const c_char) -> c_int {
    runtime::rt_strcasecmp(a, b)
}

#[no_mangle]
pub unsafe extern "C" fn rt_strncasecmp(a: *const c_char, b: *const c_char, n: usize) -> c_int {
    runtime::rt_strncasecmp(a, b, n)
}

#[no_mangle]
pub unsafe extern "C" fn rt_memcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int {
    runtime::rt_memcmp(a, b, n)
}

#[no_mangle]
pub unsafe extern "C" fn rt_bcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int {
    runtime::rt_bcmp(a, b, n)
}

#[no_mangle]
pub unsafe extern "C" fn rt_strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    runtime::rt_strncpy(dst, src, n)
}

#[no_mangle]
pub unsafe extern "C" fn rt_strlcat(dst: *mut c_char, src: *const c_char, siz: usize) -> usize {
    runtime::rt_strlcat(dst, src, siz)
}

#[no_mangle]
pub extern "C" fn rust_eh_personality() {
    runtime::rt_rust_eh_personality()
}

// Math functions
#[no_mangle]
pub extern "C" fn rt_sinf(x: f32) -> f32 {
    runtime::rt_sinf(x)
}

#[no_mangle]
pub extern "C" fn rt_cosf(x: f32) -> f32 {
    runtime::rt_cosf(x)
}

#[no_mangle]
pub extern "C" fn rt_tanf(x: f32) -> f32 {
    runtime::rt_tanf(x)
}

#[no_mangle]
pub extern "C" fn rt_atan2f(y: f32, x: f32) -> f32 {
    runtime::rt_atan2f(y, x)
}

#[no_mangle]
pub extern "C" fn rt_acosf(x: f32) -> f32 {
    runtime::rt_acosf(x)
}

#[no_mangle]
pub extern "C" fn rt_sqrtf(x: f32) -> f32 {
    runtime::rt_sqrtf(x)
}

#[no_mangle]
pub extern "C" fn rt_floorf(x: f32) -> f32 {
    runtime::rt_floorf(x)
}

#[no_mangle]
pub extern "C" fn rt_ceilf(x: f32) -> f32 {
    runtime::rt_ceilf(x)
}

#[no_mangle]
pub extern "C" fn rt_roundf(x: f32) -> f32 {
    runtime::rt_roundf(x)
}

#[no_mangle]
pub extern "C" fn rt_fmodf(x: f32, y: f32) -> f32 {
    runtime::rt_fmodf(x, y)
}

#[no_mangle]
pub extern "C" fn rt_fabsf(x: f32) -> f32 {
    runtime::rt_fabsf(x)
}

// State accessors
#[no_mangle]
pub unsafe extern "C" fn rt_get_svg_dirty() -> c_int {
    state::get_svg_dirty()
}

#[no_mangle]
pub unsafe extern "C" fn rt_set_svg_dirty(dirty: c_int) {
    state::set_svg_dirty(dirty);
}

#[no_mangle]
pub unsafe extern "C" fn rt_get_scroll_y() -> f32 {
    state::get_scroll_y()
}

#[no_mangle]
pub unsafe extern "C" fn rt_set_scroll_y(y: f32) {
    state::set_scroll_y(y);
}

#[no_mangle]
pub unsafe extern "C" fn rt_get_target_scroll_y() -> f32 {
    state::get_target_scroll_y()
}

#[no_mangle]
pub unsafe extern "C" fn rt_set_target_scroll_y(y: f32) {
    state::set_target_scroll_y(y);
}

#[no_mangle]
pub unsafe extern "C" fn rt_get_timer_ticks() -> u32 {
    state::get_timer_ticks()
}

#[no_mangle]
pub unsafe extern "C" fn rt_set_timer_ticks(ticks: u32) {
    state::set_timer_ticks(ticks);
}

#[no_mangle]
pub unsafe extern "C" fn rt_get_cpu_idle() -> c_int {
    state::get_cpu_idle()
}

#[no_mangle]
pub unsafe extern "C" fn rt_set_cpu_idle(idle: c_int) {
    state::set_cpu_idle(idle);
}

#[no_mangle]
pub unsafe extern "C" fn rt_get_idle_ticks() -> u32 {
    state::get_idle_ticks()
}

#[no_mangle]
pub unsafe extern "C" fn rt_set_idle_ticks(ticks: u32) {
    state::set_idle_ticks(ticks);
}

#[no_mangle]
pub unsafe extern "C" fn rt_get_dev_pointer_check() -> c_int {
    state::get_dev_pointer_check()
}

#[no_mangle]
pub unsafe extern "C" fn rt_get_dev_event_check() -> c_int {
    state::get_dev_event_check()
}

#[no_mangle]
pub unsafe extern "C" fn rt_get_dev_show_hud() -> c_int {
    state::get_dev_show_hud()
}

// Main entry point
#[no_mangle]
pub unsafe extern "C" fn kmain(magic: u32, mbi: *mut core::ffi::c_void) -> ! {
    kernel::run_kmain(magic, mbi)
}
