use crate::*;
use c_runtime;

pub const STBTT_MAC_EID_ARABIC: i32 = 4;
pub const STBTT_MAC_EID_CHINESE_TRAD: i32 = 2;
pub const STBTT_MAC_EID_GREEK: i32 = 6;
pub const STBTT_MAC_EID_HEBREW: i32 = 5;
pub const STBTT_MAC_EID_JAPANESE: i32 = 1;
pub const STBTT_MAC_EID_KOREAN: i32 = 3;
pub const STBTT_MAC_EID_ROMAN: i32 = 0;
pub const STBTT_MAC_EID_RUSSIAN: i32 = 7;
pub const STBTT_MAC_LANG_ARABIC: i32 = 12;
pub const STBTT_MAC_LANG_CHINESE_SIMPLIFIED: i32 = 33;
pub const STBTT_MAC_LANG_CHINESE_TRAD: i32 = 19;
pub const STBTT_MAC_LANG_DUTCH: i32 = 4;
pub const STBTT_MAC_LANG_ENGLISH: i32 = 0;
pub const STBTT_MAC_LANG_FRENCH: i32 = 1;
pub const STBTT_MAC_LANG_GERMAN: i32 = 2;
pub const STBTT_MAC_LANG_HEBREW: i32 = 10;
pub const STBTT_MAC_LANG_ITALIAN: i32 = 3;
pub const STBTT_MAC_LANG_JAPANESE: i32 = 11;
pub const STBTT_MAC_LANG_KOREAN: i32 = 23;
pub const STBTT_MAC_LANG_RUSSIAN: i32 = 32;
pub const STBTT_MAC_LANG_SPANISH: i32 = 6;
pub const STBTT_MAC_LANG_SWEDISH: i32 = 5;
pub const STBTT_MS_EID_SHIFTJIS: i32 = 2;
pub const STBTT_MS_EID_SYMBOL: i32 = 0;
pub const STBTT_MS_EID_UNICODE_BMP: i32 = 1;
pub const STBTT_MS_EID_UNICODE_FULL: i32 = 10;
pub const STBTT_MS_LANG_CHINESE: i32 = 2052;
pub const STBTT_MS_LANG_DUTCH: i32 = 1043;
pub const STBTT_MS_LANG_ENGLISH: i32 = 1033;
pub const STBTT_MS_LANG_FRENCH: i32 = 1036;
pub const STBTT_MS_LANG_GERMAN: i32 = 1031;
pub const STBTT_MS_LANG_HEBREW: i32 = 1037;
pub const STBTT_MS_LANG_ITALIAN: i32 = 1040;
pub const STBTT_MS_LANG_JAPANESE: i32 = 1041;
pub const STBTT_MS_LANG_KOREAN: i32 = 1042;
pub const STBTT_MS_LANG_RUSSIAN: i32 = 1049;
pub const STBTT_MS_LANG_SPANISH: i32 = 1033;
pub const STBTT_MS_LANG_SWEDISH: i32 = 1053;
pub const STBTT_PLATFORM_ID_ISO: i32 = 2;
pub const STBTT_PLATFORM_ID_MAC: i32 = 1;
pub const STBTT_PLATFORM_ID_MICROSOFT: i32 = 3;
pub const STBTT_PLATFORM_ID_UNICODE: i32 = 0;
pub const STBTT_UNICODE_EID_ISO_10646: i32 = 2;
pub const STBTT_UNICODE_EID_UNICODE_1_0: i32 = 0;
pub const STBTT_UNICODE_EID_UNICODE_1_1: i32 = 1;
pub const STBTT_UNICODE_EID_UNICODE_2_0_BMP: i32 = 3;
pub const STBTT_UNICODE_EID_UNICODE_2_0_FULL: i32 = 4;
pub const STBTT_vcubic: i32 = 4;
pub const STBTT_vcurve: i32 = 3;
pub const STBTT_vline: i32 = 2;
pub const STBTT_vmove: i32 = 1;

#[derive(Debug, Copy, Clone)]
pub struct stbtt__point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct stbtt_aligned_quad {
    pub x0: f32,
    pub y0: f32,
    pub s0: f32,
    pub t0: f32,
    pub x1: f32,
    pub y1: f32,
    pub s1: f32,
    pub t1: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct stbtt_bakedchar {
    pub x0: u16,
    pub y0: u16,
    pub x1: u16,
    pub y1: u16,
    pub xoff: f32,
    pub yoff: f32,
    pub xadvance: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct stbtt_packedchar {
    pub x0: u16,
    pub y0: u16,
    pub x1: u16,
    pub y1: u16,
    pub xoff: f32,
    pub yoff: f32,
    pub xadvance: f32,
    pub xoff2: f32,
    pub yoff2: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct stbtt_vertex {
    pub x: i16,
    pub y: i16,
    pub cx: i16,
    pub cy: i16,
    pub cx1: i16,
    pub cy1: i16,
    pub _type_: u8,
    pub padding: u8,
}

impl core::default::Default for stbtt__point {
    fn default() -> Self {
        stbtt__point {
            x: 0.0f32,
            y: 0.0f32,
        }
    }
}

impl core::default::Default for stbtt_aligned_quad {
    fn default() -> Self {
        stbtt_aligned_quad {
            x0: 0.0f32,
            y0: 0.0f32,
            s0: 0.0f32,
            t0: 0.0f32,
            x1: 0.0f32,
            y1: 0.0f32,
            s1: 0.0f32,
            t1: 0.0f32,
        }
    }
}

impl core::default::Default for stbtt_bakedchar {
    fn default() -> Self {
        stbtt_bakedchar {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 0,
            xoff: 0.0f32,
            yoff: 0.0f32,
            xadvance: 0.0f32,
        }
    }
}

impl core::default::Default for stbtt_packedchar {
    fn default() -> Self {
        stbtt_packedchar {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 0,
            xoff: 0.0f32,
            yoff: 0.0f32,
            xadvance: 0.0f32,
            xoff2: 0.0f32,
            yoff2: 0.0f32,
        }
    }
}

impl core::default::Default for stbtt_vertex {
    fn default() -> Self {
        stbtt_vertex {
            x: 0,
            y: 0,
            cx: 0,
            cy: 0,
            cx1: 0,
            cy1: 0,
            _type_: 0,
            padding: 0,
        }
    }
}

pub unsafe fn equal(a: *mut f32, b: *mut f32) -> i32 {
    return (if *a.offset((0) as isize) == *b.offset((0) as isize)
        && *a.offset((1) as isize) == *b.offset((1) as isize)
    {
        1
    } else {
        0
    }) as i32;
}

pub unsafe fn stbtt__add_point(points: *mut stbtt__point, n: i32, x: f32, y: f32) {
    if points == core::ptr::null_mut() {
        return;
    }
    (*points.offset((n) as isize)).x = (x) as f32;
    (*points.offset((n) as isize)).y = (y) as f32;
}

pub unsafe fn stbtt__cff_index_get(mut b: stbtt__buf, i: i32) -> stbtt__buf {
    let mut count: i32 = 0;
    let mut offsize: i32 = 0;
    let mut start: i32 = 0;
    let mut end: i32 = 0;
    stbtt__buf_seek((&mut b) as *mut stbtt__buf, 0);
    count = (stbtt__buf_get((&mut b) as *mut stbtt__buf, 2)) as i32;
    offsize = (stbtt__buf_get8((&mut b) as *mut stbtt__buf)) as i32;

    stbtt__buf_skip((&mut b) as *mut stbtt__buf, i * offsize);
    start = (stbtt__buf_get((&mut b) as *mut stbtt__buf, offsize)) as i32;
    end = (stbtt__buf_get((&mut b) as *mut stbtt__buf, offsize)) as i32;
    return (stbtt__buf_range(
        (&mut b) as *mut stbtt__buf,
        2 + (count + 1) * offsize + start,
        end - start,
    )) as stbtt__buf;
}

pub unsafe fn stbtt__CompareUTF8toUTF16_bigendian_prefix(
    s1: *mut u8,
    len1: i32,
    mut s2: *mut u8,
    mut len2: i32,
) -> i32 {
    let mut i: i32 = 0;
    while (len2) != 0 {
        let ch: u16 = (((*s2.offset((0) as isize)) as i32) * 256
            + ((*s2.offset((1) as isize)) as i32)) as u16;
        if ((ch) as i32) < 0x80 {
            if i >= len1 {
                return (-1) as i32;
            }
            if ((*s1.offset((c_runtime::postInc(&mut i)) as isize)) as i32) != ((ch) as i32) {
                return (-1) as i32;
            }
        } else {
            if ((ch) as i32) < 0x800 {
                if i + 1 >= len1 {
                    return (-1) as i32;
                }
                if ((*s1.offset((c_runtime::postInc(&mut i)) as isize)) as i32)
                    != 0xc0 + (((ch) as i32) >> 6)
                {
                    return (-1) as i32;
                }
                if ((*s1.offset((c_runtime::postInc(&mut i)) as isize)) as i32)
                    != 0x80 + (((ch) as i32) & 0x3f)
                {
                    return (-1) as i32;
                }
            } else {
                if ((ch) as i32) >= 0xd800 && ((ch) as i32) < 0xdc00 {
                    let mut c: u32 = 0;
                    let ch2: u16 = (((*s2.offset((2) as isize)) as i32) * 256
                        + ((*s2.offset((3) as isize)) as i32))
                        as u16;
                    if i + 3 >= len1 {
                        return (-1) as i32;
                    }
                    c = (((((ch) as i32) - 0xd800) << 10) + (((ch2) as i32) - 0xdc00) + 0x10000)
                        as u32;
                    if ((*s1.offset((c_runtime::postInc(&mut i)) as isize)) as u32)
                        != ((0xf0) as u32) + (c >> 18)
                    {
                        return (-1) as i32;
                    }
                    if ((*s1.offset((c_runtime::postInc(&mut i)) as isize)) as u32)
                        != ((0x80) as u32) + ((c >> 12) & ((0x3f) as u32))
                    {
                        return (-1) as i32;
                    }
                    if ((*s1.offset((c_runtime::postInc(&mut i)) as isize)) as u32)
                        != ((0x80) as u32) + ((c >> 6) & ((0x3f) as u32))
                    {
                        return (-1) as i32;
                    }
                    if ((*s1.offset((c_runtime::postInc(&mut i)) as isize)) as u32)
                        != ((0x80) as u32) + ((c) & ((0x3f) as u32))
                    {
                        return (-1) as i32;
                    }
                    s2 = s2.offset((2) as isize);
                    len2 -= (2) as i32;
                } else {
                    if ((ch) as i32) >= 0xdc00 && ((ch) as i32) < 0xe000 {
                        return (-1) as i32;
                    } else {
                        if i + 2 >= len1 {
                            return (-1) as i32;
                        }
                        if ((*s1.offset((c_runtime::postInc(&mut i)) as isize)) as i32)
                            != 0xe0 + (((ch) as i32) >> 12)
                        {
                            return (-1) as i32;
                        }
                        if ((*s1.offset((c_runtime::postInc(&mut i)) as isize)) as i32)
                            != 0x80 + ((((ch) as i32) >> 6) & 0x3f)
                        {
                            return (-1) as i32;
                        }
                        if ((*s1.offset((c_runtime::postInc(&mut i)) as isize)) as i32)
                            != 0x80 + (((ch) as i32) & 0x3f)
                        {
                            return (-1) as i32;
                        }
                    }
                }
            }
        }
        s2 = s2.offset((2) as isize);
        len2 -= (2) as i32;
    }
    return (i) as i32;
}

pub unsafe fn stbtt__compute_crossings_x(
    x: f32,
    mut y: f32,
    nverts: i32,
    verts: *mut stbtt_vertex,
) -> i32 {
    let mut i: i32 = 0;
    let mut orig: [f32; 2] = [0.0f32; 2];
    let ray: [f32; 2] = [((1) as f32), ((0) as f32)];
    let mut y_frac: f32 = 0.0f32;
    let mut winding: i32 = 0;
    y_frac = c_runtime::fmod(y, 1.0f32);
    if y_frac < 0.01f32 {
        y += (0.01f32) as f32;
    } else {
        if y_frac > 0.99f32 {
            y -= (0.01f32) as f32;
        }
    }
    orig[(0) as usize] = (x) as f32;
    orig[(1) as usize] = (y) as f32;
    i = (0) as i32;
    while i < nverts {
        if (((*verts.offset((i) as isize))._type_) as i32) == STBTT_vline {
            let x0: i32 = ((*verts.offset((i - 1) as isize)).x) as i32;
            let y0: i32 = ((*verts.offset((i - 1) as isize)).y) as i32;
            let x1: i32 = ((*verts.offset((i) as isize)).x) as i32;
            let y1: i32 = ((*verts.offset((i) as isize)).y) as i32;
            if y > ((if (y0) < (y1) { y0 } else { y1 }) as f32)
                && y < ((if (y0) < (y1) { y1 } else { y0 }) as f32)
                && x > ((if (x0) < (x1) { x0 } else { x1 }) as f32)
            {
                let x_inter: f32 =
                    (y - ((y0) as f32)) / ((y1 - y0) as f32) * ((x1 - x0) as f32) + ((x0) as f32);
                if x_inter < x {
                    winding += (if y0 < y1 { 1 } else { -1 }) as i32;
                }
            }
        }
        if (((*verts.offset((i) as isize))._type_) as i32) == STBTT_vcurve {
            let mut x0: i32 = ((*verts.offset((i - 1) as isize)).x) as i32;
            let mut y0: i32 = ((*verts.offset((i - 1) as isize)).y) as i32;
            let mut x1: i32 = ((*verts.offset((i) as isize)).cx) as i32;
            let mut y1: i32 = ((*verts.offset((i) as isize)).cy) as i32;
            let x2: i32 = ((*verts.offset((i) as isize)).x) as i32;
            let y2: i32 = ((*verts.offset((i) as isize)).y) as i32;
            let ax: i32 = if (x0) < (if (x1) < (x2) { x1 } else { x2 }) {
                x0
            } else {
                if (x1) < (x2) {
                    x1
                } else {
                    x2
                }
            };
            let ay: i32 = if (y0) < (if (y1) < (y2) { y1 } else { y2 }) {
                y0
            } else {
                if (y1) < (y2) {
                    y1
                } else {
                    y2
                }
            };
            let by: i32 = if (y0) < (if (y1) < (y2) { y2 } else { y1 }) {
                if (y1) < (y2) {
                    y2
                } else {
                    y1
                }
            } else {
                y0
            };
            if y > ((ay) as f32) && y < ((by) as f32) && x > ((ax) as f32) {
                let mut q0: [f32; 2] = [0.0f32; 2];
                let mut q1: [f32; 2] = [0.0f32; 2];
                let mut q2: [f32; 2] = [0.0f32; 2];
                let hits: [[f32; 2]; 2] = [[0.0f32; 2]; 2];
                q0[(0) as usize] = (x0) as f32;
                q0[(1) as usize] = (y0) as f32;
                q1[(0) as usize] = (x1) as f32;
                q1[(1) as usize] = (y1) as f32;
                q2[(0) as usize] = (x2) as f32;
                q2[(1) as usize] = (y2) as f32;
                if (equal((q0.as_mut_ptr()) as *mut f32, (q1.as_mut_ptr()) as *mut f32)) != 0
                    || (equal((q1.as_mut_ptr()) as *mut f32, (q2.as_mut_ptr()) as *mut f32)) != 0
                {
                    x0 = ((*verts.offset((i - 1) as isize)).x) as i32;
                    y0 = ((*verts.offset((i - 1) as isize)).y) as i32;
                    x1 = ((*verts.offset((i) as isize)).x) as i32;
                    y1 = ((*verts.offset((i) as isize)).y) as i32;
                    if y > ((if (y0) < (y1) { y0 } else { y1 }) as f32)
                        && y < ((if (y0) < (y1) { y1 } else { y0 }) as f32)
                        && x > ((if (x0) < (x1) { x0 } else { x1 }) as f32)
                    {
                        let x_inter: f32 = (y - ((y0) as f32)) / ((y1 - y0) as f32)
                            * ((x1 - x0) as f32)
                            + ((x0) as f32);
                        if x_inter < x {
                            winding += (if y0 < y1 { 1 } else { -1 }) as i32;
                        }
                    }
                } else {
                    let num_hits: i32 = stbtt__ray_intersect_bezier(orig, ray, q0, q1, q2, hits);
                    if num_hits >= 1 {
                        if hits[(0) as usize][(0) as usize] < ((0) as f32) {
                            winding += (if hits[(0) as usize][(1) as usize] < ((0) as f32) {
                                -1
                            } else {
                                1
                            }) as i32;
                        }
                    }
                    if num_hits >= 2 {
                        if hits[(1) as usize][(0) as usize] < ((0) as f32) {
                            winding += (if hits[(1) as usize][(1) as usize] < ((0) as f32) {
                                -1
                            } else {
                                1
                            }) as i32;
                        }
                    }
                }
            }
        }
        c_runtime::preInc(&mut i);
    }
    return (winding) as i32;
}

pub unsafe fn stbtt__cuberoot(x: f32) -> f32 {
    if x < ((0) as f32) {
        return -((c_runtime::pow((-x) as f32, (1.0f32 / 3.0f32) as f32)) as f32);
    } else {
        return (c_runtime::pow((x) as f32, (1.0f32 / 3.0f32) as f32)) as f32;
    }
}

pub unsafe fn stbtt__find_table(data: *const u8, fontstart: u32, tag: &str) -> u32 {
    let num_tables: i32 =
        (ttUSHORT(((data).offset((fontstart) as isize)).offset((4) as isize))) as i32;
    let tabledir: u32 = fontstart + ((12) as u32);
    let mut i: i32 = 0;
    i = (0) as i32;
    while i < num_tables {
        let loc: u32 = tabledir + ((16 * i) as u32);
        if ((*(((data).offset((loc) as isize)).offset((0) as isize)).offset((0) as isize)) as i32)
            == (tag.chars().nth(0).unwrap() as i32)
            && ((*(((data).offset((loc) as isize)).offset((0) as isize)).offset((1) as isize))
                as i32)
                == (tag.chars().nth(1).unwrap() as i32)
            && ((*(((data).offset((loc) as isize)).offset((0) as isize)).offset((2) as isize))
                as i32)
                == (tag.chars().nth(2).unwrap() as i32)
            && ((*(((data).offset((loc) as isize)).offset((0) as isize)).offset((3) as isize))
                as i32)
                == (tag.chars().nth(3).unwrap() as i32)
        {
            return (ttULONG(((data).offset((loc) as isize)).offset((8) as isize))) as u32;
        }
        c_runtime::preInc(&mut i);
    }
    return (0) as u32;
}

pub unsafe fn stbtt__get_subr(mut idx: stbtt__buf, mut n: i32) -> stbtt__buf {
    let count: i32 = stbtt__cff_index_count((&mut idx) as *mut stbtt__buf);
    let mut bias: i32 = 107;
    if count >= 33900 {
        bias = (32768) as i32;
    } else {
        if count >= 1240 {
            bias = (1131) as i32;
        }
    }
    n += (bias) as i32;
    if n < 0 || n >= count {
        return (stbtt__new_buf(core::ptr::null_mut(), (0) as u64)) as stbtt__buf;
    }
    return (stbtt__cff_index_get(idx, n)) as stbtt__buf;
}

pub unsafe fn stbtt__get_subrs(mut cff: stbtt__buf, mut fontdict: stbtt__buf) -> stbtt__buf {
    let mut subrsoff: u32 = (0) as u32;
    let mut private_loc: [u32; 2] = [((0) as u32), ((0) as u32)];
    let mut pdict: stbtt__buf = stbtt__buf::default();
    stbtt__dict_get_ints(
        (&mut fontdict) as *mut stbtt__buf,
        18,
        2,
        (private_loc.as_mut_ptr()) as *mut u32,
    );
    if private_loc[(1) as usize] == 0 || private_loc[(0) as usize] == 0 {
        return (stbtt__new_buf(core::ptr::null_mut(), (0) as u64)) as stbtt__buf;
    }
    pdict = (stbtt__buf_range(
        (&mut cff) as *mut stbtt__buf,
        (private_loc[(1) as usize]) as i32,
        (private_loc[(0) as usize]) as i32,
    )) as stbtt__buf;
    stbtt__dict_get_ints(
        (&mut pdict) as *mut stbtt__buf,
        19,
        1,
        (&mut subrsoff) as *mut u32,
    );
    if subrsoff == 0 {
        return (stbtt__new_buf(core::ptr::null_mut(), (0) as u64)) as stbtt__buf;
    }
    stbtt__buf_seek(
        (&mut cff) as *mut stbtt__buf,
        (private_loc[(1) as usize] + subrsoff) as i32,
    );
    return (stbtt__cff_get_index((&mut cff) as *mut stbtt__buf)) as stbtt__buf;
}

pub unsafe fn stbtt__h_prefilter(
    mut pixels: *mut u8,
    w: i32,
    h: i32,
    stride_in_bytes: i32,
    kernel_width: u32,
) {
    let mut buffer: [u8; 8] = [0; 8];
    let safe_w: i32 = (((w) as u32) - kernel_width) as i32;
    let mut j: i32 = 0;
    c_runtime::memset((buffer.as_mut_ptr()) as *mut u8, 0, (8) as u64);
    j = (0) as i32;
    while j < h {
        let mut i: i32 = 0;
        let mut total: u32 = 0;
        c_runtime::memset((buffer.as_mut_ptr()) as *mut u8, 0, (kernel_width) as u64);
        total = (0) as u32;
        if kernel_width == ((2) as u32) {
            i = (0) as i32;
            while i <= safe_w {
                total += (((*pixels.offset((i) as isize)) as i32)
                    - ((buffer[(i & (8 - 1)) as usize]) as i32)) as u32;
                buffer[((((i) as u32) + kernel_width) & ((8 - 1) as u32)) as usize] =
                    (*pixels.offset((i) as isize)) as u8;
                *pixels.offset((i) as isize) = (total / ((2) as u32)) as u8;
                c_runtime::preInc(&mut i);
            }
        } else if kernel_width == ((3) as u32) {
            i = (0) as i32;
            while i <= safe_w {
                total += (((*pixels.offset((i) as isize)) as i32)
                    - ((buffer[(i & (8 - 1)) as usize]) as i32)) as u32;
                buffer[((((i) as u32) + kernel_width) & ((8 - 1) as u32)) as usize] =
                    (*pixels.offset((i) as isize)) as u8;
                *pixels.offset((i) as isize) = (total / ((3) as u32)) as u8;
                c_runtime::preInc(&mut i);
            }
        } else if kernel_width == ((4) as u32) {
            i = (0) as i32;
            while i <= safe_w {
                total += (((*pixels.offset((i) as isize)) as i32)
                    - ((buffer[(i & (8 - 1)) as usize]) as i32)) as u32;
                buffer[((((i) as u32) + kernel_width) & ((8 - 1) as u32)) as usize] =
                    (*pixels.offset((i) as isize)) as u8;
                *pixels.offset((i) as isize) = (total / ((4) as u32)) as u8;
                c_runtime::preInc(&mut i);
            }
        } else if kernel_width == ((5) as u32) {
            i = (0) as i32;
            while i <= safe_w {
                total += (((*pixels.offset((i) as isize)) as i32)
                    - ((buffer[(i & (8 - 1)) as usize]) as i32)) as u32;
                buffer[((((i) as u32) + kernel_width) & ((8 - 1) as u32)) as usize] =
                    (*pixels.offset((i) as isize)) as u8;
                *pixels.offset((i) as isize) = (total / ((5) as u32)) as u8;
                c_runtime::preInc(&mut i);
            }
        } else {
            i = (0) as i32;
            while i <= safe_w {
                total += (((*pixels.offset((i) as isize)) as i32)
                    - ((buffer[(i & (8 - 1)) as usize]) as i32)) as u32;
                buffer[((((i) as u32) + kernel_width) & ((8 - 1) as u32)) as usize] =
                    (*pixels.offset((i) as isize)) as u8;
                *pixels.offset((i) as isize) = (total / kernel_width) as u8;
                c_runtime::preInc(&mut i);
            }
        };
        while i < w {
            total -= (buffer[(i & (8 - 1)) as usize]) as u32;
            *pixels.offset((i) as isize) = (total / kernel_width) as u8;
            c_runtime::preInc(&mut i);
        }
        pixels = pixels.offset((stride_in_bytes) as isize);
        c_runtime::preInc(&mut j);
    }
}

pub unsafe fn stbtt__isfont(font: *const u8) -> i32 {
    if ((*(font).offset((0) as isize)) as i32) == (49)
        && ((*(font).offset((1) as isize)) as i32) == (0)
        && ((*(font).offset((2) as isize)) as i32) == (0)
        && ((*(font).offset((3) as isize)) as i32) == (0)
    {
        return (1) as i32;
    }
    if ((*(font).offset((0) as isize)) as i32) == ('t' as i32)
        && ((*(font).offset((1) as isize)) as i32) == ('y' as i32)
        && ((*(font).offset((2) as isize)) as i32) == ('p' as i32)
        && ((*(font).offset((3) as isize)) as i32) == ('1' as i32)
    {
        return (1) as i32;
    }
    if ((*(font).offset((0) as isize)) as i32) == ('O' as i32)
        && ((*(font).offset((1) as isize)) as i32) == ('T' as i32)
        && ((*(font).offset((2) as isize)) as i32) == ('T' as i32)
        && ((*(font).offset((3) as isize)) as i32) == ('O' as i32)
    {
        return (1) as i32;
    }
    if ((*(font).offset((0) as isize)) as i32) == (0)
        && ((*(font).offset((1) as isize)) as i32) == (1)
        && ((*(font).offset((2) as isize)) as i32) == (0)
        && ((*(font).offset((3) as isize)) as i32) == (0)
    {
        return (1) as i32;
    }
    if ((*(font).offset((0) as isize)) as i32) == ('t' as i32)
        && ((*(font).offset((1) as isize)) as i32) == ('r' as i32)
        && ((*(font).offset((2) as isize)) as i32) == ('u' as i32)
        && ((*(font).offset((3) as isize)) as i32) == ('e' as i32)
    {
        return (1) as i32;
    }
    return (0) as i32;
}

pub unsafe fn stbtt__matches(fc: *mut u8, offset: u32, name: *mut u8, flags: i32) -> i32 {
    let nlen: i32 = (c_runtime::strlen(name)) as i32;
    let mut nm: u32 = 0;
    let mut hd: u32 = 0;
    if stbtt__isfont((fc).offset((offset) as isize)) == 0 {
        return (0) as i32;
    }
    if (flags) != 0 {
        hd = (stbtt__find_table(fc, offset, "head")) as u32;
        if (((ttUSHORT(((fc).offset((hd) as isize)).offset((44) as isize))) as i32) & 7)
            != (flags & 7)
        {
            return (0) as i32;
        }
    }
    nm = (stbtt__find_table(fc, offset, "name")) as u32;
    if nm == 0 {
        return (0) as i32;
    }
    if (flags) != 0 {
        if (stbtt__matchpair(fc, nm, name, nlen, 16, -1)) != 0 {
            return (1) as i32;
        }
        if (stbtt__matchpair(fc, nm, name, nlen, 1, -1)) != 0 {
            return (1) as i32;
        }
        if (stbtt__matchpair(fc, nm, name, nlen, 3, -1)) != 0 {
            return (1) as i32;
        }
    } else {
        if (stbtt__matchpair(fc, nm, name, nlen, 16, 17)) != 0 {
            return (1) as i32;
        }
        if (stbtt__matchpair(fc, nm, name, nlen, 1, 2)) != 0 {
            return (1) as i32;
        }
        if (stbtt__matchpair(fc, nm, name, nlen, 3, -1)) != 0 {
            return (1) as i32;
        }
    }
    return (0) as i32;
}

pub unsafe fn stbtt__matchpair(
    fc: *mut u8,
    nm: u32,
    name: *mut u8,
    nlen: i32,
    target_id: i32,
    next_id: i32,
) -> i32 {
    let mut i: i32 = 0;
    let count: i32 = (ttUSHORT(((fc).offset((nm) as isize)).offset((2) as isize))) as i32;
    let stringOffset: i32 =
        (nm + ((ttUSHORT(((fc).offset((nm) as isize)).offset((4) as isize))) as u32)) as i32;
    i = (0) as i32;
    while i < count {
        let loc: u32 = nm + ((6) as u32) + ((12 * i) as u32);
        let id: i32 = (ttUSHORT(((fc).offset((loc) as isize)).offset((6) as isize))) as i32;
        if id == target_id {
            let platform: i32 =
                (ttUSHORT(((fc).offset((loc) as isize)).offset((0) as isize))) as i32;
            let encoding: i32 =
                (ttUSHORT(((fc).offset((loc) as isize)).offset((2) as isize))) as i32;
            let language: i32 =
                (ttUSHORT(((fc).offset((loc) as isize)).offset((4) as isize))) as i32;
            if platform == 0
                || (platform == 3 && encoding == 1)
                || (platform == 3 && encoding == 10)
            {
                let mut slen: i32 =
                    (ttUSHORT(((fc).offset((loc) as isize)).offset((8) as isize))) as i32;
                let mut off: i32 =
                    (ttUSHORT(((fc).offset((loc) as isize)).offset((10) as isize))) as i32;
                let mut matchlen: i32 = stbtt__CompareUTF8toUTF16_bigendian_prefix(
                    name,
                    nlen,
                    ((fc).offset((stringOffset) as isize)).offset((off) as isize),
                    slen,
                );
                if matchlen >= 0 {
                    if i + 1 < count
                        && ((ttUSHORT(
                            (((fc).offset((loc) as isize)).offset((12) as isize))
                                .offset((6) as isize),
                        )) as i32)
                            == next_id
                        && ((ttUSHORT(((fc).offset((loc) as isize)).offset((12) as isize))) as i32)
                            == platform
                        && ((ttUSHORT(
                            (((fc).offset((loc) as isize)).offset((12) as isize))
                                .offset((2) as isize),
                        )) as i32)
                            == encoding
                        && ((ttUSHORT(
                            (((fc).offset((loc) as isize)).offset((12) as isize))
                                .offset((4) as isize),
                        )) as i32)
                            == language
                    {
                        slen = (ttUSHORT(
                            (((fc).offset((loc) as isize)).offset((12) as isize))
                                .offset((8) as isize),
                        )) as i32;
                        off = (ttUSHORT(
                            (((fc).offset((loc) as isize)).offset((12) as isize))
                                .offset((10) as isize),
                        )) as i32;
                        if slen == 0 {
                            if matchlen == nlen {
                                return (1) as i32;
                            }
                        } else {
                            if matchlen < nlen && ((*name.offset((matchlen) as isize)) as i32) == 32
                            {
                                c_runtime::preInc(&mut matchlen);
                                if (stbtt_CompareUTF8toUTF16_bigendian_internal(
                                    (((name).offset((matchlen) as isize)) as *mut i8) as *mut i8,
                                    nlen - matchlen,
                                    ((((fc).offset((stringOffset) as isize)).offset((off) as isize))
                                        as *mut i8) as *mut i8,
                                    slen,
                                )) != 0
                                {
                                    return (1) as i32;
                                }
                            }
                        }
                    } else {
                        if matchlen == nlen {
                            return (1) as i32;
                        }
                    }
                }
            }
        }
        c_runtime::preInc(&mut i);
    }
    return (0) as i32;
}

pub unsafe fn stbtt__oversample_shift(oversample: i32) -> f32 {
    if oversample == 0 {
        return (0.0f32) as f32;
    }
    return ((-(oversample - 1)) as f32) / (2.0f32 * ((oversample) as f32));
}

pub unsafe fn stbtt__position_trapezoid_area(
    height: f32,
    tx0: f32,
    tx1: f32,
    bx0: f32,
    bx1: f32,
) -> f32 {
    return (stbtt__sized_trapezoid_area(height, tx1 - tx0, bx1 - bx0)) as f32;
}

pub unsafe fn stbtt__ray_intersect_bezier(
    orig: [f32; 2],
    ray: [f32; 2],
    q0: [f32; 2],
    q1: [f32; 2],
    q2: [f32; 2],
    mut hits: [[f32; 2]; 2],
) -> i32 {
    let q0perp: f32 = q0[(1) as usize] * ray[(0) as usize] - q0[(0) as usize] * ray[(1) as usize];
    let q1perp: f32 = q1[(1) as usize] * ray[(0) as usize] - q1[(0) as usize] * ray[(1) as usize];
    let q2perp: f32 = q2[(1) as usize] * ray[(0) as usize] - q2[(0) as usize] * ray[(1) as usize];
    let roperp: f32 =
        orig[(1) as usize] * ray[(0) as usize] - orig[(0) as usize] * ray[(1) as usize];
    let a: f32 = q0perp - ((2) as f32) * q1perp + q2perp;
    let b: f32 = q1perp - q0perp;
    let c: f32 = q0perp - roperp;
    let mut s0: f32 = (0.32) as f32;
    let mut s1: f32 = (0.32) as f32;
    let mut num_s: i32 = 0;
    if ((a) as f64) != 0.032 {
        let discr: f32 = b * b - a * c;
        if ((discr) as f64) > 0.032 {
            let rcpna: f32 = ((-1) as f32) / a;
            let d: f32 = c_runtime::sqrt(discr);
            s0 = ((b + d) * rcpna) as f32;
            s1 = ((b - d) * rcpna) as f32;
            if ((s0) as f64) >= 0.032 && ((s0) as f64) <= 1.032 {
                num_s = (1) as i32;
            }
            if ((d) as f64) > 0.032 && ((s1) as f64) >= 0.032 && ((s1) as f64) <= 1.032 {
                if num_s == 0 {
                    s0 = (s1) as f32;
                }
                c_runtime::preInc(&mut num_s);
            }
        }
    } else {
        s0 = c / (((-2) as f32) * b);
        if ((s0) as f64) >= 0.032 && ((s0) as f64) <= 1.032 {
            num_s = (1) as i32;
        }
    }
    if num_s == 0 {
        return (0) as i32;
    } else {
        let rcp_len2: f32 = ((1) as f32)
            / (ray[(0) as usize] * ray[(0) as usize] + ray[(1) as usize] * ray[(1) as usize]);
        let rayn_x: f32 = ray[(0) as usize] * rcp_len2;
        let rayn_y: f32 = ray[(1) as usize] * rcp_len2;
        let q0d: f32 = q0[(0) as usize] * rayn_x + q0[(1) as usize] * rayn_y;
        let q1d: f32 = q1[(0) as usize] * rayn_x + q1[(1) as usize] * rayn_y;
        let q2d: f32 = q2[(0) as usize] * rayn_x + q2[(1) as usize] * rayn_y;
        let rod: f32 = orig[(0) as usize] * rayn_x + orig[(1) as usize] * rayn_y;
        let q10d: f32 = q1d - q0d;
        let q20d: f32 = q2d - q0d;
        let q0rd: f32 = q0d - rod;
        hits[(0) as usize][(0) as usize] =
            (q0rd + s0 * (2.0f32 - 2.0f32 * s0) * q10d + s0 * s0 * q20d) as f32;
        hits[(0) as usize][(1) as usize] = (a * s0 + b) as f32;
        if num_s > 1 {
            hits[(1) as usize][(0) as usize] =
                (q0rd + s1 * (2.0f32 - 2.0f32 * s1) * q10d + s1 * s1 * q20d) as f32;
            hits[(1) as usize][(1) as usize] = (a * s1 + b) as f32;
            return (2) as i32;
        } else {
            return (1) as i32;
        }
    }
}

pub unsafe fn stbtt__sized_trapezoid_area(height: f32, top_width: f32, bottom_width: f32) -> f32 {
    return ((top_width + bottom_width) / 2.0f32 * height) as f32;
}

pub unsafe fn stbtt__sized_triangle_area(height: f32, width: f32) -> f32 {
    return height * width / ((2) as f32);
}

pub unsafe fn stbtt__solve_cubic(a: f32, b: f32, c: f32, r: *mut f32) -> i32 {
    let s: f32 = -a / ((3) as f32);
    let p: f32 = b - a * a / ((3) as f32);
    let q: f32 = a * (((2) as f32) * a * a - ((9) as f32) * b) / ((27) as f32) + c;
    let p3: f32 = p * p * p;
    let d: f32 = q * q + ((4) as f32) * p3 / ((27) as f32);
    if d >= ((0) as f32) {
        let z: f32 = c_runtime::sqrt(d);
        let mut u: f32 = (-q + z) / ((2) as f32);
        let mut v: f32 = (-q - z) / ((2) as f32);
        u = (stbtt__cuberoot(u)) as f32;
        v = (stbtt__cuberoot(v)) as f32;
        *r.offset((0) as isize) = (s + u + v) as f32;
        return (1) as i32;
    } else {
        let u: f32 = (c_runtime::sqrt((-p / ((3) as f32)) as f32)) as f32;
        let v: f32 = ((c_runtime::acos(
            -c_runtime::sqrt((((-27) as f32) / p3) as f32) * ((q) as f32) / ((2) as f32),
        )) as f32)
            / ((3) as f32);
        let m: f32 = (c_runtime::cos((v) as f32)) as f32;
        let n: f32 =
            ((c_runtime::cos(((v) as f32) - 3.14159232 / ((2) as f32))) as f32) * 1.732050808f32;
        *r.offset((0) as isize) = s + u * ((2) as f32) * m;
        *r.offset((1) as isize) = (s - u * (m + n)) as f32;
        *r.offset((2) as isize) = (s - u * (m - n)) as f32;
        return (3) as i32;
    }
}

pub unsafe fn stbtt__tesselate_cubic(
    points: *mut stbtt__point,
    num_points: *mut i32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    x3: f32,
    y3: f32,
    objspace_flatness_squared: f32,
    n: i32,
) {
    let dx0: f32 = x1 - x0;
    let dy0: f32 = y1 - y0;
    let dx1: f32 = x2 - x1;
    let dy1: f32 = y2 - y1;
    let dx2: f32 = x3 - x2;
    let dy2: f32 = y3 - y2;
    let dx: f32 = x3 - x0;
    let dy: f32 = y3 - y0;
    let longlen: f32 = (c_runtime::sqrt((dx0 * dx0 + dy0 * dy0) as f32)
        + c_runtime::sqrt((dx1 * dx1 + dy1 * dy1) as f32)
        + c_runtime::sqrt((dx2 * dx2 + dy2 * dy2) as f32)) as f32;
    let shortlen: f32 = (c_runtime::sqrt((dx * dx + dy * dy) as f32)) as f32;
    let flatness_squared: f32 = longlen * longlen - shortlen * shortlen;
    if n > 16 {
        return;
    }
    if flatness_squared > objspace_flatness_squared {
        let x01: f32 = (x0 + x1) / ((2) as f32);
        let y01: f32 = (y0 + y1) / ((2) as f32);
        let x12: f32 = (x1 + x2) / ((2) as f32);
        let y12: f32 = (y1 + y2) / ((2) as f32);
        let x23: f32 = (x2 + x3) / ((2) as f32);
        let y23: f32 = (y2 + y3) / ((2) as f32);
        let xa: f32 = (x01 + x12) / ((2) as f32);
        let ya: f32 = (y01 + y12) / ((2) as f32);
        let xb: f32 = (x12 + x23) / ((2) as f32);
        let yb: f32 = (y12 + y23) / ((2) as f32);
        let mx: f32 = (xa + xb) / ((2) as f32);
        let my: f32 = (ya + yb) / ((2) as f32);
        stbtt__tesselate_cubic(
            points,
            num_points,
            x0,
            y0,
            x01,
            y01,
            xa,
            ya,
            mx,
            my,
            objspace_flatness_squared,
            n + 1,
        );
        stbtt__tesselate_cubic(
            points,
            num_points,
            mx,
            my,
            xb,
            yb,
            x23,
            y23,
            x3,
            y3,
            objspace_flatness_squared,
            n + 1,
        );
    } else {
        stbtt__add_point(points, *num_points, x3, y3);
        *num_points = (*num_points + 1) as i32;
    }
}

pub unsafe fn stbtt__tesselate_curve(
    points: *mut stbtt__point,
    num_points: *mut i32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    objspace_flatness_squared: f32,
    n: i32,
) -> i32 {
    let mx: f32 = (x0 + ((2) as f32) * x1 + x2) / ((4) as f32);
    let my: f32 = (y0 + ((2) as f32) * y1 + y2) / ((4) as f32);
    let dx: f32 = (x0 + x2) / ((2) as f32) - mx;
    let dy: f32 = (y0 + y2) / ((2) as f32) - my;
    if n > 16 {
        return (1) as i32;
    }
    if dx * dx + dy * dy > objspace_flatness_squared {
        stbtt__tesselate_curve(
            points,
            num_points,
            x0,
            y0,
            (x0 + x1) / 2.0f32,
            (y0 + y1) / 2.0f32,
            mx,
            my,
            objspace_flatness_squared,
            n + 1,
        );
        stbtt__tesselate_curve(
            points,
            num_points,
            mx,
            my,
            (x1 + x2) / 2.0f32,
            (y1 + y2) / 2.0f32,
            x2,
            y2,
            objspace_flatness_squared,
            n + 1,
        );
    } else {
        stbtt__add_point(points, *num_points, x2, y2);
        *num_points = (*num_points + 1) as i32;
    }
    return (1) as i32;
}

pub unsafe fn stbtt__v_prefilter(
    mut pixels: *mut u8,
    w: i32,
    h: i32,
    stride_in_bytes: i32,
    kernel_width: u32,
) {
    let mut buffer: [u8; 8] = [0; 8];
    let safe_h: i32 = (((h) as u32) - kernel_width) as i32;
    let mut j: i32 = 0;
    c_runtime::memset((buffer.as_mut_ptr()) as *mut u8, 0, (8) as u64);
    j = (0) as i32;
    while j < w {
        let mut i: i32 = 0;
        let mut total: u32 = 0;
        c_runtime::memset((buffer.as_mut_ptr()) as *mut u8, 0, (kernel_width) as u64);
        total = (0) as u32;
        if kernel_width == ((2) as u32) {
            i = (0) as i32;
            while i <= safe_h {
                total += (((*pixels.offset((i * stride_in_bytes) as isize)) as i32)
                    - ((buffer[(i & (8 - 1)) as usize]) as i32)) as u32;
                buffer[((((i) as u32) + kernel_width) & ((8 - 1) as u32)) as usize] =
                    (*pixels.offset((i * stride_in_bytes) as isize)) as u8;
                *pixels.offset((i * stride_in_bytes) as isize) = (total / ((2) as u32)) as u8;
                c_runtime::preInc(&mut i);
            }
        } else if kernel_width == ((3) as u32) {
            i = (0) as i32;
            while i <= safe_h {
                total += (((*pixels.offset((i * stride_in_bytes) as isize)) as i32)
                    - ((buffer[(i & (8 - 1)) as usize]) as i32)) as u32;
                buffer[((((i) as u32) + kernel_width) & ((8 - 1) as u32)) as usize] =
                    (*pixels.offset((i * stride_in_bytes) as isize)) as u8;
                *pixels.offset((i * stride_in_bytes) as isize) = (total / ((3) as u32)) as u8;
                c_runtime::preInc(&mut i);
            }
        } else if kernel_width == ((4) as u32) {
            i = (0) as i32;
            while i <= safe_h {
                total += (((*pixels.offset((i * stride_in_bytes) as isize)) as i32)
                    - ((buffer[(i & (8 - 1)) as usize]) as i32)) as u32;
                buffer[((((i) as u32) + kernel_width) & ((8 - 1) as u32)) as usize] =
                    (*pixels.offset((i * stride_in_bytes) as isize)) as u8;
                *pixels.offset((i * stride_in_bytes) as isize) = (total / ((4) as u32)) as u8;
                c_runtime::preInc(&mut i);
            }
        } else if kernel_width == ((5) as u32) {
            i = (0) as i32;
            while i <= safe_h {
                total += (((*pixels.offset((i * stride_in_bytes) as isize)) as i32)
                    - ((buffer[(i & (8 - 1)) as usize]) as i32)) as u32;
                buffer[((((i) as u32) + kernel_width) & ((8 - 1) as u32)) as usize] =
                    (*pixels.offset((i * stride_in_bytes) as isize)) as u8;
                *pixels.offset((i * stride_in_bytes) as isize) = (total / ((5) as u32)) as u8;
                c_runtime::preInc(&mut i);
            }
        } else {
            i = (0) as i32;
            while i <= safe_h {
                total += (((*pixels.offset((i * stride_in_bytes) as isize)) as i32)
                    - ((buffer[(i & (8 - 1)) as usize]) as i32)) as u32;
                buffer[((((i) as u32) + kernel_width) & ((8 - 1) as u32)) as usize] =
                    (*pixels.offset((i * stride_in_bytes) as isize)) as u8;
                *pixels.offset((i * stride_in_bytes) as isize) = (total / kernel_width) as u8;
                c_runtime::preInc(&mut i);
            }
        };
        while i < h {
            total -= (buffer[(i & (8 - 1)) as usize]) as u32;
            *pixels.offset((i * stride_in_bytes) as isize) = (total / kernel_width) as u8;
            c_runtime::preInc(&mut i);
        }
        pixels = pixels.offset((1) as isize);
        c_runtime::preInc(&mut j);
    }
}

pub unsafe fn stbtt_BakeFontBitmap(
    data: *const u8,
    offset: i32,
    pixel_height: f32,
    pixels: *mut u8,
    pw: i32,
    ph: i32,
    first_char: i32,
    num_chars: i32,
    chardata: *mut stbtt_bakedchar,
) -> i32 {
    return (stbtt_BakeFontBitmap_internal(
        data,
        offset,
        pixel_height,
        pixels,
        pw,
        ph,
        first_char,
        num_chars,
        chardata,
    )) as i32;
}

pub unsafe fn stbtt_BakeFontBitmap_internal(
    data: *const u8,
    offset: i32,
    pixel_height: f32,
    pixels: *mut u8,
    pw: i32,
    ph: i32,
    first_char: i32,
    num_chars: i32,
    chardata: *mut stbtt_bakedchar,
) -> i32 {
    let mut scale: f32 = 0.0f32;
    let mut x: i32 = 0;
    let mut y: i32 = 0;
    let mut bottom_y: i32 = 0;
    let mut i: i32 = 0;
    let mut f: stbtt_fontinfo = stbtt_fontinfo::default();
    f.userdata = core::ptr::null_mut();
    if stbtt_InitFont((&mut f) as *mut stbtt_fontinfo, data, offset) == 0 {
        return (-1) as i32;
    }
    c_runtime::memset(pixels, 0, (pw * ph) as u64);
    let hebron_tmp0 = 1;
    x = hebron_tmp0;
    y = hebron_tmp0;
    bottom_y = (1) as i32;
    scale = (stbtt_ScaleForPixelHeight((&mut f) as *mut stbtt_fontinfo, pixel_height)) as f32;
    i = (0) as i32;
    while i < num_chars {
        let mut advance: i32 = 0;
        let mut lsb: i32 = 0;
        let mut x0: i32 = 0;
        let mut y0: i32 = 0;
        let mut x1: i32 = 0;
        let mut y1: i32 = 0;
        let mut gw: i32 = 0;
        let mut gh: i32 = 0;
        let g: i32 = stbtt_FindGlyphIndex((&mut f) as *mut stbtt_fontinfo, first_char + i);
        stbtt_GetGlyphHMetrics(
            (&mut f) as *mut stbtt_fontinfo,
            g,
            (&mut advance) as *mut i32,
            (&mut lsb) as *mut i32,
        );
        stbtt_GetGlyphBitmapBox(
            (&mut f) as *mut stbtt_fontinfo,
            g,
            scale,
            scale,
            (&mut x0) as *mut i32,
            (&mut y0) as *mut i32,
            (&mut x1) as *mut i32,
            (&mut y1) as *mut i32,
        );
        gw = (x1 - x0) as i32;
        gh = (y1 - y0) as i32;
        if x + gw + 1 >= pw {
            y = (bottom_y) as i32;
            x = (1) as i32;
        }
        if y + gh + 1 >= ph {
            return (-i) as i32;
        }
        stbtt_MakeGlyphBitmap(
            (&mut f) as *mut stbtt_fontinfo,
            ((pixels).offset((x) as isize)).offset((y * pw) as isize),
            gw,
            gh,
            pw,
            scale,
            scale,
            g,
        );
        (*chardata.offset((i) as isize)).x0 = ((x) as i16) as u16;
        (*chardata.offset((i) as isize)).y0 = ((y) as i16) as u16;
        (*chardata.offset((i) as isize)).x1 = ((x + gw) as i16) as u16;
        (*chardata.offset((i) as isize)).y1 = ((y + gh) as i16) as u16;
        (*chardata.offset((i) as isize)).xadvance = scale * ((advance) as f32);
        (*chardata.offset((i) as isize)).xoff = (x0) as f32;
        (*chardata.offset((i) as isize)).yoff = (y0) as f32;
        x = (x + gw + 1) as i32;
        if y + gh + 1 > bottom_y {
            bottom_y = (y + gh + 1) as i32;
        }
        c_runtime::preInc(&mut i);
    }
    return (bottom_y) as i32;
}

pub unsafe fn stbtt_CompareUTF8toUTF16_bigendian(
    s1: *mut i8,
    len1: i32,
    s2: *mut i8,
    len2: i32,
) -> i32 {
    return (stbtt_CompareUTF8toUTF16_bigendian_internal(s1, len1, s2, len2)) as i32;
}

pub unsafe fn stbtt_CompareUTF8toUTF16_bigendian_internal(
    s1: *mut i8,
    len1: i32,
    s2: *mut i8,
    len2: i32,
) -> i32 {
    return (if len1
        == stbtt__CompareUTF8toUTF16_bigendian_prefix(
            ((s1) as *mut u8) as *mut u8,
            len1,
            ((s2) as *mut u8) as *mut u8,
            len2,
        ) {
        1
    } else {
        0
    }) as i32;
}

pub unsafe fn stbtt_FindMatchingFont(fontdata: *const u8, name: *mut i8, flags: i32) -> i32 {
    return (stbtt_FindMatchingFont_internal(fontdata, name, flags)) as i32;
}

pub unsafe fn stbtt_FindMatchingFont_internal(
    font_collection: *const u8,
    name_utf8: *mut i8,
    flags: i32,
) -> i32 {
    let mut i: i32 = 0;
    i = (0) as i32;
    loop {
        let off: i32 = stbtt_GetFontOffsetForIndex(font_collection, i);
        if off < 0 {
            return (off) as i32;
        }
        if (stbtt__matches(
            (font_collection) as *mut u8,
            (off) as u32,
            ((name_utf8) as *mut u8) as *mut u8,
            flags,
        )) != 0
        {
            return (off) as i32;
        }
        c_runtime::preInc(&mut i);
    }

    return 0;
}

pub unsafe fn stbtt_FlattenCurves(
    vertices: *mut stbtt_vertex,
    num_verts: i32,
    objspace_flatness: f32,
    contour_lengths: *mut *mut i32,
    num_contours: *mut i32,
    _userdata: *const u8,
) -> *mut stbtt__point {
    let mut points: *mut stbtt__point = core::ptr::null_mut();
    let mut num_points: i32 = 0;
    let objspace_flatness_squared: f32 = objspace_flatness * objspace_flatness;
    let mut i: i32 = 0;
    let mut n: i32 = 0;
    let mut start: i32 = 0;
    let mut pass: i32 = 0;
    i = (0) as i32;
    while i < num_verts {
        if (((*vertices.offset((i) as isize))._type_) as i32) == STBTT_vmove {
            c_runtime::preInc(&mut n);
        }
        c_runtime::preInc(&mut i);
    }
    *num_contours = (n) as i32;
    if n == 0 {
        return core::ptr::null_mut();
    }
    *contour_lengths =
        (c_runtime::malloc(core::mem::size_of::<i32>() as u64 * ((n) as u64))) as *mut i32;
    if *contour_lengths == ((core::ptr::null_mut()) as *mut i32) {
        *num_contours = (0) as i32;
        return core::ptr::null_mut();
    }
    pass = (0) as i32;

    'outer: loop {
        while pass < 2 {
            let mut x: f32 = (0) as f32;
            let mut y: f32 = (0) as f32;
            if pass == 1 {
                points = (c_runtime::malloc(
                    ((num_points) as u64) * core::mem::size_of::<stbtt__point>() as u64,
                )) as *mut stbtt__point;
                if points == core::ptr::null_mut() {
                    break 'outer;
                }
            }
            num_points = (0) as i32;
            n = (-1) as i32;
            i = (0) as i32;
            while i < num_verts {
                if (((*vertices.offset((i) as isize))._type_) as i32) == STBTT_vmove {
                    if n >= 0 {
                        *(*contour_lengths).offset((n) as isize) = (num_points - start) as i32;
                    }
                    c_runtime::preInc(&mut n);
                    start = (num_points) as i32;
                    x = ((*vertices.offset((i) as isize)).x) as f32;
                    y = ((*vertices.offset((i) as isize)).y) as f32;
                    stbtt__add_point(points, c_runtime::postInc(&mut num_points), x, y);
                } else if (((*vertices.offset((i) as isize))._type_) as i32) == STBTT_vline {
                    x = ((*vertices.offset((i) as isize)).x) as f32;
                    y = ((*vertices.offset((i) as isize)).y) as f32;
                    stbtt__add_point(points, c_runtime::postInc(&mut num_points), x, y);
                } else if (((*vertices.offset((i) as isize))._type_) as i32) == STBTT_vcurve {
                    stbtt__tesselate_curve(
                        points,
                        (&mut num_points) as *mut i32,
                        x,
                        y,
                        ((*vertices.offset((i) as isize)).cx) as f32,
                        ((*vertices.offset((i) as isize)).cy) as f32,
                        ((*vertices.offset((i) as isize)).x) as f32,
                        ((*vertices.offset((i) as isize)).y) as f32,
                        objspace_flatness_squared,
                        0,
                    );
                    x = ((*vertices.offset((i) as isize)).x) as f32;
                    y = ((*vertices.offset((i) as isize)).y) as f32;
                } else if (((*vertices.offset((i) as isize))._type_) as i32) == STBTT_vcubic {
                    stbtt__tesselate_cubic(
                        points,
                        (&mut num_points) as *mut i32,
                        x,
                        y,
                        ((*vertices.offset((i) as isize)).cx) as f32,
                        ((*vertices.offset((i) as isize)).cy) as f32,
                        ((*vertices.offset((i) as isize)).cx1) as f32,
                        ((*vertices.offset((i) as isize)).cy1) as f32,
                        ((*vertices.offset((i) as isize)).x) as f32,
                        ((*vertices.offset((i) as isize)).y) as f32,
                        objspace_flatness_squared,
                        0,
                    );
                    x = ((*vertices.offset((i) as isize)).x) as f32;
                    y = ((*vertices.offset((i) as isize)).y) as f32;
                }
                c_runtime::preInc(&mut i);
            }
            *(*contour_lengths).offset((n) as isize) = (num_points - start) as i32;
            c_runtime::preInc(&mut pass);
        }
        return points;
    }
    c_runtime::free((points) as *mut u8);
    c_runtime::free((*contour_lengths) as *mut u8);
    *contour_lengths = (core::ptr::null_mut()) as *mut i32;
    *num_contours = (0) as i32;
    return core::ptr::null_mut();
}

pub unsafe fn stbtt_FreeBitmap(bitmap: *mut u8, _userdata: *const u8) {
    c_runtime::free(bitmap);
}

pub unsafe fn stbtt_FreeSDF(bitmap: *mut u8, _userdata: *const u8) {
    c_runtime::free(bitmap);
}

pub unsafe fn stbtt_GetBakedQuad(
    chardata: *mut stbtt_bakedchar,
    pw: i32,
    ph: i32,
    char_index: i32,
    xpos: *mut f32,
    ypos: *mut f32,
    q: *mut stbtt_aligned_quad,
    opengl_fillrule: i32,
) {
    let d3d_bias: f32 = if (opengl_fillrule) != 0 {
        (0) as f32
    } else {
        -0.5f32
    };
    let ipw: f32 = 1.0f32 / ((pw) as f32);
    let iph: f32 = 1.0f32 / ((ph) as f32);
    let b: *mut stbtt_bakedchar = (chardata).offset((char_index) as isize);
    let round_x: i32 = (c_runtime::floor(((*xpos + (*b).xoff) + 0.5f32) as f32)) as i32;
    let round_y: i32 = (c_runtime::floor(((*ypos + (*b).yoff) + 0.5f32) as f32)) as i32;
    (*q).x0 = ((round_x) as f32) + d3d_bias;
    (*q).y0 = ((round_y) as f32) + d3d_bias;
    (*q).x1 = ((round_x + (((*b).x1) as i32) - (((*b).x0) as i32)) as f32) + d3d_bias;
    (*q).y1 = ((round_y + (((*b).y1) as i32) - (((*b).y0) as i32)) as f32) + d3d_bias;
    (*q).s0 = ((((*b).x0) as i32) as f32) * ipw;
    (*q).t0 = ((((*b).y0) as i32) as f32) * iph;
    (*q).s1 = ((((*b).x1) as i32) as f32) * ipw;
    (*q).t1 = ((((*b).y1) as i32) as f32) * iph;
    *xpos += ((*b).xadvance) as f32;
}

pub unsafe fn stbtt_GetFontOffsetForIndex(data: *const u8, index: i32) -> i32 {
    return (stbtt_GetFontOffsetForIndex_internal(data, index)) as i32;
}

pub unsafe fn stbtt_GetFontOffsetForIndex_internal(font_collection: *const u8, index: i32) -> i32 {
    if (stbtt__isfont(font_collection)) != 0 {
        return (if index == 0 { 0 } else { -1 }) as i32;
    }
    if ((*(font_collection).offset((0) as isize)) as i32) == ('t' as i32)
        && ((*(font_collection).offset((1) as isize)) as i32) == ('t' as i32)
        && ((*(font_collection).offset((2) as isize)) as i32) == ('c' as i32)
        && ((*(font_collection).offset((3) as isize)) as i32) == ('f' as i32)
    {
        if ttULONG((font_collection).offset((4) as isize)) == ((0x00010000) as u32)
            || ttULONG((font_collection).offset((4) as isize)) == ((0x00020000) as u32)
        {
            let n: i32 = ttLONG((font_collection).offset((8) as isize));
            if index >= n {
                return (-1) as i32;
            }
            return (ttULONG(((font_collection).offset((12) as isize)).offset((index * 4) as isize)))
                as i32;
        }
    }
    return (-1) as i32;
}

pub unsafe fn stbtt_GetNumberOfFonts(data: *const u8) -> i32 {
    return (stbtt_GetNumberOfFonts_internal(data)) as i32;
}

pub unsafe fn stbtt_GetNumberOfFonts_internal(font_collection: *const u8) -> i32 {
    if (stbtt__isfont(font_collection)) != 0 {
        return (1) as i32;
    }
    if ((*(font_collection).offset((0) as isize)) as i32) == ('t' as i32)
        && ((*(font_collection).offset((1) as isize)) as i32) == ('t' as i32)
        && ((*(font_collection).offset((2) as isize)) as i32) == ('c' as i32)
        && ((*(font_collection).offset((3) as isize)) as i32) == ('f' as i32)
    {
        if ttULONG((font_collection).offset((4) as isize)) == ((0x00010000) as u32)
            || ttULONG((font_collection).offset((4) as isize)) == ((0x00020000) as u32)
        {
            return (ttLONG((font_collection).offset((8) as isize))) as i32;
        }
    }
    return (0) as i32;
}

pub unsafe fn stbtt_GetPackedQuad(
    chardata: *mut stbtt_packedchar,
    pw: i32,
    ph: i32,
    char_index: i32,
    xpos: *mut f32,
    ypos: *mut f32,
    q: *mut stbtt_aligned_quad,
    align_to_integer: i32,
) {
    let ipw: f32 = 1.0f32 / ((pw) as f32);
    let iph: f32 = 1.0f32 / ((ph) as f32);
    let b: *mut stbtt_packedchar = (chardata).offset((char_index) as isize);
    if (align_to_integer) != 0 {
        let x: f32 = ((c_runtime::floor(((*xpos + (*b).xoff) + 0.5f32) as f32)) as i32) as f32;
        let y: f32 = ((c_runtime::floor(((*ypos + (*b).yoff) + 0.5f32) as f32)) as i32) as f32;
        (*q).x0 = (x) as f32;
        (*q).y0 = (y) as f32;
        (*q).x1 = (x + (*b).xoff2 - (*b).xoff) as f32;
        (*q).y1 = (y + (*b).yoff2 - (*b).yoff) as f32;
    } else {
        (*q).x0 = (*xpos + (*b).xoff) as f32;
        (*q).y0 = (*ypos + (*b).yoff) as f32;
        (*q).x1 = (*xpos + (*b).xoff2) as f32;
        (*q).y1 = (*ypos + (*b).yoff2) as f32;
    }
    (*q).s0 = ((((*b).x0) as i32) as f32) * ipw;
    (*q).t0 = ((((*b).y0) as i32) as f32) * iph;
    (*q).s1 = ((((*b).x1) as i32) as f32) * ipw;
    (*q).t1 = ((((*b).y1) as i32) as f32) * iph;
    *xpos += ((*b).xadvance) as f32;
}

pub unsafe fn stbtt_GetScaledFontVMetrics(
    fontdata: *const u8,
    index: i32,
    size: f32,
    ascent: *mut f32,
    descent: *mut f32,
    lineGap: *mut f32,
) {
    let mut i_ascent: i32 = 0;
    let mut i_descent: i32 = 0;
    let mut i_lineGap: i32 = 0;
    let mut scale: f32 = 0.0f32;
    let mut info: stbtt_fontinfo = stbtt_fontinfo::default();
    stbtt_InitFont(
        (&mut info) as *mut stbtt_fontinfo,
        fontdata,
        (stbtt_GetFontOffsetForIndex(fontdata, index)) as i32,
    );
    scale = if size > ((0) as f32) {
        stbtt_ScaleForPixelHeight((&mut info) as *mut stbtt_fontinfo, size)
    } else {
        stbtt_ScaleForMappingEmToPixels((&mut info) as *mut stbtt_fontinfo, -size)
    };
    stbtt_GetFontVMetrics(
        (&mut info) as *mut stbtt_fontinfo,
        (&mut i_ascent) as *mut i32,
        (&mut i_descent) as *mut i32,
        (&mut i_lineGap) as *mut i32,
    );
    *ascent = ((i_ascent) as f32) * scale;
    *descent = ((i_descent) as f32) * scale;
    *lineGap = ((i_lineGap) as f32) * scale;
}

pub unsafe fn stbtt_setvertex(
    v: *mut stbtt_vertex,
    mut _type_: u8,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
) {
    (*v)._type_ = (_type_) as u8;
    (*v).x = (x) as i16;
    (*v).y = (y) as i16;
    (*v).cx = (cx) as i16;
    (*v).cy = (cy) as i16;
}

pub unsafe fn ttLONG(p: *const u8) -> i32 {
    return (((*p.offset((0) as isize)) as i32) << 24)
        + (((*p.offset((1) as isize)) as i32) << 16)
        + (((*p.offset((2) as isize)) as i32) << 8)
        + ((*p.offset((3) as isize)) as i32);
}

pub unsafe fn ttSHORT(p: *const u8) -> i16 {
    return (((*p.offset((0) as isize)) as i32) * 256 + ((*p.offset((1) as isize)) as i32)) as i16;
}

pub unsafe fn ttULONG(p: *const u8) -> u32 {
    return ((((*p.offset((0) as isize)) as i32) << 24)
        + (((*p.offset((1) as isize)) as i32) << 16)
        + (((*p.offset((2) as isize)) as i32) << 8)
        + ((*p.offset((3) as isize)) as i32)) as u32;
}

pub unsafe fn ttUSHORT(p: *const u8) -> u16 {
    return (((*p.offset((0) as isize)) as i32) * 256 + ((*p.offset((1) as isize)) as i32)) as u16;
}
