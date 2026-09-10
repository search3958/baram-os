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


