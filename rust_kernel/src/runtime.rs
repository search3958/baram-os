//! Runtime utilities - math, string, and memory functions
//! This module provides low-level utilities needed for the kernel

use core::ffi::{c_char, c_int, c_void};
use core::mem::size_of;

const RUST_HEAP_SIZE: usize = 32 * 1024 * 1024;

#[repr(C)]
struct BlockHeader {
    size: usize,
    used: c_int,
}

static mut RUST_HEAP: [u8; RUST_HEAP_SIZE] = [0; RUST_HEAP_SIZE];
static mut HEAP_INITIALIZED: bool = false;

fn block_hdr_size() -> usize {
    size_of::<BlockHeader>()
}

pub unsafe fn heap_init(addr: *mut c_void, size: usize) {
    if HEAP_INITIALIZED {
        return;
    }
    // Use provided heap address instead of static array
    let heap_ptr = addr as *mut u8;
    let first = heap_ptr as *mut BlockHeader;
    (*first).size = size - block_hdr_size();
    (*first).used = 0;
    HEAP_INITIALIZED = true;
}

pub fn tolower_ascii(c: c_int) -> c_int {
    if c >= b'A' as c_int && c <= b'Z' as c_int {
        c + (b'a' - b'A') as c_int
    } else {
        c
    }
}

pub unsafe fn rt_memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    let mut p = s as *mut u8;
    let v = c as u8;
    let mut i = 0usize;
    while i < n {
        *p = v;
        p = p.add(1);
        i += 1;
    }
    s
}

pub unsafe fn rt_malloc(size: usize) -> *mut c_void {
    if size == 0 {
        return core::ptr::null_mut();
    }
    if !HEAP_INITIALIZED {
        return core::ptr::null_mut();
    }
    
    // 8-byte alignment
    let size = (size + 7) & !7usize;
    
    // Get heap boundaries from the first block header
    let start = (&mut RUST_HEAP as *mut u8);
    let first = start as *const BlockHeader;
    let heap_base = start;
    let heap_size = (*first).size + block_hdr_size();
    let end = heap_base.add(heap_size);
    
    let mut p = start;
    while p.add(block_hdr_size()) <= end {
        let hdr = p as *mut BlockHeader;
        if (*hdr).used == 0 && (*hdr).size >= size {
            let remaining = (*hdr).size - size;
            if remaining > block_hdr_size() + 8 {
                let next = p.add(block_hdr_size() + size) as *mut BlockHeader;
                (*next).size = remaining - block_hdr_size();
                (*next).used = 0;
                (*hdr).size = size;
            }
            (*hdr).used = 1;
            return p.add(block_hdr_size()) as *mut c_void;
        }
        p = p.add(block_hdr_size() + (*hdr).size);
    }
    core::ptr::null_mut()
}

pub unsafe fn rt_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    if !HEAP_INITIALIZED {
        return;
    }
    
    let start = (&mut RUST_HEAP as *mut u8);
    let first = start as *const BlockHeader;
    let heap_size = (*first).size + block_hdr_size();
    let end = start.add(heap_size);
    
    let hdr = (ptr as *mut u8).sub(block_hdr_size()) as *mut BlockHeader;
    (*hdr).used = 0;

    // Merge adjacent free blocks
    let mut p = start;
    while p.add(block_hdr_size()) <= end {
        let cur = p as *mut BlockHeader;
        let next_p = p.add(block_hdr_size() + (*cur).size);
        if (*cur).used == 0 && next_p.add(block_hdr_size()) <= end {
            let next = next_p as *mut BlockHeader;
            if (*next).used == 0 {
                (*cur).size += block_hdr_size() + (*next).size;
                continue;
            }
        }
        p = next_p;
    }
}

pub unsafe fn rt_realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() {
        return rt_malloc(size);
    }
    if size == 0 {
        rt_free(ptr);
        return core::ptr::null_mut();
    }
    let hdr = (ptr as *mut u8).sub(block_hdr_size()) as *mut BlockHeader;
    if (*hdr).size >= size {
        return ptr;
    }
    let next = rt_malloc(size);
    if next.is_null() {
        return core::ptr::null_mut();
    }
    rt_memcpy(next, ptr, (*hdr).size);
    rt_free(ptr);
    next
}

pub unsafe fn rt_memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let mut d = dest as *mut u8;
    let mut s = src as *const u8;
    let mut i = 0usize;
    while i < n {
        *d = *s;
        d = d.add(1);
        s = s.add(1);
        i += 1;
    }
    dest
}

pub unsafe fn rt_strlen(s: *const c_char) -> usize {
    if s.is_null() {
        return 0;
    }
    let mut n = 0usize;
    while *s.add(n) != 0 {
        n += 1;
    }
    n
}

pub unsafe fn rt_strcmp(a: *const c_char, b: *const c_char) -> c_int {
    let mut i = 0usize;
    loop {
        let ca = *a.add(i) as u8;
        let cb = *b.add(i) as u8;
        if ca != cb || ca == 0 || cb == 0 {
            return ca as c_int - cb as c_int;
        }
        i += 1;
    }
}

pub unsafe fn rt_strncmp(a: *const c_char, b: *const c_char, n: usize) -> c_int {
    let mut i = 0usize;
    while i < n {
        let ca = *a.add(i) as u8;
        let cb = *b.add(i) as u8;
        if ca != cb || ca == 0 || cb == 0 {
            return ca as c_int - cb as c_int;
        }
        i += 1;
    }
    0
}

pub unsafe fn rt_strcasecmp(a: *const c_char, b: *const c_char) -> c_int {
    let mut i = 0usize;
    loop {
        let ca = tolower_ascii(*a.add(i) as c_int) as u8;
        let cb = tolower_ascii(*b.add(i) as c_int) as u8;
        if ca != cb || ca == 0 || cb == 0 {
            return ca as c_int - cb as c_int;
        }
        i += 1;
    }
}

pub unsafe fn rt_strncasecmp(a: *const c_char, b: *const c_char, n: usize) -> c_int {
    let mut i = 0usize;
    while i < n {
        let ca = tolower_ascii(*a.add(i) as c_int) as u8;
        let cb = tolower_ascii(*b.add(i) as c_int) as u8;
        if ca != cb || ca == 0 || cb == 0 {
            return ca as c_int - cb as c_int;
        }
        i += 1;
    }
    0
}

pub unsafe fn rt_memcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int {
    let pa = a as *const u8;
    let pb = b as *const u8;
    let mut i = 0usize;
    while i < n {
        let va = *pa.add(i);
        let vb = *pb.add(i);
        if va != vb {
            return va as c_int - vb as c_int;
        }
        i += 1;
    }
    0
}

pub unsafe fn rt_bcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int {
    rt_memcmp(a, b, n)
}

pub unsafe fn rt_strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    let mut i = 0usize;
    while i < n && *src.add(i) != 0 {
        *dst.add(i) = *src.add(i);
        i += 1;
    }
    while i < n {
        *dst.add(i) = 0;
        i += 1;
    }
    dst
}

pub unsafe fn rt_strlcat(dst: *mut c_char, src: *const c_char, siz: usize) -> usize {
    let dlen = rt_strlen(dst);
    let slen = rt_strlen(src);
    if dlen >= siz {
        return siz + slen;
    }
    if slen < siz - dlen {
        rt_memcpy(dst.add(dlen) as *mut c_void, src as *const c_void, slen + 1);
    } else {
        rt_memcpy(dst.add(dlen) as *mut c_void, src as *const c_void, siz - dlen - 1);
        *dst.add(siz - 1) = 0;
    }
    dlen + slen
}

pub fn rt_rust_eh_personality() {}

// Math functions
fn wrap_pi(x: f32) -> f32 {
    const PI: f32 = core::f32::consts::PI;
    const TWO_PI: f32 = 2.0 * PI;
    let mut x = x;
    while x > PI {
        x -= TWO_PI;
    }
    while x < -PI {
        x += TWO_PI;
    }
    x
}

pub fn rt_sinf(x: f32) -> f32 {
    let x = wrap_pi(x);
    let x2 = x * x;
    x * (1.0 - x2 / 6.0 + (x2 * x2) / 120.0 - (x2 * x2 * x2) / 5040.0)
}

pub fn rt_cosf(x: f32) -> f32 {
    let x = wrap_pi(x);
    let x2 = x * x;
    1.0 - x2 / 2.0 + (x2 * x2) / 24.0 - (x2 * x2 * x2) / 720.0
}

pub fn rt_tanf(x: f32) -> f32 {
    let c = rt_cosf(x);
    if c == 0.0 {
        return 0.0;
    }
    rt_sinf(x) / c
}

fn atan_approx(z: f32) -> f32 {
    const PI: f32 = core::f32::consts::PI;
    if z > 1.0 {
        return (PI * 0.5) - atan_approx(1.0 / z);
    }
    if z < -1.0 {
        return -(PI * 0.5) - atan_approx(1.0 / z);
    }
    z / (1.0 + 0.28 * z * z)
}

pub fn rt_atan2f(y: f32, x: f32) -> f32 {
    const PI: f32 = core::f32::consts::PI;
    if x > 0.0 {
        return atan_approx(y / x);
    }
    if x < 0.0 {
        if y >= 0.0 {
            return atan_approx(y / x) + PI;
        }
        return atan_approx(y / x) - PI;
    }
    if y > 0.0 {
        return PI * 0.5;
    }
    if y < 0.0 {
        return -PI * 0.5;
    }
    0.0
}

pub fn rt_acosf(x: f32) -> f32 {
    const PI: f32 = core::f32::consts::PI;
    if x <= -1.0 {
        return PI;
    }
    if x >= 1.0 {
        return 0.0;
    }
    rt_atan2f(rt_sqrtf(1.0 - x * x), x)
}

pub fn rt_sqrtf(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut r = x;
    for _ in 0..12 {
        r = 0.5 * (r + x / r);
    }
    r
}

pub fn rt_floorf(x: f32) -> f32 {
    let i = x as i32;
    if (i as f32) < x {
        (i - 1) as f32
    } else {
        i as f32
    }
}

pub fn rt_ceilf(x: f32) -> f32 {
    let i = x as i32;
    if (i as f32) > x {
        (i + 1) as f32
    } else {
        i as f32
    }
}

pub fn rt_roundf(x: f32) -> f32 {
    if x >= 0.0 {
        rt_floorf(x + 0.5)
    } else {
        rt_ceilf(x - 0.5)
    }
}

pub fn rt_fmodf(x: f32, y: f32) -> f32 {
    if y == 0.0 {
        return 0.0;
    }
    let q = (x / y) as i32;
    x - (q as f32) * y
}

pub fn rt_fabsf(x: f32) -> f32 {
    if x < 0.0 { -x } else { x }
}
