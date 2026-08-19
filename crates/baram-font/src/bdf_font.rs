//! Small read-only BDF 2.1 reader for fixed-pixel fonts.
//!
//! The font remains in the image and glyphs are scanned on demand. This is
//! intentional: materializing all 7,170 glyphs would consume more RAM than
//! Xiao's compact-runtime budget.

#![allow(dead_code)]

static mut DATA: Option<&'static [u8]> = None;

pub fn init(data: &'static [u8]) {
    unsafe { DATA = Some(data); }
}

pub fn is_available() -> bool {
    unsafe { DATA.is_some() }
}

pub fn with_glyph<F>(ch: char, mut draw: F) -> bool
where
    F: FnMut(&[u8], i32, i32, i32, i32) -> bool,
{
    let Some(data) = (unsafe { DATA }) else { return false };
    let wanted = ch as u32;
    let Ok(text) = core::str::from_utf8(data) else { return false };
    let mut encoding = u32::MAX;
    let mut advance = 8i32;
    let mut width = 0i32;
    let mut height = 0i32;
    let mut y_off = 0i32;
    let mut bitmap = [0u8; 64];
    let mut bitmap_row = 0usize;
    let mut in_bitmap = false;

    for line in text.lines() {
        if line == "STARTCHAR" || line.starts_with("STARTCHAR ") {
            encoding = u32::MAX;
            advance = 8;
            width = 0;
            height = 0;
            y_off = 0;
            bitmap_row = 0;
            in_bitmap = false;
        } else if let Some(value) = line.strip_prefix("ENCODING ") {
            encoding = value.split_whitespace().next().and_then(parse_u32).unwrap_or(u32::MAX);
        } else if let Some(value) = line.strip_prefix("DWIDTH ") {
            advance = value.split_whitespace().next().and_then(parse_i32).unwrap_or(8);
        } else if let Some(value) = line.strip_prefix("BBX ") {
            let mut values = value.split_whitespace().filter_map(parse_i32);
            width = values.next().unwrap_or(0).clamp(0, 8);
            height = values.next().unwrap_or(0).clamp(0, 8);
            let _x_off = values.next().unwrap_or(0);
            y_off = values.next().unwrap_or(0);
        } else if line == "BITMAP" {
            in_bitmap = true;
            bitmap_row = 0;
        } else if line == "ENDCHAR" {
            if encoding == wanted && width > 0 && height > 0 {
                return draw(&bitmap[..(width * height) as usize], width, height, advance, y_off);
            }
            in_bitmap = false;
        } else if in_bitmap && bitmap_row < height as usize {
            let value = u32::from_str_radix(line.trim(), 16).unwrap_or(0);
            for col in 0..width as usize {
                let bit = 7usize.saturating_sub(col);
                bitmap[bitmap_row * width as usize + col] =
                    if (value >> bit) & 1 == 1 { 255 } else { 0 };
            }
            bitmap_row += 1;
        }
    }
    false
}

pub fn advance(ch: char) -> i32 {
    let mut result = 8;
    let _ = with_glyph(ch, |_data, _w, _h, glyph_advance, _y_off| {
        result = glyph_advance;
        true
    });
    result
}

fn parse_u32(value: &str) -> Option<u32> { value.parse().ok() }
fn parse_i32(value: &str) -> Option<i32> { value.parse().ok() }
