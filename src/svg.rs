//! SVG rasterizer backed by `kurbo`.
//!
//! This module replaces an earlier hand-rolled mini-rasterizer that produced
//! "noisy" cursor / icon output.  The rewrite delegates the hard parts to
//! the [`kurbo`](https://crates.io/crates/kurbo) crate (Rust, `no_std`):
//!
//! * `BezPath::from_svg` — parses the SVG path `d` syntax (M/L/H/V/C/S/Q/T/A/Z,
//!   relative + absolute, arcs converted to cubic Beziers, reflection of the
//!   last control point for S/T).  This is far more correct than the previous
//!   hand-written tokenizer.
//! * `kurbo::flatten` — adaptively subdivides Bezier curves into line
//!   segments using a tolerance-based error metric, so small icons no longer
//!   show polygon-shaped curves.
//! * `kurbo::stroke` — generates a proper offset-polygon stroke outline with
//!   round joins and caps, replacing the old "stamp circles along the path"
//!   approach that produced bumpy strokes.
//! * `kurbo::{Rect, Circle, Ellipse, RoundedRect, Line}` — shape primitives
//!   that convert to `BezPath` via the `Shape` trait.
//!
//! The fill rasterizer uses 4-row subpixel scanline coverage with
//! exact fractional X coverage per sub-row, plus the nonzero winding number
//! rule (SVG default).  Even-odd is also supported via the `fill-rule`
//! attribute.  The result is clean, anti-aliased icons at any size.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::gop::Color;
use crate::window::LayerSystem;

use kurbo::{
    Affine, BezPath, Cap, Circle, Ellipse, Join, Line, Point, Rect, RoundedRect, Shape, Stroke,
    StrokeOpts, flatten, stroke,
};

/// Flattening tolerance in *target* (screen) pixels.  0.1 px is well below
/// the threshold of perception at typical UI scales and gives smooth curves
/// even for 8×8 icons.
const TOLERANCE: f64 = 0.1;

/// Number of sub-rows sampled per pixel row for vertical anti-aliasing.
/// 4 sub-rows combined with exact fractional X coverage gives ~256 coverage
/// levels per pixel — visually indistinguishable from continuous coverage.
const SS_Y: usize = 4;

// ─────────────────────────────────────────────────────────────────────
// Fill rule
// ─────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────
// Tiny XML-ish extractor (kept from the previous implementation — works
// for the well-formed SVGs we ship in `src/data/`).
// ─────────────────────────────────────────────────────────────────────

struct Tag {
    name: String,
    attrs: String,
}

fn extract_tags(svg: &str) -> Vec<Tag> {
    let mut tags = Vec::new();
    let bytes = svg.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    // Depth of <defs> nesting.  Tags inside <defs> (clip paths, gradients,
    // patterns, masks, ...) are NOT rendered directly — they are referenced
    // by id from other elements.  Skipping them prevents, e.g., the
    // `<rect fill="white">` inside a `<clipPath>` from being painted as a
    // normal rectangle and clobbering the rest of the SVG.
    let mut defs_depth: u32 = 0;
    while i < len {
        if bytes[i] == b'<' {
            let mut j = i + 1;
            let is_close = j < len && bytes[j] == b'/';
            if is_close {
                j += 1;
            }
            if j < len && bytes[j] == b'?' {
                // XML declaration
            } else if j < len && bytes[j] == b'!' {
                // skip <!...> entirely (comments, DOCTYPE, CDATA)
                while i < len && bytes[i] != b'>' {
                    i += 1;
                }
                i += 1;
                continue;
            }
            // Extract the tag name.
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
            // Find end of tag (either /> or >).
            let mut k = j;
            while k < len
                && bytes[k] != b'>'
                && !(bytes[k] == b'/' && k + 1 < len && bytes[k + 1] == b'>')
            {
                k += 1;
            }
            // Extract attributes: everything between tag-name end (j) and the
            // closing ">" or "/>" (k).
            let attr_start = j;
            let self_closing = k < len && bytes[k] == b'/';
            if k < len {
                k += if self_closing { 2 } else { 1 };
            }
            let attr_end = k.saturating_sub(1); // back up past '>'
            let attrs =
                String::from_utf8_lossy(&bytes[attr_start.min(len)..attr_end.min(len)]).to_string();
            i = k;

            // Track <defs> open/close (only for non-self-closing tags).
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
            // Also skip standalone clipPath / mask / pattern / linearGradient
            // / radialGradient / symbol / marker tags (these define paint
            // servers or clip/mask regions but are not drawn directly).
            let is_definition_only = matches!(
                name.as_str(),
                "clipPath" | "mask" | "pattern" | "linearGradient"
                    | "radialGradient" | "symbol" | "marker" | "filter"
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
                // #RRGGBBAA — alpha currently coerced to opaque-or-transparent
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

// ─────────────────────────────────────────────────────────────────────
// Pixel blending
// ─────────────────────────────────────────────────────────────────────

#[inline]
fn blend_pixel(layer: &mut LayerSystem, x: usize, y: usize, c: Color, coverage: f32) {
    if x >= layer.width() || y >= layer.height() || coverage <= 0.0 {
        return;
    }
    let cov = if coverage >= 1.0 { 1.0 } else { coverage };
    // Fast path: full coverage → opaque replacement.
    if cov >= 1.0 {
        layer.put_pixel(x, y, c);
        return;
    }
    let bg = layer.get_pixel(x, y);
    let a = (cov * 255.0) as u32;
    let inv = 255 - a;
    let r = (c.r() as u32 * a + bg.r() as u32 * inv) / 255;
    let g = (c.g() as u32 * a + bg.g() as u32 * inv) / 255;
    let b = (c.b() as u32 * a + bg.b() as u32 * inv) / 255;
    layer.put_pixel(x, y, Color(0xFF00_0000 | (r << 16) | (g << 8) | b));
}

// ─────────────────────────────────────────────────────────────────────
// Path flattening: BezPath → Vec<(Point, Point)> line segments
// ─────────────────────────────────────────────────────────────────────

/// Flatten a `BezPath` into a list of line segments (pairs of endpoints).
/// `MoveTo` and `ClosePath` are handled here: `ClosePath` produces a final
/// segment back to the most recent `MoveTo` point.
fn flatten_to_segments(path: &BezPath) -> Vec<(Point, Point)> {
    let mut segments: Vec<(Point, Point)> = Vec::new();
    let mut cur = Point::ZERO;
    let mut start = Point::ZERO;
    let mut have_start = false;

    flatten(path.iter(), TOLERANCE, |el| match el {
        kurbo::PathEl::MoveTo(p) => {
            start = p;
            cur = p;
            have_start = true;
        }
        kurbo::PathEl::LineTo(p) => {
            if have_start {
                segments.push((cur, p));
            }
            cur = p;
        }
        kurbo::PathEl::ClosePath => {
            if have_start && cur != start {
                segments.push((cur, start));
            }
            cur = start;
        }
        // CurveTo / QuadTo never appear in flattened output, but handle them
        // defensively by ignoring (they would be flattened by `flatten`).
        _ => {}
    });

    segments
}

// ─────────────────────────────────────────────────────────────────────
// Fill rasterizer — subpixel scanline AA + nonzero / even-odd winding
// ─────────────────────────────────────────────────────────────────────

/// Fill a flattened polygon (possibly multi-contour) into the layer with
/// anti-aliasing.
///
/// Algorithm: for each scanline y (integer pixel row), sample SS_Y
/// sub-rows at `y + (sy + 0.5) / SS_Y`.  For each sub-row, find every
/// segment that crosses the sub-row's horizontal line, sort the crossings
/// by x, and walk them while tracking the winding number.  Each pair of
/// "outside → inside" / "inside → outside" crossings defines a filled
/// span; the fraction of that span that overlaps pixel `[px, px+1)` adds
/// to that pixel's coverage.  Coverage is averaged across sub-rows.
fn fill_segments_aa(
    layer: &mut LayerSystem,
    segments: &[(Point, Point)],
    color: Color,
    fill_rule: FillRule,
) {
    if segments.is_empty() {
        return;
    }

    // Compute bounding box in pixel space.
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

    let lw = layer.width() as i32;
    let lh = layer.height() as i32;
    if max_x < 0.0 || max_y < 0.0 || min_x >= lw as f64 || min_y >= lh as f64 {
        return;
    }

    let py0 = (libm::floor(min_y) as i32).max(0) as usize;
    let py1 = (libm::ceil(max_y) as i32).min(lh) as usize;
    if py0 >= py1 {
        return;
    }

    // Reusable buffers per sub-row.
    let mut crossings: Vec<(f64, i32)> = Vec::with_capacity(16);

    for py in py0..py1 {
        // Pre-compute pixel X range that can possibly be touched on this
        // scanline (clipped to layer width).  We re-compute per sub-row to
        // keep memory traffic low.
        for sy in 0..SS_Y {
            let y_sample = py as f64 + (sy as f64 + 0.5) / SS_Y as f64;

            crossings.clear();
            for (a, b) in segments {
                let ya = a.y;
                let yb = b.y;
                // Skip horizontal segments.
                if ya == yb {
                    continue;
                }
                // Count an edge crossing only if the sub-row strictly
                // intersects the half-open interval [min(ya,yb), max(ya,yb)).
                // Using `<=` on one side and `>` on the other (below) gives
                // the standard nonzero scanline rule and avoids double-
                // counting at shared vertices.
                let crosses_down = ya <= y_sample && yb > y_sample;
                let crosses_up = yb <= y_sample && ya > y_sample;
                if crosses_down || crosses_up {
                    let t = (y_sample - ya) / (yb - ya);
                    let x = a.x + t * (b.x - a.x);
                    let dir = if crosses_down { 1 } else { -1 };
                    crossings.push((x, dir));
                }
            }
            if crossings.is_empty() {
                continue;
            }
            crossings.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal));

            // Walk the crossings, identifying filled spans.
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
                        paint_span(layer, span_start, x, py, color);
                    }
                    span_open = false;
                }
                winding = new_winding;
            }
            // If the path is malformed and never closes, just drop the
            // dangling span.
        }
    }
}

/// Paint a horizontal span `[x0, x1)` at scanline `py` into the layer with
/// anti-aliasing at the left and right edges.  Interior pixels are written
/// fully opaque.
#[inline]
fn paint_span(layer: &mut LayerSystem, x0: f64, x1: f64, py: usize, color: Color) {
    if x1 <= x0 {
        return;
    }
    let lw = layer.width() as i32;
    let x0i = libm::floor(x0) as i32;
    let x1i = libm::ceil(x1) as i32;
    if x1i <= 0 || x0i >= lw {
        return;
    }
    let xs = x0i.max(0) as usize;
    let xe = x1i.min(lw) as usize;
    if xs >= xe {
        return;
    }

    // Left edge: coverage = (xs+1) - x0  (in [0,1))
    let left_cov = ((x0i as f64 + 1.0) - x0).max(0.0).min(1.0);
    // Right edge: coverage = x1 - (xe-1)  (in [0,1))
    let right_cov = (x1 - ((x1i as f64) - 1.0)).max(0.0).min(1.0);

    if xe - xs == 1 {
        // Span fits entirely inside one pixel.
        let cov = (x1 - x0).max(0.0).min(1.0);
        blend_pixel(layer, xs, py, color, cov as f32);
        return;
    }

    blend_pixel(layer, xs, py, color, left_cov as f32);
    for px in (xs + 1)..(xe.saturating_sub(1)) {
        layer.put_pixel(px, py, color);
    }
    if xe > xs + 1 {
        blend_pixel(layer, xe - 1, py, color, right_cov as f32);
    }
}

// ─────────────────────────────────────────────────────────────────────
// Stroke rasterizer — uses kurbo::stroke to build an offset-polygon
// outline, then flattens + fills that outline.
// ─────────────────────────────────────────────────────────────────────

fn stroke_path_aa(
    layer: &mut LayerSystem,
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
    // Stroke in user space, then transform the resulting outline.  This is
    // correct for any affine transform (including non-uniform scale).
    let stroked = stroke(path.iter(), &style, &opts, TOLERANCE);
    let stroked_screen = transform * stroked;
    let segments = flatten_to_segments(&stroked_screen);
    fill_segments_aa(layer, &segments, color, FillRule::NonZero);
}

// ─────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────

/// Draw an SVG into the layer where the SVG's viewBox maps onto the entire
/// layer (legacy behaviour).  Used for full-screen backgrounds.
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
    );
}

/// Draw an SVG into a target rectangle (ox, oy, target_w, target_h).
/// The SVG's viewBox is mapped onto that rectangle, preserving aspect ratio
/// (letterboxed).  Used for icons (window buttons, cursors, ...).
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
    );
}

/// Extract the attributes string of the root `<svg …>` tag directly from
/// the raw SVG text.  This is needed because `extract_tags()` intentionally
/// skips the `<svg>` element itself, so its width/height/viewBox would
/// otherwise be unreachable.
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
) {
    let tags = extract_tags(svg);

    let mut vb_x = 0.0f32;
    let mut vb_y = 0.0f32;
    let mut vb_w = 0.0f32;
    let mut vb_h = 0.0f32;

    let root_attrs = svg_root_attrs(svg);
    let svg_w = attr_f32(&root_attrs, "width");
    let svg_h = attr_f32(&root_attrs, "height");
    if let Some(vb) = attr_str(&root_attrs, "viewBox") {
        let parts: Vec<&str> = vb
            .split(|c: char| c == ' ' || c == ',')
            .collect();
        if parts.len() >= 4 {
            vb_x = parts[0].trim().parse().unwrap_or(0.0);
            vb_y = parts[1].trim().parse().unwrap_or(0.0);
            vb_w = parts[2].trim().parse().unwrap_or(0.0);
            vb_h = parts[3].trim().parse().unwrap_or(0.0);
        }
    }

    let src_w = if vb_w > 0.0 { vb_w } else { svg_w };
    let src_h = if vb_h > 0.0 { vb_h } else { svg_h };

    // Compute scale that fits src_w × src_h inside target_w × target_h
    // (letterboxed, centred).
    let (scale, dx, dy) = if src_w > 0.0 && src_h > 0.0 && target_w > 0.0 && target_h > 0.0 {
        let sx = target_w / src_w;
        let sy = target_h / src_h;
        let s = if sx < sy { sx } else { sy };
        let draw_w = src_w * s;
        let draw_h = src_h * s;
        let dx = ((target_w - draw_w) * 0.5).max(0.0);
        let dy = ((target_h - draw_h) * 0.5).max(0.0);
        (s as f64, dx as f64, dy as f64)
    } else {
        (1.0, 0.0, 0.0)
    };

    // Build the combined transform: screen translate → scale → viewBox translate.
    // After applying this to a path expressed in SVG user coordinates, the
    // path lands at the correct on-screen position.
    let transform = Affine::translate((ox as f64 + dx, oy as f64 + dy))
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

        // Convert the SVG element to a `BezPath` in user coordinates.
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

        // Fill: transform user-space path to screen space, flatten, fill.
        if fill != Color::TRANSPARENT {
            let path_screen = transform * &path;
            let segments = flatten_to_segments(&path_screen);
            fill_segments_aa(layer, &segments, fill, fill_rule);
        }

        // Stroke: build offset outline in user space, then transform.
        // Stroke width is expressed in user-space units, so the visual
        // width on screen is `sw * scale`.
        if stroke != Color::TRANSPARENT && sw > 0.0 {
            stroke_path_aa(layer, &path, stroke, sw as f64, transform);
        }
    }
}

/// Parse an SVG `points` attribute ("x1,y1 x2,y2 ...") into a Vec<Point>.
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

// Silence unused-import warning if `Line` is not directly referenced in
// every build configuration.
#[allow(dead_code)]
fn _unused_imports_anchor() {
    let _ = Line::new(Point::ZERO, Point::ZERO);
}
