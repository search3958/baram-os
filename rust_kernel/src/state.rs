use core::ffi::{c_char, c_int};

#[repr(C)]
#[derive(Copy, Clone)]
struct GlobalVar {
    key: [u8; 64],
    val: [u8; 512],
}

const MAX_GLOBAL_VARS: usize = 128;
const MAX_PENDING_COMMANDS: usize = 8;
const PENDING_COMMAND_LEN: usize = 256;
const HUD_STATUS_LEN: usize = 64;

static mut G_DEV_POINTER_CHECK: c_int = 1;
static mut G_DEV_EVENT_CHECK: c_int = 0;
static mut G_DEV_SHOW_HUD: c_int = 1;
static mut G_GLOBAL_VARS: [GlobalVar; MAX_GLOBAL_VARS] = [GlobalVar {
    key: [0; 64],
    val: [0; 512],
}; MAX_GLOBAL_VARS];
static mut G_GLOBAL_VAR_COUNT: usize = 0;
static mut G_PENDING_COMMANDS: [[u8; PENDING_COMMAND_LEN]; MAX_PENDING_COMMANDS] =
    [[0; PENDING_COMMAND_LEN]; MAX_PENDING_COMMANDS];
static mut G_PENDING_COMMAND_COUNT: usize = 0;
static mut G_HUD_STATUS: [u8; HUD_STATUS_LEN] = [0; HUD_STATUS_LEN];

fn str_len(ptr: *const c_char) -> usize {
    if ptr.is_null() {
        return 0;
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
    }
    len
}

fn cstr_eq(ptr: *const c_char, lit: &[u8]) -> bool {
    let len = str_len(ptr);
    if len != lit.len() {
        return false;
    }
    let mut i = 0usize;
    while i < len {
        unsafe {
            if *ptr.add(i) as u8 != lit[i] {
                return false;
            }
        }
        i += 1;
    }
    true
}

fn copy_cstr_to_buf(dst: &mut [u8], src: *const c_char) {
    if dst.is_empty() {
        return;
    }
    let mut i = 0usize;
    unsafe {
        while i + 1 < dst.len() && !src.is_null() && *src.add(i) != 0 {
            dst[i] = *src.add(i) as u8;
            i += 1;
        }
    }
    dst[i.min(dst.len() - 1)] = 0;
}

fn copy_bytes_to_buf(dst: &mut [u8], src: &[u8]) {
    if dst.is_empty() {
        return;
    }
    let mut i = 0usize;
    while i + 1 < dst.len() && i < src.len() {
        dst[i] = src[i];
        i += 1;
    }
    dst[i.min(dst.len() - 1)] = 0;
}

fn append_log_line(dst: &mut [u8], src: *const c_char) {
    let mut len = 0usize;
    while len < dst.len() && dst[len] != 0 {
        len += 1;
    }
    if len + 1 < dst.len() && len != 0 {
        dst[len] = b'\n';
        len += 1;
    }
    let mut i = 0usize;
    unsafe {
        while len + 1 < dst.len() && !src.is_null() && *src.add(i) != 0 {
            dst[len] = *src.add(i) as u8;
            len += 1;
            i += 1;
        }
    }
    if !dst.is_empty() {
        dst[len.min(dst.len() - 1)] = 0;
    }
}

fn find_global_index(key: *const c_char) -> Option<usize> {
    let key_len = str_len(key);
    let mut i = 0usize;
    unsafe {
        while i < G_GLOBAL_VAR_COUNT {
            let mut j = 0usize;
            let mut matches = true;
            while j < key_len {
                if G_GLOBAL_VARS[i].key[j] != *key.add(j) as u8 {
                    matches = false;
                    break;
                }
                j += 1;
            }
            if matches && G_GLOBAL_VARS[i].key[key_len] == 0 {
                return Some(i);
            }
            i += 1;
        }
    }
    None
}

fn update_dev_flags(key: *const c_char, val: *const c_char) {
    let enabled = cstr_eq(val, b"true");
    unsafe {
        if cstr_eq(key, b"~~dev/pointerCheck") {
            G_DEV_POINTER_CHECK = if enabled { 1 } else { 0 };
        } else if cstr_eq(key, b"~~dev/eventCheck") {
            G_DEV_EVENT_CHECK = if enabled { 1 } else { 0 };
        } else if cstr_eq(key, b"~~dev/showHUD") {
            G_DEV_SHOW_HUD = if enabled { 1 } else { 0 };
        }
    }
}

pub unsafe fn get_global_ptr(key: *const c_char) -> *const c_char {
    if key.is_null() {
        return c"".as_ptr();
    }
    if cstr_eq(key, b"~~dev/pointerCheck") {
        return if G_DEV_POINTER_CHECK != 0 {
            c"true".as_ptr()
        } else {
            c"false".as_ptr()
        };
    }
    if cstr_eq(key, b"~~dev/eventCheck") {
        return if G_DEV_EVENT_CHECK != 0 {
            c"true".as_ptr()
        } else {
            c"false".as_ptr()
        };
    }
    if cstr_eq(key, b"~~dev/showHUD") {
        return if G_DEV_SHOW_HUD != 0 {
            c"true".as_ptr()
        } else {
            c"false".as_ptr()
        };
    }
    if let Some(index) = find_global_index(key) {
        return G_GLOBAL_VARS[index].val.as_ptr() as *const c_char;
    }
    c"".as_ptr()
}

pub unsafe fn set_global_value(key: *const c_char, val: *const c_char) {
    if key.is_null() || val.is_null() {
        return;
    }
    update_dev_flags(key, val);

    let effective_key = if cstr_eq(key, b"~~json/main/dark") {
        c"~~main/dark".as_ptr()
    } else {
        key
    };
    let is_log = cstr_eq(effective_key, b"--warpSystemLog");

    if let Some(index) = find_global_index(effective_key) {
        if is_log {
            append_log_line(&mut G_GLOBAL_VARS[index].val, val);
        } else {
            copy_cstr_to_buf(&mut G_GLOBAL_VARS[index].val, val);
        }
        return;
    }

    if G_GLOBAL_VAR_COUNT >= MAX_GLOBAL_VARS {
        return;
    }
    let index = G_GLOBAL_VAR_COUNT;
    copy_cstr_to_buf(&mut G_GLOBAL_VARS[index].key, effective_key);
    if is_log {
        G_GLOBAL_VARS[index].val[0] = 0;
        append_log_line(&mut G_GLOBAL_VARS[index].val, val);
    } else {
        copy_cstr_to_buf(&mut G_GLOBAL_VARS[index].val, val);
    }
    G_GLOBAL_VAR_COUNT += 1;
}

pub unsafe fn enqueue_pending_command(cmd: *const c_char) {
    if cmd.is_null() || G_PENDING_COMMAND_COUNT >= MAX_PENDING_COMMANDS {
        return;
    }
    copy_cstr_to_buf(&mut G_PENDING_COMMANDS[G_PENDING_COMMAND_COUNT], cmd);
    G_PENDING_COMMAND_COUNT += 1;
}

pub unsafe fn set_hud_status(status: *const c_char) {
    copy_cstr_to_buf(&mut G_HUD_STATUS, status);
}

pub unsafe fn hud_status_ptr() -> *const c_char {
    G_HUD_STATUS.as_ptr() as *const c_char
}

pub unsafe fn set_global_literal(key: &[u8], val: &[u8]) {
    let mut key_buf = [0u8; 64];
    let mut val_buf = [0u8; 512];
    copy_bytes_to_buf(&mut key_buf, key);
    copy_bytes_to_buf(&mut val_buf, val);
    set_global_value(key_buf.as_ptr() as *const c_char, val_buf.as_ptr() as *const c_char);
}
