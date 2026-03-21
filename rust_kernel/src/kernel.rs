use core::ffi::{c_char, c_void, CStr};

use crate::windows::RustWindow;

unsafe extern "C" {
    fn warp_context_set_state(ctx: *mut c_void, key: *const c_char, val: *const c_char);
    fn warp1_context_set_state(ctx: *mut c_void, key: *const c_char, val: *const c_char);
    fn kernel_window_update_caches(win: *mut RustWindow);
}

#[repr(C)]
pub struct RustGlobalVar {
    pub key: [c_char; 64],
    pub val: [c_char; 512],
}

fn copy_c_string<const N: usize>(dst: &mut [c_char; N], src: &[u8]) {
    let mut i = 0usize;
    while i + 1 < N && i < src.len() {
        dst[i] = src[i] as c_char;
        i += 1;
    }
    if N > 0 {
        dst[i.min(N - 1)] = 0;
    }
}

fn c_buf_len<const N: usize>(buf: &[c_char; N]) -> usize {
    let mut len = 0usize;
    while len < N && buf[len] != 0 {
        len += 1;
    }
    len
}

fn c_buf_equals<const N: usize>(buf: &[c_char; N], value: &[u8]) -> bool {
    let len = c_buf_len(buf);
    len == value.len() && value.iter().enumerate().all(|(i, b)| buf[i] as u8 == *b)
}

fn append_with_newline<const N: usize>(dst: &mut [c_char; N], src: &[u8]) {
    let mut len = c_buf_len(dst);
    if len + 1 < N {
        dst[len] = b'\n' as c_char;
        len += 1;
    }
    let mut i = 0usize;
    while len + 1 < N && i < src.len() {
        dst[len] = src[i] as c_char;
        len += 1;
        i += 1;
    }
    if N > 0 {
        dst[len.min(N - 1)] = 0;
    }
}

pub unsafe fn find_global(
    key: *const c_char,
    vars: *const RustGlobalVar,
    count: i32,
) -> *const c_char {
    if key.is_null() || vars.is_null() || count <= 0 {
        return core::ptr::null();
    }

    let key_bytes = CStr::from_ptr(key).to_bytes();
    let vars = core::slice::from_raw_parts(vars, count as usize);
    for var in vars {
        if c_buf_equals(&var.key, key_bytes) {
            return var.val.as_ptr();
        }
    }
    core::ptr::null()
}

pub unsafe fn upsert_global(
    key: *const c_char,
    val: *const c_char,
    vars: *mut RustGlobalVar,
    max_vars: i32,
    count: *mut i32,
    append_mode: i32,
) -> i32 {
    if key.is_null() || val.is_null() || vars.is_null() || count.is_null() || max_vars <= 0 {
        return 0;
    }

    let key_bytes = CStr::from_ptr(key).to_bytes();
    let val_bytes = CStr::from_ptr(val).to_bytes();
    let current_count = if *count < 0 { 0 } else { *count as usize };
    let max_vars = max_vars as usize;
    let vars = core::slice::from_raw_parts_mut(vars, max_vars);

    let mut i = 0usize;
    while i < current_count && i < max_vars {
        if c_buf_equals(&vars[i].key, key_bytes) {
            if append_mode != 0 {
                append_with_newline(&mut vars[i].val, val_bytes);
                return 1;
            }
            if c_buf_equals(&vars[i].val, val_bytes) {
                return 0;
            }
            copy_c_string(&mut vars[i].val, val_bytes);
            return 1;
        }
        i += 1;
    }

    if current_count >= max_vars {
        return 0;
    }

    copy_c_string(&mut vars[current_count].key, key_bytes);
    copy_c_string(&mut vars[current_count].val, val_bytes);
    *count = (current_count + 1) as i32;
    1
}

pub unsafe fn sync_all_window_themes(
    windows: *mut RustWindow,
    count: i32,
    last_is_dark: *mut i32,
    system_dark: i32,
) -> i32 {
    if windows.is_null() || last_is_dark.is_null() || count <= 0 {
        if !last_is_dark.is_null() {
            *last_is_dark = system_dark;
        }
        return 0;
    }
    if *last_is_dark == system_dark {
        return 0;
    }

    let key = c"~~main/dark".as_ptr();
    let true_val = c"true".as_ptr();
    let false_val = c"false".as_ptr();
    let windows = core::slice::from_raw_parts_mut(windows, count as usize);

    for win in windows.iter_mut() {
        let win_dark = if win.force_dark != -1 {
            win.force_dark
        } else {
            system_dark
        };
        let target = if win_dark != 0 { true_val } else { false_val };

        if win.is_warp1 != 0 && !win.warp1_ctx.is_null() {
            warp1_context_set_state(win.warp1_ctx, key, target);
        } else if !win.warp_ctx.is_null() {
            warp_context_set_state(win.warp_ctx, key, target);
        }

        kernel_window_update_caches(win as *mut RustWindow);
        win.is_dirty = 1;
    }

    *last_is_dark = system_dark;
    1
}

pub unsafe fn append_uint(mut p: *mut u8, mut v: u32) -> *mut u8 {
    let mut tmp = [0u8; 10];
    let mut n = 0usize;

    if v == 0 {
        *p = b'0';
        return p.add(1);
    }

    while v > 0 && n < tmp.len() {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }

    while n > 0 {
        n -= 1;
        *p = tmp[n];
        p = p.add(1);
    }

    p
}
