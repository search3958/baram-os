// SVG rasterizer backed by `kurbo`.
//
// This module replaces an earlier hand-rolled mini-rasterizer that produced
// "noisy" cursor / icon output.  The rewrite delegates the hard parts to
// the [`kurbo`](https://crates.io/crates/kurbo) crate (Rust, `no_std`):
//
// * `BezPath::from_svg` — parses the SVG path `d` syntax (M/L/H/V/C/S/Q/T/A/Z,
//   relative + absolute, arcs converted to cubic Beziers, reflection of the
//   last control point for S/T).  This is far more correct than the previous
//   hand-written tokenizer.
// * `kurbo::flatten` — adaptively subdivides Bezier curves into line
//   segments using a tolerance-based error metric, so small icons no longer
//   show polygon-shaped curves.
// * `kurbo::stroke` — generates a proper offset-polygon stroke outline with
//   round joins and caps, replacing the old "stamp circles along the path"
//   approach that produced bumpy strokes.
// * `kurbo::{Rect, Circle, Ellipse, RoundedRect, Line}` — shape primitives
//   that convert to `BezPath` via the `Shape` trait.
//
// The fill rasterizer uses 4-row subpixel scanline coverage with
// exact fractional X coverage per sub-row, plus the nonzero winding number
// rule (SVG default).  Even-odd is also supported via the `fill-rule`
// attribute.  The result is clean, anti-aliased icons at any size.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use baram_core::Color;
use baram_core::LayerSystem;

use kurbo::{
    flatten, stroke, Affine, BezPath, Cap, Circle, Ellipse, Join, Line, Point, Rect, RoundedRect,
    Shape, Stroke, StrokeOpts,
};

const TOLERANCE: f64 = 0.1;
const SS_Y: usize = 4;

#[inline(always)]
fn blend_pixel_u32(bg: u32, sr: u8, sg: u8, sb: u8, a: u32, inv: u32) -> u32 {
    let r = (sr as u32 * a + ((bg >> 16) & 0xFF) * inv) / 255;
    let g = (sg as u32 * a + ((bg >> 8) & 0xFF) * inv) / 255;
    let b = (sb as u32 * a + (bg & 0xFF) * inv) / 255;
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

struct CachedSvg {
    svg_ptr: *const str,
    width: usize,
    height: usize,

    pixels: Vec<u8>,
}

const SVG_CACHE_CAP: usize = 32;

static mut SVG_CACHE: [Option<CachedSvg>; SVG_CACHE_CAP] = [
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
];

fn cache_lookup(svg: &str, w: usize, h: usize) -> Option<(*const u8, usize, usize)> {
    let ptr = svg as *const str;
    unsafe {
        for entry in SVG_CACHE.iter() {
            if let Some(c) = entry {
                if c.svg_ptr == ptr && c.width == w && c.height == h {
                    return Some((c.pixels.as_ptr(), c.width, c.height));
                }
            }
        }
    }
    None
}

fn cache_store(svg: &str, w: usize, h: usize, pixels: Vec<u8>) {
    let ptr = svg as *const str;
    unsafe {
        for entry in SVG_CACHE.iter_mut() {
            if entry.is_none() {
                *entry = Some(CachedSvg {
                    svg_ptr: ptr,
                    width: w,
                    height: h,
                    pixels,
                });
                return;
            }
        }

        SVG_CACHE[0] = Some(CachedSvg {
            svg_ptr: ptr,
            width: w,
            height: h,
            pixels,
        });
    }
}

pub fn rasterize_svg_to_buffer(svg: &str, target_w: usize, target_h: usize) -> Vec<u8> {
    let mut buf: Vec<u8> = alloc::vec![0u8; target_w * target_h * 4];
    let tags = extract_tags(svg);

    let mut vb_x = 0.0f32;
    let mut vb_y = 0.0f32;
    let mut vb_w = 0.0f32;
    let mut vb_h = 0.0f32;

    let root_attrs = svg_root_attrs(svg);
    let svg_w = attr_f32(&root_attrs, "width");
    let svg_h = attr_f32(&root_attrs, "height");
    if let Some(vb) = attr_str(&root_attrs, "viewBox") {
        let parts: Vec<&str> = vb.split(|c: char| c == ' ' || c == ',').collect();
        if parts.len() >= 4 {
            vb_x = parts[0].trim().parse().unwrap_or(0.0);
            vb_y = parts[1].trim().parse().unwrap_or(0.0);
            vb_w = parts[2].trim().parse().unwrap_or(0.0);
            vb_h = parts[3].trim().parse().unwrap_or(0.0);
        }
    }

    let src_w = if vb_w > 0.0 { vb_w } else { svg_w };
    let src_h = if vb_h > 0.0 { vb_h } else { svg_h };

    let (scale, dx, dy) = if src_w > 0.0 && src_h > 0.0 {
        let sx = target_w as f32 / src_w;
        let sy = target_h as f32 / src_h;
        let s = if sx < sy { sx } else { sy };
        let draw_w = src_w * s;
        let draw_h = src_h * s;
        let dx = ((target_w as f32 - draw_w) * 0.5).max(0.0);
        let dy = ((target_h as f32 - draw_h) * 0.5).max(0.0);
        (s as f64, dx as f64, dy as f64)
    } else {
        (1.0, 0.0, 0.0)
    };

    let transform = Affine::translate((dx, dy))
        * Affine::scale(scale)
        * Affine::translate((-vb_x as f64, -vb_y as f64));

    for tag in &tags {
        let a = &tag.attrs;
        let fill = attr_color(a, "fill");
        let stroke = attr_color(a, "stroke");
        let sw: f32 = attr_str(a, "stroke-width")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0);
        let fill_rule = attr_fill_rule(a);

        let path_opt: Option<BezPath> = match tag.name.as_str() {
            "rect" => {
                let x = attr_f32(a, "x") as f64;
                let y = attr_f32(a, "y") as f64;
                let w = attr_f32(a, "width") as f64;
                let h = attr_f32(a, "height") as f64;
                if w <= 0.0 || h <= 0.0 {
                    None
                } else {
                    let rx_f = attr_f32(a, "rx") as f64;
                    let ry_f = attr_f32(a, "ry") as f64;
                    let r = if rx_f > 0.0 || ry_f > 0.0 {
                        rx_f.max(ry_f)
                    } else {
                        0.0
                    };
                    if r > 0.0 {
                        Some(RoundedRect::new(x, y, x + w, y + h, r).to_path(TOLERANCE))
                    } else {
                        Some(Rect::new(x, y, x + w, y + h).to_path(TOLERANCE))
                    }
                }
            }
            "circle" => {
                let cx = attr_f32(a, "cx") as f64;
                let cy = attr_f32(a, "cy") as f64;
                let r = attr_f32(a, "r") as f64;
                if r > 0.0 {
                    Some(Circle::new((cx, cy), r).to_path(TOLERANCE))
                } else {
                    None
                }
            }
            "ellipse" => {
                let cx = attr_f32(a, "cx") as f64;
                let cy = attr_f32(a, "cy") as f64;
                let rx = attr_f32(a, "rx") as f64;
                let ry = attr_f32(a, "ry") as f64;
                if rx > 0.0 && ry > 0.0 {
                    Some(Ellipse::new((cx, cy), (rx, ry), 0.0).to_path(TOLERANCE))
                } else {
                    None
                }
            }
            "line" => {
                let x1 = attr_f32(a, "x1") as f64;
                let y1 = attr_f32(a, "y1") as f64;
                let x2 = attr_f32(a, "x2") as f64;
                let y2 = attr_f32(a, "y2") as f64;
                let mut p = BezPath::new();
                p.move_to((x1, y1));
                p.line_to((x2, y2));
                Some(p)
            }
            "polygon" | "polyline" => {
                if let Some(s) = attr_str(a, "points") {
                    let pts = parse_pts_f64(s);
                    if pts.is_empty() {
                        None
                    } else {
                        let mut p = BezPath::new();
                        p.move_to(pts[0]);
                        for pt in &pts[1..] {
                            p.line_to(*pt);
                        }
                        if tag.name == "polygon" {
                            p.close_path();
                        }
                        Some(p)
                    }
                } else {
                    None
                }
            }
            "path" => attr_str(a, "d").and_then(|d| BezPath::from_svg(d).ok()),
            _ => None,
        };

        let Some(path) = path_opt else { continue };

        if fill != Color::TRANSPARENT {
            let path_screen = transform * &path;
            let segments = flatten_to_segments(&path_screen);
            fill_segments_to_buf(&mut buf, target_w, target_h, &segments, fill, fill_rule);
        }
        if stroke != Color::TRANSPARENT && sw > 0.0 {
            stroke_path_to_buf(
                &mut buf, target_w, target_h, &path, stroke, sw as f64, transform,
            );
        }
    }

    buf
}

fn fill_segments_to_buf(
    buf: &mut [u8],
    buf_w: usize,
    buf_h: usize,
    segments: &[(Point, Point)],
    color: Color,
    fill_rule: FillRule,
) {
    if segments.is_empty() {
        return;
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (a, b) in segments {
        min_x = min_x.min(a.x).min(b.x);
        min_y = min_y.min(a.y).min(b.y);
        max_x = max_x.max(a.x).max(b.x);
        max_y = max_y.max(a.y).max(b.y);
    }
    if !min_x.is_finite() || !min_y.is_finite() {
        return;
    }
    let py0 = (libm::floor(min_y) as i32).max(0) as usize;
    let py1 = (libm::ceil(max_y) as i32).min(buf_h as i32) as usize;
    if py0 >= py1 {
        return;
    }

    let mut crossings: Vec<(f64, i32)> = Vec::with_capacity(16);
    let cr = color.r() as u32;
    let cg = color.g() as u32;
    let cb = color.b() as u32;
    let stride = buf_w * 4;
    let mut coverage_f: Vec<f32> = alloc::vec![0.0; buf_w];

    for py in py0..py1 {
        let row = py * stride;
        coverage_f.fill(0.0);

        for sy in 0..SS_Y {
            let y_sample = py as f64 + (sy as f64 + 0.5) / SS_Y as f64;
            crossings.clear();
            for &(a, b) in segments {
                let ya = a.y;
                let yb = b.y;
                if ya == yb {
                    continue;
                }
                let crosses_down = ya <= y_sample && yb > y_sample;
                let crosses_up = yb <= y_sample && ya > y_sample;
                if crosses_down || crosses_up {
                    let t = (y_sample - ya) / (yb - ya);
                    let x = a.x + t * (b.x - a.x);
                    crossings.push((x, if crosses_down { 1 } else { -1 }));
                }
            }
            if crossings.is_empty() {
                continue;
            }
            crossings.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal));
            let mut winding: i32 = 0;
            let mut span_start: f64 = 0.0;
            let mut span_open = false;
            for &(x, dir) in &crossings {
                let new_winding = winding + dir;
                let was_inside = fill_rule.is_inside(winding);
                let is_inside = fill_rule.is_inside(new_winding);
                if !was_inside && is_inside {
                    span_start = x;
                    span_open = true;
                } else if was_inside && !is_inside {
                    if span_open {
                        accumulate_span(&mut coverage_f, buf_w, span_start, x);
                    }
                    span_open = false;
                }
                winding = new_winding;
            }
        }

        let ss_y_inv = 1.0 / SS_Y as f32;
        for px in 0..buf_w {
            let cov_sum = coverage_f[px];
            if cov_sum <= 0.0 {
                continue;
            }
            let alpha = (cov_sum * ss_y_inv * 255.0).min(255.0) as u32;
            let off = row + px * 4;
            let a = alpha;
            let inv = 255 - a;
            let br = buf[off] as u32;
            let bg = buf[off + 1] as u32;
            let bb = buf[off + 2] as u32;
            let ba = buf[off + 3] as u32;
            let new_a = (a * 255 + ba * inv) / 255;
            if new_a > 0 {
                buf[off] = ((cr * a * 255 + br * ba * inv) / (new_a * 255)) as u8;
                buf[off + 1] = ((cg * a * 255 + bg * ba * inv) / (new_a * 255)) as u8;
                buf[off + 2] = ((cb * a * 255 + bb * ba * inv) / (new_a * 255)) as u8;
                buf[off + 3] = new_a as u8;
            }
        }
    }
}

fn accumulate_span(coverage_f: &mut [f32], buf_w: usize, x0: f64, x1: f64) {
    if x1 <= x0 {
        return;
    }
    let x0i = libm::floor(x0) as i32;
    let x1i = libm::ceil(x1) as i32;
    let xs = x0i.max(0) as usize;
    let xe = x1i.min(buf_w as i32) as usize;
    if xs >= xe {
        return;
    }

    if xe - xs == 1 {
        let cov = (x1 - x0).max(0.0).min(1.0) as f32;
        coverage_f[xs] += cov;
        return;
    }

    let left_cov = ((x0i as f64 + 1.0) - x0).max(0.0).min(1.0) as f32;
    coverage_f[xs] += left_cov;

    for px in (xs + 1)..(xe - 1) {
        coverage_f[px] += 1.0;
    }

    let right_cov = (x1 - (x1i as f64 - 1.0)).max(0.0).min(1.0) as f32;
    coverage_f[xe - 1] += right_cov;
}

fn stroke_path_to_buf(
    buf: &mut [u8],
    buf_w: usize,
    buf_h: usize,
    path: &BezPath,
    color: Color,
    width: f64,
    transform: Affine,
) {
    if width <= 0.0 {
        return;
    }
    let style = Stroke::new(width)
        .with_join(Join::Round)
        .with_caps(Cap::Round);
    let opts = StrokeOpts::default();
    let stroked = stroke(path.iter(), &style, &opts, TOLERANCE);
    let stroked_screen = transform * stroked;
    let segments = flatten_to_segments(&stroked_screen);
    fill_segments_to_buf(buf, buf_w, buf_h, &segments, color, FillRule::NonZero);
}


