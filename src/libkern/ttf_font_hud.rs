extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use stb_truetype_rust::*;

const FONT_SIZE: f32 = 16.0;
const CACHE_INIT_CAP: usize = 64;

static mut FONT_DATA: Option<Vec<u8>> = None;
static mut FONT_INFO: Option<stbtt_fontinfo> = None;
static mut FONT_SCALE: f32 = 0.0;
static mut ASCENT: i32 = 0;
static mut CACHE: Vec<GlyphEntry> = Vec::new();

#[derive(Clone)]
struct GlyphEntry {
    ch: char,
    bitmap: Vec<u8>,
    w: i32,
    h: i32,
    advance: i32,
    y_off: i32,
}

pub fn init() {
    let data = include_bytes!("../data/GoogleSansFlex_24pt-Medium.ttf");
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
        CACHE = Vec::with_capacity(CACHE_INIT_CAP);
    }
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
            return GlyphBitmap { data: Vec::new(), w: 0, h: 0, advance: 0, y_off: 0 };
        }

        let info = FONT_INFO.as_mut().unwrap();
        let scale = stb_truetype_rust::stbtt_ScaleForPixelHeight(info, pixel_size);
        let glyph_id = stbtt_FindGlyphIndex(info, ch as i32);
        if glyph_id == 0 && ch != '\0' {
            return GlyphBitmap { data: Vec::new(), w: 0, h: 0, advance: 0, y_off: 0 };
        }

        let mut advance = 0;
        let mut lsb = 0;
        stbtt_GetGlyphHMetrics(info, glyph_id, &mut advance, &mut lsb);
        let scaled_advance = (advance as f32 * scale + 0.5) as i32;

        let mut x0 = 0;
        let mut y0 = 0;
        let mut x1 = 0;
        let mut y1 = 0;
        stbtt_GetGlyphBitmapBox(info, glyph_id, scale, scale, &mut x0, &mut y0, &mut x1, &mut y1);

        let gw = x1 - x0;
        let gh = y1 - y0;

        if gw <= 0 || gh <= 0 {
            return GlyphBitmap { data: Vec::new(), w: 0, h: 0, advance: scaled_advance, y_off: 0 };
        }

        let mut bitmap = vec![0u8; (gw * gh) as usize];
        stbtt_MakeGlyphBitmap(info, bitmap.as_mut_ptr(), gw, gh, gw, scale, scale, glyph_id);

        GlyphBitmap { data: bitmap, w: gw, h: gh, advance: scaled_advance, y_off: y0 }
    }
}

pub fn ascent_at_size(pixel_size: f32) -> i32 {
    unsafe {
        if FONT_INFO.is_none() { return 0; }
        let info = FONT_INFO.as_mut().unwrap();
        let scale = stb_truetype_rust::stbtt_ScaleForPixelHeight(info, pixel_size);
        let mut ascent = 0;
        let mut descent = 0;
        let mut line_gap = 0;
        stbtt_GetFontVMetrics(info, &mut ascent, &mut descent, &mut line_gap);
        (ascent as f32 * scale + 0.5) as i32
    }
}

pub fn glyph(ch: char) -> GlyphBitmap {
    unsafe {
        if FONT_INFO.is_none() {
            return GlyphBitmap { data: Vec::new(), w: 0, h: 0, advance: 0, y_off: 0 };
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

        let info = FONT_INFO.as_mut().unwrap();
        let glyph_id = stbtt_FindGlyphIndex(info, ch as i32);
        if glyph_id == 0 && ch != '\0' {
            let entry = GlyphEntry {
                ch,
                bitmap: Vec::new(),
                w: 0,
                h: 0,
                advance: 0,
                y_off: 0,
            };
            CACHE.push(entry);
            return GlyphBitmap { data: Vec::new(), w: 0, h: 0, advance: 0, y_off: 0 };
        }

        let mut advance = 0;
        let mut lsb = 0;
        stbtt_GetGlyphHMetrics(info, glyph_id, &mut advance, &mut lsb);
        let scaled_advance = (advance as f32 * FONT_SCALE + 0.5) as i32;

        let mut x0 = 0;
        let mut y0 = 0;
        let mut x1 = 0;
        let mut y1 = 0;
        stbtt_GetGlyphBitmapBox(info, glyph_id, FONT_SCALE, FONT_SCALE, &mut x0, &mut y0, &mut x1, &mut y1);

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
            return GlyphBitmap { data: Vec::new(), w: 0, h: 0, advance: scaled_advance, y_off: 0 };
        }

        let mut bitmap = vec![0u8; (gw * gh) as usize];
        stbtt_MakeGlyphBitmap(info, bitmap.as_mut_ptr(), gw, gh, gw, FONT_SCALE, FONT_SCALE, glyph_id);

        let entry = GlyphEntry {
            ch,
            bitmap: bitmap.clone(),
            w: gw,
            h: gh,
            advance: scaled_advance,
            y_off: y0,
        };
        CACHE.push(entry);

        GlyphBitmap { data: bitmap, w: gw, h: gh, advance: scaled_advance, y_off: y0 }
    }
}
