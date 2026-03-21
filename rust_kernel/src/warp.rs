use core::ffi::{c_char, CStr};

#[repr(C)]
pub struct RustWindowConfig {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub no_decoration: i32,
    pub is_movable: i32,
    pub is_resizing_enabled: i32,
    pub is_always_full_res: i32,
    pub is_sticky: i32,
    pub background_color: u32,
    pub force_dark: i32,
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let mut i = 0usize;
    while i + needle.len() <= haystack.len() {
        if &haystack[i..i + needle.len()] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn parse_int_ascii(bytes: &[u8]) -> i32 {
    if bytes.is_empty() {
        return 0;
    }
    let mut idx = 0usize;
    let mut sign = 1i32;
    if bytes[0] == b'-' {
        sign = -1;
        idx = 1;
    }
    let mut value = 0i32;
    while idx < bytes.len() {
        let ch = bytes[idx];
        if !ch.is_ascii_digit() {
            break;
        }
        value = value.saturating_mul(10).saturating_add((ch - b'0') as i32);
        idx += 1;
    }
    value.saturating_mul(sign)
}

fn parse_hex_color_ascii(bytes: &[u8]) -> u32 {
    let hex = if !bytes.is_empty() && bytes[0] == b'#' {
        &bytes[1..]
    } else {
        bytes
    };
    let mut value = 0u32;
    let mut i = 0usize;
    while i < hex.len() {
        let ch = hex[i];
        let digit = match ch {
            b'0'..=b'9' => (ch - b'0') as u32,
            b'a'..=b'f' => (ch - b'a' + 10) as u32,
            b'A'..=b'F' => (ch - b'A' + 10) as u32,
            _ => break,
        };
        value = (value << 4) | digit;
        i += 1;
    }
    if hex.len() == 6 {
        0xff00_0000 | value
    } else if hex.len() == 8 {
        let r = (value >> 24) & 0xff;
        let g = (value >> 16) & 0xff;
        let b = (value >> 8) & 0xff;
        let a = value & 0xff;
        (a << 24) | (r << 16) | (g << 8) | b
    } else {
        0xffff_ffff
    }
}

unsafe fn skip_separators(bytes: &[u8], idx: &mut usize) {
    while *idx < bytes.len() {
        let ch = bytes[*idx];
        if matches!(ch, b' ' | b'\t' | b'\n' | b'\r' | b',' | b':' | b'(' | b'"') {
            *idx += 1;
        } else {
            break;
        }
    }
}

pub unsafe fn parse_baram_config(code: *const c_char, screen_width: i32, cfg: *mut RustWindowConfig) {
    if code.is_null() || cfg.is_null() {
        return;
    }

    let bytes = CStr::from_ptr(code).to_bytes();
    let tag = b"baram-os-config";
    let Some(tag_pos) = find_subslice(bytes, tag) else {
        return;
    };
    let rest = &bytes[tag_pos..];
    let Some(open_pos) = rest.iter().position(|&b| b == b'{') else {
        return;
    };

    let mut idx = open_pos + 1;
    let mut brace_level = 1i32;

    while idx < rest.len() && brace_level > 0 {
        skip_separators(rest, &mut idx);
        if idx >= rest.len() || rest[idx] == b'}' {
            break;
        }

        let key_start = idx;
        while idx < rest.len() && rest[idx] != b':' && rest[idx] != b' ' {
            idx += 1;
        }
        let key = &rest[key_start..idx];
        skip_separators(rest, &mut idx);

        let val_start = idx;
        while idx < rest.len()
            && !matches!(rest[idx], b'"' | b')' | b' ' | b',' | b'}')
        {
            idx += 1;
        }
        let val = &rest[val_start..idx];
        let cfg_ref = &mut *cfg;

        if key == b"height" {
            cfg_ref.h = parse_int_ascii(val);
        } else if key == b"width" {
            if val.len() >= 2 && &val[val.len() - 2..] == b"vw" {
                cfg_ref.w = parse_int_ascii(&val[..val.len() - 2]).saturating_mul(screen_width) / 100;
            } else {
                cfg_ref.w = parse_int_ascii(val);
            }
        } else if key == b"left" {
            cfg_ref.x = parse_int_ascii(val);
        } else if key == b"top" {
            cfg_ref.y = parse_int_ascii(val);
        } else if key == b"showBar" {
            cfg_ref.no_decoration = if val == b"false" { 1 } else { 0 };
        } else if key == b"move" {
            cfg_ref.is_movable = if val == b"true" { 1 } else { 0 };
        } else if key == b"resize" {
            cfg_ref.is_resizing_enabled = if val == b"true" { 1 } else { 0 };
        } else if key == b"front" {
            let enabled = if val == b"true" { 1 } else { 0 };
            cfg_ref.is_always_full_res = enabled;
            cfg_ref.is_sticky = enabled;
        } else if key == b"background" {
            cfg_ref.background_color = parse_hex_color_ascii(val);
        } else if key == b"dark" {
            cfg_ref.force_dark = if val == b"true" { 1 } else { 0 };
        }

        while idx < rest.len() && rest[idx] != b',' && rest[idx] != b'}' {
            if rest[idx] == b'{' {
                brace_level += 1;
            }
            if rest[idx] == b'}' {
                brace_level -= 1;
                if brace_level <= 0 {
                    break;
                }
            }
            idx += 1;
        }
        if idx < rest.len() && rest[idx] == b',' {
            idx += 1;
        }
    }
}
