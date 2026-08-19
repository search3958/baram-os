//! Small read-only BDF 2.1 reader for fixed-pixel fonts.
//!
//! The font is kept as a file on the boot volume and glyphs are scanned on
//! demand. The BDF is never copied into a heap buffer. Only glyphs used by
//! the display are copied as small bitmaps (at most 8x8); the cache survives
//! repaint and scrolling so the same text is not rasterized again.

#![allow(dead_code)]

use alloc::vec::Vec;
use uefi::boot;
use uefi::proto::media::file::{File, FileAttribute, FileMode, RegularFile};
use uefi::CStr16;

#[derive(Clone, Copy)]
enum FontSource {
    None,
    Embedded(&'static [u8]),
    File(&'static str),
}

static mut SOURCE: FontSource = FontSource::None;
static mut CACHE: Option<Vec<GlyphCacheEntry>> = None;

struct GlyphCacheEntry {
    ch: char,
    width: i32,
    height: i32,
    advance: i32,
    y_off: i32,
    bitmap: [u8; 64],
}

pub fn init(data: &'static [u8]) {
    unsafe {
        SOURCE = FontSource::Embedded(data);
        CACHE = None;
    }
}

/// Use a BDF file directly from the image filesystem.
///
/// This is the xiao path. `path` is only a small static filename; the BDF
/// contents are read in 512-byte chunks and are not retained after scanning.
pub fn init_file(path: &'static str) {
    unsafe {
        SOURCE = FontSource::File(path);
        CACHE = None;
    }
}

/// Drop every cached glyph explicitly when changing the font source.
/// Normal repaint, pointer clicks, application changes, and scrolling must
/// not call this.
pub fn clear_cache() {
    unsafe {
        CACHE = None;
    }
}

pub fn is_available() -> bool {
    !matches!(unsafe { SOURCE }, FontSource::None)
}

/// Prime the small glyph cache for text that may be shown next. The BDF file
/// remains on storage; only the individual bitmap glyphs are retained.
pub fn preload_text(text: &str) {
    preload_texts(&[text]);
}

/// Prime several strings with one sequential pass over a file-backed BDF.
/// Calling `preload_text` separately for every label would otherwise reopen
/// and rescan the font once per label.
pub fn preload_texts(texts: &[&str]) {
    if !is_available() {
        return;
    }
    if let FontSource::File(path) = unsafe { SOURCE } {
        let mut wanted = Vec::new();
        for text in texts {
            for ch in text.chars() {
                if matches!(ch, '\n' | '\r' | '\t')
                    || cached_glyph(ch).is_some()
                    || wanted.iter().any(|cached| *cached == ch)
                {
                    continue;
                }
                wanted.push(ch);
            }
        }
        if wanted.is_empty() {
            return;
        }
        preload_file_glyphs(path, &wanted);
        for ch in wanted {
            if cached_glyph(ch).is_none() {
                // A zero-size entry is a negative cache. It preserves the
                // default advance without retaining any font data.
                cache_glyph(ch, &[], 0, 0, 8, 0);
            }
        }
        return;
    }
    for text in texts {
        for ch in text.chars() {
            if matches!(ch, '\n' | '\r' | '\t') || cached_glyph(ch).is_some() {
                continue;
            }
            let _ = with_glyph(ch, |_data, _width, _height, _advance, _y_off| true);
        }
    }
}

pub fn with_glyph<F>(ch: char, mut draw: F) -> bool
where
    F: FnMut(&[u8], i32, i32, i32, i32) -> bool,
{
    if let Some(entry) = cached_glyph(ch) {
        return draw(
            &entry.bitmap[..(entry.width * entry.height) as usize],
            entry.width,
            entry.height,
            entry.advance,
            entry.y_off,
        );
    }
    let source = unsafe { SOURCE };
    let drawn = match source {
        FontSource::None => return false,
        // Embedded mode is retained for non-xiao callers and uses the same
        // small persistent glyph cache as the file-backed Xiao path.
        FontSource::Embedded(data) => with_embedded_glyph(data, ch, draw),
        FontSource::File(path) => with_file_glyph(path, ch, draw),
    };
    if !drawn {
        // Cache misses as an empty glyph so advance()/put_str() never scan
        // the storage-backed BDF repeatedly for the same character.
        cache_glyph(ch, &[], 0, 0, 8, 0);
    }
    drawn
}

fn cached_glyph(ch: char) -> Option<GlyphCacheEntry> {
    unsafe {
        CACHE
            .as_ref()?
            .iter()
            .find(|entry| entry.ch == ch)
            .map(|entry| GlyphCacheEntry {
                ch: entry.ch,
                width: entry.width,
                height: entry.height,
                advance: entry.advance,
                y_off: entry.y_off,
                bitmap: entry.bitmap,
            })
    }
}

fn cached_advance(ch: char) -> Option<i32> {
    unsafe {
        CACHE
            .as_ref()?
            .iter()
            .find(|entry| entry.ch == ch)
            .map(|entry| entry.advance)
    }
}

fn cache_glyph(ch: char, bitmap: &[u8], width: i32, height: i32, advance: i32, y_off: i32) {
    let mut cached = [0u8; 64];
    cached[..bitmap.len()].copy_from_slice(bitmap);
    unsafe {
        let cache = CACHE.get_or_insert_with(Vec::new);
        cache.push(GlyphCacheEntry {
            ch,
            width,
            height,
            advance,
            y_off,
            bitmap: cached,
        });
    }
}

fn preload_file_glyphs(path: &str, wanted: &[char]) {
    let Some(file) = open_file(path) else {
        return;
    };
    let mut reader = BdfReader::new(file);
    let mut line = [0u8; 256];
    let mut encoding = u32::MAX;
    let mut advance = 8i32;
    let mut width = 0i32;
    let mut height = 0i32;
    let mut y_off = 0i32;
    let mut bitmap = [0u8; 64];
    let mut bitmap_row = 0usize;
    let mut in_bitmap = false;

    while let Some(len) = reader.line(&mut line) {
        let Ok(line) = core::str::from_utf8(&line[..len]) else {
            continue;
        };
        if line == "STARTCHAR" || line.starts_with("STARTCHAR ") {
            encoding = u32::MAX;
            advance = 8;
            width = 0;
            height = 0;
            y_off = 0;
            bitmap_row = 0;
            in_bitmap = false;
        } else if let Some(value) = line.strip_prefix("ENCODING ") {
            encoding = value
                .split_whitespace()
                .next()
                .and_then(parse_u32)
                .unwrap_or(u32::MAX);
        } else if let Some(value) = line.strip_prefix("DWIDTH ") {
            advance = value
                .split_whitespace()
                .next()
                .and_then(parse_i32)
                .unwrap_or(8);
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
            if let Some(&ch) = wanted.iter().find(|candidate| **candidate as u32 == encoding) {
                cache_glyph(
                    ch,
                    &bitmap[..(width * height) as usize],
                    width,
                    height,
                    advance,
                    y_off,
                );
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
}

fn with_embedded_glyph<F>(data: &'static [u8], ch: char, mut draw: F) -> bool
where
    F: FnMut(&[u8], i32, i32, i32, i32) -> bool,
{
    let wanted = ch as u32;
    let Ok(text) = core::str::from_utf8(data) else {
        return false;
    };
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
            encoding = value
                .split_whitespace()
                .next()
                .and_then(parse_u32)
                .unwrap_or(u32::MAX);
        } else if let Some(value) = line.strip_prefix("DWIDTH ") {
            advance = value
                .split_whitespace()
                .next()
                .and_then(parse_i32)
                .unwrap_or(8);
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
            if encoding == wanted {
                let drawn = draw(
                    &bitmap[..(width * height) as usize],
                    width,
                    height,
                    advance,
                    y_off,
                );
                cache_glyph(
                    ch,
                    &bitmap[..(width * height) as usize],
                    width,
                    height,
                    advance,
                    y_off,
                );
                return drawn;
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

fn with_file_glyph<F>(path: &str, ch: char, mut draw: F) -> bool
where
    F: FnMut(&[u8], i32, i32, i32, i32) -> bool,
{
    let Some(file) = open_file(path) else {
        return false;
    };
    let wanted = ch as u32;
    let mut reader = BdfReader::new(file);
    let mut line = [0u8; 256];
    let mut encoding = u32::MAX;
    let mut advance = 8i32;
    let mut width = 0i32;
    let mut height = 0i32;
    let mut y_off = 0i32;
    let mut bitmap = [0u8; 64];
    let mut bitmap_row = 0usize;
    let mut in_bitmap = false;

    while let Some(len) = reader.line(&mut line) {
        let Ok(line) = core::str::from_utf8(&line[..len]) else {
            continue;
        };
        if line == "STARTCHAR" || line.starts_with("STARTCHAR ") {
            encoding = u32::MAX;
            advance = 8;
            width = 0;
            height = 0;
            y_off = 0;
            bitmap_row = 0;
            in_bitmap = false;
        } else if let Some(value) = line.strip_prefix("ENCODING ") {
            encoding = value
                .split_whitespace()
                .next()
                .and_then(parse_u32)
                .unwrap_or(u32::MAX);
        } else if let Some(value) = line.strip_prefix("DWIDTH ") {
            advance = value
                .split_whitespace()
                .next()
                .and_then(parse_i32)
                .unwrap_or(8);
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
            if encoding == wanted {
                let drawn = draw(
                    &bitmap[..(width * height) as usize],
                    width,
                    height,
                    advance,
                    y_off,
                );
                // File-backed Xiao glyphs use the same persistent display
                // cache as embedded glyphs. Scrolling must repaint from the
                // cached bitmap instead of rescanning the BDF on every frame.
                cache_glyph(
                    ch,
                    &bitmap[..(width * height) as usize],
                    width,
                    height,
                    advance,
                    y_off,
                );
                return drawn;
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

struct BdfReader {
    file: RegularFile,
    chunk: [u8; 512],
    position: usize,
    length: usize,
}

impl BdfReader {
    fn new(file: RegularFile) -> Self {
        Self {
            file,
            chunk: [0; 512],
            position: 0,
            length: 0,
        }
    }

    fn byte(&mut self) -> Option<u8> {
        if self.position == self.length {
            self.length = self.file.read(&mut self.chunk).ok()?;
            self.position = 0;
            if self.length == 0 {
                return None;
            }
        }
        let byte = self.chunk[self.position];
        self.position += 1;
        Some(byte)
    }

    fn line(&mut self, output: &mut [u8; 256]) -> Option<usize> {
        let mut length = 0usize;
        let mut received = false;
        loop {
            match self.byte() {
                Some(b'\n') => return Some(length),
                Some(byte) => {
                    received = true;
                    if length < output.len() {
                        output[length] = byte;
                        length += 1;
                    }
                }
                None => return received.then_some(length),
            }
        }
    }
}

fn open_file(path: &str) -> Option<RegularFile> {
    let image = boot::image_handle();
    let mut fs = boot::get_image_file_system(image).ok()?;
    let mut root = fs.open_volume().ok()?;
    let mut path_buf = [0u16; 128];
    let mut length = 0usize;
    for byte in path.bytes() {
        if length + 1 >= path_buf.len() {
            return None;
        }
        path_buf[length] = if byte == b'/' { b'\\' } else { byte } as u16;
        length += 1;
    }
    path_buf[length] = 0;
    let path = CStr16::from_u16_with_nul(&path_buf[..=length]).ok()?;
    root.open(path, FileMode::Read, FileAttribute::empty())
        .ok()?
        .into_regular_file()
}

pub fn advance(ch: char) -> i32 {
    if let Some(advance) = cached_advance(ch) {
        return advance;
    }
    let mut result = 8;
    let _ = with_glyph(ch, |_data, _w, _h, glyph_advance, _y_off| {
        result = glyph_advance;
        true
    });
    result
}

fn parse_u32(value: &str) -> Option<u32> {
    value.parse().ok()
}
fn parse_i32(value: &str) -> Option<i32> {
    value.parse().ok()
}
