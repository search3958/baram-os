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

unsafe fn heap_init() {
    if HEAP_INITIALIZED {
        return;
    }
    let first = RUST_HEAP.as_mut_ptr() as *mut BlockHeader;
    (*first).size = RUST_HEAP_SIZE - block_hdr_size();
    (*first).used = 0;
    HEAP_INITIALIZED = true;
}

fn tolower_ascii(c: c_int) -> c_int {
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
    heap_init();
    let size = (size + 7) & !7usize;
    let start = RUST_HEAP.as_mut_ptr();
    let end = start.add(RUST_HEAP_SIZE);
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
    heap_init();
    let start = RUST_HEAP.as_mut_ptr();
    let end = start.add(RUST_HEAP_SIZE);
    let hdr = (ptr as *mut u8).sub(block_hdr_size()) as *mut BlockHeader;
    (*hdr).used = 0;

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
