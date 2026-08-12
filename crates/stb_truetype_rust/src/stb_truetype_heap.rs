

use crate::*;
use c_runtime;

#[derive(Debug, Copy, Clone)]
pub struct stbtt__hheap {
    pub head: *mut stbtt__hheap_chunk,
    pub first_free: *mut u8,
    pub num_remaining_in_head_chunk: i32,
}

#[derive(Debug, Copy, Clone)]
pub struct stbtt__hheap_chunk {
    pub next: *mut stbtt__hheap_chunk,
}

impl core::default::Default for stbtt__hheap {
    fn default() -> Self {
        stbtt__hheap {
            head: core::ptr::null_mut(),
            first_free: core::ptr::null_mut(),
            num_remaining_in_head_chunk: 0,
        }
    }
}

impl core::default::Default for stbtt__hheap_chunk {
    fn default() -> Self {
        stbtt__hheap_chunk {
            next: core::ptr::null_mut(),
        }
    }
}

pub unsafe fn stbtt__hheap_alloc(
    hh: *mut stbtt__hheap,
    size: u64,
    _userdata: *const u8,
) -> *mut u8 {
    if ((*hh).first_free) != core::ptr::null_mut() {
        let p: *mut u8 = (*hh).first_free;
        (*hh).first_free = *((p) as *mut *mut u8);
        return p;
    } else {
        if (*hh).num_remaining_in_head_chunk == 0 {
            let count: i32 = if size < ((32) as u64) {
                2000
            } else {
                if size < ((128) as u64) {
                    800
                } else {
                    100
                }
            };
            let c: *mut stbtt__hheap_chunk = (c_runtime::malloc(
                core::mem::size_of::<stbtt__hheap_chunk>() as u64 + size * ((count) as u64),
            )) as *mut stbtt__hheap_chunk;
            if c == core::ptr::null_mut() {
                return core::ptr::null_mut();
            }
            (*c).next = (*hh).head;
            (*hh).head = c;
            (*hh).num_remaining_in_head_chunk = (count) as i32;
        }
        c_runtime::preDec(&mut (*hh).num_remaining_in_head_chunk);
        return ((((*hh).head) as *mut u8)
            .offset((core::mem::size_of::<stbtt__hheap_chunk>() as u64) as isize))
        .offset((size * (((*hh).num_remaining_in_head_chunk) as u64)) as isize);
    }
}

pub unsafe fn stbtt__hheap_cleanup(hh: *mut stbtt__hheap, _userdata: *const u8) {
    let mut c: *mut stbtt__hheap_chunk = (*hh).head;
    while (c) != core::ptr::null_mut() {
        let n: *mut stbtt__hheap_chunk = (*c).next;
        c_runtime::free((c) as *mut u8);
        c = n;
    }
}

pub unsafe fn stbtt__hheap_free(hh: *mut stbtt__hheap, p: *mut u8) {
    *((p) as *mut *mut u8) = (*hh).first_free;
    (*hh).first_free = p;
}

pub unsafe fn stbtt__new_active(
    hh: *mut stbtt__hheap,
    e: *mut stbtt__edge,
    off_x: i32,
    start_point: f32,
    userdata: *const u8,
) -> *mut stbtt__active_edge {
    let z: *mut stbtt__active_edge = (stbtt__hheap_alloc(
        hh,
        core::mem::size_of::<stbtt__active_edge>() as u64,
        userdata,
    )) as *mut stbtt__active_edge;
    let dxdy: f32 = ((*e).x1 - (*e).x0) / ((*e).y1 - (*e).y0);

    if z == core::ptr::null_mut() {
        return z;
    }
    (*z).fdx = (dxdy) as f32;
    (*z).fdy = (if dxdy != 0.0f32 {
        1.0f32 / dxdy
    } else {
        0.0f32
    }) as f32;
    (*z).fx = ((*e).x0 + dxdy * (start_point - (*e).y0)) as f32;
    (*z).fx -= (off_x) as f32;
    (*z).direction = (if ((*e).invert) != 0 { 1.0f32 } else { -1.0f32 }) as f32;
    (*z).sy = ((*e).y0) as f32;
    (*z).ey = ((*e).y1) as f32;
    (*z).next = core::ptr::null_mut();
    return z;
}
