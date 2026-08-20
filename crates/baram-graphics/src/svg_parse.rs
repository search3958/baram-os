#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FillRule {
    NonZero,
    EvenOdd,
}

impl FillRule {
    #[inline]
    fn is_inside(self, winding: i32) -> bool {
        match self {
            FillRule::NonZero => winding != 0,
            FillRule::EvenOdd => (winding & 1) != 0,
        }
    }
}

struct Tag {
    name: String,
    attrs: String,
}

fn extract_tags(svg: &str) -> Vec<Tag> {
    let mut tags = Vec::new();
    let bytes = svg.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    let mut defs_depth: u32 = 0;
    while i < len {
        if bytes[i] == b'<' {
            let mut j = i + 1;
            let is_close = j < len && bytes[j] == b'/';
            if is_close {
                j += 1;
            }
            if j < len && bytes[j] == b'?' {
            } else if j < len && bytes[j] == b'!' {
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
            let name = String::from_utf8_lossy(&bytes[name_start..j]).to_string();

            let mut k = j;
            while k < len
                && bytes[k] != b'>'
                && !(bytes[k] == b'/' && k + 1 < len && bytes[k + 1] == b'>')
            {
                k += 1;
            }

            let attr_start = j;
            let self_closing = k < len && bytes[k] == b'/';
            if k < len {
                k += if self_closing { 2 } else { 1 };
            }
            let attr_end = k.saturating_sub(1);
            let attrs =
                String::from_utf8_lossy(&bytes[attr_start.min(len)..attr_end.min(len)]).to_string();
            i = k;

            if name == "defs" {
                if is_close {
                    if defs_depth > 0 {
                        defs_depth -= 1;
                    }
                } else if !self_closing {
                    defs_depth += 1;
                }
                continue;
            }

            let is_definition_only = matches!(
                name.as_str(),
                "clipPath"
                    | "mask"
                    | "pattern"
                    | "linearGradient"
                    | "radialGradient"
                    | "symbol"
                    | "marker"
                    | "filter"
            );
            if is_definition_only && !is_close && !self_closing {
                defs_depth = defs_depth.saturating_add(1);
                continue;
            }
            if is_definition_only && is_close {
                if defs_depth > 0 {
                    defs_depth -= 1;
                }
                continue;
            }

            if defs_depth > 0 {
                continue;
            }
            if !name.is_empty()
                && name != "svg"
                && name != "g"
                && name != "?xml"
                && !name.starts_with('!')
            {
                tags.push(Tag { name, attrs });
            }
        } else {
            i += 1;
        }
    }
    tags
}

fn attr_f32(attrs: &str, key: &str) -> f32 {
    let needle = format!("{}=\"", key);
    if let Some(s) = attrs.find(&needle) {
        let v = s + needle.len();
        if let Some(e) = attrs[v..].find('"') {
            return attrs[v..v + e].trim().parse().unwrap_or(0.0);
        }
    }
    0.0
}

fn attr_str<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{}=\"", key);
    if let Some(s) = attrs.find(&needle) {
        let v = s + needle.len();
        if let Some(e) = attrs[v..].find('"') {
            return Some(&attrs[v..v + e]);
        }
    }
    None
}

fn parse_color(s: &str) -> Color {
    let s = s.trim();
    if s.starts_with('#') {
        let h = &s[1..];
        match h.len() {
            3 => {
                let r = u8::from_str_radix(&h[0..1], 16).unwrap_or(0) * 17;
                let g = u8::from_str_radix(&h[1..2], 16).unwrap_or(0) * 17;
                let b = u8::from_str_radix(&h[2..3], 16).unwrap_or(0) * 17;
                Color::rgb(r, g, b)
            }
            6 => {
                let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
                Color::rgb(r, g, b)
            }
            8 => {
                let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
                let a = u8::from_str_radix(&h[6..8], 16).unwrap_or(0);
                if a == 0 {
                    Color::TRANSPARENT
                } else {
                    Color::rgb(r, g, b)
                }
            }
            _ => Color::BLACK,
        }
    } else {
        match s {
            "black" => Color::BLACK,
            "white" => Color::rgb(255, 255, 255),
            "red" => Color::rgb(255, 0, 0),
            "green" => Color::rgb(0, 128, 0),
            "blue" => Color::rgb(0, 0, 255),
            "yellow" => Color::rgb(255, 255, 0),
            "cyan" => Color::rgb(0, 255, 255),
            "magenta" => Color::rgb(255, 0, 255),
            "gray" | "grey" => Color::rgb(128, 128, 128),
            "orange" => Color::rgb(255, 165, 0),
            "none" | "" => Color::TRANSPARENT,
            _ => Color::BLACK,
        }
    }
}

fn attr_color(attrs: &str, key: &str) -> Color {
    attr_str(attrs, key)
        .map(parse_color)
        .unwrap_or(Color::TRANSPARENT)
}

fn attr_fill_rule(attrs: &str) -> FillRule {
    match attr_str(attrs, "fill-rule").unwrap_or("") {
        "evenodd" => FillRule::EvenOdd,
        _ => FillRule::NonZero,
    }
}


