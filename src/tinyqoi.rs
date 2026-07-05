//! QOI image decoder — minimal, no_std compatible.

use crate::gop::Color;

const HEADER_LEN: usize = 14;
const END_MARKER: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

#[derive(Debug)]
pub struct QoiImage {
    pub width: u32,
    pub height: u32,
    pub pixels: alloc::vec::Vec<Color>,
}

pub fn decode(data: &[u8]) -> Option<QoiImage> {
    if data.len() < HEADER_LEN + END_MARKER.len() {
        return None;
    }

    let (header, rest) = data.split_at(HEADER_LEN);
    if &header[0..4] != b"qoif" {
        return None;
    }
    let end = &rest[rest.len() - END_MARKER.len()..];
    if end != END_MARKER {
        return None;
    }

    
    let width = u32::from_be_bytes(header[4..8].try_into().ok()?);
    let height = u32::from_be_bytes(header[8..12].try_into().ok()?);

    let pixel_data = &rest[..rest.len() - END_MARKER.len()];
    let total = (width as usize) * (height as usize);

    let mut pixels = alloc::vec::Vec::with_capacity(total);
    let mut index = [(0u8, 0u8, 0u8, 0u8); 64];
    let mut prev_r: u8 = 0;
    let mut prev_g: u8 = 0;
    let mut prev_b: u8 = 0;
    let mut prev_a: u8 = 255;
    let mut pos = 0;
    let mut run: i32 = 0;

    for _ in 0..total {
        let (r, g, b, a) = if run > 0 {
            run -= 1;
            (prev_r, prev_g, prev_b, prev_a)
        } else {
            if pos >= pixel_data.len() {
                return None;
            }
            let byte = pixel_data[pos];
            pos += 1;

            match byte {
                0xFE => {
                    if pos + 2 >= pixel_data.len() { return None; }
                    let c = (pixel_data[pos], pixel_data[pos+1], pixel_data[pos+2]);
                    pos += 3;
                    (c.0, c.1, c.2, prev_a)
                }
                0xFF => {
                    if pos + 3 >= pixel_data.len() { return None; }
                    let c = (pixel_data[pos], pixel_data[pos+1], pixel_data[pos+2], pixel_data[pos+3]);
                    pos += 4;
                    c
                }
                b if b & 0xC0 == 0x00 => {
                    index[(b & 0x3F) as usize]
                }
                b if b & 0xC0 == 0x40 => {
                    let dr = ((b >> 4) & 0x3) as i16 - 2;
                    let dg = ((b >> 2) & 0x3) as i16 - 2;
                    let db = (b & 0x3) as i16 - 2;
                    (
                        (prev_r as i16 + dr) as u8,
                        (prev_g as i16 + dg) as u8,
                        (prev_b as i16 + db) as u8,
                        prev_a,
                    )
                }
                b if b & 0xC0 == 0x80 => {
                    if pos >= pixel_data.len() { return None; }
                    let byte2 = pixel_data[pos];
                    pos += 1;
                    let dg = (b & 0x3F) as i16 - 32;
                    let dr = ((byte2 >> 4) & 0x0F) as i16 - 8 + dg;
                    let db = (byte2 & 0x0F) as i16 - 8 + dg;
                    (
                        (prev_r as i16 + dr) as u8,
                        (prev_g as i16 + dg) as u8,
                        (prev_b as i16 + db) as u8,
                        prev_a,
                    )
                }
                b => {
                    
                    run = (b & 0x3F) as i32;
                    (prev_r, prev_g, prev_b, prev_a)
                }
            }
        };

        let h = ((r as usize * 3 + g as usize * 5 + b as usize * 7 + a as usize * 11) % 64) as usize;
        index[h] = (r, g, b, a);
        prev_r = r;
        prev_g = g;
        prev_b = b;
        prev_a = a;

        pixels.push(Color::rgb(r, g, b));
    }

    Some(QoiImage { width, height, pixels })
}
