# Overview
stb_truetype_rust is Rust port of stb_truetype.h, which is library to save images in BMP, JPG, PNG and TGA formats

# Crate

# Sample Code
```rust
use stb_truetype_rust::ImageWriter::ImageWriter;

fn main() {
    let mut writer = ImageWriter::new("output.jpg");
    writer.write_jpg(width, height, components, image_data, 90);
}

```

