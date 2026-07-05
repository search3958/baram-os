//! Minimal 8x16 bitmap font (VGA-style).
//!
//! Each glyph is 16 bytes; each byte is one scanline, MSB = leftmost pixel.
//! Only printable ASCII (0x20..0x7E) is included; everything else renders as a
//! blank glyph.  The data lives in `font_data.rs` (auto-generated).


pub const GLYPH_W: usize = 8;

pub const GLYPH_H: usize = 16;


const FIRST_CHAR: u8 = 0x20;
const LAST_CHAR: u8 = 0x7E;

include!("font_data.rs");




pub fn glyph(c: u8) -> &'static [u8; GLYPH_H] {
    if (FIRST_CHAR..=LAST_CHAR).contains(&c) {
        let idx = (c - FIRST_CHAR) as usize;
        &FONT_DATA[idx]
    } else {
        &BLANK_GLYPH
    }
}

static BLANK_GLYPH: [u8; GLYPH_H] = [0; GLYPH_H];
