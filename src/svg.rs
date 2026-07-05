extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::gop::Color;
use crate::window::LayerSystem;

const TAU: f32 = core::f32::consts::TAU;
const HALF_PI: f32 = core::f32::consts::FRAC_PI_2;
const PI: f32 = core::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
struct Pt { x: f32, y: f32 }

#[derive(Clone, Debug)]
enum Elem {
    Rect { x: f32, y: f32, w: f32, h: f32, rx: f32, fill: Color, stroke: Color, sw: f32 },
    Circle { cx: f32, cy: f32, r: f32, fill: Color, stroke: Color, sw: f32 },
    Ellipse { cx: f32, cy: f32, rx: f32, ry: f32, fill: Color, stroke: Color, sw: f32 },
    Line { x1: f32, y1: f32, x2: f32, y2: f32, stroke: Color, sw: f32 },
    Poly { pts: Vec<Pt>, fill: Color, stroke: Color, sw: f32 },
    Path { d: String, fill: Color, stroke: Color, sw: f32 },
}

// ── tiny XML-ish extractor ───────────────────────────────────────────

struct Tag { name: String, attrs: String }

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
            if is_close { j += 1; }
            if j < len && bytes[j] == b'?' { }
            else if j < len && bytes[j] == b'!' {
                while i < len && bytes[i] != b'>' { i += 1; }
                i += 1;
                continue;
            }
            let name_start = j;
            while j < len && bytes[j] != b'>' && bytes[j] != b' ' && bytes[j] != b'\t'
                    && bytes[j] != b'\n' && bytes[j] != b'\r' && bytes[j] != b'/' {
                j += 1;
            }
            let name = String::from_utf8_lossy(&bytes[name_start..j]).to_string();
            let mut k = j;
            while k < len && bytes[k] != b'>' && !(bytes[k] == b'/' && k + 1 < len && bytes[k+1] == b'>') {
                k += 1;
            }
            let attr_start = j;
            let self_closing = k < len && bytes[k] == b'/';
            if k < len { k += if self_closing { 2 } else { 1 }; }
            let attr_end = k.saturating_sub(1);
            let attrs = String::from_utf8_lossy(&bytes[attr_start.min(len)..attr_end.min(len)]).to_string();
            i = k;

            if name == "defs" {
                if is_close { if defs_depth > 0 { defs_depth -= 1; } }
                else if !self_closing { defs_depth += 1; }
                continue;
            }
            let is_definition_only = matches!(name.as_str(),
                "clipPath" | "mask" | "pattern" | "linearGradient"
                | "radialGradient" | "symbol" | "marker" | "filter");
            if is_definition_only && !is_close && !self_closing {
                defs_depth = defs_depth.saturating_add(1);
                continue;
            }
            if is_definition_only && is_close {
                if defs_depth > 0 { defs_depth -= 1; }
                continue;
            }

            if defs_depth > 0 { continue; }
            if !name.is_empty() && name != "svg" && name != "g" && name != "?xml" && !name.starts_with('!') {
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
    attr_str(attrs, key).map(parse_color).unwrap_or(Color::TRANSPARENT)
}

// ── SVG path 'd' parser (M/L/H/V/Z only, everything else → L to endpoint) ─

fn eval_d(d: &str) -> Vec<Vec<Pt>> {
    let b: Vec<char> = d.chars().collect();
    let n = b.len();
    let mut i = 0;

    let mut contours: Vec<Vec<Pt>> = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    let mut cx: f32 = 0.0;
    let mut cy: f32 = 0.0;
    let mut sx: f32 = 0.0;
    let mut sy: f32 = 0.0;
    let mut has_start = false;

    while i < n {
        if b[i].is_whitespace() || b[i] == ',' { i += 1; continue; }

        let cmd = b[i];
        i += 1;

        let mut nums: Vec<f32> = Vec::new();
        while i < n {
            while i < n && (b[i].is_whitespace() || b[i] == ',') { i += 1; }
            if i >= n { break; }
            if (b[i] == '-' || b[i] == '+') && !nums.is_empty() {
                if let Some(last) = nums.last() {
                    let s = format!("{}", last);
                    if !s.ends_with('e') && !s.ends_with('E') { break; }
                }
            }
            if b[i] == '-' || b[i] == '+' || b[i].is_ascii_digit() || b[i] == '.' {
                let s = i;
                i += 1;
                while i < n && (b[i].is_ascii_digit() || b[i] == '.' || b[i] == 'e' || b[i] == 'E'
                    || ((b[i] == '-' || b[i] == '+') && i > 0 && (b[i-1] == 'e' || b[i-1] == 'E')))
                { i += 1; }
                let t: String = b[s..i].iter().collect();
                if let Ok(v) = t.parse::<f32>() { nums.push(v); }
            } else { break; }
        }

        let is_abs = cmd.is_uppercase();
        let lx = cx;
        let ly = cy;
        let coord = |v: f32| -> f32 { if is_abs { v } else { lx + v } };
        let coordy = |v: f32| -> f32 { if is_abs { v } else { ly + v } };

        match cmd {
            'M' | 'm' => {
                let mut j = 0;
                while j + 1 < nums.len() {
                    cx = coord(nums[j]);
                    cy = coordy(nums[j + 1]);
                    j += 2;
                    if !has_start { sx = cx; sy = cy; has_start = true; }
                    if cur.len() > 1 { contours.push(cur); }
                    cur = Vec::new();
                    cur.push(Pt { x: cx, y: cy });
                }
            }
            'L' | 'l' => {
                let mut j = 0;
                while j + 1 < nums.len() {
                    cx = coord(nums[j]);
                    cy = coordy(nums[j + 1]);
                    cur.push(Pt { x: cx, y: cy });
                    j += 2;
                }
            }
            'H' | 'h' => {
                for &v in &nums {
                    cx = coord(v);
                    cur.push(Pt { x: cx, y: cy });
                }
            }
            'V' | 'v' => {
                for &v in &nums {
                    cy = coordy(v);
                    cur.push(Pt { x: cx, y: cy });
                }
            }
            'C' | 'c' => {
                if nums.len() >= 6 {
                    cx = coord(nums[4]);
                    cy = coordy(nums[5]);
                    cur.push(Pt { x: cx, y: cy });
                }
            }
            'S' | 's' => {
                if nums.len() >= 4 {
                    cx = coord(nums[2]);
                    cy = coordy(nums[3]);
                    cur.push(Pt { x: cx, y: cy });
                }
            }
            'Q' | 'q' => {
                if nums.len() >= 4 {
                    cx = coord(nums[2]);
                    cy = coordy(nums[3]);
                    cur.push(Pt { x: cx, y: cy });
                }
            }
            'T' | 't' => {
                let mut j = 0;
                while j + 1 < nums.len() {
                    cx = coord(nums[j]);
                    cy = coordy(nums[j + 1]);
                    cur.push(Pt { x: cx, y: cy });
                    j += 2;
                }
            }
            'Z' | 'z' => {
                if !cur.is_empty() {
                    if (cur.last().unwrap().x - sx).abs() > 0.001
                        || (cur.last().unwrap().y - sy).abs() > 0.001 {
                        cur.push(Pt { x: sx, y: sy });
                    }
                }
                cx = sx;
                cy = sy;
                has_start = false;
            }
            _ => {}
        }
    }
    if cur.len() > 1 { contours.push(cur); }
    contours
}

// ── rasterizer ───────────────────────────────────────────────────────

fn blend_pixel(layer: &mut LayerSystem, x: usize, y: usize, c: Color, coverage: f32) {
    if x >= layer.width() || y >= layer.height() || coverage <= 0.0 { return; }
    let cov = coverage.min(1.0);
    let bg = layer.get_pixel(x, y);
    let a = (cov * 255.0) as u32;
    let inv = 255 - a;
    let r = (c.r() as u32 * a + bg.r() as u32 * inv) / 255;
    let g = (c.g() as u32 * a + bg.g() as u32 * inv) / 255;
    let b = (c.b() as u32 * a + bg.b() as u32 * inv) / 255;
    layer.put_pixel(x, y, Color(0xFF00_0000 | (r << 16) | (g << 8) | b));
}

fn rasterize_fill(layer: &mut LayerSystem, pts: &[Pt], c: Color, ox: i32, oy: i32) {
    let n = pts.len();
    if n < 3 { return; }
    let sw = layer.width() as i32;
    let sh = layer.height() as i32;

    let sp: alloc::vec::Vec<Pt> = pts.iter()
        .map(|p| Pt { x: p.x + ox as f32, y: p.y + oy as f32 })
        .collect();

    let mut min_y = libm::floorf(sp[0].y) as i32;
    let mut max_y = min_y;
    for p in &sp {
        let py = libm::floorf(p.y) as i32;
        if py < min_y { min_y = py; }
        if py > max_y { max_y = py; }
    }
    min_y = min_y.max(0);
    max_y = max_y.min(sh - 1);
    if min_y > max_y { return; }

    for sy in min_y..=max_y {
        let yf = sy as f32 + 0.5;
        let mut xints: alloc::vec::Vec<f32> = Vec::new();
        for i in 0..n {
            let j = (i + 1) % n;
            let ya = sp[i].y;
            let yb = sp[j].y;
            if (ya - yb).abs() < 0.001 { continue; }
            let down = ya <= yf && yb > yf;
            let up   = yb <= yf && ya > yf;
            if down || up {
                let t = (yf - ya) / (yb - ya);
                xints.push(sp[i].x + t * (sp[j].x - sp[i].x));
            }
        }
        xints.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));

        let mut k = 0;
        while k + 1 < xints.len() {
            let x0f = xints[k];
            let x1f = xints[k + 1];
            let x0 = libm::ceilf(x0f) as i32;
            let x1 = libm::floorf(x1f) as i32;
            let yi = sy as usize;
            for x in x0..=x1 {
                if x >= 0 && x < sw {
                    layer.put_pixel(x as usize, yi, c);
                }
            }
            k += 2;
        }
    }
}

fn rasterize_fill_multi(layer: &mut LayerSystem, contours: &[Vec<Pt>], c: Color, ox: i32, oy: i32) {
    let sw = layer.width() as i32;
    let sh = layer.height() as i32;

    let sc: alloc::vec::Vec<alloc::vec::Vec<Pt>> = contours.iter()
        .map(|cont| cont.iter()
            .map(|p| Pt { x: p.x + ox as f32, y: p.y + oy as f32 })
            .collect())
        .collect();

    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    let mut any = false;
    for cont in &sc {
        if cont.len() < 2 { continue; }
        for p in cont {
            let py = libm::floorf(p.y) as i32;
            if py < min_y { min_y = py; }
            if py > max_y { max_y = py; }
            any = true;
        }
    }
    if !any { return; }
    min_y = min_y.max(0);
    max_y = max_y.min(sh - 1);
    if min_y > max_y { return; }

    for sy in min_y..=max_y {
        let yf = sy as f32 + 0.5;
        let mut xints: alloc::vec::Vec<f32> = Vec::new();
        for cont in &sc {
            let n = cont.len();
            if n < 2 { continue; }
            for i in 0..n {
                let j = (i + 1) % n;
                let ya = cont[i].y;
                let yb = cont[j].y;
                if (ya - yb).abs() < 0.001 { continue; }
                let down = ya <= yf && yb > yf;
                let up   = yb <= yf && ya > yf;
                if down || up {
                    let t = (yf - ya) / (yb - ya);
                    xints.push(cont[i].x + t * (cont[j].x - cont[i].x));
                }
            }
        }
        xints.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));

        let mut k = 0;
        while k + 1 < xints.len() {
            let x0f = xints[k];
            let x1f = xints[k + 1];
            let x0 = libm::ceilf(x0f) as i32;
            let x1 = libm::floorf(x1f) as i32;
            let yi = sy as usize;
            for x in x0..=x1 {
                if x >= 0 && x < sw {
                    layer.put_pixel(x as usize, yi, c);
                }
            }
            k += 2;
        }
    }
}

fn rasterize_stroke(layer: &mut LayerSystem, pts: &[Pt], c: Color, sw: f32, ox: i32, oy: i32) {
    if pts.len() < 2 { return; }
    let hw = (sw * 0.5).max(0.5);
    for i in 0..pts.len()-1 {
        let x0 = pts[i].x + ox as f32;
        let y0 = pts[i].y + oy as f32;
        let x1 = pts[i+1].x + ox as f32;
        let y1 = pts[i+1].y + oy as f32;
        draw_aa_thick_line(layer, x0, y0, x1, y1, c, hw);
    }
}

fn draw_aa_thick_line(layer: &mut LayerSystem, x0: f32, y0: f32, x1: f32, y1: f32, c: Color, hw: f32) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = libm::sqrtf(dx * dx + dy * dy);
    if len < 0.001 {
        let r = hw;
        let min_x = libm::floorf(x0 - r) as i32;
        let max_x = libm::ceilf(x0 + r) as i32;
        let min_y = libm::floorf(y0 - r) as i32;
        let max_y = libm::ceilf(y0 + r) as i32;
        for py in min_y..=max_y {
            for px in min_x..=max_x {
                let ddx = px as f32 + 0.5 - x0;
                let ddy = py as f32 + 0.5 - y0;
                let dist = libm::sqrtf(ddx * ddx + ddy * ddy);
                if dist <= r {
                    blend_pixel(layer, px as usize, py as usize, c, 1.0);
                } else if dist <= r + 1.0 {
                    blend_pixel(layer, px as usize, py as usize, c, 1.0 - (dist - r));
                }
            }
        }
        return;
    }

    let steps = libm::ceilf(len * 2.0) as i32;
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let cx = x0 + dx * t;
        let cy = y0 + dy * t;
        let min_px = libm::floorf(cx - hw - 1.0) as i32;
        let max_px = libm::ceilf(cx + hw + 1.0) as i32;
        let min_py = libm::floorf(cy - hw - 1.0) as i32;
        let max_py = libm::ceilf(cy + hw + 1.0) as i32;
        for py in min_py..=max_py {
            for px in min_px..=max_px {
                let pcx = px as f32 + 0.5 - cx;
                let pcy = py as f32 + 0.5 - cy;
                let dist = libm::sqrtf(pcx * pcx + pcy * pcy);
                if dist <= hw {
                    blend_pixel(layer, px as usize, py as usize, c, 1.0);
                } else if dist <= hw + 1.0 {
                    blend_pixel(layer, px as usize, py as usize, c, 1.0 - (dist - hw));
                }
            }
        }
    }
}

fn circle_pts(cx: f32, cy: f32, r: f32) -> Vec<Pt> {
    let mut pts = Vec::new();
    for i in 0..48 {
        let a = (i as f32 / 48.0) * TAU;
        pts.push(Pt { x: cx + r * libm::cosf(a), y: cy + r * libm::sinf(a) });
    }
    pts
}

fn ellipse_pts(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<Pt> {
    let mut pts = Vec::new();
    for i in 0..48 {
        let a = (i as f32 / 48.0) * TAU;
        pts.push(Pt { x: cx + rx * libm::cosf(a), y: cy + ry * libm::sinf(a) });
    }
    pts
}

fn rounded_rect_pts(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<Pt> {
    let mut pts = Vec::new();
    let corners = [
        (x+w-r, y+r,   HALF_PI, PI),
        (x+w-r, y+h-r, PI,      PI+HALF_PI),
        (x+r,   y+h-r, PI+HALF_PI, TAU),
        (x+r,   y+r,   TAU,     TAU+HALF_PI),
    ];
    for (cx, cy, a0, a1) in corners {
        for i in 0..=8 {
            let a = a0 + (a1 - a0) * i as f32 / 8.0;
            pts.push(Pt { x: cx + r * libm::cosf(a), y: cy + r * libm::sinf(a) });
        }
    }
    pts
}

fn scale_pts(pts: &[Pt], sx: f32, sy: f32) -> Vec<Pt> {
    pts.iter().map(|p| Pt { x: p.x * sx, y: p.y * sy }).collect()
}

fn scaled_stroke_width(sw: f32, sx: f32, sy: f32) -> f32 {
    let scale = (sx.abs() + sy.abs()) * 0.5;
    (sw * scale).max(0.5)
}

// ── public API ───────────────────────────────────────────────────────

pub fn draw_svg(layer: &mut LayerSystem, svg: &str, ox: i32, oy: i32) {
    draw_svg_scaled_into(layer, svg, ox, oy,
        layer.width() as f32, layer.height() as f32,
        layer.width() as f32,
        layer.height() as f32);
}

pub fn draw_svg_into(layer: &mut LayerSystem, svg: &str,
                     ox: i32, oy: i32, target_w: f32, target_h: f32) {
    draw_svg_scaled_into(layer, svg, ox, oy,
        layer.width() as f32, layer.height() as f32,
        target_w, target_h);
}

fn svg_root_attrs(svg: &str) -> String {
    let bytes = svg.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'<' {
            let mut j = i + 1;
            if j < len && bytes[j] == b'/' { i += 1; continue; }
            if j < len && (bytes[j] == b'?' || bytes[j] == b'!') {
                while i < len && bytes[i] != b'>' { i += 1; }
                i += 1;
                continue;
            }
            let name_start = j;
            while j < len && bytes[j] != b'>' && bytes[j] != b' '
                    && bytes[j] != b'\t' && bytes[j] != b'\n'
                    && bytes[j] != b'\r' && bytes[j] != b'/' {
                j += 1;
            }
            let name = &bytes[name_start..j];
            if name == b"svg" {
                let attr_start = j;
                let mut k = j;
                while k < len && bytes[k] != b'>' && !(bytes[k] == b'/' && k + 1 < len && bytes[k+1] == b'>') {
                    k += 1;
                }
                let attr_end = k.saturating_sub(1);
                return String::from_utf8_lossy(&bytes[attr_start.min(len)..attr_end.min(len)]).to_string();
            }
            while i < len && bytes[i] != b'>' { i += 1; }
            i += 1;
        } else {
            i += 1;
        }
    }
    String::new()
}

fn draw_svg_scaled_into(layer: &mut LayerSystem, svg: &str,
                        ox: i32, oy: i32,
                        _layer_w: f32, _layer_h: f32,
                        target_w: f32, target_h: f32) {
    let tags = extract_tags(svg);

    let mut vb_w = 0.0f32;
    let mut vb_h = 0.0f32;
    let root_attrs = svg_root_attrs(svg);
    let svg_w = attr_f32(&root_attrs, "width");
    let svg_h = attr_f32(&root_attrs, "height");
    if let Some(vb) = attr_str(&root_attrs, "viewBox") {
        let parts: Vec<&str> = vb.split(|c: char| c == ' ' || c == ',').collect();
        if parts.len() >= 4 {
            vb_w = parts[2].trim().parse().unwrap_or(0.0);
            vb_h = parts[3].trim().parse().unwrap_or(0.0);
        }
    }

    let src_w = if vb_w > 0.0 { vb_w } else { svg_w };
    let src_h = if vb_h > 0.0 { vb_h } else { svg_h };

    let (sx, sy, dx, dy) = if src_w > 0.0 && src_h > 0.0 {
        let scale = (target_w / src_w).min(target_h / src_h);
        let draw_w = src_w * scale;
        let draw_h = src_h * scale;
        let dx = ((target_w - draw_w) * 0.5).max(0.0);
        let dy = ((target_h - draw_h) * 0.5).max(0.0);
        (scale, scale, dx, dy)
    } else {
        (1.0, 1.0, 0.0, 0.0)
    };

    let ox = ox + (dx as i32);
    let oy = oy + (dy as i32);

    let mut elems: Vec<Elem> = Vec::new();

    for tag in &tags {
        let a = &tag.attrs;
        let fill = attr_color(a, "fill");
        let stroke = attr_color(a, "stroke");
        let sw: f32 = attr_str(a, "stroke-width").and_then(|v| v.parse().ok()).unwrap_or(1.0);

        match tag.name.as_str() {
            "rect" => {
                let rx_f = attr_f32(a, "rx");
                elems.push(Elem::Rect {
                    x: attr_f32(a, "x"), y: attr_f32(a, "y"),
                    w: attr_f32(a, "width"), h: attr_f32(a, "height"),
                    rx: rx_f, fill, stroke, sw,
                });
            }
            "circle" => elems.push(Elem::Circle {
                cx: attr_f32(a, "cx"), cy: attr_f32(a, "cy"),
                r: attr_f32(a, "r"), fill, stroke, sw,
            }),
            "ellipse" => elems.push(Elem::Ellipse {
                cx: attr_f32(a, "cx"), cy: attr_f32(a, "cy"),
                rx: attr_f32(a, "rx"), ry: attr_f32(a, "ry"), fill, stroke, sw,
            }),
            "line" => elems.push(Elem::Line {
                x1: attr_f32(a, "x1"), y1: attr_f32(a, "y1"),
                x2: attr_f32(a, "x2"), y2: attr_f32(a, "y2"), stroke, sw,
            }),
            "polygon" => {
                if let Some(s) = attr_str(a, "points") {
                    elems.push(Elem::Poly { pts: parse_pts(s), fill, stroke, sw });
                }
            }
            "polyline" => {
                if let Some(s) = attr_str(a, "points") {
                    let mut pts = parse_pts(s);
                    if let Some(first) = pts.first().cloned() { pts.push(first); }
                    elems.push(Elem::Poly { pts, fill, stroke, sw });
                }
            }
            "path" => {
                if let Some(d) = attr_str(a, "d") {
                    elems.push(Elem::Path { d: d.to_string(), fill, stroke, sw });
                }
            }
            _ => {}
        }
    }

    for elem in &elems {
        match elem {
            Elem::Rect { x, y, w, h, rx, fill, stroke, sw } => {
                if *rx > 0.0 {
                    let pts = rounded_rect_pts(*x, *y, *w, *h, *rx);
                    let s = scale_pts(&pts, sx, sy);
                    if *fill != Color::TRANSPARENT { rasterize_fill(layer, &s, *fill, ox, oy); }
                    if *stroke != Color::TRANSPARENT {
                        rasterize_stroke(layer, &s, *stroke, scaled_stroke_width(*sw, sx, sy), ox, oy);
                    }
                } else {
                    if *fill != Color::TRANSPARENT {
                        layer.fill_rect(
                            ((*x * sx) as i32 + ox).max(0) as usize,
                            ((*y * sy) as i32 + oy).max(0) as usize,
                            (*w * sx) as usize, (*h * sy) as usize, *fill);
                    }
                    if *stroke != Color::TRANSPARENT {
                        layer.rect_outline(
                            ((*x * sx) as i32 + ox).max(0) as usize,
                            ((*y * sy) as i32 + oy).max(0) as usize,
                            (*w * sx) as usize, (*h * sy) as usize, *stroke);
                    }
                }
            }
            Elem::Circle { cx, cy, r, fill, stroke, sw } => {
                let pts = circle_pts(*cx, *cy, *r);
                let s = scale_pts(&pts, sx, sy);
                if *fill != Color::TRANSPARENT { rasterize_fill(layer, &s, *fill, ox, oy); }
                if *stroke != Color::TRANSPARENT {
                    rasterize_stroke(layer, &s, *stroke, scaled_stroke_width(*sw, sx, sy), ox, oy);
                }
            }
            Elem::Ellipse { cx, cy, rx, ry, fill, stroke, sw } => {
                let pts = ellipse_pts(*cx, *cy, *rx, *ry);
                let s = scale_pts(&pts, sx, sy);
                if *fill != Color::TRANSPARENT { rasterize_fill(layer, &s, *fill, ox, oy); }
                if *stroke != Color::TRANSPARENT {
                    rasterize_stroke(layer, &s, *stroke, scaled_stroke_width(*sw, sx, sy), ox, oy);
                }
            }
            Elem::Line { x1, y1, x2, y2, stroke, sw } => {
                if *stroke != Color::TRANSPARENT {
                    draw_aa_thick_line(layer,
                        (*x1 * sx) as f32 + ox as f32, (*y1 * sy) as f32 + oy as f32,
                        (*x2 * sx) as f32 + ox as f32, (*y2 * sy) as f32 + oy as f32,
                        *stroke, scaled_stroke_width(*sw, sx, sy));
                }
            }
            Elem::Poly { pts, fill, stroke, sw } => {
                let s = scale_pts(pts, sx, sy);
                if *fill != Color::TRANSPARENT { rasterize_fill(layer, &s, *fill, ox, oy); }
                if *stroke != Color::TRANSPARENT {
                    rasterize_stroke(layer, &s, *stroke, scaled_stroke_width(*sw, sx, sy), ox, oy);
                }
            }
            Elem::Path { d, fill, stroke, sw } => {
                let contours = eval_d(d);
                let scaled: Vec<Vec<Pt>> = contours.iter()
                    .map(|c| scale_pts(c, sx, sy))
                    .collect();
                if *fill != Color::TRANSPARENT {
                    rasterize_fill_multi(layer, &scaled, *fill, ox, oy);
                }
                if *stroke != Color::TRANSPARENT {
                    for c in &scaled {
                        rasterize_stroke(layer, c, *stroke, scaled_stroke_width(*sw, sx, sy), ox, oy);
                    }
                }
            }
        }
    }
}

fn parse_pts(s: &str) -> Vec<Pt> {
    let mut pts = Vec::new();
    let nums: Vec<&str> = s.split(|c: char| c == ',' || c == ' ' || c == '\n' || c == '\r' || c == '\t')
        .filter(|x| !x.is_empty()).collect();
    let mut i = 0;
    while i + 1 < nums.len() {
        if let (Ok(x), Ok(y)) = (nums[i].trim().parse(), nums[i+1].trim().parse()) {
            pts.push(Pt { x, y });
        }
        i += 2;
    }
    pts
}
