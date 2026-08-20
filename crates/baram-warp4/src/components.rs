use super::*;
pub(crate) fn fill_ui_rounded_rect(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    color: Color,
) {
    if is_xiao() {
        if w == 0 || h == 0 {
            return;
        }
        let radius = radius.min(4).min(w / 2).min(h / 2);
        for row in 0..h {
            let inset = if row < radius {
                radius - row
            } else if row >= h.saturating_sub(radius) {
                radius - (h - 1 - row)
            } else {
                0
            };
            let width = w.saturating_sub(inset.saturating_mul(2));
            if width > 0 {
                layer.fill_rect(x + inset, y + row, width, 1, color);
            }
        }
    } else {
        layer.fill_rounded_rect(x, y, w, h, radius, color);
    }
}

pub(crate) fn fill_ui_gradient_rounded_rect(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    border: Color,
    top: Color,
    bottom: Color,
) {
    if w == 0 || h == 0 {
        return;
    }
    fill_ui_rounded_rect(layer, x, y, w, h, radius, border);
    if w <= 2 || h <= 2 {
        return;
    }
    let inner_w = w - 2;
    let inner_h = h - 2;
    let inner_radius = radius.saturating_sub(1).min(inner_w / 2).min(inner_h / 2);
    for row in 0..inner_h {
        let inset = if row < inner_radius {
            inner_radius - row
        } else if row >= inner_h.saturating_sub(inner_radius) {
            inner_radius - (inner_h - 1 - row)
        } else {
            0
        };
        let width = inner_w.saturating_sub(inset.saturating_mul(2));
        if width == 0 {
            continue;
        }
        let amount = if inner_h <= 1 {
            0.0
        } else {
            row as f32 / (inner_h - 1) as f32
        };
        layer.fill_rect(
            x + 1 + inset,
            y + 1 + row,
            width,
            1,
            mix_color(top, bottom, amount),
        );
    }
}

pub(crate) fn outline_ui_rounded_rect(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    border: Color,
    fill: Color,
) {
    if is_xiao() {
        fill_ui_rounded_rect(layer, x, y, w, h, radius, border);
        if w > 2 && h > 2 {
            fill_ui_rounded_rect(
                layer,
                x + 1,
                y + 1,
                w - 2,
                h - 2,
                radius.saturating_sub(1),
                fill,
            );
        }
    } else {
        layer.rounded_rect_outline(x, y, w, h, radius, border, fill);
    }
}

pub(crate) fn fill_ui_circle(
    layer: &mut LayerSystem,
    cx: usize,
    cy: usize,
    radius: usize,
    color: Color,
) {
    if !is_xiao() {
        layer.fill_circle(cx, cy, radius, color);
        return;
    }
    let radius = radius.max(1) as i32;
    let center_x = cx as i32;
    let center_y = cy as i32;
    let radius_sq = radius * radius;
    for dy in -radius..=radius {
        let mut span = radius;
        while span > 0 && span * span + dy * dy > radius_sq {
            span -= 1;
        }
        let left = (center_x - span).max(0) as usize;
        layer.fill_rect(
            left,
            (center_y + dy).max(0) as usize,
            (span * 2 + 1) as usize,
            1,
            color,
        );
    }
}

pub(crate) fn draw_ui_button(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    color: Color,
    _pressed: bool,
    _primary: bool,
) {
    if !is_xiao() {
        layer.fill_rounded_rect(x, y, w, h, radius, color);
        return;
    }
    // Xiao keeps the System 9 face and edge treatment, while the XML-resolved
    // node size controls the actual button bounds.
    let p = palette();
    fill_ui_rounded_rect(layer, x, y, w, h, p.button_radius, p.button_border);
    if w <= 2 || h <= 2 {
        return;
    }
    fill_ui_rounded_rect(layer, x + 1, y + 1, w - 2, h - 2, 1, color);
    if w > 4 && h > 4 {
        let horizontal_w = w - 4;
        let vertical_h = h - 4;
        if _pressed {
            layer.fill_rect(x + 2, y + 1, horizontal_w, 1, p.button_shadow_1);
            layer.fill_rect(x + 1, y + 2, 1, vertical_h, p.button_shadow_1);
            layer.fill_rect(x + 2, y + 2, horizontal_w, 1, p.button_shadow_2);
            layer.fill_rect(x + 2, y + 2, 1, vertical_h, p.button_shadow_2);
            layer.fill_rect(x + 2, y + h - 2, horizontal_w, 1, p.button_highlight);
            layer.fill_rect(x + w - 2, y + 2, 1, vertical_h, p.button_highlight);
        } else {
            layer.fill_rect(x + 2, y + 1, horizontal_w, 1, p.button_inner_edge);
            layer.fill_rect(x + 1, y + 2, 1, vertical_h, p.button_inner_edge);
            layer.fill_rect(x + 2, y + 2, horizontal_w, 1, p.button_highlight);
            layer.fill_rect(x + 2, y + 2, 1, vertical_h, p.button_highlight);
            layer.fill_rect(x + 2, y + h - 2, horizontal_w, 1, p.button_shadow_1);
            layer.fill_rect(x + w - 2, y + 2, 1, vertical_h, p.button_shadow_1);
            layer.fill_rect(x + 2, y + h - 3, horizontal_w, 1, p.button_shadow_2);
            layer.fill_rect(x + w - 3, y + 2, 1, vertical_h, p.button_shadow_2);
        }
    }
}

pub(crate) fn ui_button_face(active: bool, selected: bool, hover: bool, primary: bool) -> Color {
    let p = palette();
    if is_xiao() {
        return if active {
            Color::rgb(190, 190, 190)
        } else if hover {
            Color::rgb(235, 235, 235)
        } else {
            p.button_face
        };
    }
    let primary_color = if active {
        Color::rgb(0, 96, 196)
    } else if selected {
        Color::rgb(88, 148, 216)
    } else if hover {
        Color::rgb(0, 112, 232)
    } else {
        p.warp4_primary
    };
    if primary {
        primary_color
    } else if selected {
        Color::rgb(198, 222, 248)
    } else if active {
        Color::rgb(224, 224, 226)
    } else if hover {
        Color::rgb(244, 244, 245)
    } else {
        p.warp4_button_bg
    }
}

pub(crate) fn draw_ui_switch(layer: &mut LayerSystem, x: i32, y: i32, h: i32, on: bool) {
    let p = palette();
    let track_w = ui_px_usize(40).max(12);
    let track_h = ui_px_usize(20).max(8);
    let sy = (y + (h - track_h as i32).max(0) / 2).max(0) as usize;
    fill_ui_gradient_rounded_rect(
        layer,
        x.max(0) as usize,
        sy,
        track_w,
        track_h,
        ui_px_usize(5).max(2),
        p.warp3_border,
        p.warp9_highlight,
        p.warp9_mid,
    );
    let knob_w = track_w.saturating_sub(6).min(ui_px_usize(16).max(6));
    let knob_h = track_h.saturating_sub(4).max(3);
    let knob_x = if on {
        x.max(0) as usize + track_w.saturating_sub(knob_w + 3)
    } else {
        x.max(0) as usize + 3
    };
    fill_ui_gradient_rounded_rect(
        layer,
        knob_x,
        sy + 2,
        knob_w,
        knob_h,
        ui_px_usize(3).max(1),
        p.warp9_dark_shadow,
        if on {
            Color::rgb(0x75, 0x9A, 0xDA)
        } else {
            p.warp9_highlight
        },
        if on { p.warp4_primary } else { p.warp9_shadow },
    );
}

pub(crate) fn draw_ui_input(layer: &mut LayerSystem, x: usize, y: usize, w: usize, h: usize) {
    let p = palette();
    fill_ui_rounded_rect(layer, x, y, w, h, 4, p.warp4_input_border);
    if w > 2 && h > 2 {
        fill_ui_rounded_rect(layer, x + 1, y + 1, w - 2, h - 2, 3, p.warp4_input_bg);
        if w > 4 && h > 4 {
            layer.fill_rect(x + 1, y + 1, w - 2, 1, p.warp9_shadow);
            layer.fill_rect(x + 1, y + h - 2, w - 2, 1, p.warp9_highlight);
            layer.fill_rect(x + 1, y + 1, 1, h - 2, p.warp9_shadow);
            layer.fill_rect(x + w - 2, y + 1, 1, h - 2, p.warp9_highlight);
        }
    }
}

pub(crate) fn draw_ui_checkbox(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    size: usize,
    checked: bool,
    hover: bool,
) {
    let p = palette();
    let border = if checked || hover {
        p.warp4_primary
    } else {
        p.warp3_muted
    };
    let (top, bottom) = if checked {
        (Color::rgb(0x78, 0x9E, 0xDE), p.warp4_primary)
    } else {
        (p.warp9_highlight, p.warp9_mid)
    };
    fill_ui_gradient_rounded_rect(layer, x, y, size, size, 3, border, top, bottom);
    if checked {
        draw_check_icon(
            layer,
            (x + ui_px_usize(5)) as i32,
            (y + ui_px_usize(5)) as i32,
        );
    }
}
