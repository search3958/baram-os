use core::ffi::{c_char, c_void, CStr};

use crate::windows::RustWindow;

unsafe extern "C" {
    fn warp_context_set_state(ctx: *mut c_void, key: *const c_char, val: *const c_char);
    fn warp1_context_set_state(ctx: *mut c_void, key: *const c_char, val: *const c_char);
    fn kernel_window_update_caches(win: *mut RustWindow);
    fn kernel_bootstrap_c(magic: u32, mbi: *mut c_void);
    fn kernel_process_pending_commands() -> i32;
    fn kernel_try_autoboot_to_desktop() -> i32;
    fn kernel_get_current_os_mode() -> i32;
    fn kernel_main_iteration_classic();
    fn kernel_main_iteration_desktop();
    fn kernel_finish_iteration();
    fn get_w1_global(key: *const c_char) -> *const c_char;
    fn set_w1_global(key: *const c_char, val: *const c_char);
    fn set_pending_command(cmd: *const c_char);
    fn kernel_load_wallpaper_from_settings(name: *const c_char);
    fn kernel_mark_os_settings_ready();
    fn kernel_add_window_for_command(title: *const c_char, x: i32, y: i32, w: i32, h: i32, is_warp1: i32);
    fn kernel_close_active_window_for_command();
    fn kernel_find_warp_module(name: *const c_char, out_is_warp1: *mut i32) -> *const c_char;
    fn kernel_list_warp_modules(out_buf: *mut c_char, out_buf_len: i32);
    fn kernel_storage_sync_command();
    fn kernel_storage_ls_command();
    fn sys_restart();
    fn kernel_set_hud_status(status: *const c_char);
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

pub unsafe fn run_kmain(magic: u32, mbi: *mut c_void) -> ! {
    kernel_bootstrap_c(magic, mbi);
    loop {
        kernel_process_pending_commands();
        kernel_try_autoboot_to_desktop();
        if kernel_get_current_os_mode() == 0 {
            kernel_main_iteration_classic();
        } else {
            kernel_main_iteration_desktop();
        }
        kernel_finish_iteration();
    }
}

fn skip_ws(bytes: &[u8], mut idx: usize) -> usize {
    while idx < bytes.len() && matches!(bytes[idx], b' ' | b'\t' | b'\n' | b'\r') {
        idx += 1;
    }
    idx
}

fn find_token(bytes: &[u8], token: &[u8]) -> Option<usize> {
    if token.is_empty() || bytes.len() < token.len() {
        return None;
    }
    let mut i = 0usize;
    while i + token.len() <= bytes.len() {
        if &bytes[i..i + token.len()] == token {
            return Some(i);
        }
        i += 1;
    }
    None
}

unsafe fn set_global_lit(key: &'static [u8], val: &'static [u8]) {
    set_w1_global(key.as_ptr() as *const c_char, val.as_ptr() as *const c_char);
}

pub unsafe fn parse_os_settings(buf: *const c_char) {
    if buf.is_null() {
        return;
    }
    let bytes = CStr::from_ptr(buf).to_bytes();

    if find_token(bytes, b"\"dark\"").is_some() {
        if let Some(pos) = find_token(bytes, b"\"dark\"") {
            let mut i = skip_ws(bytes, pos + 6);
            if i < bytes.len() && bytes[i] == b':' {
                i = skip_ws(bytes, i + 1);
                if i + 4 <= bytes.len() && &bytes[i..i + 4] == b"true" {
                    set_global_lit(b"~~main/dark\0", b"true\0");
                } else {
                    set_global_lit(b"~~main/dark\0", b"false\0");
                }
            }
        }
    }

    if let Some(pos) = find_token(bytes, b"\"pointerCheck\"") {
        let mut i = skip_ws(bytes, pos + 14);
        if i < bytes.len() && bytes[i] == b':' {
            i = skip_ws(bytes, i + 1);
            if i + 4 <= bytes.len() && &bytes[i..i + 4] == b"true" {
                set_global_lit(b"~~dev/pointerCheck\0", b"true\0");
            } else if i + 5 <= bytes.len() && &bytes[i..i + 5] == b"false" {
                set_global_lit(b"~~dev/pointerCheck\0", b"false\0");
            }
        }
    }

    if let Some(pos) = find_token(bytes, b"\"wallpaper\"") {
        let mut i = skip_ws(bytes, pos + 11);
        if i < bytes.len() && bytes[i] == b':' {
            i = skip_ws(bytes, i + 1);
            if i < bytes.len() && bytes[i] == b'"' {
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += 1;
                }
                if i > start {
                    let len = core::cmp::min(i - start, 63);
                    let mut wp = [0 as c_char; 64];
                    copy_c_string(&mut wp, &bytes[start..start + len]);
                    set_w1_global(c"~~main/wallpaper".as_ptr(), wp.as_ptr());
                    kernel_load_wallpaper_from_settings(wp.as_ptr());
                }
            }
        }
    }

    if find_token(bytes, b"\"eventCheck\": true").is_some() {
        set_global_lit(b"~~dev/eventCheck\0", b"true\0");
    } else if find_token(bytes, b"\"eventCheck\": false").is_some() {
        set_global_lit(b"~~dev/eventCheck\0", b"false\0");
    }

    if find_token(bytes, b"\"showHUD\": true").is_some() {
        set_global_lit(b"~~dev/showHUD\0", b"true\0");
    } else if find_token(bytes, b"\"showHUD\": false").is_some() {
        set_global_lit(b"~~dev/showHUD\0", b"false\0");
    }

    if let Some(pos) = find_token(bytes, b"\"firstboot\"") {
        let mut i = pos;
        while i < bytes.len() && bytes[i] != b'[' {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'[' {
            i += 1;
            while i < bytes.len() && bytes[i] != b']' {
                i = skip_ws(bytes, i);
                if i < bytes.len() && bytes[i] == b',' {
                    i += 1;
                    continue;
                }
                if i < bytes.len() && bytes[i] == b'"' {
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'"' {
                        i += 1;
                    }
                    if i > start {
                        let len = core::cmp::min(i - start, 127);
                        let mut cmd = [0 as c_char; 128];
                        copy_c_string(&mut cmd, &bytes[start..start + len]);
                        set_pending_command(cmd.as_ptr());
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }
    }

    let dark_val = get_w1_global(c"~~main/dark".as_ptr());
    let theme = if dark_val.is_null() {
        c"false".as_ptr()
    } else {
        dark_val
    };
    let mut msg = [0 as c_char; 128];
    copy_c_string(&mut msg, b"OSReady Theme:");
    let mut end = c_buf_len(&msg);
    let theme_bytes = CStr::from_ptr(theme).to_bytes();
    let mut i = 0usize;
    while end + 1 < msg.len() && i < theme_bytes.len() {
        msg[end] = theme_bytes[i] as c_char;
        end += 1;
        i += 1;
    }
    msg[end.min(msg.len() - 1)] = 0;
    set_w1_global(c"--warpSystemLog".as_ptr(), msg.as_ptr());
    kernel_mark_os_settings_ready();
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0usize;
    let mut end = bytes.len();
    while start < end && matches!(bytes[start], b' ' | b'\t' | b'\n' | b'\r') {
        start += 1;
    }
    while end > start && matches!(bytes[end - 1], b' ' | b'\t' | b'\n' | b'\r') {
        end -= 1;
    }
    &bytes[start..end]
}

pub unsafe fn handle_terminal_command(cmd: *const c_char) {
    if cmd.is_null() {
        return;
    }
    let raw = CStr::from_ptr(cmd).to_bytes();
    let trimmed = trim_ascii(raw);
    if trimmed.is_empty() {
        return;
    }

    let mut file: Option<&[u8]> = None;
    if trimmed.starts_with(b"warp ") {
        file = Some(trim_ascii(&trimmed[5..]));
    } else if trimmed.starts_with(b"./") {
        file = Some(trim_ascii(&trimmed[2..]));
    } else if find_token(trimmed, b".warp").is_some() || find_token(trimmed, b".warpc").is_some() {
        file = Some(trimmed);
    }

    if let Some(mut file_name) = file {
        if file_name.len() >= 2 && ((file_name[0] == b'"' && *file_name.last().unwrap() == b'"') || (file_name[0] == b'\'' && *file_name.last().unwrap() == b'\'')) {
            file_name = &file_name[1..file_name.len() - 1];
        }
        let copy_len = core::cmp::min(file_name.len(), 127);
        let mut filename = [0 as c_char; 128];
        copy_c_string(&mut filename, &file_name[..copy_len]);

        let mut is_warp1 = 0i32;
        let canonical_name = kernel_find_warp_module(filename.as_ptr(), &mut is_warp1);
        if !canonical_name.is_null() {
            kernel_add_window_for_command(canonical_name, 200, 200, 640, 480, is_warp1);
            return;
        }

        let lower = file_name;
        if lower.eq_ignore_ascii_case(b"terminal.warp") || lower.eq_ignore_ascii_case(b"terminal") {
            kernel_add_window_for_command(c"Terminal".as_ptr(), 200, 200, 600, 400, 1);
            return;
        }
        if lower.eq_ignore_ascii_case(b"menubar.warp") || lower.eq_ignore_ascii_case(b"topbar.warp") || lower.eq_ignore_ascii_case(b"menubar") {
            kernel_add_window_for_command(c"Menubar".as_ptr(), 0, 0, 1280, 32, 1);
            return;
        }

        let mut err = [0 as c_char; 512];
        copy_c_string(&mut err, b"Not found: ");
        let mut end = c_buf_len(&err);
        let mut i = 0usize;
        while end + 1 < err.len() && i < file_name.len() {
            err[end] = file_name[i] as c_char;
            end += 1;
            i += 1;
        }
        err[end.min(err.len() - 1)] = 0;
        set_w1_global(c"--warpSystemLog".as_ptr(), err.as_ptr());
        return;
    }

    if trimmed.eq_ignore_ascii_case(b"ls") || trimmed.eq_ignore_ascii_case(b"list") {
        let mut list_buf = [0 as c_char; 512];
        kernel_list_warp_modules(list_buf.as_mut_ptr(), list_buf.len() as i32);
        set_w1_global(c"--warpSystemLog".as_ptr(), list_buf.as_ptr());
    } else if trimmed == b"reboot" {
        sys_restart();
    } else if trimmed == b"exit" {
        kernel_close_active_window_for_command();
    } else if trimmed == b"help" {
        set_w1_global(c"--warpSystemLog".as_ptr(), c"Commands: <file.warp>, warp <file>, reboot, exit, help, ls".as_ptr());
    } else if trimmed.starts_with(b"dev pointerCheck=") {
        let value = &trimmed[17..];
        if value == b"true" {
            kernel_set_hud_status(c"PtrCheck:ON".as_ptr());
        } else {
            kernel_set_hud_status(c"PtrCheck:OFF".as_ptr());
        }
        let mut buf = [0 as c_char; 6];
        copy_c_string(&mut buf, if value == b"true" { b"true" } else { b"false" });
        set_w1_global(c"~~dev/pointerCheck".as_ptr(), buf.as_ptr());
    } else if trimmed.starts_with(b"dev dark=") {
        let value = &trimmed[9..];
        let mut buf = [0 as c_char; 6];
        copy_c_string(&mut buf, if value == b"true" { b"true" } else { b"false" });
        set_w1_global(c"~~main/dark".as_ptr(), buf.as_ptr());
        kernel_set_hud_status(if value == b"true" { c"Dark:ON".as_ptr() } else { c"Dark:OFF".as_ptr() });
    } else if trimmed == b"storage sync" {
        kernel_storage_sync_command();
    } else if trimmed == b"storage ls" {
        kernel_storage_ls_command();
    } else {
        let mut err = [0 as c_char; 256];
        copy_c_string(&mut err, b"Unknown: ");
        let mut end = c_buf_len(&err);
        let mut i = 0usize;
        while end + 1 < err.len() && i < trimmed.len() {
            err[end] = trimmed[i] as c_char;
            end += 1;
            i += 1;
        }
        err[end.min(err.len() - 1)] = 0;
        set_w1_global(c"--warpSystemLog".as_ptr(), err.as_ptr());
    }
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
