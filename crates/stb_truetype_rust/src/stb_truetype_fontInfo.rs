

use crate::*;
use c_runtime;

#[derive(Debug, Copy, Clone)]
pub struct stbtt_fontinfo {
    pub userdata: *mut u8,
    pub data: *const u8,
    pub fontstart: i32,
    pub numGlyphs: i32,
    pub loca: i32,
    pub head: i32,
    pub glyf: i32,
    pub hhea: i32,
    pub hmtx: i32,
    pub kern: i32,
    pub gpos: i32,
    pub svg: i32,
    pub index_map: i32,
    pub indexToLocFormat: i32,
    pub cff: stbtt__buf,
    pub charstrings: stbtt__buf,
    pub gsubrs: stbtt__buf,
    pub subrs: stbtt__buf,
    pub fontdicts: stbtt__buf,
    pub fdselect: stbtt__buf,
}

#[derive(Debug, Copy, Clone)]
pub struct stbtt_kerningentry {
    pub glyph1: i32,
    pub glyph2: i32,
    pub advance: i32,
}

impl core::default::Default for stbtt_fontinfo {
    fn default() -> Self {
        stbtt_fontinfo {
            userdata: core::ptr::null_mut(),
            data: core::ptr::null_mut(),
            fontstart: 0,
            numGlyphs: 0,
            loca: 0,
            head: 0,
            glyf: 0,
            hhea: 0,
            hmtx: 0,
            kern: 0,
            gpos: 0,
            svg: 0,
            index_map: 0,
            indexToLocFormat: 0,
            cff: stbtt__buf::default(),
            charstrings: stbtt__buf::default(),
            gsubrs: stbtt__buf::default(),
            subrs: stbtt__buf::default(),
            fontdicts: stbtt__buf::default(),
            fdselect: stbtt__buf::default(),
        }
    }
}

impl core::default::Default for stbtt_kerningentry {
    fn default() -> Self {
        stbtt_kerningentry {
            glyph1: 0,
            glyph2: 0,
            advance: 0,
        }
    }
}

pub unsafe fn stbtt__cid_get_glyph_subrs(
    mut info: *mut stbtt_fontinfo,
    mut glyph_index: i32,
) -> stbtt__buf {
    let mut fdselect: stbtt__buf = (*info).fdselect;
    let mut nranges: i32 = 0;
    let mut start: i32 = 0;
    let mut end: i32 = 0;
    let mut v: i32 = 0;
    let mut fmt: i32 = 0;
    let mut fdselector: i32 = -1;
    let mut i: i32 = 0;
    stbtt__buf_seek(((&mut fdselect) as *mut stbtt__buf), 0);
    fmt = ((stbtt__buf_get8(((&mut fdselect) as *mut stbtt__buf))) as i32);
    if fmt == 0 {
        stbtt__buf_skip(((&mut fdselect) as *mut stbtt__buf), glyph_index);
        fdselector = ((stbtt__buf_get8(((&mut fdselect) as *mut stbtt__buf))) as i32);
    } else {
        if fmt == 3 {
            nranges = ((stbtt__buf_get(((&mut fdselect) as *mut stbtt__buf), 2)) as i32);
            start = ((stbtt__buf_get(((&mut fdselect) as *mut stbtt__buf), 2)) as i32);
            i = ((0) as i32);
            while (i < nranges) {
                v = ((stbtt__buf_get8(((&mut fdselect) as *mut stbtt__buf))) as i32);
                end = ((stbtt__buf_get(((&mut fdselect) as *mut stbtt__buf), 2)) as i32);
                if glyph_index >= start && glyph_index < end {
                    fdselector = ((v) as i32);
                    break;
                }
                start = ((end) as i32);
                c_runtime::postInc(&mut i);
            }
        }
    }
    if fdselector == -1 {
        stbtt__new_buf(core::ptr::null_mut(), ((0) as u64));
    }
    return stbtt__get_subrs(
        (*info).cff,
        ((stbtt__cff_index_get((*info).fontdicts, fdselector)) as stbtt__buf),
    );
}

pub unsafe fn stbtt__close_shape(
    mut vertices: *mut stbtt_vertex,
    mut num_vertices: i32,
    mut was_off: i32,
    mut start_off: i32,
    mut sx: i32,
    mut sy: i32,
    mut scx: i32,
    mut scy: i32,
    mut cx: i32,
    mut cy: i32,
) -> i32 {
    if (start_off) != 0 {
        if (was_off) != 0 {
            stbtt_setvertex(
                ((&mut *vertices.offset((c_runtime::postInc(&mut num_vertices)) as isize))
                    as *mut stbtt_vertex),
                ((STBTT_vcurve) as u8),
                (cx + scx) >> 1,
                (cy + scy) >> 1,
                cx,
                cy,
            );
        }
        stbtt_setvertex(
            ((&mut *vertices.offset((c_runtime::postInc(&mut num_vertices)) as isize))
                as *mut stbtt_vertex),
            ((STBTT_vcurve) as u8),
            sx,
            sy,
            scx,
            scy,
        );
    } else {
        if (was_off) != 0 {
            stbtt_setvertex(
                ((&mut *vertices.offset((c_runtime::postInc(&mut num_vertices)) as isize))
                    as *mut stbtt_vertex),
                ((STBTT_vcurve) as u8),
                sx,
                sy,
                cx,
                cy,
            );
        } else {
            stbtt_setvertex(
                ((&mut *vertices.offset((c_runtime::postInc(&mut num_vertices)) as isize))
                    as *mut stbtt_vertex),
                ((STBTT_vline) as u8),
                sx,
                sy,
                0,
                0,
            );
        }
    }
    return ((num_vertices) as i32);
}

pub unsafe fn stbtt__get_svg(mut info: *mut stbtt_fontinfo) -> i32 {
    let mut t: u32 = 0;
    if (*info).svg < 0 {
        t = (stbtt__find_table((*info).data, (((*info).fontstart) as u32), "SVG "));
        if (t) != 0 {
            let mut offset: u32 =
                ttULONG((((*info).data).offset((t) as isize)).offset((2) as isize));
            (*info).svg = ((t + offset) as i32);
        } else {
            (*info).svg = ((0) as i32);
        }
    }
    return (((*info).svg) as i32);
}

pub unsafe fn stbtt__GetCoverageIndex(mut coverageTable: *const u8, mut glyph: i32) -> i32 {
    let mut coverageFormat: u16 = ttUSHORT(coverageTable);
    if ((coverageFormat) as i32) == 1 {
        let mut glyphCount: u16 = ttUSHORT((coverageTable).offset((2) as isize));
        let mut l: i32 = 0;
        let mut r: i32 = ((glyphCount) as i32) - 1;
        let mut m: i32 = 0;
        let mut straw: i32 = 0;
        let mut needle: i32 = glyph;
        while (l <= r) {
            let mut glyphArray: *const u8 = (coverageTable).offset((4) as isize);
            let mut glyphID: u16 = 0;
            m = (((l + r) >> 1) as i32);
            glyphID = ((ttUSHORT((glyphArray).offset((2 * m) as isize))) as u16);
            straw = ((glyphID) as i32);
            if needle < straw {
                r = ((m - 1) as i32);
            } else {
                if needle > straw {
                    l = ((m + 1) as i32);
                } else {
                    return ((m) as i32);
                }
            }
        }
    } else if ((coverageFormat) as i32) == 2 {
        let mut rangeCount: u16 = ttUSHORT((coverageTable).offset((2) as isize));
        let mut rangeArray: *const u8 = (coverageTable).offset((4) as isize);
        let mut l: i32 = 0;
        let mut r: i32 = ((rangeCount) as i32) - 1;
        let mut m: i32 = 0;
        let mut strawStart: i32 = 0;
        let mut strawEnd: i32 = 0;
        let mut needle: i32 = glyph;
        while (l <= r) {
            let mut rangeRecord: *const u8 = core::ptr::null_mut();
            m = (((l + r) >> 1) as i32);
            rangeRecord = (rangeArray).offset((6 * m) as isize);
            strawStart = ((ttUSHORT(rangeRecord)) as i32);
            strawEnd = ((ttUSHORT((rangeRecord).offset((2) as isize))) as i32);
            if needle < strawStart {
                r = ((m - 1) as i32);
            } else {
                if needle > strawEnd {
                    l = ((m + 1) as i32);
                } else {
                    let mut startCoverageIndex: u16 = ttUSHORT((rangeRecord).offset((4) as isize));
                    return ((startCoverageIndex) as i32) + glyph - strawStart;
                }
            }
        }
    } else {
        return ((-1) as i32);
    }
    return ((-1) as i32);
}

pub unsafe fn stbtt__GetGlyfOffset(mut info: *mut stbtt_fontinfo, mut glyph_index: i32) -> i32 {
    let mut g1: i32 = 0;
    let mut g2: i32 = 0;

    if glyph_index >= (*info).numGlyphs {
        return ((-1) as i32);
    }
    if (*info).indexToLocFormat >= 2 {
        return ((-1) as i32);
    }
    if (*info).indexToLocFormat == 0 {
        g1 = ((*info).glyf
            + ((ttUSHORT(
                (((*info).data).offset(((*info).loca) as isize)).offset((glyph_index * 2) as isize),
            )) as i32)
                * 2);
        g2 = ((*info).glyf
            + ((ttUSHORT(
                ((((*info).data).offset(((*info).loca) as isize))
                    .offset((glyph_index * 2) as isize))
                .offset((2) as isize),
            )) as i32)
                * 2);
    } else {
        g1 = (((((*info).glyf) as u32)
            + ttULONG(
                (((*info).data).offset(((*info).loca) as isize)).offset((glyph_index * 4) as isize),
            )) as i32);
        g2 = (((((*info).glyf) as u32)
            + ttULONG(
                ((((*info).data).offset(((*info).loca) as isize))
                    .offset((glyph_index * 4) as isize))
                .offset((4) as isize),
            )) as i32);
    }
    return ((if g1 == g2 { -1 } else { g1 }) as i32);
}

pub unsafe fn stbtt__GetGlyphClass(mut classDefTable: *const u8, mut glyph: i32) -> i32 {
    let mut classDefFormat: u16 = ttUSHORT(classDefTable);
    if ((classDefFormat) as i32) == 1 {
        let mut startGlyphID: u16 = ttUSHORT((classDefTable).offset((2) as isize));
        let mut glyphCount: u16 = ttUSHORT((classDefTable).offset((4) as isize));
        let mut classDef1ValueArray: *const u8 = (classDefTable).offset((6) as isize);
        if glyph >= ((startGlyphID) as i32)
            && glyph < ((startGlyphID) as i32) + ((glyphCount) as i32)
        {
            return ((ttUSHORT(
                (classDef1ValueArray).offset((2 * (glyph - ((startGlyphID) as i32))) as isize),
            )) as i32);
        }
    } else if ((classDefFormat) as i32) == 2 {
        let mut classRangeCount: u16 = ttUSHORT((classDefTable).offset((2) as isize));
        let mut classRangeRecords: *const u8 = (classDefTable).offset((4) as isize);
        let mut l: i32 = 0;
        let mut r: i32 = ((classRangeCount) as i32) - 1;
        let mut m: i32 = 0;
        let mut strawStart: i32 = 0;
        let mut strawEnd: i32 = 0;
        let mut needle: i32 = glyph;
        while (l <= r) {
            let mut classRangeRecord: *const u8 = core::ptr::null_mut();
            m = (((l + r) >> 1) as i32);
            classRangeRecord = (classRangeRecords).offset((6 * m) as isize);
            strawStart = ((ttUSHORT(classRangeRecord)) as i32);
            strawEnd = ((ttUSHORT((classRangeRecord).offset((2) as isize))) as i32);
            if needle < strawStart {
                r = ((m - 1) as i32);
            } else {
                if needle > strawEnd {
                    l = ((m + 1) as i32);
                } else {
                    return ((ttUSHORT((classRangeRecord).offset((4) as isize))) as i32);
                }
            }
        }
    } else {
        return ((-1) as i32);
    }
    return ((0) as i32);
}

pub unsafe fn stbtt__GetGlyphGPOSInfoAdvance(
    mut info: *mut stbtt_fontinfo,
    mut glyph1: i32,
    mut glyph2: i32,
) -> i32 {
    let mut lookupListOffset: u16 = 0;
    let mut lookupList: *const u8 = core::ptr::null_mut();
    let mut lookupCount: u16 = 0;
    let mut data: *const u8 = core::ptr::null_mut();
    let mut i: i32 = 0;
    let mut sti: i32 = 0;
    if (*info).gpos == 0 {
        return ((0) as i32);
    }
    data = ((*info).data).offset(((*info).gpos) as isize);
    if ((ttUSHORT((data).offset((0) as isize))) as i32) != 1 {
        return ((0) as i32);
    }
    if ((ttUSHORT((data).offset((2) as isize))) as i32) != 0 {
        return ((0) as i32);
    }
    lookupListOffset = ((ttUSHORT((data).offset((8) as isize))) as u16);
    lookupList = (data).offset(((lookupListOffset) as i32) as isize);
    lookupCount = ((ttUSHORT(lookupList)) as u16);
    i = ((0) as i32);
    while (i < ((lookupCount) as i32)) {
        let mut lookupOffset: u16 =
            ttUSHORT(((lookupList).offset((2) as isize)).offset((2 * i) as isize));
        let mut lookupTable: *const u8 = (lookupList).offset(((lookupOffset) as i32) as isize);
        let mut lookupType: u16 = ttUSHORT(lookupTable);
        let mut subTableCount: u16 = ttUSHORT((lookupTable).offset((4) as isize));
        let mut subTableOffsets: *const u8 = (lookupTable).offset((6) as isize);
        if ((lookupType) as i32) != 2 {
            c_runtime::preInc(&mut i);
            continue;
        }
        sti = ((0) as i32);
        while (sti < ((subTableCount) as i32)) {
            let mut subtableOffset: u16 = ttUSHORT((subTableOffsets).offset((2 * sti) as isize));
            let mut table: *const u8 = (lookupTable).offset(((subtableOffset) as i32) as isize);
            let mut posFormat: u16 = ttUSHORT(table);
            let mut coverageOffset: u16 = ttUSHORT((table).offset((2) as isize));
            let mut coverageIndex: i32 =
                stbtt__GetCoverageIndex((table).offset(((coverageOffset) as i32) as isize), glyph1);
            if coverageIndex == -1 {
                c_runtime::postInc(&mut sti);
                continue;
            }
            if ((posFormat) as i32) == 1 {
                let mut l: i32 = 0;
                let mut r: i32 = 0;
                let mut m: i32 = 0;
                let mut straw: i32 = 0;
                let mut needle: i32 = 0;
                let mut valueFormat1: u16 = ttUSHORT((table).offset((4) as isize));
                let mut valueFormat2: u16 = ttUSHORT((table).offset((6) as isize));
                if ((valueFormat1) as i32) == 4 && ((valueFormat2) as i32) == 0 {
                    let mut valueRecordPairSizeInBytes: i32 = 2;
                    let mut pairSetCount: u16 = ttUSHORT((table).offset((8) as isize));
                    let mut pairPosOffset: u16 = ttUSHORT(
                        ((table).offset((10) as isize)).offset((2 * coverageIndex) as isize),
                    );
                    let mut pairValueTable: *const u8 =
                        (table).offset(((pairPosOffset) as i32) as isize);
                    let mut pairValueCount: u16 = ttUSHORT(pairValueTable);
                    let mut pairValueArray: *const u8 = (pairValueTable).offset((2) as isize);
                    if coverageIndex >= ((pairSetCount) as i32) {
                        return ((0) as i32);
                    }
                    needle = ((glyph2) as i32);
                    r = (((pairValueCount) as i32) - 1);
                    l = ((0) as i32);
                    while (l <= r) {
                        let mut secondGlyph: u16 = 0;
                        let mut pairValue: *const u8 = core::ptr::null_mut();
                        m = (((l + r) >> 1) as i32);
                        pairValue = (pairValueArray)
                            .offset(((2 + valueRecordPairSizeInBytes) * m) as isize);
                        secondGlyph = ((ttUSHORT(pairValue)) as u16);
                        straw = ((secondGlyph) as i32);
                        if needle < straw {
                            r = ((m - 1) as i32);
                        } else {
                            if needle > straw {
                                l = ((m + 1) as i32);
                            } else {
                                let mut xAdvance: i16 = ttSHORT((pairValue).offset((2) as isize));
                                return ((xAdvance) as i32);
                            }
                        }
                    }
                } else {
                    return ((0) as i32);
                }
            } else if ((posFormat) as i32) == 2 {
                let mut valueFormat1: u16 = ttUSHORT((table).offset((4) as isize));
                let mut valueFormat2: u16 = ttUSHORT((table).offset((6) as isize));
                if ((valueFormat1) as i32) == 4 && ((valueFormat2) as i32) == 0 {
                    let mut classDef1Offset: u16 = ttUSHORT((table).offset((8) as isize));
                    let mut classDef2Offset: u16 = ttUSHORT((table).offset((10) as isize));
                    let mut glyph1class: i32 = stbtt__GetGlyphClass(
                        (table).offset(((classDef1Offset) as i32) as isize),
                        glyph1,
                    );
                    let mut glyph2class: i32 = stbtt__GetGlyphClass(
                        (table).offset(((classDef2Offset) as i32) as isize),
                        glyph2,
                    );
                    let mut class1Count: u16 = ttUSHORT((table).offset((12) as isize));
                    let mut class2Count: u16 = ttUSHORT((table).offset((14) as isize));
                    let mut class1Records: *const u8 = core::ptr::null_mut();
                    let mut class2Records: *const u8 = core::ptr::null_mut();
                    let mut xAdvance: i16 = 0;
                    if glyph1class < 0 || glyph1class >= ((class1Count) as i32) {
                        return ((0) as i32);
                    }
                    if glyph2class < 0 || glyph2class >= ((class2Count) as i32) {
                        return ((0) as i32);
                    }
                    class1Records = (table).offset((16) as isize);
                    class2Records = (class1Records)
                        .offset((2 * (glyph1class * ((class2Count) as i32))) as isize);
                    xAdvance =
                        ((ttSHORT((class2Records).offset((2 * glyph2class) as isize))) as i16);
                    return ((xAdvance) as i32);
                } else {
                    return ((0) as i32);
                }
            } else {
                return ((0) as i32);
            }
            c_runtime::postInc(&mut sti);
        }
        c_runtime::preInc(&mut i);
    }
    return ((0) as i32);
}

pub unsafe fn stbtt__GetGlyphInfoT2(
    mut info: *mut stbtt_fontinfo,
    mut glyph_index: i32,
    mut x0: *mut i32,
    mut y0: *mut i32,
    mut x1: *mut i32,
    mut y1: *mut i32,
) -> i32 {
    let mut c: stbtt__csctx = Default::default();
    c.bounds = 1;
    let mut r: i32 = stbtt__run_charstring(info, glyph_index, ((&mut c) as *mut stbtt__csctx));
    if (x0) != core::ptr::null_mut() {
        *x0 = ((if (r) != 0 { c.min_x } else { 0 }) as i32);
    }
    if (y0) != core::ptr::null_mut() {
        *y0 = ((if (r) != 0 { c.min_y } else { 0 }) as i32);
    }
    if (x1) != core::ptr::null_mut() {
        *x1 = ((if (r) != 0 { c.max_x } else { 0 }) as i32);
    }
    if (y1) != core::ptr::null_mut() {
        *y1 = ((if (r) != 0 { c.max_y } else { 0 }) as i32);
    }
    return ((if (r) != 0 { c.num_vertices } else { 0 }) as i32);
}

pub unsafe fn stbtt__GetGlyphKernInfoAdvance(
    mut info: *mut stbtt_fontinfo,
    mut glyph1: i32,
    mut glyph2: i32,
) -> i32 {
    let mut data: *const u8 = ((*info).data).offset(((*info).kern) as isize);
    let mut needle: u32 = 0;
    let mut straw: u32 = 0;
    let mut l: i32 = 0;
    let mut r: i32 = 0;
    let mut m: i32 = 0;
    if (*info).kern == 0 {
        return ((0) as i32);
    }
    if ((ttUSHORT((data).offset((2) as isize))) as i32) < 1 {
        return ((0) as i32);
    }
    if ((ttUSHORT((data).offset((8) as isize))) as i32) != 1 {
        return ((0) as i32);
    }
    l = ((0) as i32);
    r = (((ttUSHORT((data).offset((10) as isize))) as i32) - 1);
    needle = ((glyph1 << 16 | glyph2) as u32);
    while (l <= r) {
        m = (((l + r) >> 1) as i32);
        straw = ((ttULONG(((data).offset((18) as isize)).offset((m * 6) as isize))) as u32);
        if needle < straw {
            r = ((m - 1) as i32);
        } else {
            if needle > straw {
                l = ((m + 1) as i32);
            } else {
                return ((ttSHORT(((data).offset((22) as isize)).offset((m * 6) as isize))) as i32);
            }
        }
    }
    return ((0) as i32);
}

pub unsafe fn stbtt__GetGlyphShapeT2(
    mut info: *mut stbtt_fontinfo,
    mut glyph_index: i32,
    mut pvertices: *mut *mut stbtt_vertex,
) -> i32 {
    let mut count_ctx: stbtt__csctx = Default::default();
    count_ctx.bounds = 1;
    let mut output_ctx: stbtt__csctx = Default::default();
    if (stbtt__run_charstring(info, glyph_index, ((&mut count_ctx) as *mut stbtt__csctx))) != 0 {
        *pvertices = ((c_runtime::malloc(
            ((count_ctx.num_vertices) as u64) * core::mem::size_of::<stbtt_vertex>() as u64,
        )) as *mut stbtt_vertex);
        output_ctx.pvertices = *pvertices;
        if (stbtt__run_charstring(info, glyph_index, ((&mut output_ctx) as *mut stbtt__csctx))) != 0
        {
            return ((output_ctx.num_vertices) as i32);
        }
    }
    *pvertices = core::ptr::null_mut();
    return ((0) as i32);
}

pub unsafe fn stbtt__GetGlyphShapeTT(
    mut info: *mut stbtt_fontinfo,
    mut glyph_index: i32,
    mut pvertices: *mut *mut stbtt_vertex,
) -> i32 {
    let mut numberOfContours: i16 = 0;
    let mut endPtsOfContours: *const u8 = core::ptr::null_mut();
    let mut data: *const u8 = (*info).data;
    let mut vertices: *mut stbtt_vertex = core::ptr::null_mut();
    let mut num_vertices: i32 = 0;
    let mut g: i32 = stbtt__GetGlyfOffset(info, glyph_index);
    *pvertices = core::ptr::null_mut();
    if g < 0 {
        return ((0) as i32);
    }
    numberOfContours = ((ttSHORT((data).offset((g) as isize))) as i16);
    if ((numberOfContours) as i32) > 0 {
        let mut flags: u8 = ((0) as u8);
        let mut flagcount: u8 = 0;
        let mut ins: i32 = 0;
        let mut i: i32 = 0;
        let mut j: i32 = 0;
        let mut m: i32 = 0;
        let mut n: i32 = 0;
        let mut next_move: i32 = 0;
        let mut was_off: i32 = 0;
        let mut off: i32 = 0;
        let mut start_off: i32 = 0;
        let mut x: i32 = 0;
        let mut y: i32 = 0;
        let mut cx: i32 = 0;
        let mut cy: i32 = 0;
        let mut sx: i32 = 0;
        let mut sy: i32 = 0;
        let mut scx: i32 = 0;
        let mut scy: i32 = 0;
        let mut points: *const u8 = core::ptr::null_mut();
        endPtsOfContours = (((data).offset((g) as isize)).offset((10) as isize));
        ins = ((ttUSHORT(
            (((data).offset((g) as isize)).offset((10) as isize))
                .offset((((numberOfContours) as i32) * 2) as isize),
        )) as i32);
        points = (((((data).offset((g) as isize)).offset((10) as isize))
            .offset((((numberOfContours) as i32) * 2) as isize))
        .offset((2) as isize))
        .offset((ins) as isize);
        n = (1
            + ((ttUSHORT(
                ((endPtsOfContours).offset((((numberOfContours) as i32) * 2) as isize))
                    .offset(-((2) as isize)),
            )) as i32));
        m = (n + 2 * ((numberOfContours) as i32));
        vertices = ((c_runtime::malloc(((m) as u64) * core::mem::size_of::<stbtt_vertex>() as u64))
            as *mut stbtt_vertex);
        if vertices == core::ptr::null_mut() {
            return ((0) as i32);
        }
        next_move = ((0) as i32);
        flagcount = ((0) as u8);
        off = ((m - n) as i32);
        i = ((0) as i32);
        while (i < n) {
            if ((flagcount) as i32) == 0 {
                flags = ((*c_runtime::postIncConstPtr(&mut points)) as u8);
                if (((flags) as i32) & 8) != 0 {
                    flagcount = ((*c_runtime::postIncConstPtr(&mut points)) as u8);
                }
            } else {
                c_runtime::preDec(&mut flagcount);
            }
            (*vertices.offset((off + i) as isize))._type_ = ((flags) as u8);
            c_runtime::preInc(&mut i);
        }
        x = ((0) as i32);
        i = ((0) as i32);
        while (i < n) {
            flags = (((*vertices.offset((off + i) as isize))._type_) as u8);
            if (((flags) as i32) & 2) != 0 {
                let mut dx: i16 = ((*c_runtime::postIncConstPtr(&mut points)) as i16);
                x += (if (((flags) as i32) & 16) != 0 {
                    ((dx) as i32)
                } else {
                    -((dx) as i32)
                });
            } else {
                if (((flags) as i32) & 16) == 0 {
                    x = (x
                        + (((((*points.offset((0) as isize)) as i32) * 256
                            + ((*points.offset((1) as isize)) as i32))
                            as i16) as i32));
                    points = points.offset((2) as isize);
                }
            }
            (*vertices.offset((off + i) as isize)).x = ((x) as i16);
            c_runtime::preInc(&mut i);
        }
        y = ((0) as i32);
        i = ((0) as i32);
        while (i < n) {
            flags = (((*vertices.offset((off + i) as isize))._type_) as u8);
            if (((flags) as i32) & 4) != 0 {
                let mut dy: i16 = ((*c_runtime::postIncConstPtr(&mut points)) as i16);
                y += (if (((flags) as i32) & 32) != 0 {
                    ((dy) as i32)
                } else {
                    -((dy) as i32)
                });
            } else {
                if (((flags) as i32) & 32) == 0 {
                    y = (y
                        + (((((*points.offset((0) as isize)) as i32) * 256
                            + ((*points.offset((1) as isize)) as i32))
                            as i16) as i32));
                    points = points.offset((2) as isize);
                }
            }
            (*vertices.offset((off + i) as isize)).y = ((y) as i16);
            c_runtime::preInc(&mut i);
        }
        num_vertices = ((0) as i32);
        let hebron_tmp15 = 0;
        sx = hebron_tmp15;
        sy = hebron_tmp15;
        cx = hebron_tmp15;
        cy = hebron_tmp15;
        scx = hebron_tmp15;
        scy = hebron_tmp15;
        i = ((0) as i32);
        while (i < n) {
            flags = (((*vertices.offset((off + i) as isize))._type_) as u8);
            x = (((*vertices.offset((off + i) as isize)).x) as i32);
            y = (((*vertices.offset((off + i) as isize)).y) as i32);
            if next_move == i {
                if i != 0 {
                    num_vertices = ((stbtt__close_shape(
                        vertices,
                        num_vertices,
                        was_off,
                        start_off,
                        sx,
                        sy,
                        scx,
                        scy,
                        cx,
                        cy,
                    )) as i32);
                }
                start_off = if (flags & 1) != 0 { 0 } else { 1 };
                if (start_off) != 0 {
                    scx = ((x) as i32);
                    scy = ((y) as i32);
                    if ((((*vertices.offset((off + i + 1) as isize))._type_) as i32) & 1) == 0 {
                        sx = ((x + (((*vertices.offset((off + i + 1) as isize)).x) as i32)) >> 1);
                        sy = ((y + (((*vertices.offset((off + i + 1) as isize)).y) as i32)) >> 1);
                    } else {
                        sx = (((*vertices.offset((off + i + 1) as isize)).x) as i32);
                        sy = (((*vertices.offset((off + i + 1) as isize)).y) as i32);
                        c_runtime::preInc(&mut i);
                    }
                } else {
                    sx = ((x) as i32);
                    sy = ((y) as i32);
                }
                stbtt_setvertex(
                    ((&mut *vertices.offset((c_runtime::postInc(&mut num_vertices)) as isize))
                        as *mut stbtt_vertex),
                    ((STBTT_vmove) as u8),
                    sx,
                    sy,
                    0,
                    0,
                );
                was_off = ((0) as i32);
                next_move = (1 + ((ttUSHORT((endPtsOfContours).offset((j * 2) as isize))) as i32));
                c_runtime::preInc(&mut j);
            } else {
                if (((flags) as i32) & 1) == 0 {
                    if (was_off) != 0 {
                        stbtt_setvertex(
                            ((&mut *vertices
                                .offset((c_runtime::postInc(&mut num_vertices)) as isize))
                                as *mut stbtt_vertex),
                            ((STBTT_vcurve) as u8),
                            (cx + x) >> 1,
                            (cy + y) >> 1,
                            cx,
                            cy,
                        );
                    }
                    cx = ((x) as i32);
                    cy = ((y) as i32);
                    was_off = ((1) as i32);
                } else {
                    if (was_off) != 0 {
                        stbtt_setvertex(
                            ((&mut *vertices
                                .offset((c_runtime::postInc(&mut num_vertices)) as isize))
                                as *mut stbtt_vertex),
                            ((STBTT_vcurve) as u8),
                            x,
                            y,
                            cx,
                            cy,
                        );
                    } else {
                        stbtt_setvertex(
                            ((&mut *vertices
                                .offset((c_runtime::postInc(&mut num_vertices)) as isize))
                                as *mut stbtt_vertex),
                            ((STBTT_vline) as u8),
                            x,
                            y,
                            0,
                            0,
                        );
                    }
                    was_off = ((0) as i32);
                }
            }
            c_runtime::preInc(&mut i);
        }
        num_vertices = ((stbtt__close_shape(
            vertices,
            num_vertices,
            was_off,
            start_off,
            sx,
            sy,
            scx,
            scy,
            cx,
            cy,
        )) as i32);
    } else {
        if ((numberOfContours) as i32) < 0 {
            let mut more: i32 = 1;
            let mut comp: *const u8 = ((data).offset((g) as isize)).offset((10) as isize);
            num_vertices = ((0) as i32);
            vertices = core::ptr::null_mut();
            while ((more) != 0) {
                let mut flags: u16 = 0;
                let mut gidx: u16 = 0;
                let mut comp_num_verts: i32 = 0;
                let mut i: i32 = 0;
                let mut comp_verts: *mut stbtt_vertex = core::ptr::null_mut();
                let mut tmp: *mut stbtt_vertex = core::ptr::null_mut();
                let mut mtx: [f32; 6] = [
                    ((1) as f32),
                    ((0) as f32),
                    ((0) as f32),
                    ((1) as f32),
                    ((0) as f32),
                    ((0) as f32),
                ];
                let mut m: f32 = 0.0f32;
                let mut n: f32 = 0.0f32;
                flags = ((ttSHORT(comp)) as u16);
                comp = comp.offset((2) as isize);
                gidx = ((ttSHORT(comp)) as u16);
                comp = comp.offset((2) as isize);
                if (((flags) as i32) & 2) != 0 {
                    if (((flags) as i32) & 1) != 0 {
                        mtx[(4) as usize] = ((ttSHORT(comp)) as f32);
                        comp = comp.offset((2) as isize);
                        mtx[(5) as usize] = ((ttSHORT(comp)) as f32);
                        comp = comp.offset((2) as isize);
                    } else {
                        mtx[(4) as usize] = ((*((comp) as *mut i8)) as f32);
                        comp = comp.offset((1) as isize);
                        mtx[(5) as usize] = ((*((comp) as *mut i8)) as f32);
                        comp = comp.offset((1) as isize);
                    }
                } else {
                }
                if (((flags) as i32) & (1 << 3)) != 0 {
                    let hebron_tmp16 = (((ttSHORT(comp)) as i32) as f32) / 16384.0f32;
                    mtx[(0) as usize] = hebron_tmp16;
                    mtx[(3) as usize] = hebron_tmp16;
                    comp = comp.offset((2) as isize);
                    let hebron_tmp17 = ((0) as f32);
                    mtx[(1) as usize] = hebron_tmp17;
                    mtx[(2) as usize] = hebron_tmp17;
                } else {
                    if (((flags) as i32) & (1 << 6)) != 0 {
                        mtx[(0) as usize] = ((((ttSHORT(comp)) as i32) as f32) / 16384.0f32);
                        comp = comp.offset((2) as isize);
                        let hebron_tmp18 = ((0) as f32);
                        mtx[(1) as usize] = hebron_tmp18;
                        mtx[(2) as usize] = hebron_tmp18;
                        mtx[(3) as usize] = ((((ttSHORT(comp)) as i32) as f32) / 16384.0f32);
                        comp = comp.offset((2) as isize);
                    } else {
                        if (((flags) as i32) & (1 << 7)) != 0 {
                            mtx[(0) as usize] = ((((ttSHORT(comp)) as i32) as f32) / 16384.0f32);
                            comp = comp.offset((2) as isize);
                            mtx[(1) as usize] = ((((ttSHORT(comp)) as i32) as f32) / 16384.0f32);
                            comp = comp.offset((2) as isize);
                            mtx[(2) as usize] = ((((ttSHORT(comp)) as i32) as f32) / 16384.0f32);
                            comp = comp.offset((2) as isize);
                            mtx[(3) as usize] = ((((ttSHORT(comp)) as i32) as f32) / 16384.0f32);
                            comp = comp.offset((2) as isize);
                        }
                    }
                }
                m = ((c_runtime::sqrt(
                    ((mtx[(0) as usize] * mtx[(0) as usize] + mtx[(1) as usize] * mtx[(1) as usize])
                        as f32),
                )) as f32);
                n = ((c_runtime::sqrt(
                    ((mtx[(2) as usize] * mtx[(2) as usize] + mtx[(3) as usize] * mtx[(3) as usize])
                        as f32),
                )) as f32);
                comp_num_verts = (stbtt_GetGlyphShape(
                    info,
                    ((gidx) as i32),
                    ((&mut comp_verts) as *mut *mut stbtt_vertex),
                ));
                if comp_num_verts > 0 {
                    i = ((0) as i32);
                    while (i < comp_num_verts) {
                        let mut v: *mut stbtt_vertex = &mut *comp_verts.offset((i) as isize);
                        let mut x: i16 = 0;
                        let mut y: i16 = 0;
                        x = (((*v).x) as i16);
                        y = (((*v).y) as i16);
                        (*v).x = ((m
                            * (mtx[(0) as usize] * (((x) as i32) as f32)
                                + mtx[(2) as usize] * (((y) as i32) as f32)
                                + mtx[(4) as usize])) as i16);
                        (*v).y = ((n
                            * (mtx[(1) as usize] * (((x) as i32) as f32)
                                + mtx[(3) as usize] * (((y) as i32) as f32)
                                + mtx[(5) as usize])) as i16);
                        x = (((*v).cx) as i16);
                        y = (((*v).cy) as i16);
                        (*v).cx = ((m
                            * (mtx[(0) as usize] * (((x) as i32) as f32)
                                + mtx[(2) as usize] * (((y) as i32) as f32)
                                + mtx[(4) as usize])) as i16);
                        (*v).cy = ((n
                            * (mtx[(1) as usize] * (((x) as i32) as f32)
                                + mtx[(3) as usize] * (((y) as i32) as f32)
                                + mtx[(5) as usize])) as i16);
                        c_runtime::preInc(&mut i);
                    }
                    tmp = ((c_runtime::malloc(
                        ((num_vertices + comp_num_verts) as u64)
                            * core::mem::size_of::<stbtt_vertex>() as u64,
                    )) as *mut stbtt_vertex);
                    if tmp == core::ptr::null_mut() {
                        if (vertices) != core::ptr::null_mut() {
                            c_runtime::free(((vertices) as *mut u8));
                        }
                        if (comp_verts) != core::ptr::null_mut() {
                            c_runtime::free(((comp_verts) as *mut u8));
                        }
                        return ((0) as i32);
                    }
                    if num_vertices > 0 && (vertices) != core::ptr::null_mut() {
                        c_runtime::memcpy(
                            ((tmp) as *mut u8),
                            ((vertices) as *mut u8),
                            ((num_vertices) as u64) * core::mem::size_of::<stbtt_vertex>() as u64,
                        );
                    }
                    c_runtime::memcpy(
                        (((tmp).offset((num_vertices) as isize)) as *mut u8),
                        ((comp_verts) as *mut u8),
                        ((comp_num_verts) as u64) * core::mem::size_of::<stbtt_vertex>() as u64,
                    );
                    if (vertices) != core::ptr::null_mut() {
                        c_runtime::free(((vertices) as *mut u8));
                    }
                    vertices = tmp;
                    c_runtime::free(((comp_verts) as *mut u8));
                    num_vertices += ((comp_num_verts) as i32);
                }
                more = (((flags) as i32) & (1 << 5));
            }
        } else {
        }
    }
    *pvertices = vertices;
    return ((num_vertices) as i32);
}

pub unsafe fn stbtt_FindGlyphIndex(
    mut info: *mut stbtt_fontinfo,
    mut unicode_codepoint: i32,
) -> i32 {
    let mut data: *const u8 = (*info).data;
    let mut index_map: u32 = (((*info).index_map) as u32);
    let mut format: u16 = ttUSHORT(((data).offset((index_map) as isize)).offset((0) as isize));
    if ((format) as i32) == 0 {
        let mut bytes: i32 =
            ((ttUSHORT(((data).offset((index_map) as isize)).offset((2) as isize))) as i32);
        if unicode_codepoint < bytes - 6 {
            return ((*((((data).offset((index_map) as isize)).offset((6) as isize))
                .offset((unicode_codepoint) as isize))) as i32);
        }
        return ((0) as i32);
    } else {
        if ((format) as i32) == 6 {
            let mut first: u32 =
                ((ttUSHORT(((data).offset((index_map) as isize)).offset((6) as isize))) as u32);
            let mut count: u32 =
                ((ttUSHORT(((data).offset((index_map) as isize)).offset((8) as isize))) as u32);
            if ((unicode_codepoint) as u32) >= first && ((unicode_codepoint) as u32) < first + count
            {
                return ((ttUSHORT(
                    (((data).offset((index_map) as isize)).offset((10) as isize))
                        .offset(((((unicode_codepoint) as u32) - first) * ((2) as u32)) as isize),
                )) as i32);
            }
            return ((0) as i32);
        } else {
            if ((format) as i32) == 2 {
                return ((0) as i32);
            } else {
                if ((format) as i32) == 4 {
                    let mut segcount: u16 =
                        ((((ttUSHORT(((data).offset((index_map) as isize)).offset((6) as isize)))
                            as i32)
                            >> 1) as u16);
                    let mut searchRange: u16 =
                        ((((ttUSHORT(((data).offset((index_map) as isize)).offset((8) as isize)))
                            as i32)
                            >> 1) as u16);
                    let mut entrySelector: u16 =
                        ttUSHORT(((data).offset((index_map) as isize)).offset((10) as isize));
                    let mut rangeShift: u16 =
                        ((((ttUSHORT(((data).offset((index_map) as isize)).offset((12) as isize)))
                            as i32)
                            >> 1) as u16);
                    let mut endCount: u32 = index_map + ((14) as u32);
                    let mut search: u32 = endCount;
                    if unicode_codepoint > 0xffff {
                        return ((0) as i32);
                    }
                    if unicode_codepoint
                        >= ((ttUSHORT(
                            ((data).offset((search) as isize))
                                .offset((((rangeShift) as i32) * 2) as isize),
                        )) as i32)
                    {
                        search += ((((rangeShift) as i32) * 2) as u32);
                    }
                    search -= ((2) as u32);
                    while ((entrySelector) != 0) {
                        let mut end: u16 = 0;
                        searchRange >>= 1;
                        end = ((ttUSHORT(
                            ((data).offset((search) as isize))
                                .offset((((searchRange) as i32) * 2) as isize),
                        )) as u16);
                        if unicode_codepoint > ((end) as i32) {
                            search += ((((searchRange) as i32) * 2) as u32);
                        }
                        c_runtime::preDec(&mut entrySelector);
                    }
                    search += ((2) as u32);
                    {
                        let mut offset: u16 = 0;
                        let mut start: u16 = 0;
                        let mut last: u16 = 0;
                        let mut item: u16 = (((search - endCount) >> 1) as u16);
                        start = ((ttUSHORT(
                            (((((data).offset((index_map) as isize)).offset((14) as isize))
                                .offset((((segcount) as i32) * 2) as isize))
                            .offset((2) as isize))
                            .offset((2 * ((item) as i32)) as isize),
                        )) as u16);
                        last = ((ttUSHORT(
                            ((data).offset((endCount) as isize))
                                .offset((2 * ((item) as i32)) as isize),
                        )) as u16);
                        if unicode_codepoint < ((start) as i32)
                            || unicode_codepoint > ((last) as i32)
                        {
                            return ((0) as i32);
                        }
                        offset = ((ttUSHORT(
                            (((((data).offset((index_map) as isize)).offset((14) as isize))
                                .offset((((segcount) as i32) * 6) as isize))
                            .offset((2) as isize))
                            .offset((2 * ((item) as i32)) as isize),
                        )) as u16);
                        if ((offset) as i32) == 0 {
                            return (((unicode_codepoint
                                + ((ttSHORT(
                                    (((((data).offset((index_map) as isize))
                                        .offset((14) as isize))
                                    .offset((((segcount) as i32) * 4) as isize))
                                    .offset((2) as isize))
                                    .offset((2 * ((item) as i32)) as isize),
                                )) as i32)) as u16) as i32);
                        }
                        return ((ttUSHORT(
                            (((((((data).offset(((offset) as i32) as isize))
                                .offset(((unicode_codepoint - ((start) as i32)) * 2) as isize))
                            .offset((index_map) as isize))
                            .offset((14) as isize))
                            .offset((((segcount) as i32) * 6) as isize))
                            .offset((2) as isize))
                            .offset((2 * ((item) as i32)) as isize),
                        )) as i32);
                    }
                } else {
                    if ((format) as i32) == 12 || ((format) as i32) == 13 {
                        let mut ngroups: u32 =
                            ttULONG(((data).offset((index_map) as isize)).offset((12) as isize));
                        let mut low: i32 = 0;
                        let mut high: i32 = 0;
                        low = ((0) as i32);
                        high = ((ngroups) as i32);
                        while (low < high) {
                            let mut mid: i32 = low + ((high - low) >> 1);
                            let mut start_char: u32 = ttULONG(
                                (((data).offset((index_map) as isize)).offset((16) as isize))
                                    .offset((mid * 12) as isize),
                            );
                            let mut end_char: u32 = ttULONG(
                                ((((data).offset((index_map) as isize)).offset((16) as isize))
                                    .offset((mid * 12) as isize))
                                .offset((4) as isize),
                            );
                            if ((unicode_codepoint) as u32) < start_char {
                                high = ((mid) as i32);
                            } else {
                                if ((unicode_codepoint) as u32) > end_char {
                                    low = ((mid + 1) as i32);
                                } else {
                                    let mut start_glyph: u32 = ttULONG(
                                        ((((data).offset((index_map) as isize))
                                            .offset((16) as isize))
                                        .offset((mid * 12) as isize))
                                        .offset((8) as isize),
                                    );
                                    if ((format) as i32) == 12 {
                                        return ((start_glyph + ((unicode_codepoint) as u32)
                                            - start_char)
                                            as i32);
                                    } else {
                                        return ((start_glyph) as i32);
                                    }
                                }
                            }
                        }
                        return ((0) as i32);
                    }
                }
            }
        }
    }

    return ((0) as i32);
}

pub unsafe fn stbtt_FindSVGDoc(mut info: *mut stbtt_fontinfo, mut gl: i32) -> *const u8 {
    let mut i: i32 = 0;
    let mut data: *const u8 = (*info).data;
    let mut svg_doc_list: *const u8 =
        (data).offset((stbtt__get_svg(((info) as *mut stbtt_fontinfo))) as isize);
    let mut numEntries: i32 = ((ttUSHORT(svg_doc_list)) as i32);
    let mut svg_docs: *const u8 = (svg_doc_list).offset((2) as isize);
    i = ((0) as i32);
    while (i < numEntries) {
        let mut svg_doc: *const u8 = (svg_docs).offset((12 * i) as isize);
        if (gl >= ((ttUSHORT(svg_doc)) as i32))
            && (gl <= ((ttUSHORT((svg_doc).offset((2) as isize))) as i32))
        {
            return svg_doc;
        }
        c_runtime::postInc(&mut i);
    }
    return ((core::ptr::null_mut()) as *const u8);
}

pub unsafe fn stbtt_FreeShape(mut info: *mut stbtt_fontinfo, mut v: *mut stbtt_vertex) {
    c_runtime::free(((v) as *mut u8));
}

pub unsafe fn stbtt_GetCodepointBitmap(
    mut info: *mut stbtt_fontinfo,
    mut scale_x: f32,
    mut scale_y: f32,
    mut codepoint: i32,
    mut width: *mut i32,
    mut height: *mut i32,
    mut xoff: *mut i32,
    mut yoff: *mut i32,
) -> *mut u8 {
    return stbtt_GetCodepointBitmapSubpixel(
        info, scale_x, scale_y, 0.0f32, 0.0f32, codepoint, width, height, xoff, yoff,
    );
}

pub unsafe fn stbtt_GetCodepointBitmapBox(
    mut font: *mut stbtt_fontinfo,
    mut codepoint: i32,
    mut scale_x: f32,
    mut scale_y: f32,
    mut ix0: *mut i32,
    mut iy0: *mut i32,
    mut ix1: *mut i32,
    mut iy1: *mut i32,
) {
    stbtt_GetCodepointBitmapBoxSubpixel(
        font, codepoint, scale_x, scale_y, 0.0f32, 0.0f32, ix0, iy0, ix1, iy1,
    );
}

pub unsafe fn stbtt_GetCodepointBitmapBoxSubpixel(
    mut font: *mut stbtt_fontinfo,
    mut codepoint: i32,
    mut scale_x: f32,
    mut scale_y: f32,
    mut shift_x: f32,
    mut shift_y: f32,
    mut ix0: *mut i32,
    mut iy0: *mut i32,
    mut ix1: *mut i32,
    mut iy1: *mut i32,
) {
    stbtt_GetGlyphBitmapBoxSubpixel(
        font,
        ((stbtt_FindGlyphIndex(font, codepoint)) as i32),
        scale_x,
        scale_y,
        shift_x,
        shift_y,
        ix0,
        iy0,
        ix1,
        iy1,
    );
}

pub unsafe fn stbtt_GetCodepointBitmapSubpixel(
    mut info: *mut stbtt_fontinfo,
    mut scale_x: f32,
    mut scale_y: f32,
    mut shift_x: f32,
    mut shift_y: f32,
    mut codepoint: i32,
    mut width: *mut i32,
    mut height: *mut i32,
    mut xoff: *mut i32,
    mut yoff: *mut i32,
) -> *mut u8 {
    return stbtt_GetGlyphBitmapSubpixel(
        info,
        scale_x,
        scale_y,
        shift_x,
        shift_y,
        ((stbtt_FindGlyphIndex(info, codepoint)) as i32),
        width,
        height,
        xoff,
        yoff,
    );
}

pub unsafe fn stbtt_GetCodepointBox(
    mut info: *mut stbtt_fontinfo,
    mut codepoint: i32,
    mut x0: *mut i32,
    mut y0: *mut i32,
    mut x1: *mut i32,
    mut y1: *mut i32,
) -> i32 {
    return stbtt_GetGlyphBox(
        info,
        ((stbtt_FindGlyphIndex(info, codepoint)) as i32),
        x0,
        y0,
        x1,
        y1,
    );
}

pub unsafe fn stbtt_GetCodepointHMetrics(
    mut info: *mut stbtt_fontinfo,
    mut codepoint: i32,
    mut advanceWidth: *mut i32,
    mut leftSideBearing: *mut i32,
) {
    stbtt_GetGlyphHMetrics(
        info,
        ((stbtt_FindGlyphIndex(info, codepoint)) as i32),
        advanceWidth,
        leftSideBearing,
    );
}

pub unsafe fn stbtt_GetCodepointKernAdvance(
    mut info: *mut stbtt_fontinfo,
    mut ch1: i32,
    mut ch2: i32,
) -> i32 {
    if (*info).kern == 0 && (*info).gpos == 0 {
        return ((0) as i32);
    }
    return stbtt_GetGlyphKernAdvance(
        info,
        ((stbtt_FindGlyphIndex(info, ch1)) as i32),
        ((stbtt_FindGlyphIndex(info, ch2)) as i32),
    );
}

pub unsafe fn stbtt_GetCodepointSDF(
    mut info: *mut stbtt_fontinfo,
    mut scale: f32,
    mut codepoint: i32,
    mut padding: i32,
    mut onedge_value: u8,
    mut pixel_dist_scale: f32,
    mut width: *mut i32,
    mut height: *mut i32,
    mut xoff: *mut i32,
    mut yoff: *mut i32,
) -> *mut u8 {
    return stbtt_GetGlyphSDF(
        info,
        scale,
        ((stbtt_FindGlyphIndex(info, codepoint)) as i32),
        padding,
        onedge_value,
        pixel_dist_scale,
        width,
        height,
        xoff,
        yoff,
    );
}

pub unsafe fn stbtt_GetCodepointShape(
    mut info: *mut stbtt_fontinfo,
    mut unicode_codepoint: i32,
    mut vertices: *mut *mut stbtt_vertex,
) -> i32 {
    return stbtt_GetGlyphShape(
        info,
        ((stbtt_FindGlyphIndex(info, unicode_codepoint)) as i32),
        vertices,
    );
}

pub unsafe fn stbtt_GetCodepointSVG(
    mut info: *mut stbtt_fontinfo,
    mut unicode_codepoint: i32,
    mut svg: *mut *mut i8,
) -> i32 {
    return stbtt_GetGlyphSVG(
        info,
        ((stbtt_FindGlyphIndex(info, unicode_codepoint)) as i32),
        svg,
    );
}

pub unsafe fn stbtt_GetFontBoundingBox(
    mut info: *mut stbtt_fontinfo,
    mut x0: *mut i32,
    mut y0: *mut i32,
    mut x1: *mut i32,
    mut y1: *mut i32,
) {
    *x0 =
        ((ttSHORT((((*info).data).offset(((*info).head) as isize)).offset((36) as isize))) as i32);
    *y0 =
        ((ttSHORT((((*info).data).offset(((*info).head) as isize)).offset((38) as isize))) as i32);
    *x1 =
        ((ttSHORT((((*info).data).offset(((*info).head) as isize)).offset((40) as isize))) as i32);
    *y1 =
        ((ttSHORT((((*info).data).offset(((*info).head) as isize)).offset((42) as isize))) as i32);
}

pub unsafe fn stbtt_GetFontNameString(
    mut font: *mut stbtt_fontinfo,
    mut length: *mut i32,
    mut platformID: i32,
    mut encodingID: i32,
    mut languageID: i32,
    mut nameID: i32,
) -> *mut i8 {
    let mut i: i32 = 0;
    let mut count: i32 = 0;
    let mut stringOffset: i32 = 0;
    let mut fc: *const u8 = (*font).data;
    let mut offset: u32 = (((*font).fontstart) as u32);
    let mut nm: u32 = stbtt__find_table(fc, offset, "name");
    if nm == 0 {
        return core::ptr::null_mut();
    }
    count = ((ttUSHORT(((fc).offset((nm) as isize)).offset((2) as isize))) as i32);
    stringOffset =
        ((nm + ((ttUSHORT(((fc).offset((nm) as isize)).offset((4) as isize))) as u32)) as i32);
    i = ((0) as i32);
    while (i < count) {
        let mut loc: u32 = nm + ((6) as u32) + ((12 * i) as u32);
        if platformID == ((ttUSHORT(((fc).offset((loc) as isize)).offset((0) as isize))) as i32)
            && encodingID == ((ttUSHORT(((fc).offset((loc) as isize)).offset((2) as isize))) as i32)
            && languageID == ((ttUSHORT(((fc).offset((loc) as isize)).offset((4) as isize))) as i32)
            && nameID == ((ttUSHORT(((fc).offset((loc) as isize)).offset((6) as isize))) as i32)
        {
            *length = ((ttUSHORT(((fc).offset((loc) as isize)).offset((8) as isize))) as i32);
            return ((((fc).offset((stringOffset) as isize)).offset(
                ((ttUSHORT(((fc).offset((loc) as isize)).offset((10) as isize))) as i32) as isize,
            )) as *mut i8);
        }
        c_runtime::preInc(&mut i);
    }
    return core::ptr::null_mut();
}

pub unsafe fn stbtt_GetFontVMetrics(
    mut info: *mut stbtt_fontinfo,
    mut ascent: *mut i32,
    mut descent: *mut i32,
    mut lineGap: *mut i32,
) {
    if (ascent) != core::ptr::null_mut() {
        *ascent = ((ttSHORT((((*info).data).offset(((*info).hhea) as isize)).offset((4) as isize)))
            as i32);
    }
    if (descent) != core::ptr::null_mut() {
        *descent = ((ttSHORT((((*info).data).offset(((*info).hhea) as isize)).offset((6) as isize)))
            as i32);
    }
    if (lineGap) != core::ptr::null_mut() {
        *lineGap = ((ttSHORT((((*info).data).offset(((*info).hhea) as isize)).offset((8) as isize)))
            as i32);
    }
}

pub unsafe fn stbtt_GetFontVMetricsOS2(
    mut info: *mut stbtt_fontinfo,
    mut typoAscent: *mut i32,
    mut typoDescent: *mut i32,
    mut typoLineGap: *mut i32,
) -> i32 {
    let mut tab: i32 =
        ((stbtt__find_table((*info).data, (((*info).fontstart) as u32), "OS/2")) as i32);
    if tab == 0 {
        return ((0) as i32);
    }
    if (typoAscent) != core::ptr::null_mut() {
        *typoAscent =
            ((ttSHORT((((*info).data).offset((tab) as isize)).offset((68) as isize))) as i32);
    }
    if (typoDescent) != core::ptr::null_mut() {
        *typoDescent =
            ((ttSHORT((((*info).data).offset((tab) as isize)).offset((70) as isize))) as i32);
    }
    if (typoLineGap) != core::ptr::null_mut() {
        *typoLineGap =
            ((ttSHORT((((*info).data).offset((tab) as isize)).offset((72) as isize))) as i32);
    }
    return ((1) as i32);
}

pub unsafe fn stbtt_GetGlyphBitmap(
    mut info: *mut stbtt_fontinfo,
    mut scale_x: f32,
    mut scale_y: f32,
    mut glyph: i32,
    mut width: *mut i32,
    mut height: *mut i32,
    mut xoff: *mut i32,
    mut yoff: *mut i32,
) -> *mut u8 {
    return stbtt_GetGlyphBitmapSubpixel(
        info, scale_x, scale_y, 0.0f32, 0.0f32, glyph, width, height, xoff, yoff,
    );
}

pub unsafe fn stbtt_GetGlyphBitmapBox(
    mut font: *mut stbtt_fontinfo,
    mut glyph: i32,
    mut scale_x: f32,
    mut scale_y: f32,
    mut ix0: *mut i32,
    mut iy0: *mut i32,
    mut ix1: *mut i32,
    mut iy1: *mut i32,
) {
    stbtt_GetGlyphBitmapBoxSubpixel(
        font, glyph, scale_x, scale_y, 0.0f32, 0.0f32, ix0, iy0, ix1, iy1,
    );
}

pub unsafe fn stbtt_GetGlyphBitmapBoxSubpixel(
    mut font: *mut stbtt_fontinfo,
    mut glyph: i32,
    mut scale_x: f32,
    mut scale_y: f32,
    mut shift_x: f32,
    mut shift_y: f32,
    mut ix0: *mut i32,
    mut iy0: *mut i32,
    mut ix1: *mut i32,
    mut iy1: *mut i32,
) {
    let mut x0: i32 = 0;
    let mut y0: i32 = 0;
    let mut x1: i32 = 0;
    let mut y1: i32 = 0;
    if stbtt_GetGlyphBox(
        font,
        glyph,
        ((&mut x0) as *mut i32),
        ((&mut y0) as *mut i32),
        ((&mut x1) as *mut i32),
        ((&mut y1) as *mut i32),
    ) == 0
    {
        if (ix0) != core::ptr::null_mut() {
            *ix0 = ((0) as i32);
        }
        if (iy0) != core::ptr::null_mut() {
            *iy0 = ((0) as i32);
        }
        if (ix1) != core::ptr::null_mut() {
            *ix1 = ((0) as i32);
        }
        if (iy1) != core::ptr::null_mut() {
            *iy1 = ((0) as i32);
        }
    } else {
        if (ix0) != core::ptr::null_mut() {
            *ix0 = ((c_runtime::floor(((((x0) as f32) * scale_x + shift_x) as f32))) as i32);
        }
        if (iy0) != core::ptr::null_mut() {
            *iy0 = ((c_runtime::floor(((((-y1) as f32) * scale_y + shift_y) as f32))) as i32);
        }
        if (ix1) != core::ptr::null_mut() {
            *ix1 = ((c_runtime::ceil(((((x1) as f32) * scale_x + shift_x) as f32))) as i32);
        }
        if (iy1) != core::ptr::null_mut() {
            *iy1 = ((c_runtime::ceil(((((-y0) as f32) * scale_y + shift_y) as f32))) as i32);
        }
    }
}

pub unsafe fn stbtt_GetGlyphBitmapSubpixel(
    mut info: *mut stbtt_fontinfo,
    mut scale_x: f32,
    mut scale_y: f32,
    mut shift_x: f32,
    mut shift_y: f32,
    mut glyph: i32,
    mut width: *mut i32,
    mut height: *mut i32,
    mut xoff: *mut i32,
    mut yoff: *mut i32,
) -> *mut u8 {
    let mut ix0: i32 = 0;
    let mut iy0: i32 = 0;
    let mut ix1: i32 = 0;
    let mut iy1: i32 = 0;
    let mut gbm: stbtt__bitmap = stbtt__bitmap::default();
    let mut vertices: *mut stbtt_vertex = core::ptr::null_mut();
    let mut num_verts: i32 =
        stbtt_GetGlyphShape(info, glyph, ((&mut vertices) as *mut *mut stbtt_vertex));
    if scale_x == ((0) as f32) {
        scale_x = ((scale_y) as f32);
    }
    if scale_y == ((0) as f32) {
        if scale_x == ((0) as f32) {
            c_runtime::free(((vertices) as *mut u8));
            return core::ptr::null_mut();
        }
        scale_y = ((scale_x) as f32);
    }
    stbtt_GetGlyphBitmapBoxSubpixel(
        info,
        glyph,
        scale_x,
        scale_y,
        shift_x,
        shift_y,
        ((&mut ix0) as *mut i32),
        ((&mut iy0) as *mut i32),
        ((&mut ix1) as *mut i32),
        ((&mut iy1) as *mut i32),
    );
    gbm.w = ((ix1 - ix0) as i32);
    gbm.h = ((iy1 - iy0) as i32);
    gbm.pixels = core::ptr::null_mut();
    if (width) != core::ptr::null_mut() {
        *width = ((gbm.w) as i32);
    }
    if (height) != core::ptr::null_mut() {
        *height = ((gbm.h) as i32);
    }
    if (xoff) != core::ptr::null_mut() {
        *xoff = ((ix0) as i32);
    }
    if (yoff) != core::ptr::null_mut() {
        *yoff = ((iy0) as i32);
    }
    if (gbm.w) != 0 && (gbm.h) != 0 {
        gbm.pixels = c_runtime::malloc(((gbm.w * gbm.h) as u64));
        if (gbm.pixels) != core::ptr::null_mut() {
            gbm.stride = ((gbm.w) as i32);
            stbtt_Rasterize(
                ((&mut gbm) as *mut stbtt__bitmap),
                0.35f32,
                vertices,
                num_verts,
                scale_x,
                scale_y,
                shift_x,
                shift_y,
                ix0,
                iy0,
                1,
                (*info).userdata,
            );
        }
    }
    c_runtime::free(((vertices) as *mut u8));
    return gbm.pixels;
}

pub unsafe fn stbtt_GetGlyphBox(
    mut info: *mut stbtt_fontinfo,
    mut glyph_index: i32,
    mut x0: *mut i32,
    mut y0: *mut i32,
    mut x1: *mut i32,
    mut y1: *mut i32,
) -> i32 {
    if ((*info).cff.size) != 0 {
        stbtt__GetGlyphInfoT2(info, glyph_index, x0, y0, x1, y1);
    } else {
        let mut g: i32 = stbtt__GetGlyfOffset(info, glyph_index);
        if g < 0 {
            return ((0) as i32);
        }
        if (x0) != core::ptr::null_mut() {
            *x0 = ((ttSHORT((((*info).data).offset((g) as isize)).offset((2) as isize))) as i32);
        }
        if (y0) != core::ptr::null_mut() {
            *y0 = ((ttSHORT((((*info).data).offset((g) as isize)).offset((4) as isize))) as i32);
        }
        if (x1) != core::ptr::null_mut() {
            *x1 = ((ttSHORT((((*info).data).offset((g) as isize)).offset((6) as isize))) as i32);
        }
        if (y1) != core::ptr::null_mut() {
            *y1 = ((ttSHORT((((*info).data).offset((g) as isize)).offset((8) as isize))) as i32);
        }
    }
    return ((1) as i32);
}

pub unsafe fn stbtt_GetGlyphHMetrics(
    mut info: *mut stbtt_fontinfo,
    mut glyph_index: i32,
    mut advanceWidth: *mut i32,
    mut leftSideBearing: *mut i32,
) {
    let mut numOfLongHorMetrics: u16 =
        ttUSHORT((((*info).data).offset(((*info).hhea) as isize)).offset((34) as isize));
    if glyph_index < ((numOfLongHorMetrics) as i32) {
        if (advanceWidth) != core::ptr::null_mut() {
            *advanceWidth = ((ttSHORT(
                (((*info).data).offset(((*info).hmtx) as isize)).offset((4 * glyph_index) as isize),
            )) as i32);
        }
        if (leftSideBearing) != core::ptr::null_mut() {
            *leftSideBearing = ((ttSHORT(
                ((((*info).data).offset(((*info).hmtx) as isize))
                    .offset((4 * glyph_index) as isize))
                .offset((2) as isize),
            )) as i32);
        }
    } else {
        if (advanceWidth) != core::ptr::null_mut() {
            *advanceWidth = ((ttSHORT(
                (((*info).data).offset(((*info).hmtx) as isize))
                    .offset((4 * (((numOfLongHorMetrics) as i32) - 1)) as isize),
            )) as i32);
        }
        if (leftSideBearing) != core::ptr::null_mut() {
            *leftSideBearing = ((ttSHORT(
                ((((*info).data).offset(((*info).hmtx) as isize))
                    .offset((4 * ((numOfLongHorMetrics) as i32)) as isize))
                .offset((2 * (glyph_index - ((numOfLongHorMetrics) as i32))) as isize),
            )) as i32);
        }
    }
}

pub unsafe fn stbtt_GetGlyphKernAdvance(
    mut info: *mut stbtt_fontinfo,
    mut g1: i32,
    mut g2: i32,
) -> i32 {
    let mut xAdvance: i32 = 0;
    if ((*info).gpos) != 0 {
        xAdvance += ((stbtt__GetGlyphGPOSInfoAdvance(info, g1, g2)) as i32);
    } else {
        if ((*info).kern) != 0 {
            xAdvance += ((stbtt__GetGlyphKernInfoAdvance(info, g1, g2)) as i32);
        }
    }
    return ((xAdvance) as i32);
}

pub unsafe fn stbtt_GetGlyphSDF(
    mut info: *mut stbtt_fontinfo,
    mut scale: f32,
    mut glyph: i32,
    mut padding: i32,
    mut onedge_value: u8,
    mut pixel_dist_scale: f32,
    mut width: *mut i32,
    mut height: *mut i32,
    mut xoff: *mut i32,
    mut yoff: *mut i32,
) -> *mut u8 {
    let mut scale_x: f32 = scale;
    let mut scale_y: f32 = scale;
    let mut ix0: i32 = 0;
    let mut iy0: i32 = 0;
    let mut ix1: i32 = 0;
    let mut iy1: i32 = 0;
    let mut w: i32 = 0;
    let mut h: i32 = 0;
    let mut data: *mut u8 = core::ptr::null_mut();
    if scale == ((0) as f32) {
        return core::ptr::null_mut();
    }
    stbtt_GetGlyphBitmapBoxSubpixel(
        info,
        glyph,
        scale,
        scale,
        0.0f32,
        0.0f32,
        ((&mut ix0) as *mut i32),
        ((&mut iy0) as *mut i32),
        ((&mut ix1) as *mut i32),
        ((&mut iy1) as *mut i32),
    );
    if ix0 == ix1 || iy0 == iy1 {
        return core::ptr::null_mut();
    }
    ix0 -= ((padding) as i32);
    iy0 -= ((padding) as i32);
    ix1 += ((padding) as i32);
    iy1 += ((padding) as i32);
    w = ((ix1 - ix0) as i32);
    h = ((iy1 - iy0) as i32);
    if (width) != core::ptr::null_mut() {
        *width = ((w) as i32);
    }
    if (height) != core::ptr::null_mut() {
        *height = ((h) as i32);
    }
    if (xoff) != core::ptr::null_mut() {
        *xoff = ((ix0) as i32);
    }
    if (yoff) != core::ptr::null_mut() {
        *yoff = ((iy0) as i32);
    }
    scale_y = ((-scale_y) as f32);
    {
        let mut x: i32 = 0;
        let mut y: i32 = 0;
        let mut i: i32 = 0;
        let mut j: i32 = 0;
        let mut precompute: *mut f32 = core::ptr::null_mut();
        let mut verts: *mut stbtt_vertex = core::ptr::null_mut();
        let mut num_verts: i32 =
            stbtt_GetGlyphShape(info, glyph, ((&mut verts) as *mut *mut stbtt_vertex));
        data = c_runtime::malloc(((w * h) as u64));
        precompute = ((c_runtime::malloc(((num_verts) as u64) * core::mem::size_of::<f32>() as u64))
            as *mut f32);
        i = ((0) as i32);
        j = ((num_verts - 1) as i32);
        while (i < num_verts) {
            if (((*verts.offset((i) as isize))._type_) as i32) == STBTT_vline {
                let mut x0: f32 = ((((*verts.offset((i) as isize)).x) as i32) as f32) * scale_x;
                let mut y0: f32 = ((((*verts.offset((i) as isize)).y) as i32) as f32) * scale_y;
                let mut x1: f32 = ((((*verts.offset((j) as isize)).x) as i32) as f32) * scale_x;
                let mut y1: f32 = ((((*verts.offset((j) as isize)).y) as i32) as f32) * scale_y;
                let mut dist: f32 =
                    ((c_runtime::sqrt((((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)) as f32)))
                        as f32);
                *precompute.offset((i) as isize) = (if (dist == ((0) as f32)) {
                    0.0f32
                } else {
                    1.0f32 / dist
                });
            } else {
                if (((*verts.offset((i) as isize))._type_) as i32) == STBTT_vcurve {
                    let mut x2: f32 = ((((*verts.offset((j) as isize)).x) as i32) as f32) * scale_x;
                    let mut y2: f32 = ((((*verts.offset((j) as isize)).y) as i32) as f32) * scale_y;
                    let mut x1: f32 =
                        ((((*verts.offset((i) as isize)).cx) as i32) as f32) * scale_x;
                    let mut y1: f32 =
                        ((((*verts.offset((i) as isize)).cy) as i32) as f32) * scale_y;
                    let mut x0: f32 = ((((*verts.offset((i) as isize)).x) as i32) as f32) * scale_x;
                    let mut y0: f32 = ((((*verts.offset((i) as isize)).y) as i32) as f32) * scale_y;
                    let mut bx: f32 = x0 - ((2) as f32) * x1 + x2;
                    let mut by: f32 = y0 - ((2) as f32) * y1 + y2;
                    let mut len2: f32 = bx * bx + by * by;
                    if len2 != 0.0f32 {
                        *precompute.offset((i) as isize) = ((1.0f32 / (bx * bx + by * by)) as f32);
                    } else {
                        *precompute.offset((i) as isize) = ((0.0f32) as f32);
                    }
                } else {
                    *precompute.offset((i) as isize) = ((0.0f32) as f32);
                }
            }
            j = ((c_runtime::postInc(&mut i)) as i32);
        }
        y = ((iy0) as i32);
        while (y < iy1) {
            x = ((ix0) as i32);
            while (x < ix1) {
                let mut val: f32 = 0.0f32;
                let mut min_dist: f32 = 999999.0f32;
                let mut sx: f32 = ((x) as f32) + 0.5f32;
                let mut sy: f32 = ((y) as f32) + 0.5f32;
                let mut x_gspace: f32 = (sx / scale_x);
                let mut y_gspace: f32 = (sy / scale_y);
                let mut winding: i32 =
                    stbtt__compute_crossings_x(x_gspace, y_gspace, num_verts, verts);
                i = ((0) as i32);
                while (i < num_verts) {
                    let mut x0: f32 = ((((*verts.offset((i) as isize)).x) as i32) as f32) * scale_x;
                    let mut y0: f32 = ((((*verts.offset((i) as isize)).y) as i32) as f32) * scale_y;
                    if (((*verts.offset((i) as isize))._type_) as i32) == STBTT_vline
                        && *precompute.offset((i) as isize) != 0.0f32
                    {
                        let mut x1: f32 =
                            ((((*verts.offset((i - 1) as isize)).x) as i32) as f32) * scale_x;
                        let mut y1: f32 =
                            ((((*verts.offset((i - 1) as isize)).y) as i32) as f32) * scale_y;
                        let mut dist: f32 = 0.0f32;
                        let mut dist2: f32 = (x0 - sx) * (x0 - sx) + (y0 - sy) * (y0 - sy);
                        if dist2 < min_dist * min_dist {
                            min_dist = ((c_runtime::sqrt(((dist2) as f32))) as f32);
                        }
                        dist = ((((c_runtime::fabs(
                            (((x1 - x0) * (y0 - sy) - (y1 - y0) * (x0 - sx)) as f32),
                        )) as f32)
                            * *precompute.offset((i) as isize))
                            as f32);
                        if dist < min_dist {
                            let mut dx: f32 = x1 - x0;
                            let mut dy: f32 = y1 - y0;
                            let mut px: f32 = x0 - sx;
                            let mut py: f32 = y0 - sy;
                            let mut t: f32 = -(px * dx + py * dy) / (dx * dx + dy * dy);
                            if t >= 0.0f32 && t <= 1.0f32 {
                                min_dist = ((dist) as f32);
                            }
                        }
                    } else {
                        if (((*verts.offset((i) as isize))._type_) as i32) == STBTT_vcurve {
                            let mut x2: f32 =
                                ((((*verts.offset((i - 1) as isize)).x) as i32) as f32) * scale_x;
                            let mut y2: f32 =
                                ((((*verts.offset((i - 1) as isize)).y) as i32) as f32) * scale_y;
                            let mut x1: f32 =
                                ((((*verts.offset((i) as isize)).cx) as i32) as f32) * scale_x;
                            let mut y1: f32 =
                                ((((*verts.offset((i) as isize)).cy) as i32) as f32) * scale_y;
                            let mut box_x0: f32 = (if (if (x0) < (x1) { (x0) } else { (x1) }) < (x2)
                            {
                                (if (x0) < (x1) { (x0) } else { (x1) })
                            } else {
                                (x2)
                            });
                            let mut box_y0: f32 = (if (if (y0) < (y1) { (y0) } else { (y1) }) < (y2)
                            {
                                (if (y0) < (y1) { (y0) } else { (y1) })
                            } else {
                                (y2)
                            });
                            let mut box_x1: f32 = (if (if (x0) < (x1) { (x1) } else { (x0) }) < (x2)
                            {
                                (x2)
                            } else {
                                (if (x0) < (x1) { (x1) } else { (x0) })
                            });
                            let mut box_y1: f32 = (if (if (y0) < (y1) { (y1) } else { (y0) }) < (y2)
                            {
                                (y2)
                            } else {
                                (if (y0) < (y1) { (y1) } else { (y0) })
                            });
                            if sx > box_x0 - min_dist
                                && sx < box_x1 + min_dist
                                && sy > box_y0 - min_dist
                                && sy < box_y1 + min_dist
                            {
                                let mut num: i32 = 0;
                                let mut ax: f32 = x1 - x0;
                                let mut ay: f32 = y1 - y0;
                                let mut bx: f32 = x0 - ((2) as f32) * x1 + x2;
                                let mut by: f32 = y0 - ((2) as f32) * y1 + y2;
                                let mut mx: f32 = x0 - sx;
                                let mut my: f32 = y0 - sy;
                                let mut res: [f32; 3] = [0.0f32, 0.0f32, 0.0f32];
                                let mut px: f32 = 0.0f32;
                                let mut py: f32 = 0.0f32;
                                let mut t: f32 = 0.0f32;
                                let mut it: f32 = 0.0f32;
                                let mut dist2: f32 = 0.0f32;
                                let mut a_inv: f32 = *precompute.offset((i) as isize);
                                if ((a_inv) as f64) == 0.032 {
                                    let mut a: f32 = ((3) as f32) * (ax * bx + ay * by);
                                    let mut b: f32 =
                                        ((2) as f32) * (ax * ax + ay * ay) + (mx * bx + my * by);
                                    let mut c: f32 = mx * ax + my * ay;
                                    if ((a) as f64) == 0.032 {
                                        if ((b) as f64) != 0.032 {
                                            res[(c_runtime::postInc(&mut num)) as usize] =
                                                ((-c / b) as f32);
                                        }
                                    } else {
                                        let mut discriminant: f32 = b * b - ((4) as f32) * a * c;
                                        if discriminant < ((0) as f32) {
                                            num = ((0) as i32);
                                        } else {
                                            let mut root: f32 =
                                                ((c_runtime::sqrt(((discriminant) as f32))) as f32);
                                            res[(0) as usize] = ((-b - root) / (((2) as f32) * a));
                                            res[(1) as usize] = ((-b + root) / (((2) as f32) * a));
                                            num = ((2) as i32);
                                        }
                                    }
                                } else {
                                    let mut b: f32 = ((3) as f32) * (ax * bx + ay * by) * a_inv;
                                    let mut c: f32 = (((2) as f32) * (ax * ax + ay * ay)
                                        + (mx * bx + my * by))
                                        * a_inv;
                                    let mut d: f32 = (mx * ax + my * ay) * a_inv;
                                    num = ((stbtt__solve_cubic(
                                        b,
                                        c,
                                        d,
                                        ((res.as_mut_ptr()) as *mut f32),
                                    )) as i32);
                                }
                                dist2 = (((x0 - sx) * (x0 - sx) + (y0 - sy) * (y0 - sy)) as f32);
                                if dist2 < min_dist * min_dist {
                                    min_dist = ((c_runtime::sqrt(((dist2) as f32))) as f32);
                                }
                                if num >= 1
                                    && res[(0) as usize] >= 0.0f32
                                    && res[(0) as usize] <= 1.0f32
                                {
                                    t = ((res[(0) as usize]) as f32);
                                    it = ((1.0f32 - t) as f32);
                                    px = (it * it * x0 + ((2) as f32) * t * it * x1 + t * t * x2);
                                    py = (it * it * y0 + ((2) as f32) * t * it * y1 + t * t * y2);
                                    dist2 =
                                        (((px - sx) * (px - sx) + (py - sy) * (py - sy)) as f32);
                                    if dist2 < min_dist * min_dist {
                                        min_dist = ((c_runtime::sqrt(((dist2) as f32))) as f32);
                                    }
                                }
                                if num >= 2
                                    && res[(1) as usize] >= 0.0f32
                                    && res[(1) as usize] <= 1.0f32
                                {
                                    t = ((res[(1) as usize]) as f32);
                                    it = ((1.0f32 - t) as f32);
                                    px = (it * it * x0 + ((2) as f32) * t * it * x1 + t * t * x2);
                                    py = (it * it * y0 + ((2) as f32) * t * it * y1 + t * t * y2);
                                    dist2 =
                                        (((px - sx) * (px - sx) + (py - sy) * (py - sy)) as f32);
                                    if dist2 < min_dist * min_dist {
                                        min_dist = ((c_runtime::sqrt(((dist2) as f32))) as f32);
                                    }
                                }
                                if num >= 3
                                    && res[(2) as usize] >= 0.0f32
                                    && res[(2) as usize] <= 1.0f32
                                {
                                    t = ((res[(2) as usize]) as f32);
                                    it = ((1.0f32 - t) as f32);
                                    px = (it * it * x0 + ((2) as f32) * t * it * x1 + t * t * x2);
                                    py = (it * it * y0 + ((2) as f32) * t * it * y1 + t * t * y2);
                                    dist2 =
                                        (((px - sx) * (px - sx) + (py - sy) * (py - sy)) as f32);
                                    if dist2 < min_dist * min_dist {
                                        min_dist = ((c_runtime::sqrt(((dist2) as f32))) as f32);
                                    }
                                }
                            }
                        }
                    }
                    c_runtime::preInc(&mut i);
                }
                if winding == 0 {
                    min_dist = ((-min_dist) as f32);
                }
                val = ((((onedge_value) as i32) as f32) + pixel_dist_scale * min_dist);
                if val < ((0) as f32) {
                    val = ((0) as f32);
                } else {
                    if val > ((255) as f32) {
                        val = ((255) as f32);
                    }
                }
                *data.offset(((y - iy0) * w + (x - ix0)) as isize) = ((val) as u8);
                c_runtime::preInc(&mut x);
            }
            c_runtime::preInc(&mut y);
        }
        c_runtime::free(((precompute) as *mut u8));
        c_runtime::free(((verts) as *mut u8));
    }
    return data;
}

pub unsafe fn stbtt_GetGlyphShape(
    mut info: *mut stbtt_fontinfo,
    mut glyph_index: i32,
    mut pvertices: *mut *mut stbtt_vertex,
) -> i32 {
    if (*info).cff.size == 0 {
        return ((stbtt__GetGlyphShapeTT(info, glyph_index, pvertices)) as i32);
    } else {
        return ((stbtt__GetGlyphShapeT2(info, glyph_index, pvertices)) as i32);
    }
}

pub unsafe fn stbtt_GetGlyphSVG(
    mut info: *mut stbtt_fontinfo,
    mut gl: i32,
    mut svg: *mut *mut i8,
) -> i32 {
    let mut data: *const u8 = (*info).data;
    let mut svg_doc: *const u8 = core::ptr::null_mut();
    if (*info).svg == 0 {
        return ((0) as i32);
    }
    svg_doc = stbtt_FindSVGDoc(info, gl);
    if svg_doc != core::ptr::null_mut() {
        *svg = (((data) as *mut i8).offset(((*info).svg) as isize))
            .offset((ttULONG((svg_doc).offset((4) as isize))) as isize);
        return ((ttULONG((svg_doc).offset((8) as isize))) as i32);
    } else {
        return ((0) as i32);
    }
}

pub unsafe fn stbtt_GetKerningTable(
    mut info: *mut stbtt_fontinfo,
    mut table: *mut stbtt_kerningentry,
    mut table_length: i32,
) -> i32 {
    let mut data: *const u8 = ((*info).data).offset(((*info).kern) as isize);
    let mut k: i32 = 0;
    let mut length: i32 = 0;
    if (*info).kern == 0 {
        return ((0) as i32);
    }
    if ((ttUSHORT((data).offset((2) as isize))) as i32) < 1 {
        return ((0) as i32);
    }
    if ((ttUSHORT((data).offset((8) as isize))) as i32) != 1 {
        return ((0) as i32);
    }
    length = ((ttUSHORT((data).offset((10) as isize))) as i32);
    if table_length < length {
        length = ((table_length) as i32);
    }
    k = ((0) as i32);
    while (k < length) {
        (*table.offset((k) as isize)).glyph1 =
            ((ttUSHORT(((data).offset((18) as isize)).offset((k * 6) as isize))) as i32);
        (*table.offset((k) as isize)).glyph2 =
            ((ttUSHORT(((data).offset((20) as isize)).offset((k * 6) as isize))) as i32);
        (*table.offset((k) as isize)).advance =
            ((ttSHORT(((data).offset((22) as isize)).offset((k * 6) as isize))) as i32);
        c_runtime::postInc(&mut k);
    }
    return ((length) as i32);
}

pub unsafe fn stbtt_GetKerningTableLength(mut info: *mut stbtt_fontinfo) -> i32 {
    let mut data: *const u8 = ((*info).data).offset(((*info).kern) as isize);
    if (*info).kern == 0 {
        return ((0) as i32);
    }
    if ((ttUSHORT((data).offset((2) as isize))) as i32) < 1 {
        return ((0) as i32);
    }
    if ((ttUSHORT((data).offset((8) as isize))) as i32) != 1 {
        return ((0) as i32);
    }
    return ((ttUSHORT((data).offset((10) as isize))) as i32);
}

pub unsafe fn stbtt_InitFont(
    mut info: *mut stbtt_fontinfo,
    mut data: *const u8,
    mut offset: i32,
) -> i32 {
    return ((stbtt_InitFont_internal(info, data, offset)) as i32);
}

pub unsafe fn stbtt_InitFont_internal(
    mut info: *mut stbtt_fontinfo,
    mut data: *const u8,
    mut fontstart: i32,
) -> i32 {
    let mut cmap: u32 = 0;
    let mut t: u32 = 0;
    let mut i: i32 = 0;
    let mut numTables: i32 = 0;
    (*info).data = data;
    (*info).fontstart = ((fontstart) as i32);
    (*info).cff = ((stbtt__new_buf(core::ptr::null_mut(), ((0) as u64))) as stbtt__buf);
    cmap = (stbtt__find_table(data, ((fontstart) as u32), "cmap"));
    (*info).loca = ((stbtt__find_table(data, ((fontstart) as u32), "loca")) as i32);
    (*info).head = ((stbtt__find_table(data, ((fontstart) as u32), "head")) as i32);
    (*info).glyf = ((stbtt__find_table(data, ((fontstart) as u32), "glyf")) as i32);
    (*info).hhea = ((stbtt__find_table(data, ((fontstart) as u32), "hhea")) as i32);
    (*info).hmtx = ((stbtt__find_table(data, ((fontstart) as u32), "hmtx")) as i32);
    (*info).kern = ((stbtt__find_table(data, ((fontstart) as u32), "kern")) as i32);
    (*info).gpos = ((stbtt__find_table(data, ((fontstart) as u32), "GPOS")) as i32);
    if cmap == 0 || (*info).head == 0 || (*info).hhea == 0 || (*info).hmtx == 0 {
        return ((0) as i32);
    }
    if ((*info).glyf) != 0 {
        if (*info).loca == 0 {
            return ((0) as i32);
        }
    } else {
        let mut b: stbtt__buf = stbtt__buf::default();
        let mut topdict: stbtt__buf = stbtt__buf::default();
        let mut topdictidx: stbtt__buf = stbtt__buf::default();
        let mut cstype: u32 = ((2) as u32);
        let mut charstrings: u32 = ((0) as u32);
        let mut fdarrayoff: u32 = ((0) as u32);
        let mut fdselectoff: u32 = ((0) as u32);
        let mut cff: u32 = 0;
        cff = (stbtt__find_table(data, ((fontstart) as u32), "CFF "));
        if cff == 0 {
            return ((0) as i32);
        }
        (*info).fontdicts = ((stbtt__new_buf(core::ptr::null_mut(), ((0) as u64))) as stbtt__buf);
        (*info).fdselect = ((stbtt__new_buf(core::ptr::null_mut(), ((0) as u64))) as stbtt__buf);
        (*info).cff = ((stbtt__new_buf((data).offset((cff) as isize), ((512 * 1024 * 1024) as u64)))
            as stbtt__buf);
        b = (((*info).cff) as stbtt__buf);
        stbtt__buf_skip(((&mut b) as *mut stbtt__buf), 2);
        stbtt__buf_seek(
            ((&mut b) as *mut stbtt__buf),
            ((stbtt__buf_get8(((&mut b) as *mut stbtt__buf))) as i32),
        );
        stbtt__cff_get_index(((&mut b) as *mut stbtt__buf));
        topdictidx = ((stbtt__cff_get_index(((&mut b) as *mut stbtt__buf))) as stbtt__buf);
        topdict = ((stbtt__cff_index_get(topdictidx, 0)) as stbtt__buf);
        stbtt__cff_get_index(((&mut b) as *mut stbtt__buf));
        (*info).gsubrs = ((stbtt__cff_get_index(((&mut b) as *mut stbtt__buf))) as stbtt__buf);
        stbtt__dict_get_ints(
            ((&mut topdict) as *mut stbtt__buf),
            17,
            1,
            ((&mut charstrings) as *mut u32),
        );
        stbtt__dict_get_ints(
            ((&mut topdict) as *mut stbtt__buf),
            0x100 | 6,
            1,
            ((&mut cstype) as *mut u32),
        );
        stbtt__dict_get_ints(
            ((&mut topdict) as *mut stbtt__buf),
            0x100 | 36,
            1,
            ((&mut fdarrayoff) as *mut u32),
        );
        stbtt__dict_get_ints(
            ((&mut topdict) as *mut stbtt__buf),
            0x100 | 37,
            1,
            ((&mut fdselectoff) as *mut u32),
        );
        (*info).subrs = ((stbtt__get_subrs(b, topdict)) as stbtt__buf);
        if cstype != ((2) as u32) {
            return ((0) as i32);
        }
        if charstrings == ((0) as u32) {
            return ((0) as i32);
        }
        if (fdarrayoff) != 0 {
            if fdselectoff == 0 {
                return ((0) as i32);
            }
            stbtt__buf_seek(((&mut b) as *mut stbtt__buf), ((fdarrayoff) as i32));
            (*info).fontdicts =
                ((stbtt__cff_get_index(((&mut b) as *mut stbtt__buf))) as stbtt__buf);
            (*info).fdselect = ((stbtt__buf_range(
                ((&mut b) as *mut stbtt__buf),
                ((fdselectoff) as i32),
                ((((b.size) as u32) - fdselectoff) as i32),
            )) as stbtt__buf);
        }
        stbtt__buf_seek(((&mut b) as *mut stbtt__buf), ((charstrings) as i32));
        (*info).charstrings = ((stbtt__cff_get_index(((&mut b) as *mut stbtt__buf))) as stbtt__buf);
    }
    t = (stbtt__find_table(data, ((fontstart) as u32), "maxp"));
    if (t) != 0 {
        (*info).numGlyphs = ((ttUSHORT(((data).offset((t) as isize)).offset((4) as isize))) as i32);
    } else {
        (*info).numGlyphs = ((0xffff) as i32);
    }
    (*info).svg = ((-1) as i32);
    numTables = ((ttUSHORT(((data).offset((cmap) as isize)).offset((2) as isize))) as i32);
    (*info).index_map = ((0) as i32);
    i = ((0) as i32);
    while (i < numTables) {
        let mut encoding_record: u32 = cmap + ((4) as u32) + ((8 * i) as u32);
        if ((ttUSHORT((data).offset((encoding_record) as isize))) as i32)
            == STBTT_PLATFORM_ID_MICROSOFT
        {
            if ((ttUSHORT(((data).offset((encoding_record) as isize)).offset((2) as isize))) as i32)
                == STBTT_MS_EID_UNICODE_BMP
                || ((ttUSHORT(((data).offset((encoding_record) as isize)).offset((2) as isize)))
                    as i32)
                    == STBTT_MS_EID_UNICODE_FULL
            {
                (*info).index_map = ((cmap
                    + ttULONG(((data).offset((encoding_record) as isize)).offset((4) as isize)))
                    as i32);
            }
        } else if ((ttUSHORT((data).offset((encoding_record) as isize))) as i32)
            == STBTT_PLATFORM_ID_UNICODE
        {
            (*info).index_map = ((cmap
                + ttULONG(((data).offset((encoding_record) as isize)).offset((4) as isize)))
                as i32);
        }
        c_runtime::preInc(&mut i);
    }
    if (*info).index_map == 0 {
        return ((0) as i32);
    }
    (*info).indexToLocFormat =
        ((ttUSHORT(((data).offset(((*info).head) as isize)).offset((50) as isize))) as i32);
    return ((1) as i32);
}

pub unsafe fn stbtt_IsGlyphEmpty(mut info: *mut stbtt_fontinfo, mut glyph_index: i32) -> i32 {
    let mut numberOfContours: i16 = 0;
    let mut g: i32 = 0;
    if ((*info).cff.size) != 0 {
        return ((if stbtt__GetGlyphInfoT2(
            info,
            glyph_index,
            ((core::ptr::null_mut()) as *mut i32),
            ((core::ptr::null_mut()) as *mut i32),
            ((core::ptr::null_mut()) as *mut i32),
            ((core::ptr::null_mut()) as *mut i32),
        ) == 0
        {
            1
        } else {
            0
        }) as i32);
    }
    g = ((stbtt__GetGlyfOffset(info, glyph_index)) as i32);
    if g < 0 {
        return ((1) as i32);
    }
    numberOfContours = ((ttSHORT(((*info).data).offset((g) as isize))) as i16);
    return if ((numberOfContours) as i32) == 0 {
        1
    } else {
        0
    };
}

pub unsafe fn stbtt_MakeCodepointBitmap(
    mut info: *mut stbtt_fontinfo,
    mut output: *mut u8,
    mut out_w: i32,
    mut out_h: i32,
    mut out_stride: i32,
    mut scale_x: f32,
    mut scale_y: f32,
    mut codepoint: i32,
) {
    stbtt_MakeCodepointBitmapSubpixel(
        info, output, out_w, out_h, out_stride, scale_x, scale_y, 0.0f32, 0.0f32, codepoint,
    );
}

pub unsafe fn stbtt_MakeCodepointBitmapSubpixel(
    mut info: *mut stbtt_fontinfo,
    mut output: *mut u8,
    mut out_w: i32,
    mut out_h: i32,
    mut out_stride: i32,
    mut scale_x: f32,
    mut scale_y: f32,
    mut shift_x: f32,
    mut shift_y: f32,
    mut codepoint: i32,
) {
    stbtt_MakeGlyphBitmapSubpixel(
        info,
        output,
        out_w,
        out_h,
        out_stride,
        scale_x,
        scale_y,
        shift_x,
        shift_y,
        ((stbtt_FindGlyphIndex(info, codepoint)) as i32),
    );
}

pub unsafe fn stbtt_MakeCodepointBitmapSubpixelPrefilter(
    mut info: *mut stbtt_fontinfo,
    mut output: *mut u8,
    mut out_w: i32,
    mut out_h: i32,
    mut out_stride: i32,
    mut scale_x: f32,
    mut scale_y: f32,
    mut shift_x: f32,
    mut shift_y: f32,
    mut oversample_x: i32,
    mut oversample_y: i32,
    mut sub_x: *mut f32,
    mut sub_y: *mut f32,
    mut codepoint: i32,
) {
    stbtt_MakeGlyphBitmapSubpixelPrefilter(
        info,
        output,
        out_w,
        out_h,
        out_stride,
        scale_x,
        scale_y,
        shift_x,
        shift_y,
        oversample_x,
        oversample_y,
        sub_x,
        sub_y,
        ((stbtt_FindGlyphIndex(info, codepoint)) as i32),
    );
}

pub unsafe fn stbtt_MakeGlyphBitmap(
    mut info: *mut stbtt_fontinfo,
    mut output: *mut u8,
    mut out_w: i32,
    mut out_h: i32,
    mut out_stride: i32,
    mut scale_x: f32,
    mut scale_y: f32,
    mut glyph: i32,
) {
    stbtt_MakeGlyphBitmapSubpixel(
        info, output, out_w, out_h, out_stride, scale_x, scale_y, 0.0f32, 0.0f32, glyph,
    );
}

pub unsafe fn stbtt_MakeGlyphBitmapSubpixel(
    mut info: *mut stbtt_fontinfo,
    mut output: *mut u8,
    mut out_w: i32,
    mut out_h: i32,
    mut out_stride: i32,
    mut scale_x: f32,
    mut scale_y: f32,
    mut shift_x: f32,
    mut shift_y: f32,
    mut glyph: i32,
) {
    let mut ix0: i32 = 0;
    let mut iy0: i32 = 0;
    let mut vertices: *mut stbtt_vertex = core::ptr::null_mut();
    let mut num_verts: i32 =
        stbtt_GetGlyphShape(info, glyph, ((&mut vertices) as *mut *mut stbtt_vertex));
    let mut gbm: stbtt__bitmap = stbtt__bitmap::default();
    stbtt_GetGlyphBitmapBoxSubpixel(
        info,
        glyph,
        scale_x,
        scale_y,
        shift_x,
        shift_y,
        ((&mut ix0) as *mut i32),
        ((&mut iy0) as *mut i32),
        (((core::ptr::null_mut()) as *mut i32) as *mut i32),
        (((core::ptr::null_mut()) as *mut i32) as *mut i32),
    );
    gbm.pixels = output;
    gbm.w = ((out_w) as i32);
    gbm.h = ((out_h) as i32);
    gbm.stride = ((out_stride) as i32);
    if (gbm.w) != 0 && (gbm.h) != 0 {
        stbtt_Rasterize(
            ((&mut gbm) as *mut stbtt__bitmap),
            0.35f32,
            vertices,
            num_verts,
            scale_x,
            scale_y,
            shift_x,
            shift_y,
            ix0,
            iy0,
            1,
            (*info).userdata,
        );
    }
    c_runtime::free(((vertices) as *mut u8));
}

pub unsafe fn stbtt_MakeGlyphBitmapSubpixelPrefilter(
    mut info: *mut stbtt_fontinfo,
    mut output: *mut u8,
    mut out_w: i32,
    mut out_h: i32,
    mut out_stride: i32,
    mut scale_x: f32,
    mut scale_y: f32,
    mut shift_x: f32,
    mut shift_y: f32,
    mut prefilter_x: i32,
    mut prefilter_y: i32,
    mut sub_x: *mut f32,
    mut sub_y: *mut f32,
    mut glyph: i32,
) {
    stbtt_MakeGlyphBitmapSubpixel(
        info,
        output,
        out_w - (prefilter_x - 1),
        out_h - (prefilter_y - 1),
        out_stride,
        scale_x,
        scale_y,
        shift_x,
        shift_y,
        glyph,
    );
    if prefilter_x > 1 {
        stbtt__h_prefilter(output, out_w, out_h, out_stride, ((prefilter_x) as u32));
    }
    if prefilter_y > 1 {
        stbtt__v_prefilter(output, out_w, out_h, out_stride, ((prefilter_y) as u32));
    }
    *sub_x = ((stbtt__oversample_shift(prefilter_x)) as f32);
    *sub_y = ((stbtt__oversample_shift(prefilter_y)) as f32);
}

pub unsafe fn stbtt_PackFontRangesGatherRects(
    mut spc: *mut stbtt_pack_context,
    mut info: *mut stbtt_fontinfo,
    mut ranges: *mut stbtt_pack_range,
    mut num_ranges: i32,
    mut rects: *mut stbrp_rect,
) -> i32 {
    let mut i: i32 = 0;
    let mut j: i32 = 0;
    let mut k: i32 = 0;
    let mut missing_glyph_added: i32 = 0;
    k = ((0) as i32);
    i = ((0) as i32);
    while (i < num_ranges) {
        let mut fh: f32 = (*ranges.offset((i) as isize)).font_size;
        let mut scale: f32 = if fh > ((0) as f32) {
            stbtt_ScaleForPixelHeight(info, fh)
        } else {
            stbtt_ScaleForMappingEmToPixels(info, -fh)
        };
        (*ranges.offset((i) as isize)).h_oversample = (((*spc).h_oversample) as u8);
        (*ranges.offset((i) as isize)).v_oversample = (((*spc).v_oversample) as u8);
        j = ((0) as i32);
        while (j < (*ranges.offset((i) as isize)).num_chars) {
            let mut x0: i32 = 0;
            let mut y0: i32 = 0;
            let mut x1: i32 = 0;
            let mut y1: i32 = 0;
            let mut codepoint: i32 = if (*ranges.offset((i) as isize)).array_of_unicode_codepoints
                == core::ptr::null_mut()
            {
                (*ranges.offset((i) as isize)).first_unicode_codepoint_in_range + j
            } else {
                *(*ranges.offset((i) as isize))
                    .array_of_unicode_codepoints
                    .offset((j) as isize)
            };
            let mut glyph: i32 = stbtt_FindGlyphIndex(info, codepoint);
            if glyph == 0 && (((*spc).skip_missing) != 0 || (missing_glyph_added) != 0) {
                let hebron_tmp0 = 0;
                (*rects.offset((k) as isize)).w = hebron_tmp0;
                (*rects.offset((k) as isize)).h = hebron_tmp0;
            } else {
                stbtt_GetGlyphBitmapBoxSubpixel(
                    info,
                    glyph,
                    scale * (((*spc).h_oversample) as f32),
                    scale * (((*spc).v_oversample) as f32),
                    ((0) as f32),
                    ((0) as f32),
                    ((&mut x0) as *mut i32),
                    ((&mut y0) as *mut i32),
                    ((&mut x1) as *mut i32),
                    ((&mut y1) as *mut i32),
                );
                (*rects.offset((k) as isize)).w = ((((x1 - x0 + (*spc).padding) as u32)
                    + (*spc).h_oversample
                    - ((1) as u32)) as i32);
                (*rects.offset((k) as isize)).h = ((((y1 - y0 + (*spc).padding) as u32)
                    + (*spc).v_oversample
                    - ((1) as u32)) as i32);
                if glyph == 0 {
                    missing_glyph_added = ((1) as i32);
                }
            }
            c_runtime::preInc(&mut k);
            c_runtime::preInc(&mut j);
        }
        c_runtime::preInc(&mut i);
    }
    return ((k) as i32);
}

pub unsafe fn stbtt_PackFontRangesRenderIntoRects(
    mut spc: *mut stbtt_pack_context,
    mut info: *mut stbtt_fontinfo,
    mut ranges: *mut stbtt_pack_range,
    mut num_ranges: i32,
    mut rects: *mut stbrp_rect,
) -> i32 {
    let mut i: i32 = 0;
    let mut j: i32 = 0;
    let mut k: i32 = 0;
    let mut missing_glyph: i32 = -1;
    let mut return_value: i32 = 1;
    let mut old_h_over: i32 = (((*spc).h_oversample) as i32);
    let mut old_v_over: i32 = (((*spc).v_oversample) as i32);
    k = ((0) as i32);
    i = ((0) as i32);
    while (i < num_ranges) {
        let mut fh: f32 = (*ranges.offset((i) as isize)).font_size;
        let mut scale: f32 = if fh > ((0) as f32) {
            stbtt_ScaleForPixelHeight(info, fh)
        } else {
            stbtt_ScaleForMappingEmToPixels(info, -fh)
        };
        let mut recip_h: f32 = 0.0f32;
        let mut recip_v: f32 = 0.0f32;
        let mut sub_x: f32 = 0.0f32;
        let mut sub_y: f32 = 0.0f32;
        (*spc).h_oversample = (((*ranges.offset((i) as isize)).h_oversample) as u32);
        (*spc).v_oversample = (((*ranges.offset((i) as isize)).v_oversample) as u32);
        recip_h = (1.0f32 / (((*spc).h_oversample) as f32));
        recip_v = (1.0f32 / (((*spc).v_oversample) as f32));
        sub_x = ((stbtt__oversample_shift((((*spc).h_oversample) as i32))) as f32);
        sub_y = ((stbtt__oversample_shift((((*spc).v_oversample) as i32))) as f32);
        j = ((0) as i32);
        while (j < (*ranges.offset((i) as isize)).num_chars) {
            let mut r: *mut stbrp_rect = &mut *rects.offset((k) as isize);
            if ((*r).was_packed) != 0 && (*r).w != 0 && (*r).h != 0 {
                let mut bc: *mut stbtt_packedchar = &mut *(*ranges.offset((i) as isize))
                    .chardata_for_range
                    .offset((j) as isize);
                let mut advance: i32 = 0;
                let mut lsb: i32 = 0;
                let mut x0: i32 = 0;
                let mut y0: i32 = 0;
                let mut x1: i32 = 0;
                let mut y1: i32 = 0;
                let mut codepoint: i32 = if (*ranges.offset((i) as isize))
                    .array_of_unicode_codepoints
                    == core::ptr::null_mut()
                {
                    (*ranges.offset((i) as isize)).first_unicode_codepoint_in_range + j
                } else {
                    *(*ranges.offset((i) as isize))
                        .array_of_unicode_codepoints
                        .offset((j) as isize)
                };
                let mut glyph: i32 = stbtt_FindGlyphIndex(info, codepoint);
                let mut pad: i32 = (*spc).padding;
                (*r).x += ((pad) as i32);
                (*r).y += ((pad) as i32);
                (*r).w -= ((pad) as i32);
                (*r).h -= ((pad) as i32);
                stbtt_GetGlyphHMetrics(
                    info,
                    glyph,
                    ((&mut advance) as *mut i32),
                    ((&mut lsb) as *mut i32),
                );
                stbtt_GetGlyphBitmapBox(
                    info,
                    glyph,
                    scale * (((*spc).h_oversample) as f32),
                    scale * (((*spc).v_oversample) as f32),
                    ((&mut x0) as *mut i32),
                    ((&mut y0) as *mut i32),
                    ((&mut x1) as *mut i32),
                    ((&mut y1) as *mut i32),
                );
                stbtt_MakeGlyphBitmapSubpixel(
                    info,
                    (((*spc).pixels).offset(((*r).x) as isize))
                        .offset(((*r).y * (*spc).stride_in_bytes) as isize),
                    (((((*r).w) as u32) - (*spc).h_oversample + ((1) as u32)) as i32),
                    (((((*r).h) as u32) - (*spc).v_oversample + ((1) as u32)) as i32),
                    (*spc).stride_in_bytes,
                    scale * (((*spc).h_oversample) as f32),
                    scale * (((*spc).v_oversample) as f32),
                    ((0) as f32),
                    ((0) as f32),
                    glyph,
                );
                if (*spc).h_oversample > ((1) as u32) {
                    stbtt__h_prefilter(
                        (((*spc).pixels).offset(((*r).x) as isize))
                            .offset(((*r).y * (*spc).stride_in_bytes) as isize),
                        (*r).w,
                        (*r).h,
                        (*spc).stride_in_bytes,
                        (*spc).h_oversample,
                    );
                }
                if (*spc).v_oversample > ((1) as u32) {
                    stbtt__v_prefilter(
                        (((*spc).pixels).offset(((*r).x) as isize))
                            .offset(((*r).y * (*spc).stride_in_bytes) as isize),
                        (*r).w,
                        (*r).h,
                        (*spc).stride_in_bytes,
                        (*spc).v_oversample,
                    );
                }
                (*bc).x0 = ((((*r).x) as i16) as u16);
                (*bc).y0 = ((((*r).y) as i16) as u16);
                (*bc).x1 = ((((*r).x + (*r).w) as i16) as u16);
                (*bc).y1 = ((((*r).y + (*r).h) as i16) as u16);
                (*bc).xadvance = (scale * ((advance) as f32));
                (*bc).xoff = (((x0) as f32) * recip_h + sub_x);
                (*bc).yoff = (((y0) as f32) * recip_v + sub_y);
                (*bc).xoff2 = (((x0 + (*r).w) as f32) * recip_h + sub_x);
                (*bc).yoff2 = (((y0 + (*r).h) as f32) * recip_v + sub_y);
                if glyph == 0 {
                    missing_glyph = ((j) as i32);
                }
            } else {
                if ((*spc).skip_missing) != 0 {
                    return_value = ((0) as i32);
                } else {
                    if ((*r).was_packed) != 0 && (*r).w == 0 && (*r).h == 0 && missing_glyph >= 0 {
                        *(*ranges.offset((i) as isize))
                            .chardata_for_range
                            .offset((j) as isize) = ((*(*ranges.offset((i) as isize))
                            .chardata_for_range
                            .offset((missing_glyph) as isize))
                            as stbtt_packedchar);
                    } else {
                        return_value = ((0) as i32);
                    }
                }
            }
            c_runtime::preInc(&mut k);
            c_runtime::preInc(&mut j);
        }
        c_runtime::preInc(&mut i);
    }
    (*spc).h_oversample = ((old_h_over) as u32);
    (*spc).v_oversample = ((old_v_over) as u32);
    return ((return_value) as i32);
}

pub unsafe fn stbtt_ScaleForMappingEmToPixels(
    mut info: *mut stbtt_fontinfo,
    mut pixels: f32,
) -> f32 {
    let mut unitsPerEm: i32 =
        ((ttUSHORT((((*info).data).offset(((*info).head) as isize)).offset((18) as isize))) as i32);
    return pixels / ((unitsPerEm) as f32);
}

pub unsafe fn stbtt_ScaleForPixelHeight(mut info: *mut stbtt_fontinfo, mut height: f32) -> f32 {
    let mut fheight: i32 =
        ((ttSHORT((((*info).data).offset(((*info).hhea) as isize)).offset((4) as isize))) as i32)
            - ((ttSHORT((((*info).data).offset(((*info).hhea) as isize)).offset((6) as isize)))
                as i32);
    return height / ((fheight) as f32);
}
