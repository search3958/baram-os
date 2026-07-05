//! Tiny text-rendering + UI helper module.
//!
//! Everything here draws into a `Screen` using the 8x16 bitmap font and the
//! palette defined in `gop::Color`.  No external dependencies.

use crate::font::{self, GLYPH_H, GLYPH_W};
use crate::gop::{Color, Screen};

/// Draw a single ASCII glyph at `(x, y)` with `fg` foreground and `bg`
/// background.  Non-ASCII bytes render as a filled background block.
pub fn put_char(screen: &mut Screen, x: usize, y: usize, c: u8, fg: Color, bg: Color) {
    if crate::ttf_font::is_available() && c >= 0x20 {
        let glyph = crate::ttf_font::glyph(c as char);
        if glyph.w > 0 && glyph.h > 0 {
            let baseline = y as i32 + crate::ttf_font::ascent();
            for row in 0..glyph.h {
                let py = baseline + glyph.y_off + row;
                if py < 0 { continue; }
                for col in 0..glyph.w {
                    let px = x as i32 + col;
                    if px < 0 { continue; }
                    let alpha = glyph.data[(row * glyph.w + col) as usize];
                    if alpha > 0 {
                        screen.put_pixel(px as usize, py as usize, fg);
                    } else {
                        screen.put_pixel(px as usize, py as usize, bg);
                    }
                }
            }
            return;
        }
    }
    let glyph = font::glyph(c);
    for row in 0..GLYPH_H {
        let bits = glyph[row];
        for col in 0..GLYPH_W {
            let bit = (bits >> (7 - col)) & 1;
            let color = if bit == 1 { fg } else { bg };
            screen.put_pixel(x + col, y + row, color);
        }
    }
}

/// Draw an ASCII string.  Stops at the first non-ASCII byte.
pub fn put_str(screen: &mut Screen, mut x: usize, y: usize, s: &str, fg: Color, bg: Color) {
    if crate::ttf_font::is_available() {
        for ch in s.chars() {
            let glyph = crate::ttf_font::glyph(ch);
            if glyph.w > 0 && glyph.h > 0 {
                let c = if ch as u32 <= 0x7E { ch as u8 } else { b'?' };
                put_char(screen, x, y, c, fg, bg);
                x += glyph.advance.max(0) as usize;
            } else {
                put_char(screen, x, y, b' ', fg, bg);
                x += GLYPH_W;
            }
        }
        return;
    }
    for &b in s.as_bytes() {
        if b >= 0x80 { break; }
        put_char(screen, x, y, b, fg, bg);
        x += GLYPH_W;
    }
}

/// Draw a string with a transparent background (skip background pixels).
pub fn put_str_transparent(screen: &mut Screen, mut x: usize, y: usize, s: &str, fg: Color) {
    for &b in s.as_bytes() {
        if b >= 0x80 { break; }
        let glyph = font::glyph(b);
        for row in 0..GLYPH_H {
            let bits = glyph[row];
            for col in 0..GLYPH_W {
                if (bits >> (7 - col)) & 1 == 1 {
                    screen.put_pixel(x + col, y + row, fg);
                }
            }
        }
        x += GLYPH_W;
    }
}

/// Format an unsigned integer into a fixed-width decimal buffer.
pub fn u32_to_str(mut n: u32, out: &mut [u8]) -> usize {
    if n == 0 {
        out[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 12];
    let mut len = 0;
    while n > 0 {
        tmp[len] = b'0' + (n % 10) as u8;
        len += 1;
        n /= 10;
    }
    // Reverse into out.
    for i in 0..len {
        out[i] = tmp[len - 1 - i];
    }
    len
}

/// Format a signed integer with optional sign.
pub fn i32_to_str(n: i32, out: &mut [u8]) -> usize {
    if n < 0 {
        out[0] = b'-';
        let m = if n == i32::MIN { 2147483648u32 } else { (-n) as u32 };
        1 + u32_to_str(m, &mut out[1..])
    } else {
        u32_to_str(n as u32, out)
    }
}

/// Right-justify a number in `width` characters.
#[allow(dead_code)]
pub fn i32_to_str_padded(n: i32, width: usize, out: &mut [u8]) -> usize {
    let mut tmp = [0u8; 16];
    let len = i32_to_str(n, &mut tmp);
    if len >= width {
        out[..len].copy_from_slice(&tmp[..len]);
        len
    } else {
        let pad = width - len;
        for i in 0..pad { out[i] = b' '; }
        out[pad..pad + len].copy_from_slice(&tmp[..len]);
        pad + len
    }
}

/// Format `value` as a hex string of `digits` width with leading zeros.
#[allow(dead_code)]
pub fn u32_to_hex(value: u32, digits: usize, out: &mut [u8]) -> usize {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for i in 0..digits {
        let shift = (digits - 1 - i) * 4;
        out[i] = HEX[((value >> shift) & 0xF) as usize];
    }
    digits
}

/// Format a byte count as a human-readable string (KiB/MiB).
#[allow(dead_code)]
pub fn bytes_human(bytes: u64, out: &mut [u8]) -> usize {
    let kb = bytes / 1024;
    if kb < 1024 {
        let mut buf = [0u8; 12];
        let n = u32_to_str(kb as u32, &mut buf);
        out[..n].copy_from_slice(&buf[..n]);
        let unit = b" KiB";
        out[n..n + 4].copy_from_slice(unit);
        n + 4
    } else {
        let mb = kb / 1024;
        let mut buf = [0u8; 12];
        let n = u32_to_str(mb as u32, &mut buf);
        out[..n].copy_from_slice(&buf[..n]);
        let unit = b" MiB";
        out[n..n + 4].copy_from_slice(unit);
        n + 4
    }
}

/// Small helper to format `prefix` + number + `suffix` directly into a
/// scratch buffer that lives on the stack.
pub struct FmtBuf {
    pub buf: [u8; 64],
    pub len: usize,
}

impl FmtBuf {
    pub fn new() -> Self { FmtBuf { buf: [0u8; 64], len: 0 } }

    pub fn clear(&mut self) { self.len = 0; }

    pub fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let n = bytes.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
    }

    pub fn push_u32(&mut self, n: u32) {
        let mut tmp = [0u8; 12];
        let l = u32_to_str(n, &mut tmp);
        let n = l.min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&tmp[..n]);
        self.len += n;
    }

    pub fn push_i32(&mut self, n: i32) {
        let mut tmp = [0u8; 16];
        let l = i32_to_str(n, &mut tmp);
        let n = l.min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&tmp[..n]);
        self.len += n;
    }

    /// View as `&str` (the buffer only contains ASCII when callers are well-behaved).
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}
