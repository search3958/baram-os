pub fn draw_svg(layer: &mut LayerSystem, svg: &str, ox: i32, oy: i32) {
    draw_svg_scaled_into(
        layer,
        svg,
        ox,
        oy,
        layer.width() as f32,
        layer.height() as f32,
        layer.width() as f32,
        layer.height() as f32,
        255,
    );
}

pub fn draw_svg_into(
    layer: &mut LayerSystem,
    svg: &str,
    ox: i32,
    oy: i32,
    target_w: f32,
    target_h: f32,
) {
    draw_svg_scaled_into(
        layer,
        svg,
        ox,
        oy,
        layer.width() as f32,
        layer.height() as f32,
        target_w,
        target_h,
        255,
    );
}

pub fn draw_svg_into_alpha(
    layer: &mut LayerSystem,
    svg: &str,
    ox: i32,
    oy: i32,
    target_w: f32,
    target_h: f32,
    alpha_scale: u32,
) {
    draw_svg_scaled_into(
        layer,
        svg,
        ox,
        oy,
        layer.width() as f32,
        layer.height() as f32,
        target_w,
        target_h,
        alpha_scale,
    );
}

fn svg_root_attrs(svg: &str) -> String {
    let bytes = svg.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'<' {
            let mut j = i + 1;
            if j < len && bytes[j] == b'/' {
                i += 1;
                continue;
            }
            if j < len && (bytes[j] == b'?' || bytes[j] == b'!') {
                while i < len && bytes[i] != b'>' {
                    i += 1;
                }
                i += 1;
                continue;
            }
            let name_start = j;
            while j < len
                && bytes[j] != b'>'
                && bytes[j] != b' '
                && bytes[j] != b'\t'
                && bytes[j] != b'\n'
                && bytes[j] != b'\r'
                && bytes[j] != b'/'
            {
                j += 1;
            }
            let name = &bytes[name_start..j];
            if name == b"svg" {
                let attr_start = j;
                let mut k = j;
                while k < len
                    && bytes[k] != b'>'
                    && !(bytes[k] == b'/' && k + 1 < len && bytes[k + 1] == b'>')
                {
                    k += 1;
                }
                let attr_end = k.saturating_sub(1);
                return String::from_utf8_lossy(&bytes[attr_start.min(len)..attr_end.min(len)])
                    .to_string();
            }
            while i < len && bytes[i] != b'>' {
                i += 1;
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    String::new()
}

fn draw_svg_scaled_into(
    layer: &mut LayerSystem,
    svg: &str,
    ox: i32,
    oy: i32,
    _layer_w: f32,
    _layer_h: f32,
    target_w: f32,
    target_h: f32,
    alpha_scale: u32,
) {
    let tw = target_w as usize;
    let th = target_h as usize;
    if tw == 0 || th == 0 {
        return;
    }

    if let Some((ptr, w, h)) = cache_lookup(svg, tw, th) {
        let len = w * h * 4;
        let pixels = unsafe { core::slice::from_raw_parts(ptr, len) };
        blit_cached_alpha(layer, pixels, w, h, ox, oy, alpha_scale);
        return;
    }

    let pixels = rasterize_svg_to_buffer(svg, tw, th);
    blit_cached_alpha(layer, &pixels, tw, th, ox, oy, alpha_scale);
    cache_store(svg, tw, th, pixels);
}

fn parse_pts_f64(s: &str) -> Vec<Point> {
    let mut pts = Vec::new();
    let nums: Vec<&str> = s
        .split(|c: char| c == ',' || c == ' ' || c == '\n' || c == '\r' || c == '\t')
        .filter(|x| !x.is_empty())
        .collect();
    let mut i = 0;
    while i + 1 < nums.len() {
        if let (Ok(x), Ok(y)) = (nums[i].trim().parse(), nums[i + 1].trim().parse()) {
            pts.push(Point::new(x, y));
        }
        i += 2;
    }
    pts
}

#[allow(dead_code)]
fn _unused_imports_anchor() {
    let _ = Line::new(Point::ZERO, Point::ZERO);
}

