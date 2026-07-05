extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
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
    while i < len {
        if bytes[i] == b'<' {
            let start = i + 1;
            i += 1;
            if i < len && bytes[i] == b'/' { i += 1; }       // skip </
            if i < len && bytes[i] == b'?' { i += 1; }       // skip <?
            if i < len && bytes[i] == b'!' {                   // skip <!...
                while i < len && bytes[i] != b'>' { i += 1; }
                i += 1;
                continue;
            }
            let tag_start = i;
            while i < len && bytes[i] != b'>' && bytes[i] != b' ' && bytes[i] != b'\t'
                    && bytes[i] != b'\n' && bytes[i] != b'\r' && bytes[i] != b'/' {
                i += 1;
            }
            let name = String::from_utf8_lossy(&bytes[tag_start..i]).to_string();
            while i < len && bytes[i] != b'>' && !(bytes[i] == b'/' && i + 1 < len && bytes[i+1] == b'>') {
                i += 1;
            }
            let attr_start = i;
            if i < len && bytes[i] == b'/' { i += 2; }        // />
            else if i < len { i += 1; }                         // >
            let attr_end = if attr_start < i { i - 1 } else { i };
            let attrs = String::from_utf8_lossy(&bytes[attr_start.min(len)..attr_end.min(len)]).to_string();
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

// ── SVG path 'd' parser ──────────────────────────────────────────────

#[derive(Clone)]
enum Cmd {
    M(f32, f32), L(f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    SC(f32, f32, f32, f32),
    Q(f32, f32, f32, f32), SQ(f32, f32), Z,
}

fn tokenize_d(d: &str) -> Vec<Cmd> {
    let b: Vec<char> = d.chars().collect();
    let n = b.len();
    let mut i = 0;
    let mut out = Vec::new();
    while i < n {
        if b[i].is_whitespace() || b[i] == ',' { i += 1; continue; }
        let cmd = b[i]; i += 1;
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
        let p = |v: f32, ref_val: f32, abs: bool| -> f32 { if abs { v } else { ref_val + v } };
        match cmd {
            'M' | 'm' => {
                let mut j = 0; let mut rx = 0.0f32; let mut ry = 0.0f32;
                while j + 1 < nums.len() {
                    let x = nums[j]; let y = nums[j+1];
                    if cmd == 'm' { rx += x; ry += y; } else { rx = x; ry = y; }
                    out.push(Cmd::M(rx, ry)); j += 2;
                }
            }
            'L' | 'l' => {
                let mut j = 0;
                while j + 1 < nums.len() {
                    let lx = if let Some(Cmd::M(x,_))|Some(Cmd::L(x,_)) = out.last() { *x } else { 0.0 };
                    let ly = if let Some(Cmd::M(_,y))|Some(Cmd::L(_,y)) = out.last() { *y } else { 0.0 };
                    out.push(Cmd::L(p(nums[j], lx, cmd=='L'), p(nums[j+1], ly, cmd=='L')));
                    j += 2;
                }
            }
            'H' | 'h' => {
                for &v in &nums {
                    let lx = if let Some(Cmd::M(x,_))|Some(Cmd::L(x,_)) = out.last() { *x } else { 0.0 };
                    let ly = if let Some(Cmd::M(_,y))|Some(Cmd::L(_,y)) = out.last() { *y } else { 0.0 };
                    out.push(Cmd::L(p(v, lx, cmd=='H'), ly));
                }
            }
            'V' | 'v' => {
                for &v in &nums {
                    let lx = if let Some(Cmd::M(x,_))|Some(Cmd::L(x,_)) = out.last() { *x } else { 0.0 };
                    let ly = if let Some(Cmd::M(_,y))|Some(Cmd::L(_,y)) = out.last() { *y } else { 0.0 };
                    out.push(Cmd::L(lx, p(v, ly, cmd=='V')));
                }
            }
            'C' | 'c' => {
                let mut j = 0;
                while j + 5 < nums.len() {
                    let lx = if let Some(Cmd::M(x,_))|Some(Cmd::L(x,_)) = out.last() { *x } else { 0.0 };
                    let ly = if let Some(Cmd::M(_,y))|Some(Cmd::L(_,y)) = out.last() { *y } else { 0.0 };
                    let abs = cmd == 'C';
                    out.push(Cmd::C(p(nums[j],lx,abs), p(nums[j+1],ly,abs),
                                     p(nums[j+2],lx,abs), p(nums[j+3],ly,abs),
                                     p(nums[j+4],lx,abs), p(nums[j+5],ly,abs)));
                    j += 6;
                }
            }
            'S' | 's' => {
                let mut j = 0;
                while j + 3 < nums.len() {
                    let lx = if let Some(Cmd::M(x,_))|Some(Cmd::L(x,_)) = out.last() { *x } else { 0.0 };
                    let ly = if let Some(Cmd::M(_,y))|Some(Cmd::L(_,y)) = out.last() { *y } else { 0.0 };
                    let abs = cmd == 'S';
                    out.push(Cmd::SC(p(nums[j],lx,abs), p(nums[j+1],ly,abs),
                                      p(nums[j+2],lx,abs), p(nums[j+3],ly,abs)));
                    j += 4;
                }
            }
            'Q' | 'q' => {
                let mut j = 0;
                while j + 3 < nums.len() {
                    let lx = if let Some(Cmd::M(x,_))|Some(Cmd::L(x,_)) = out.last() { *x } else { 0.0 };
                    let ly = if let Some(Cmd::M(_,y))|Some(Cmd::L(_,y)) = out.last() { *y } else { 0.0 };
                    let abs = cmd == 'Q';
                    out.push(Cmd::Q(p(nums[j],lx,abs), p(nums[j+1],ly,abs),
                                     p(nums[j+2],lx,abs), p(nums[j+3],ly,abs)));
                    j += 4;
                }
            }
            'T' | 't' => {
                let mut j = 0;
                while j + 1 < nums.len() {
                    let lx = if let Some(Cmd::M(x,_))|Some(Cmd::L(x,_)) = out.last() { *x } else { 0.0 };
                    let ly = if let Some(Cmd::M(_,y))|Some(Cmd::L(_,y)) = out.last() { *y } else { 0.0 };
                    let abs = cmd == 'T';
                    out.push(Cmd::SQ(p(nums[j],lx,abs), p(nums[j+1],ly,abs)));
                    j += 2;
                }
            }
            'Z' | 'z' => { out.push(Cmd::Z); }
            _ => {}
        }
    }
    out
}

fn eval_d(d: &str) -> Vec<Vec<Pt>> {
    let cmds = tokenize_d(d);
    let mut contours: Vec<Vec<Pt>> = Vec::new();
    let mut cur = Vec::new();
    let mut cp = Pt { x: 0.0, y: 0.0 };
    let mut prev = Pt { x: 0.0, y: 0.0 };
    let mut start = Pt { x: 0.0, y: 0.0 };

    for c in &cmds {
        match c {
            Cmd::M(x, y) => {
                if cur.len() > 1 { contours.push(cur); }
                cur = Vec::new();
                cp = Pt { x: *x, y: *y }; prev = cp; start = cp;
                cur.push(cp);
            }
            Cmd::L(x, y) => { cp = Pt { x: *x, y: *y }; cur.push(cp); prev = cp; }
            Cmd::C(x1,y1,x2,y2,x,y) => {
                let pts = bezier3(&cp, &Pt{x:*x1,y:*y1}, &Pt{x:*x2,y:*y2}, &Pt{x:*x,y:*y});
                for p in &pts[1..] { cur.push(p.clone()); }
                cp = Pt{x:*x,y:*y}; prev = Pt{x:*x2,y:*y2};
            }
            Cmd::SC(x2,y2,x,y) => {
                let ref_ = Pt { x: 2.0*cp.x - prev.x, y: 2.0*cp.y - prev.y };
                let pts = bezier3(&cp, &ref_, &Pt{x:*x2,y:*y2}, &Pt{x:*x,y:*y});
                for p in &pts[1..] { cur.push(p.clone()); }
                cp = Pt{x:*x,y:*y}; prev = Pt{x:*x2,y:*y2};
            }
            Cmd::Q(x1,y1,x,y) => {
                let pts = bezier2(&cp, &Pt{x:*x1,y:*y1}, &Pt{x:*x,y:*y});
                for p in &pts[1..] { cur.push(p.clone()); }
                cp = Pt{x:*x,y:*y}; prev = Pt{x:*x1,y:*y1};
            }
            Cmd::SQ(x,y) => {
                let ref_ = Pt { x: 2.0*cp.x - prev.x, y: 2.0*cp.y - prev.y };
                let pts = bezier2(&cp, &ref_, &Pt{x:*x,y:*y});
                for p in &pts[1..] { cur.push(p.clone()); }
                cp = Pt{x:*x,y:*y}; prev = ref_;
            }
            Cmd::Z => {
                if !cur.is_empty() && (cur.last().unwrap().x != start.x || cur.last().unwrap().y != start.y) {
                    cur.push(start);
                }
                cp = start; prev = start;
            }
        }
    }
    if cur.len() > 1 { contours.push(cur); }
    contours
}

fn bezier3(p0: &Pt, p1: &Pt, p2: &Pt, p3: &Pt) -> Vec<Pt> {
    let mut out = Vec::new();
    for i in 0..=16 {
        let t = i as f32 / 16.0;
        let mt = 1.0 - t;
        let x = mt*mt*mt*p0.x + 3.0*mt*mt*t*p1.x + 3.0*mt*t*t*p2.x + t*t*t*p3.x;
        let y = mt*mt*mt*p0.y + 3.0*mt*mt*t*p1.y + 3.0*mt*t*t*p2.y + t*t*t*p3.y;
        out.push(Pt{x,y});
    }
    out
}

fn bezier2(p0: &Pt, p1: &Pt, p2: &Pt) -> Vec<Pt> {
    let mut out = Vec::new();
    for i in 0..=12 {
        let t = i as f32 / 12.0;
        let mt = 1.0 - t;
        let x = mt*mt*p0.x + 2.0*mt*t*p1.x + t*t*p2.x;
        let y = mt*mt*p0.y + 2.0*mt*t*p1.y + t*t*p2.y;
        out.push(Pt{x,y});
    }
    out
}

// ── rasterizer ───────────────────────────────────────────────────────

fn rasterize_fill(layer: &mut LayerSystem, pts: &[Pt], c: Color, ox: i32, oy: i32) {
    if pts.len() < 3 { return; }
    let sw = layer.width() as i32;
    let sh = layer.height() as i32;
    let mut min_y = pts[0].y as i32;
    let mut max_y = min_y;
    for p in pts { let py = p.y as i32; if py < min_y { min_y = py; } if py > max_y { max_y = py; } }
    min_y = min_y.max(0); max_y = max_y.min(sh - 1);

    for y in min_y..=max_y {
        let mut xints: Vec<f32> = Vec::new();
        let n = pts.len();
        for i in 0..n {
            let j = (i + 1) % n;
            let ya = pts[i].y; let yb = pts[j].y;
            if (ya <= y as f32 && yb > y as f32) || (yb <= y as f32 && ya > y as f32) {
                if (ya - yb).abs() < 0.001 { continue; }
                let t = (y as f32 - ya) / (yb - ya);
                xints.push(pts[i].x + t * (pts[j].x - pts[i].x));
            }
        }
        xints.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        let mut k = 0;
        while k + 1 < xints.len() {
            let x0 = (xints[k] as i32 + ox).max(0) as usize;
            let x1 = (xints[k+1] as i32 + ox).min(sw) as usize;
            for x in x0..x1 { layer.put_pixel(x, y as usize, c); }
            k += 2;
        }
    }
}

fn rasterize_stroke(layer: &mut LayerSystem, pts: &[Pt], c: Color, sw: f32, ox: i32, oy: i32) {
    if pts.len() < 2 { return; }
    let w = (sw * 0.5).max(0.5) as i32;
    for i in 0..pts.len()-1 {
        bresenham_thick(layer,
            pts[i].x as i32 + ox, pts[i].y as i32 + oy,
            pts[i+1].x as i32 + ox, pts[i+1].y as i32 + oy, c, w);
    }
}

fn bresenham_thick(layer: &mut LayerSystem, mut x0: i32, mut y0: i32, x1: i32, y1: i32, c: Color, w: i32) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    loop {
        for wy in -w..=w {
            for wx in -w..=w {
                let px = x0 + wx;
                let py = y0 + wy;
                if px >= 0 && py >= 0 && (px as usize) < layer.width() && (py as usize) < layer.height() {
                    layer.put_pixel(px as usize, py as usize, c);
                }
            }
        }
        if x0 == x1 && y0 == y1 { break; }
        let e2 = 2 * err;
        if e2 > -dy { err -= dy; x0 += sx; }
        if e2 < dx { err += dx; y0 += sy; }
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

fn flatten(d: &str) -> Vec<Pt> {
    let mut all = Vec::new();
    for c in eval_d(d) { all.extend(c); }
    all
}

fn thick_line(layer: &mut LayerSystem, x0: i32, y0: i32, x1: i32, y1: i32, c: Color, w: i32) {
    bresenham_thick(layer, x0, y0, x1, y1, c, w);
}

// ── public API ───────────────────────────────────────────────────────

pub fn draw_svg(layer: &mut LayerSystem, svg: &str, ox: i32, oy: i32) {
    let tags = extract_tags(svg);

    let mut vb_x = 0.0f32;
    let mut vb_y = 0.0f32;
    let mut vb_w = 0.0f32;
    let mut vb_h = 0.0f32;

    for tag in &tags {
        if tag.name == "svg" {
            if let Some(vb) = attr_str(&tag.attrs, "viewBox") {
                let parts: Vec<&str> = vb.split(|c: char| c == ' ' || c == ',').collect();
                if parts.len() >= 4 {
                    vb_x = parts[0].trim().parse().unwrap_or(0.0);
                    vb_y = parts[1].trim().parse().unwrap_or(0.0);
                    vb_w = parts[2].trim().parse().unwrap_or(0.0);
                    vb_h = parts[3].trim().parse().unwrap_or(0.0);
                }
            }
        }
    }

    let (sx, sy) = if vb_w > 0.0 && vb_h > 0.0 {
        (layer.width() as f32 / vb_w, layer.height() as f32 / vb_h)
    } else { (1.0, 1.0) };

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
                    if *stroke != Color::TRANSPARENT { rasterize_stroke(layer, &s, *stroke, *sw, ox, oy); }
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
                if *stroke != Color::TRANSPARENT { rasterize_stroke(layer, &s, *stroke, *sw, ox, oy); }
            }
            Elem::Ellipse { cx, cy, rx, ry, fill, stroke, sw } => {
                let pts = ellipse_pts(*cx, *cy, *rx, *ry);
                let s = scale_pts(&pts, sx, sy);
                if *fill != Color::TRANSPARENT { rasterize_fill(layer, &s, *fill, ox, oy); }
                if *stroke != Color::TRANSPARENT { rasterize_stroke(layer, &s, *stroke, *sw, ox, oy); }
            }
            Elem::Line { x1, y1, x2, y2, stroke, sw } => {
                if *stroke != Color::TRANSPARENT {
                    thick_line(layer,
                        (*x1 * sx) as i32 + ox, (*y1 * sy) as i32 + oy,
                        (*x2 * sx) as i32 + ox, (*y2 * sy) as i32 + oy,
                        *stroke, (*sw * 0.5) as i32);
                }
            }
            Elem::Poly { pts, fill, stroke, sw } => {
                let s = scale_pts(pts, sx, sy);
                if *fill != Color::TRANSPARENT { rasterize_fill(layer, &s, *fill, ox, oy); }
                if *stroke != Color::TRANSPARENT { rasterize_stroke(layer, &s, *stroke, *sw, ox, oy); }
            }
            Elem::Path { d, fill, stroke, sw } => {
                let pts = flatten(d);
                let s = scale_pts(&pts, sx, sy);
                if *fill != Color::TRANSPARENT { rasterize_fill(layer, &s, *fill, ox, oy); }
                if *stroke != Color::TRANSPARENT { rasterize_stroke(layer, &s, *stroke, *sw, ox, oy); }
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
