use core::ffi::{c_char, CStr};
use core::ptr;

#[repr(C, packed)]
struct TarHeader {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],
    mtime: [u8; 12],
    checksum: [u8; 8],
    typeflag: u8,
    linkname: [u8; 100],
    magic: [u8; 6],
    version: [u8; 2],
    uname: [u8; 32],
    gname: [u8; 32],
    devmajor: [u8; 8],
    devminor: [u8; 8],
    prefix: [u8; 155],
}

pub unsafe fn octal_to_int(s: *const u8, len: i32) -> u32 {
    if s.is_null() || len <= 0 {
        return 0;
    }

    let mut res = 0u32;
    let mut i = 0usize;
    let len = len as usize;

    while i < len {
        let ch = *s.add(i);
        if ch != b' ' && ch != 0 {
            break;
        }
        i += 1;
    }

    while i < len {
        let ch = *s.add(i);
        if !(b'0'..=b'7').contains(&ch) {
            break;
        }
        res = res.saturating_mul(8).saturating_add((ch - b'0') as u32);
        i += 1;
    }

    res
}

unsafe fn header_name_eq(name: &[u8; 100], filename: *const c_char) -> bool {
    if filename.is_null() {
        return false;
    }
    let bytes = CStr::from_ptr(filename).to_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i >= name.len() || name[i] != bytes[i] {
            return false;
        }
        i += 1;
    }
    i < name.len() && name[i] == 0
}

pub unsafe fn tar_find_file(
    tar_data: *const u8,
    tar_size: usize,
    filename: *const c_char,
    out_size: *mut u32,
) -> *const u8 {
    if tar_data.is_null() || filename.is_null() {
        return ptr::null();
    }

    let mut p = tar_data;
    let end = tar_data.add(tar_size);
    while p.add(512) <= end {
        let h = &*(p as *const TarHeader);
        if h.name[0] == 0 {
            break;
        }
        let size = octal_to_int(h.size.as_ptr(), 12);
        if header_name_eq(&h.name, filename) {
            if !out_size.is_null() {
                *out_size = size;
            }
            return p.add(512);
        }
        let advance = 512usize + (((size as usize) + 511usize) & !511usize);
        p = p.add(advance);
    }

    ptr::null()
}
