extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use stb_truetype_rust::*;

const FONT_SIZE: f32 = 14.0;
const CACHE_INIT_CAP: usize = 128;

static mut FONT_DATA: Option<Vec<u8>> = None;
static mut FONT_INFO: Option<stbtt_fontinfo> = None;
// HarmonyOS Sans is the primary UI face.  The KCC face is retained solely as
// a glyph fallback for Korean characters it does not contain.
static mut KOREAN_FONT_DATA: Option<Vec<u8>> = None;
static mut KOREAN_FONT_INFO: Option<stbtt_fontinfo> = None;
static mut FONT_SCALE: f32 = 0.0;
static mut ASCENT: i32 = 0;
static mut CACHE: Vec<GlyphEntry> = Vec::new();
static mut SIZED_CACHE: Vec<SizedGlyphEntry> = Vec::new();

#[derive(Clone)]
struct GlyphEntry {
    ch: char,
    bitmap: Vec<u8>,
    w: i32,
    h: i32,
    advance: i32,
    y_off: i32,
}

struct SizedGlyphEntry {
    size_bits: u32,
    glyph: GlyphEntry,
}

pub fn init() {
    let data = include_bytes!("../../../data/HarmonyOS_Sans_SC_Regular.ttf");
    let korean_data = include_bytes!("../../../data/GothicA1-Medium.ttf");
    unsafe {
        FONT_DATA = Some(Vec::from(&data[..]));
        let mut info = stbtt_fontinfo::default();
        let font_vec = FONT_DATA.as_ref().unwrap();
        let res = stbtt_InitFont(&mut info, font_vec.as_ptr(), 0);
        if res == 0 {
            FONT_DATA = None;
            return;
        }
        FONT_SCALE = stbtt_ScaleForPixelHeight(&mut info, FONT_SIZE);
        let mut ascent = 0;
        let mut descent = 0;
        let mut line_gap = 0;
        stbtt_GetFontVMetrics(&mut info, &mut ascent, &mut descent, &mut line_gap);
        ASCENT = (ascent as f32 * FONT_SCALE + 0.5) as i32;
        FONT_INFO = Some(info);
        let mut korean_info = stbtt_fontinfo::default();
        KOREAN_FONT_DATA = Some(Vec::from(&korean_data[..]));
        let korean_vec = KOREAN_FONT_DATA.as_ref().unwrap();
        if stbtt_InitFont(&mut korean_info, korean_vec.as_ptr(), 0) != 0 {
            KOREAN_FONT_INFO = Some(korean_info);
        } else {
            KOREAN_FONT_DATA = None;
        }
        CACHE = Vec::with_capacity(CACHE_INIT_CAP);
        SIZED_CACHE = Vec::with_capacity(CACHE_INIT_CAP);
    }
}

unsafe fn glyph_info(ch: char) -> Option<(*mut stbtt_fontinfo, i32, bool)> {
    let primary = FONT_INFO.as_mut()?;
    let glyph_id = stbtt_FindGlyphIndex(primary, ch as i32);
    if glyph_id != 0 || ch == '\0' {
        return Some((primary as *mut _, glyph_id, false));
    }
    let fallback = KOREAN_FONT_INFO.as_mut()?;
    let glyph_id = stbtt_FindGlyphIndex(fallback, ch as i32);
    (glyph_id != 0 || ch == '\0').then_some((fallback as *mut _, glyph_id, true))
}

unsafe fn scaled_ascent(info: &mut stbtt_fontinfo, scale: f32) -> i32 {
    let mut ascent = 0;
    let mut descent = 0;
    let mut line_gap = 0;
    stbtt_GetFontVMetrics(info, &mut ascent, &mut descent, &mut line_gap);
    (ascent as f32 * scale + 0.5) as i32
}

unsafe fn primary_ascent_at_size(pixel_size: f32) -> i32 {
    let Some(info) = FONT_INFO.as_mut() else {
        return 0;
    };
    let scale = stbtt_ScaleForPixelHeight(info, pixel_size);
    scaled_ascent(info, scale)
}

pub fn is_available() -> bool {
    unsafe { FONT_INFO.is_some() }
}

pub fn ascent() -> i32 {
    unsafe { ASCENT }
}

pub struct GlyphBitmap {
    pub data: Vec<u8>,
    pub w: i32,
    pub h: i32,
    pub advance: i32,
    pub y_off: i32,
}

pub fn glyph_at_size(ch: char, pixel_size: f32) -> GlyphBitmap {
    unsafe {
        if FONT_INFO.is_none() {
            return GlyphBitmap {
                data: Vec::new(),
                w: 0,
                h: 0,
                advance: 0,
                y_off: 0,
            };
        }

        let Some((info_ptr, glyph_id, is_fallback)) = glyph_info(ch) else {
            return GlyphBitmap {
                data: Vec::new(),
                w: 0,
                h: 0,
                advance: 0,
                y_off: 0,
            };
        };
        let info = &mut *info_ptr;
        let scale = stbtt_ScaleForPixelHeight(info, pixel_size);

        let mut advance = 0;
        let mut lsb = 0;
        stbtt_GetGlyphHMetrics(info, glyph_id, &mut advance, &mut lsb);
        let scaled_advance = (advance as f32 * scale + 0.5) as i32;

        let mut x0 = 0;
        let mut y0 = 0;
        let mut x1 = 0;
        let mut y1 = 0;
        stbtt_GetGlyphBitmapBox(
            info, glyph_id, scale, scale, &mut x0, &mut y0, &mut x1, &mut y1,
        );

        let gw = x1 - x0;
        let gh = y1 - y0;

        if gw <= 0 || gh <= 0 {
            return GlyphBitmap {
                data: Vec::new(),
                w: 0,
                h: 0,
                advance: scaled_advance,
                y_off: 0,
            };
        }

        let mut bitmap = vec![0u8; (gw * gh) as usize];
        stbtt_MakeGlyphBitmap(
            info,
            bitmap.as_mut_ptr(),
            gw,
            gh,
            gw,
            scale,
            scale,
            glyph_id,
        );

        let y_off = if is_fallback {
            y0 + scaled_ascent(info, scale) - primary_ascent_at_size(pixel_size)
        } else {
            y0
        };
        GlyphBitmap {
            data: bitmap,
            w: gw,
            h: gh,
            advance: scaled_advance,
            y_off,
        }
    }
}

/// Borrow a size-specific cached glyph.  Large Warp3 labels otherwise invoke
/// the TTF rasterizer and allocate a bitmap on every paint.
pub fn with_glyph_at_size<R>(
    ch: char,
    pixel_size: f32,
    paint: impl FnOnce(&[u8], i32, i32, i32, i32) -> R,
) -> R {
    let size_bits = pixel_size.to_bits();
    unsafe {
        if FONT_INFO.is_none() {
            return paint(&[], 0, 0, 0, 0);
        }
        if let Some(entry) = SIZED_CACHE
            .iter()
            .find(|entry| entry.size_bits == size_bits && entry.glyph.ch == ch)
        {
            let glyph = &entry.glyph;
            return paint(&glyph.bitmap, glyph.w, glyph.h, glyph.advance, glyph.y_off);
        }
        let Some((info_ptr, glyph_id, is_fallback)) = glyph_info(ch) else {
            return paint(&[], 0, 0, 0, 0);
        };
        let info = &mut *info_ptr;
        let scale = stbtt_ScaleForPixelHeight(info, pixel_size);
        let mut advance = 0;
        let mut lsb = 0;
        stbtt_GetGlyphHMetrics(info, glyph_id, &mut advance, &mut lsb);
        let scaled_advance = (advance as f32 * scale + 0.5) as i32;
        let mut x0 = 0;
        let mut y0 = 0;
        let mut x1 = 0;
        let mut y1 = 0;
        stbtt_GetGlyphBitmapBox(
            info, glyph_id, scale, scale, &mut x0, &mut y0, &mut x1, &mut y1,
        );
        let w = x1 - x0;
        let h = y1 - y0;
        let mut bitmap = if w > 0 && h > 0 {
            vec![0u8; (w * h) as usize]
        } else {
            Vec::new()
        };
        if !bitmap.is_empty() {
            stbtt_MakeGlyphBitmap(info, bitmap.as_mut_ptr(), w, h, w, scale, scale, glyph_id);
        }
        let y_off = if h > 0 {
            if is_fallback {
                y0 + scaled_ascent(info, scale) - primary_ascent_at_size(pixel_size)
            } else {
                y0
            }
        } else {
            0
        };
        SIZED_CACHE.push(SizedGlyphEntry {
            size_bits,
            glyph: GlyphEntry {
                ch,
                bitmap,
                w: w.max(0),
                h: h.max(0),
                advance: scaled_advance,
                y_off,
            },
        });
        let glyph = &SIZED_CACHE.last().unwrap().glyph;
        paint(&glyph.bitmap, glyph.w, glyph.h, glyph.advance, glyph.y_off)
    }
}

pub fn ascent_at_size(pixel_size: f32) -> i32 {
    unsafe {
        if FONT_INFO.is_none() {
            return 0;
        }
        primary_ascent_at_size(pixel_size)
    }
}

pub fn glyph(ch: char) -> GlyphBitmap {
    unsafe {
        if FONT_INFO.is_none() {
            return GlyphBitmap {
                data: Vec::new(),
                w: 0,
                h: 0,
                advance: 0,
                y_off: 0,
            };
        }

        for entry in CACHE.iter() {
            if entry.ch == ch {
                return GlyphBitmap {
                    data: entry.bitmap.clone(),
                    w: entry.w,
                    h: entry.h,
                    advance: entry.advance,
                    y_off: entry.y_off,
                };
            }
        }

        let Some((info_ptr, glyph_id, is_fallback)) = glyph_info(ch) else {
            let entry = GlyphEntry {
                ch,
                bitmap: Vec::new(),
                w: 0,
                h: 0,
                advance: 0,
                y_off: 0,
            };
            CACHE.push(entry);
            return GlyphBitmap {
                data: Vec::new(),
                w: 0,
                h: 0,
                advance: 0,
                y_off: 0,
            };
        };
        let info = &mut *info_ptr;
        let scale = stbtt_ScaleForPixelHeight(info, FONT_SIZE);

        let mut advance = 0;
        let mut lsb = 0;
        stbtt_GetGlyphHMetrics(info, glyph_id, &mut advance, &mut lsb);
        let scaled_advance = (advance as f32 * scale + 0.5) as i32;

        let mut x0 = 0;
        let mut y0 = 0;
        let mut x1 = 0;
        let mut y1 = 0;
        stbtt_GetGlyphBitmapBox(
            info, glyph_id, scale, scale, &mut x0, &mut y0, &mut x1, &mut y1,
        );

        let gw = x1 - x0;
        let gh = y1 - y0;

        if gw <= 0 || gh <= 0 {
            let entry = GlyphEntry {
                ch,
                bitmap: Vec::new(),
                w: 0,
                h: 0,
                advance: scaled_advance,
                y_off: 0,
            };
            CACHE.push(entry);
            return GlyphBitmap {
                data: Vec::new(),
                w: 0,
                h: 0,
                advance: scaled_advance,
                y_off: 0,
            };
        }

        let mut bitmap = vec![0u8; (gw * gh) as usize];
        stbtt_MakeGlyphBitmap(
            info,
            bitmap.as_mut_ptr(),
            gw,
            gh,
            gw,
            scale,
            scale,
            glyph_id,
        );

        let y_off = if is_fallback {
            y0 + scaled_ascent(info, scale) - ASCENT
        } else {
            y0
        };

        let entry = GlyphEntry {
            ch,
            bitmap: bitmap.clone(),
            w: gw,
            h: gh,
            advance: scaled_advance,
            y_off,
        };
        CACHE.push(entry);

        GlyphBitmap {
            data: bitmap,
            w: gw,
            h: gh,
            advance: scaled_advance,
            y_off,
        }
    }
}

/// Return the real advance width without cloning the cached glyph bitmap.
/// Layout uses this hot path far more often than it needs raster data.
pub fn advance(ch: char) -> i32 {
    unsafe {
        if FONT_INFO.is_none() {
            return 8;
        }
        if let Some(entry) = CACHE.iter().find(|entry| entry.ch == ch) {
            return entry.advance;
        }
    }
    glyph(ch).advance.max(1)
}

/// Borrow a cached raster glyph for immediate painting.  Unlike `glyph`, this
/// does not clone the bitmap on every character, which is critical for small
/// hover/toolbar redraws.
pub fn with_glyph<R>(ch: char, paint: impl FnOnce(&[u8], i32, i32, i32, i32) -> R) -> R {
    unsafe {
        if FONT_INFO.is_none() {
            return paint(&[], 0, 0, 0, 0);
        }
        if let Some(entry) = CACHE.iter().find(|entry| entry.ch == ch) {
            return paint(&entry.bitmap, entry.w, entry.h, entry.advance, entry.y_off);
        }
    }
    let _ = glyph(ch);
    unsafe {
        if let Some(entry) = CACHE.iter().find(|entry| entry.ch == ch) {
            paint(&entry.bitmap, entry.w, entry.h, entry.advance, entry.y_off)
        } else {
            paint(&[], 0, 0, 0, 0)
        }
    }
}
