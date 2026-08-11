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
use alloc::vec::Vec;

use baram_core::Color;
use baram_core::LayerSystem;

use kurbo::{
    Affine, BezPath, Cap, Circle, Ellipse, Join, Line, Point, Rect, RoundedRect, Shape, Stroke,
    StrokeOpts, flatten, stroke,
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
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
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


pub fn rasterize_svg_to_buffer(
    svg: &str,
    target_w: usize,
    target_h: usize,
) -> Vec<u8> {
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
                if w <= 0.0 || h <= 0.0 { None }
                else {
                    let rx_f = attr_f32(a, "rx") as f64;
                    let ry_f = attr_f32(a, "ry") as f64;
                    let r = if rx_f > 0.0 || ry_f > 0.0 { rx_f.max(ry_f) } else { 0.0 };
                    if r > 0.0 { Some(RoundedRect::new(x, y, x + w, y + h, r).to_path(TOLERANCE)) }
                    else { Some(Rect::new(x, y, x + w, y + h).to_path(TOLERANCE)) }
                }
            }
            "circle" => {
                let cx = attr_f32(a, "cx") as f64;
                let cy = attr_f32(a, "cy") as f64;
                let r = attr_f32(a, "r") as f64;
                if r > 0.0 { Some(Circle::new((cx, cy), r).to_path(TOLERANCE)) } else { None }
            }
            "ellipse" => {
                let cx = attr_f32(a, "cx") as f64;
                let cy = attr_f32(a, "cy") as f64;
                let rx = attr_f32(a, "rx") as f64;
                let ry = attr_f32(a, "ry") as f64;
                if rx > 0.0 && ry > 0.0 { Some(Ellipse::new((cx, cy), (rx, ry), 0.0).to_path(TOLERANCE)) } else { None }
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
                    if pts.is_empty() { None }
                    else {
                        let mut p = BezPath::new();
                        p.move_to(pts[0]);
                        for pt in &pts[1..] { p.line_to(*pt); }
                        if tag.name == "polygon" { p.close_path(); }
                        Some(p)
                    }
                } else { None }
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
            stroke_path_to_buf(&mut buf, target_w, target_h, &path, stroke, sw as f64, transform);
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
    if segments.is_empty() { return; }
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
    if !min_x.is_finite() || !min_y.is_finite() { return; }
    let py0 = (libm::floor(min_y) as i32).max(0) as usize;
    let py1 = (libm::ceil(max_y) as i32).min(buf_h as i32) as usize;
    if py0 >= py1 { return; }

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
                if ya == yb { continue; }
                let crosses_down = ya <= y_sample && yb > y_sample;
                let crosses_up = yb <= y_sample && ya > y_sample;
                if crosses_down || crosses_up {
                    let t = (y_sample - ya) / (yb - ya);
                    let x = a.x + t * (b.x - a.x);
                    crossings.push((x, if crosses_down { 1 } else { -1 }));
                }
            }
            if crossings.is_empty() { continue; }
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
            if cov_sum <= 0.0 { continue; }
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
                buf[off]     = ((cr * a * 255 + br * ba * inv) / (new_a * 255)) as u8;
                buf[off + 1] = ((cg * a * 255 + bg * ba * inv) / (new_a * 255)) as u8;
                buf[off + 2] = ((cb * a * 255 + bb * ba * inv) / (new_a * 255)) as u8;
                buf[off + 3] = new_a as u8;
            }
        }
    }
}




fn accumulate_span(coverage_f: &mut [f32], buf_w: usize, x0: f64, x1: f64) {
    if x1 <= x0 { return; }
    let x0i = libm::floor(x0) as i32;
    let x1i = libm::ceil(x1) as i32;
    let xs = x0i.max(0) as usize;
    let xe = x1i.min(buf_w as i32) as usize;
    if xs >= xe { return; }

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
    if width <= 0.0 { return; }
    let style = Stroke::new(width).with_join(Join::Round).with_caps(Cap::Round);
    let opts = StrokeOpts::default();
    let stroked = stroke(path.iter(), &style, &opts, TOLERANCE);
    let stroked_screen = transform * stroked;
    let segments = flatten_to_segments(&stroked_screen);
    fill_segments_to_buf(buf, buf_w, buf_h, &segments, color, FillRule::NonZero);
}


pub fn blit_cached(layer: &mut LayerSystem, pixels: &[u8], w: usize, h: usize, ox: i32, oy: i32) {
    blit_cached_alpha(layer, pixels, w, h, ox, oy, 255);
}

pub fn blit_cached_alpha(layer: &mut LayerSystem, pixels: &[u8], w: usize, h: usize, ox: i32, oy: i32, alpha_scale: u32) {
    let lw = layer.width();
    let lh = layer.height();
    let buf = layer.buf_mut();
    let stride = w * 4;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use core::arch::aarch64::*;
        let _v_alpha_scale = vdupq_n_s32(alpha_scale as i32);
        let _v_255 = vdupq_n_s32(255);
        let _v_255_inv = vdupq_n_s32(0);

        for sy in 0..h {
            let dst_y = oy as usize + sy;
            if dst_y >= lh { break; }
            let row = sy * stride;
            let dst_row = dst_y * lw;
            let dst_x0 = ox.max(0) as usize;
            let sx0 = dst_x0.saturating_sub(ox as usize);

            let mut sx = sx0;
            while sx + 4 <= w {
                let dst_x = ox as usize + sx;
                if dst_x + 4 > lw { break; }

                let off0 = row + sx * 4;
                let a0 = pixels[off0 + 3] as i32;
                let a1 = pixels[off0 + 4 + 3] as i32;
                let a2 = pixels[off0 + 8 + 3] as i32;
                let a3 = pixels[off0 + 12 + 3] as i32;

                let a_scaled = vld1q_s32([
                    a0 * alpha_scale as i32 / 255,
                    a1 * alpha_scale as i32 / 255,
                    a2 * alpha_scale as i32 / 255,
                    a3 * alpha_scale as i32 / 255,
                ].as_ptr());

                let all_zero = vmaxvq_s32(a_scaled) == 0;
                if all_zero { sx += 4; continue; }

                let bg_ptr = buf[dst_row + dst_x..].as_mut_ptr();
                let bg0 = *bg_ptr.add(0);
                let bg1 = *bg_ptr.add(1);
                let bg2 = *bg_ptr.add(2);
                let bg3 = *bg_ptr.add(3);

                let inv0 = 255 - vgetq_lane_s32(a_scaled, 0);
                let inv1 = 255 - vgetq_lane_s32(a_scaled, 1);
                let inv2 = 255 - vgetq_lane_s32(a_scaled, 2);
                let inv3 = 255 - vgetq_lane_s32(a_scaled, 3);

                let out = [
                    blend_pixel_u32(bg0, pixels[off0], pixels[off0+1], pixels[off0+2], vgetq_lane_s32(a_scaled, 0) as u32, inv0 as u32),
                    blend_pixel_u32(bg1, pixels[off0+4], pixels[off0+5], pixels[off0+6], vgetq_lane_s32(a_scaled, 1) as u32, inv1 as u32),
                    blend_pixel_u32(bg2, pixels[off0+8], pixels[off0+9], pixels[off0+10], vgetq_lane_s32(a_scaled, 2) as u32, inv2 as u32),
                    blend_pixel_u32(bg3, pixels[off0+12], pixels[off0+13], pixels[off0+14], vgetq_lane_s32(a_scaled, 3) as u32, inv3 as u32),
                ];
                vst1q_u32(bg_ptr, vld1q_u32(out.as_ptr()));
                sx += 4;
            }

            for sx in sx..w {
                let dst_x = ox as usize + sx;
                if dst_x >= lw { break; }
                let off = row + sx * 4;
                let a = (pixels[off + 3] as u32 * alpha_scale / 255) as u32;
                if a == 0 { continue; }
                let dst = &mut buf[dst_row + dst_x];
                if a == 255 {
                    *dst = Color::rgb(pixels[off], pixels[off+1], pixels[off+2]).0;
                } else {
                    let cr = pixels[off] as u32;
                    let cg = pixels[off+1] as u32;
                    let cb = pixels[off+2] as u32;
                    let bg = Color(*dst);
                    let inv = 255 - a;
                    let r = (cr * a + bg.r() as u32 * inv) / 255;
                    let g = (cg * a + bg.g() as u32 * inv) / 255;
                    let b = (cb * a + bg.b() as u32 * inv) / 255;
                    *dst = Color::rgb(r as u8, g as u8, b as u8).0;
                }
            }
        }
        return;
    }

    #[cfg(not(target_arch = "aarch64"))]
    for sy in 0..h {
        let dst_y = oy as usize + sy;
        if dst_y >= lh { break; }
        let row = sy * stride;
        let dst_row = dst_y * lw;
        let dst_x0 = ox.max(0) as usize;
        let sx0 = dst_x0.saturating_sub(ox as usize);
        for sx in sx0..w {
            let dst_x = ox as usize + sx;
            if dst_x >= lw { break; }
            let off = row + sx * 4;
            let a = (pixels[off + 3] as u32 * alpha_scale / 255) as u32;
            if a == 0 { continue; }
            let dst = &mut buf[dst_row + dst_x];
            if a == 255 {
                *dst = Color::rgb(pixels[off], pixels[off+1], pixels[off+2]).0;
            } else {
                let cr = pixels[off] as u32;
                let cg = pixels[off+1] as u32;
                let cb = pixels[off+2] as u32;
                let bg = Color(*dst);
                let inv = 255 - a;
                let r = (cr * a + bg.r() as u32 * inv) / 255;
                let g = (cg * a + bg.g() as u32 * inv) / 255;
                let b = (cb * a + bg.b() as u32 * inv) / 255;
                *dst = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }
    }
}

pub fn blit_cached_scaled(layer: &mut LayerSystem, pixels: &[u8], w: usize, h: usize, ox: i32, oy: i32, scale: i32) {
    if scale <= 1 {
        blit_cached(layer, pixels, w, h, ox, oy);
        return;
    }
    let lw = layer.width();
    let lh = layer.height();
    let buf = layer.buf_mut();
    let src_stride = w * 4;
    let dst_w = w * scale as usize;
    let dst_h = h * scale as usize;
    for sy in 0..dst_h {
        let dst_y = oy as usize + sy;
        if dst_y >= lh { break; }
        let src_y = sy / scale as usize;
        if src_y >= h { break; }
        let src_row = src_y * src_stride;
        let dst_row = dst_y * lw;
        for sx in 0..dst_w {
            let dst_x = ox as usize + sx;
            if dst_x >= lw { break; }
            let src_x = sx / scale as usize;
            if src_x >= w { break; }
            let off = src_row + src_x * 4;
            let a = pixels[off + 3] as u32;
            if a == 0 { continue; }
            let dst = &mut buf[dst_row + dst_x];
            if a == 255 {
                *dst = Color::rgb(pixels[off], pixels[off+1], pixels[off+2]).0;
            } else {
                let cr = pixels[off] as u32;
                let cg = pixels[off+1] as u32;
                let cb = pixels[off+2] as u32;
                let bg = Color(*dst);
                let inv = 255 - a;
                let r = (cr * a + bg.r() as u32 * inv) / 255;
                let g = (cg * a + bg.g() as u32 * inv) / 255;
                let b = (cb * a + bg.b() as u32 * inv) / 255;
                *dst = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }
    }
}

pub fn blit_shadow(layer: &mut LayerSystem, pixels: &[u8], w: usize, h: usize, ox: i32, oy: i32) {
    let lw = layer.width();
    let lh = layer.height();
    let buf = layer.buf_mut();
    let stride = w * 4;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use core::arch::aarch64::*;
        for sy in 0..h {
            let dst_y = oy as usize + sy;
            if dst_y >= lh { break; }
            let row = sy * stride;
            let dst_row = dst_y * lw;
            let dst_x0 = ox.max(0) as usize;
            let sx0 = dst_x0.saturating_sub(ox as usize);

            let mut sx = sx0;
            while sx + 4 <= w {
                let dst_x = ox as usize + sx;
                if dst_x + 4 > lw { break; }

                let off0 = row + sx * 4;
                let a0 = pixels[off0 + 3] as i32;
                let a1 = pixels[off0 + 4 + 3] as i32;
                let a2 = pixels[off0 + 8 + 3] as i32;
                let a3 = pixels[off0 + 12 + 3] as i32;

                if a0 == 0 && a1 == 0 && a2 == 0 && a3 == 0 { sx += 4; continue; }

                let bg_ptr = buf[dst_row + dst_x..].as_mut_ptr();
                let bg0 = *bg_ptr.add(0);
                let bg1 = *bg_ptr.add(1);
                let bg2 = *bg_ptr.add(2);
                let bg3 = *bg_ptr.add(3);

                let inv0 = (255 - a0) as u32;
                let inv1 = (255 - a1) as u32;
                let inv2 = (255 - a2) as u32;
                let inv3 = (255 - a3) as u32;

                let out = [
                    ((bg0 >> 16 & 0xFF) * inv0 / 255) << 16 | ((bg0 >> 8 & 0xFF) * inv0 / 255) << 8 | (bg0 & 0xFF) * inv0 / 255,
                    ((bg1 >> 16 & 0xFF) * inv1 / 255) << 16 | ((bg1 >> 8 & 0xFF) * inv1 / 255) << 8 | (bg1 & 0xFF) * inv1 / 255,
                    ((bg2 >> 16 & 0xFF) * inv2 / 255) << 16 | ((bg2 >> 8 & 0xFF) * inv2 / 255) << 8 | (bg2 & 0xFF) * inv2 / 255,
                    ((bg3 >> 16 & 0xFF) * inv3 / 255) << 16 | ((bg3 >> 8 & 0xFF) * inv3 / 255) << 8 | (bg3 & 0xFF) * inv3 / 255,
                ];
                vst1q_u32(bg_ptr, vld1q_u32(out.as_ptr()));
                sx += 4;
            }

            for sx in sx..w {
                let dst_x = ox as usize + sx;
                if dst_x >= lw { break; }
                let a = pixels[row + sx * 4 + 3] as u32;
                if a == 0 { continue; }
                let inv = 255 - a;
                let bg = Color(buf[dst_row + dst_x]);
                let r = (bg.r() as u32 * inv) / 255;
                let g = (bg.g() as u32 * inv) / 255;
                let b = (bg.b() as u32 * inv) / 255;
                buf[dst_row + dst_x] = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }
        return;
    }

    #[cfg(not(target_arch = "aarch64"))]
    for sy in 0..h {
        let dst_y = oy as usize + sy;
        if dst_y >= lh { break; }
        let row = sy * stride;
        let dst_row = dst_y * lw;
        let dst_x0 = ox.max(0) as usize;
        let sx0 = dst_x0.saturating_sub(ox as usize);
        for sx in sx0..w {
            let dst_x = ox as usize + sx;
            if dst_x >= lw { break; }
            let a = pixels[row + sx * 4 + 3] as u32;
            if a == 0 { continue; }
            let inv = 255 - a;
            let bg = Color(buf[dst_row + dst_x]);
            let r = (bg.r() as u32 * inv) / 255;
            let g = (bg.g() as u32 * inv) / 255;
            let b = (bg.b() as u32 * inv) / 255;
            buf[dst_row + dst_x] = Color::rgb(r as u8, g as u8, b as u8).0;
        }
    }
}





static mut CURSOR_SHADOW_CACHE: Option<(usize, usize, Vec<u8>)> = None;


pub fn draw_svg_shadow(
    layer: &mut LayerSystem,
    svg: &str,
    ox: i32,
    oy: i32,
    target_w: f32,
    target_h: f32,
    blur_r: i32,
    _offset_y: i32,
) {
    let tw = target_w as usize;
    let th = target_h as usize;
    if tw == 0 || th == 0 { return; }

    
    unsafe {
        if let Some((cw, ch, ref cached)) = CURSOR_SHADOW_CACHE {
            if cw == tw && ch == th {
                
                
                let pad = blur_r as usize;
                let pw = tw + pad * 2;
                let ph = th + pad * 2;
                blit_shadow(layer, cached, pw, ph, ox - pad as i32, oy - pad as i32);
                return;
            }
        }
    }

    
    let svg_buf = rasterize_svg_to_buffer(svg, tw, th);

    
    let mut silhouette: Vec<f32> = alloc::vec![0.0; tw * th];
    for i in 0..tw * th {
        if svg_buf[i * 4 + 3] > 0 {
            silhouette[i] = 1.0;
        }
    }

    
    let pad = blur_r as usize;
    let pw = tw + pad * 2;
    let ph = th + pad * 2;
    let mut padded: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..th {
        for x in 0..tw {
            padded[(y + pad) * pw + (x + pad)] = silhouette[y * tw + x];
        }
    }

    let sigma = blur_r as f32 / 3.0;
    let mut kernel: Vec<f32> = alloc::vec![0.0; (blur_r * 2 + 1) as usize];
    let mut k_sum = 0.0f32;
    for i in 0..=blur_r * 2 {
        let x = (i - blur_r) as f32;
        let w = libm::expf(-x * x / (2.0 * sigma * sigma));
        kernel[i as usize] = w;
        k_sum += w;
    }
    for k in kernel.iter_mut() {
        *k /= k_sum;
    }

    
    let mut tmp: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..ph {
        for x in 0..pw {
            let mut sum = 0.0f32;
            for dx in -blur_r..=blur_r {
                let sx = x as i32 + dx;
                if sx >= 0 && sx < pw as i32 {
                    sum += padded[y * pw + sx as usize] * kernel[(dx + blur_r) as usize];
                }
            }
            tmp[y * pw + x] = sum;
        }
    }
    
    let mut result: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..ph {
        for x in 0..pw {
            let mut sum = 0.0f32;
            for dy in -blur_r..=blur_r {
                let sy = y as i32 + dy;
                if sy >= 0 && sy < ph as i32 {
                    sum += tmp[sy as usize * pw + x] * kernel[(dy + blur_r) as usize];
                }
            }
            result[y * pw + x] = sum;
        }
    }

    
    let mut shadow: Vec<u8> = alloc::vec![0u8; pw * ph * 4];
    for i in 0..pw * ph {
        let a = (result[i] * 120.0).min(255.0) as u8;
        shadow[i * 4] = 0;
        shadow[i * 4 + 1] = 0;
        shadow[i * 4 + 2] = 0;
        shadow[i * 4 + 3] = a;
    }

    
    blit_shadow(layer, &shadow, pw, ph, ox - pad as i32, oy - pad as i32);

    unsafe {
        CURSOR_SHADOW_CACHE = Some((tw, th, shadow));
    }
}





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





#[inline]
fn blend_pixel(layer: &mut LayerSystem, x: usize, y: usize, c: Color, coverage: f32) {
    if x >= layer.width() || y >= layer.height() || coverage <= 0.0 {
        return;
    }
    let cov = if coverage >= 1.0 { 1.0 } else { coverage };
    let w = layer.width();
    let buf = &mut layer.buf_mut()[y * w..];
    if cov >= 1.0 {
        buf[x] = c.0;
        return;
    }
    let a = (cov * 255.0) as u32;
    let inv = 255 - a;
    let bg = Color(buf[x]);
    let r = (c.r() as u32 * a + bg.r() as u32 * inv) / 255;
    let g = (c.g() as u32 * a + bg.g() as u32 * inv) / 255;
    let b = (c.b() as u32 * a + bg.b() as u32 * inv) / 255;
    buf[x] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
}








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
        
        
        _ => {}
    });

    segments
}















fn fill_segments_aa(
    layer: &mut LayerSystem,
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

    
    let mut crossings: Vec<(f64, i32)> = Vec::with_capacity(16);

    for py in py0..py1 {
        
        
        
        for sy in 0..SS_Y {
            let y_sample = py as f64 + (sy as f64 + 0.5) / SS_Y as f64;

            crossings.clear();
            for (a, b) in segments {
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
                    let dir = if crosses_down { 1 } else { -1 };
                    crossings.push((x, dir));
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
                        paint_span(layer, span_start, x, py, color);
                    }
                    span_open = false;
                }
                winding = new_winding;
            }
            
            
        }
    }
}




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

    
    let left_cov = ((x0i as f64 + 1.0) - x0).max(0.0).min(1.0);
    
    let right_cov = (x1 - ((x1i as f64) - 1.0)).max(0.0).min(1.0);

    if xe - xs == 1 {
        
        let cov = (x1 - x0).max(0.0).min(1.0);
        blend_pixel(layer, xs, py, color, cov as f32);
        return;
    }

    blend_pixel(layer, xs, py, color, left_cov as f32);
    {
        let lw = layer.width();
        let row = &mut layer.buf_mut()[py * lw..];
        let v = color.0;
        for px in (xs + 1)..(xe.saturating_sub(1)) {
            row[px] = v;
        }
    }
    if xe > xs + 1 {
        blend_pixel(layer, xe - 1, py, color, right_cov as f32);
    }
}






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
    
    
    let stroked = stroke(path.iter(), &style, &opts, TOLERANCE);
    let stroked_screen = transform * stroked;
    let segments = flatten_to_segments(&stroked_screen);
    fill_segments_aa(layer, &segments, color, FillRule::NonZero);
}







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
    if tw == 0 || th == 0 { return; }

    
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
