//! Minimal 8x16 bitmap font (VGA-style).
//!
//! Each glyph is 16 bytes; each byte is one scanline, MSB = leftmost pixel.
//! Only printable ASCII (0x20..0x7E) is included; everything else renders as a
//! blank glyph.  The data lives in `font_data.rs` (auto-generated).

/// Width of every glyph in pixels.
pub const GLYPH_W: usize = 8;
/// Height of every glyph in pixels.
pub const GLYPH_H: usize = 16;

/// Number of glyphs stored in the table (covers 0x20..0x7F).
const FIRST_CHAR: u8 = 0x20;
const LAST_CHAR: u8 = 0x7E;

include!("font_data.rs");

/// Look up the 16-byte bitmap for `c`.  Returns an all-zero row if the
/// character is outside the supported range, so callers can render any byte
/// without bounds-checking.
pub fn glyph(c: u8) -> &'static [u8; GLYPH_H] {
    if (FIRST_CHAR..=LAST_CHAR).contains(&c) {
        let idx = (c - FIRST_CHAR) as usize;
        &FONT_DATA[idx]
    } else {
        &BLANK_GLYPH
    }
}

static BLANK_GLYPH: [u8; GLYPH_H] = [0; GLYPH_H];
