extern crate stb_image_write_rust;
extern crate stb_truetype_rust;

use stb_image_write_rust::ImageWriter::ImageWriter;
use stb_truetype_rust::*;

fn main() {
    let my_str = "Hello, World!";
    let font_size = 32.0f32;
    let buffer_size = 256;
    let mut bytes = include_bytes!("resources/DroidSans.ttf");

    let mut info = stbtt_fontinfo::default();
    unsafe {
        let res = stbtt_InitFont(&mut info, bytes.as_ptr(), 0);
        if (res == 0) {
            panic!("stbtt_InitFont failed!");
        }

        let mut ascent = 0;
        let mut descent = 0;
        let mut lineGap = 0;

        stbtt_GetFontVMetrics(&mut info, &mut ascent, &mut descent, &mut lineGap);
        let mut lineHeight = ascent - descent + lineGap;

        let scale = stbtt_ScaleForPixelHeight(&mut info, font_size);

        ascent = (ascent as f32 * scale + 0.5f32) as i32;
        descent = (descent as f32 * scale - 0.5f32) as i32;
        lineHeight = (lineHeight as f32 * scale + 0.5f32) as i32;

        let mut buffer = [0 as u8; 256 * 256];

        let (mut posX, mut posY) = (0, 0);

        for (i, c) in my_str.chars().enumerate() {
            let glyphId = stbtt_FindGlyphIndex(&mut info, c as i32);
            if (glyphId == 0) {
                continue;
            }

            let (mut advanceTemp, mut lsbTemp) = (0, 0);
            stbtt_GetGlyphHMetrics(&mut info, glyphId, &mut advanceTemp, &mut lsbTemp);
            let advance = (advanceTemp as f32 * scale + 0.5f32) as i32;

            let (mut x0, mut y0, mut x1, mut y1) = (0, 0, 0, 0);
            stbtt_GetGlyphBitmapBox(
                &mut info, glyphId, scale, scale, &mut x0, &mut y0, &mut x1, &mut y1,
            );

            posX += x0;
            posY = ascent + y0;

            if (posY < 0) {
                posY = 0;
            }

            let output = buffer
                .as_mut_ptr()
                .offset((posX + posY * buffer_size) as isize);

            stbtt_MakeGlyphBitmap(
                &mut info,
                output,
                x1 - x0,
                y1 - y0,
                buffer_size,
                scale,
                scale,
                glyphId,
            );

            posX += advance;
        }

        let mut writer = ImageWriter::new("output.png");
        writer.write_png(buffer_size, buffer_size, 1, buffer.as_mut_ptr());
    }
}
