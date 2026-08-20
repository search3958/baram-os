use super::*;
pub(crate) fn parse_dim(s: &str, default: i32) -> i32 {
    let parsed = s
        .trim()
        .trim_end_matches("dip")
        .trim_end_matches("dp")
        .trim_end_matches("sp")
        .trim_end_matches("px")
        .parse()
        .ok();
    parsed.map(ui_px).unwrap_or(default)
}
pub(crate) fn dimension(raw: &str, available: i32, intrinsic: i32) -> i32 {
    match raw {
        "match_parent" | "fill_parent" => available,
        "wrap_content" | "" => intrinsic,
        "0dp" => 0,
        _ => parse_dim(raw, intrinsic),
    }
}
pub(crate) fn edges(n: &Node, base: &str) -> Edges {
    let style = n.attr("style");
    let (style_top, style_bottom) = if base == "layout_margin" && style.contains("SectionTitle") {
        (ui_px(22), ui_px(8))
    } else if base == "layout_margin" && style.contains("SectionDescription") {
        (ui_px(0), ui_px(14))
    } else if base == "layout_margin" && style.contains("ComponentLabel") {
        (ui_px(14), ui_px(6))
    } else if base == "layout_margin" && style.contains("ComponentBox") {
        (ui_px(0), ui_px(3))
    } else {
        (0, 0)
    };
    let all = parse_dim(n.attr(base), 0);
    let h = parse_dim(
        if n.attr(&format!("{base}Horizontal")).is_empty() {
            n.attr(base)
        } else {
            n.attr(&format!("{base}Horizontal"))
        },
        all,
    );
    let v = parse_dim(
        if n.attr(&format!("{base}Vertical")).is_empty() {
            n.attr(base)
        } else {
            n.attr(&format!("{base}Vertical"))
        },
        all,
    );
    let mut edges = Edges {
        top: parse_dim(
            n.attr(&format!("{base}Top")),
            if v == 0 { style_top } else { v },
        ),
        right: parse_dim(n.attr(&format!("{base}Right")), h),
        bottom: parse_dim(
            n.attr(&format!("{base}Bottom")),
            if v == 0 { style_bottom } else { v },
        ),
        left: parse_dim(n.attr(&format!("{base}Left")), h),
    };
    if is_xiao() && base == "padding" {
        // Xiao's compact profile keeps XML-owned button geometry while
        // reducing excess padding from legacy documents.
        edges.top = edges.top.min(ui_px(3));
        edges.bottom = edges.bottom.min(ui_px(2));
        edges.left = edges.left.min(ui_px(4));
        edges.right = edges.right.min(ui_px(4));
        if is_button_like(n) {
            // Button geometry comes from XML. Keep exactly 3px of vertical
            // content inset; ui_px(3) would round to zero at 25% scale.
            edges.top = 3;
            edges.bottom = 3;
        }
    }
    if is_xiao() && base == "layout_margin" {
        edges.top = edges.top.min(ui_px(6));
        edges.bottom = edges.bottom.min(ui_px(4));
        edges.left = edges.left.min(ui_px(4));
        edges.right = edges.right.min(ui_px(4));
    }
    edges
}
pub(crate) fn truth(value: &str) -> bool {
    let value = value.trim();
    value.eq_ignore_ascii_case("true") || value == "1" || value.eq_ignore_ascii_case("yes")
}

#[inline]
pub(crate) fn ascii_contains_ignore_case(value: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    !needle.is_empty()
        && value.as_bytes().windows(needle.len()).any(|window| {
            window
                .iter()
                .zip(needle.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
        })
}

pub(crate) fn is_scroll_container(n: &Node) -> bool {
    n.is("ScrollView")
        || n.is("HorizontalScrollView")
        || n.is("ListView")
        || n.is("GridView")
        || n.is("ExpandableListView")
}
pub(crate) fn contains_scroll(nodes: &[Node], idx: usize) -> bool {
    is_scroll_container(&nodes[idx])
        || nodes[idx]
            .children
            .iter()
            .any(|child| contains_scroll(nodes, *child))
}
pub(crate) fn is_fixed(n: &Node) -> bool {
    let position = n.attr("position");
    position.eq_ignore_ascii_case("fixed")
        || position.eq_ignore_ascii_case("sticky")
        || truth(n.attr("fixed"))
        || truth(n.attr("sticky"))
        || n.attr("layout_position").eq_ignore_ascii_case("fixed")
}
pub(crate) fn gravity_offset(
    gravity: &str,
    parent_w: i32,
    parent_h: i32,
    child_w: i32,
    child_h: i32,
) -> (i32, i32) {
    let x = if ascii_contains_ignore_case(gravity, "center_horizontal")
        || gravity.eq_ignore_ascii_case("center")
        || ascii_contains_ignore_case(gravity, "center|horizontal")
    {
        (parent_w - child_w) / 2
    } else if ascii_contains_ignore_case(gravity, "right")
        || ascii_contains_ignore_case(gravity, "end")
    {
        parent_w - child_w
    } else {
        0
    };
    let y = if ascii_contains_ignore_case(gravity, "center_vertical")
        || gravity.eq_ignore_ascii_case("center")
    {
        (parent_h - child_h) / 2
    } else if ascii_contains_ignore_case(gravity, "bottom") {
        parent_h - child_h
    } else {
        0
    };
    (x.max(0), y.max(0))
}
pub(crate) fn cross_offset(
    gravity: &str,
    parent_size: i32,
    child_size: i32,
    horizontal_axis: bool,
) -> i32 {
    if (horizontal_axis
        && (ascii_contains_ignore_case(gravity, "center_vertical")
            || gravity.eq_ignore_ascii_case("center")))
        || (!horizontal_axis
            && (ascii_contains_ignore_case(gravity, "center_horizontal")
                || gravity.eq_ignore_ascii_case("center")))
    {
        return (parent_size - child_size).max(0) / 2;
    }
    if (horizontal_axis && ascii_contains_ignore_case(gravity, "bottom"))
        || (!horizontal_axis
            && (ascii_contains_ignore_case(gravity, "right")
                || ascii_contains_ignore_case(gravity, "end")))
    {
        return (parent_size - child_size).max(0);
    }
    0
}
pub(crate) fn is_button_like(n: &Node) -> bool {
    n.is("Button") || n.is("PrimaryButton") || n.is("ImageButton") || n.is("ToggleButton")
}

pub(crate) fn is_text_button(n: &Node) -> bool {
    n.is("Button") || n.is("PrimaryButton") || n.is("ToggleButton")
}

pub(crate) fn is_text_input(n: &Node) -> bool {
    n.is("EditText") || n.is("AutoCompleteTextView") || n.is("MultiAutoCompleteTextView")
}

pub(crate) fn is_keyboard_focusable(n: &Node) -> bool {
    n.attr("enabled") != "false"
        && (is_button_like(n)
            || n.is("Switch")
            || n.is("CheckBox")
            || n.is("RadioButton")
            || n.is("ToggleButton")
            || n.is("Spinner")
            || n.is("SeekBar")
            || n.is("RatingBar")
            || n.is("SearchView")
            || n.is("NumberPicker")
            || n.is("DatePicker")
            || n.is("TimePicker")
            || is_text_input(n))
}

pub(crate) fn interactive(n: &Node) -> bool {
    n.attr("enabled") != "false"
        && (is_button_like(n)
            || n.is("EditText")
            || n.is("AutoCompleteTextView")
            || n.is("MultiAutoCompleteTextView")
            || n.is("Switch")
            || n.is("CheckBox")
            || n.is("RadioButton")
            || n.is("ToggleButton")
            || n.is("Spinner")
            || n.is("SeekBar")
            || n.is("RatingBar")
            || n.is("SearchView")
            || n.is("NumberPicker")
            || n.is("DatePicker")
            || n.is("TimePicker"))
}
pub(crate) fn measure(s: &str) -> i32 {
    if bdf_font::is_available() {
        return s.chars().map(bdf_font::advance).sum();
    }
    #[cfg(feature = "ttf")]
    if ttf_font::is_available() {
        s.chars().map(ttf_font::advance).sum()
    } else {
        s.len() as i32 * ui_px(8)
    }
    #[cfg(not(feature = "ttf"))]
    {
        s.len() as i32 * ui_px(8)
    }
}
pub(crate) fn measure_size(s: &str, size: f32) -> i32 {
    if bdf_font::is_available() {
        let _ = size;
        return measure(s);
    }
    #[cfg(feature = "ttf")]
    {
        if !ttf_font::is_available() {
            return measure(s);
        }
        let mut total = 0;
        for ch in s.chars() {
            let mut advance = 0;
            ttf_font::with_glyph_at_size(ch, size, |_data, _w, _h, glyph_advance, _y_off| {
                advance = glyph_advance;
            });
            total += advance.max(1);
        }
        return total;
    }
    #[cfg(not(feature = "ttf"))]
    {
        let _ = size;
        measure(s)
    }
}
pub(crate) fn text_size(n: &Node) -> f32 {
    if !n.attr("textSize").is_empty() {
        return parse_dim(n.attr("textSize"), ui_px(16)) as f32;
    }
    if is_button_like(n) {
        return ui_size(18.0);
    }
    if n.is("CheckBox") || n.is("RadioButton") || n.is("Switch") {
        return ui_size(14.0);
    }
    let style = n.attr("style");
    if style.contains("SectionTitle") {
        ui_size(24.0)
    } else if style.contains("SectionDescription") {
        ui_size(13.0)
    } else if style.contains("ComponentLabel") {
        ui_size(15.0)
    } else {
        ui_size(16.0)
    }
}
pub(crate) fn text_color(n: &Node) -> Color {
    if let Some(color) = parse_color(n.attr("textColor")) {
        return color;
    }
    if is_xiao() && (n.is("PrimaryButton") || n.is("Button")) {
        return palette().warp3_text;
    }
    if !is_xiao() && n.is("PrimaryButton") {
        return WARP4_WHITE;
    }
    if !is_xiao() && n.is("Button") {
        return palette().warp4_primary;
    }
    if n.is("EditText") || n.is("AutoCompleteTextView") || n.is("MultiAutoCompleteTextView") {
        return if is_xiao() {
            palette().warp3_text
        } else {
            WARP4_BLACK
        };
    }
    let style = n.attr("style");
    if style.contains("SectionDescription") {
        palette().warp3_muted
    } else if style.contains("ComponentLabel") {
        palette().warp3_muted
    } else {
        palette().warp3_text
    }
}
pub(crate) fn text_bold(_n: &Node) -> bool {
    false
}

pub(crate) fn mix_color(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount) as u8;
    Color::rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

pub(crate) fn draw_spinner_shadow(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    radius: usize,
) {
    if is_xiao() {
        // System 9 uses a compact, hard-edged drop shadow. Keep it to two solid
        // pixels: enough depth to read at 320x180, without blur or alpha work.
        fill_ui_rounded_rect(
            layer,
            x.saturating_add(ui_px_usize(2)),
            y.saturating_add(ui_px_usize(2)),
            width,
            height,
            radius,
            palette().warp9_shadow,
        );
        return;
    }
    // The normal profile keeps the smooth native shadow and its temporary
    // compositing layer. This branch is selected at runtime after boot.
    let pad = ui_px_usize(16);
    let offset_y = ui_px_usize(4);
    let shadow_w = width.saturating_add(pad * 2);
    let shadow_h = height.saturating_add(pad * 2 + offset_y);
    let mut mask = LayerSystem::new_transparent(shadow_w, shadow_h);
    mask.fill_rounded_rect(pad, pad + offset_y, width, height, radius, Color::BLACK);
    let mut alpha = alloc::vec![0u8; shadow_w * shadow_h];
    for (dst, src) in alpha.iter_mut().zip(mask.buf_ref()) {
        *dst = if *src == Color::TRANSPARENT.0 { 0 } else { 52 };
    }
    blur_shadow_alpha(&mut alpha, shadow_w, shadow_h, ui_px_usize(6));

    let base_x = x.saturating_sub(pad);
    let base_y = y.saturating_sub(pad);
    for sy in 0..shadow_h {
        let dy = base_y + sy;
        if dy >= layer.height() {
            continue;
        }
        for sx in 0..shadow_w {
            let dx = base_x + sx;
            let opacity = alpha[sy * shadow_w + sx];
            if dx >= layer.width() || opacity == 0 {
                continue;
            }
            let pos = dy * layer.width() + dx;
            let old = layer.buf_ref()[pos];
            layer.buf_mut()[pos] =
                LayerSystem::blend_alpha(old, Color::BLACK.0, opacity as f32 / 255.0);
        }
    }
}

pub(crate) fn blur_shadow_alpha(alpha: &mut [u8], width: usize, height: usize, radius: usize) {
    if width == 0 || height == 0 || radius == 0 {
        return;
    }
    let mut scratch = alloc::vec![0u8; alpha.len()];
    let diameter = radius * 2 + 1;
    for y in 0..height {
        for x in 0..width {
            let start = x.saturating_sub(radius);
            let end = (x + radius + 1).min(width);
            let mut sum = 0usize;
            for px in start..end {
                sum += alpha[y * width + px] as usize;
            }
            scratch[y * width + x] = (sum / diameter.min(end - start)) as u8;
        }
    }
    for y in 0..height {
        for x in 0..width {
            let start = y.saturating_sub(radius);
            let end = (y + radius + 1).min(height);
            let mut sum = 0usize;
            for py in start..end {
                sum += scratch[py * width + x] as usize;
            }
            alpha[y * width + x] = (sum / diameter.min(end - start)) as u8;
        }
    }
}

pub(crate) fn put_str_size(
    layer: &mut LayerSystem,
    x: i32,
    y: i32,
    text: &str,
    color: Color,
    size: f32,
) {
    if bdf_font::is_available() {
        if x >= 0 && y >= 0 {
            layer.put_str(x as usize, y as usize, text, color);
        }
        let _ = size;
        return;
    }
    #[cfg(feature = "ttf")]
    {
        let mut x = x;
        if !ttf_font::is_available() {
            if x >= 0 && y >= 0 {
                layer.put_str(x as usize, y as usize, text, color);
            }
            return;
        }
        let baseline = y + ttf_font::ascent_at_size(size);
        let layer_w = layer.width();
        let layer_h = layer.height();
        let (clip_x0, clip_y0, clip_x1, clip_y1) = layer.clip_bounds();
        for ch in text.chars() {
            let mut advance = 0;
            ttf_font::with_glyph_at_size(ch, size, |data, w, h, glyph_advance, y_off| {
                advance = glyph_advance;
                for row in 0..h {
                    let py = baseline + y_off + row;
                    if py < clip_y0 as i32 || py >= clip_y1.min(layer_h) as i32 {
                        continue;
                    }
                    for col in 0..w {
                        let px = x + col;
                        if px < clip_x0 as i32 || px >= clip_x1.min(layer_w) as i32 {
                            continue;
                        }
                        let alpha = data[row as usize * w as usize + col as usize] as f32 / 255.0;
                        if alpha > 0.0 {
                            let index = py as usize * layer_w + px as usize;
                            let background = layer.buf_ref()[index];
                            layer.buf_mut()[index] =
                                LayerSystem::blend_alpha(background, color.0, alpha);
                        }
                    }
                }
            });
            x += advance.max(1);
        }
        return;
    }
    #[cfg(not(feature = "ttf"))]
    {
        if x >= 0 && y >= 0 {
            layer.put_str(x as usize, y as usize, text, color);
        }
        let _ = (x, y, size);
    }
}

pub(crate) fn draw_check_icon(layer: &mut LayerSystem, x: i32, y: i32) {
    if is_xiao() {
        // The BDF/Xiao path never rasterizes an SVG mask, so no fractional
        // alpha reaches the framebuffer.
        let size = ui_px_usize(12).max(4) as i32;
        let stroke = ui_px_usize(2).max(1);
        for i in 0..(size / 3).max(1) {
            layer.fill_rect(
                (x + size / 5 + i).max(0) as usize,
                (y + size / 2 + i).max(0) as usize,
                stroke,
                stroke,
                Color::rgb(255, 255, 255),
            );
        }
        for i in 0..(size * 2 / 3).max(1) {
            layer.fill_rect(
                (x + size / 3 + i).max(0) as usize,
                (y + size * 2 / 3 - i).max(0) as usize,
                stroke,
                stroke,
                Color::rgb(255, 255, 255),
            );
        }
        return;
    }
    #[cfg(feature = "ttf")]
    {
        // The normal profile uses the shared SVG asset as a mask.
        let icon_size = ui_px_usize(12).max(1);
        let pixels = svg::rasterize_svg_to_buffer(CHECK_ICON_SVG, icon_size, icon_size);
        let (clip_x0, clip_y0, clip_x1, clip_y1) = layer.clip_bounds();
        let layer_w = layer.width();
        let layer_h = layer.height();
        for sy in 0..icon_size as i32 {
            let py = y + sy;
            if py < clip_y0 as i32 || py >= clip_y1.min(layer_h) as i32 {
                continue;
            }
            for sx in 0..icon_size as i32 {
                let px = x + sx;
                if px < clip_x0 as i32 || px >= clip_x1.min(layer_w) as i32 {
                    continue;
                }
                let alpha = pixels[(sy as usize * icon_size + sx as usize) * 4 + 3];
                if alpha == 0 {
                    continue;
                }
                let index = py as usize * layer_w + px as usize;
                let background = layer.buf_ref()[index];
                layer.buf_mut()[index] = LayerSystem::blend_alpha(
                    background,
                    Color::rgb(255, 255, 255).0,
                    alpha as f32 / 255.0,
                );
            }
        }
    }
}

pub(crate) fn parse_color(s: &str) -> Option<Color> {
    let raw = s.trim();
    if raw.eq_ignore_ascii_case("transparent") {
        return Some(Color::TRANSPARENT);
    }
    if raw.eq_ignore_ascii_case("white") {
        return Some(Color::rgb(255, 255, 255));
    }
    if raw.eq_ignore_ascii_case("black") {
        return Some(Color::rgb(0, 0, 0));
    }
    if raw.eq_ignore_ascii_case("gray") || raw.eq_ignore_ascii_case("grey") {
        return Some(Color::rgb(128, 128, 128));
    }
    if raw.eq_ignore_ascii_case("red") {
        return Some(Color::rgb(255, 0, 0));
    }
    if raw.eq_ignore_ascii_case("green") {
        return Some(Color::rgb(0, 128, 0));
    }
    if raw.eq_ignore_ascii_case("blue") {
        return Some(Color::rgb(0, 0, 255));
    }
    let named = raw;
    let hex = named.strip_prefix('#')?;
    let hex = if hex.len() == 3 {
        let mut expanded = String::new();
        for c in hex.chars() {
            expanded.push(c);
            expanded.push(c);
        }
        expanded
    } else {
        hex.into()
    };
    let v = u32::from_str_radix(&hex, 16).ok()?;
    if hex.len() == 6 {
        Some(Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    } else if hex.len() == 8 {
        Some(Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    } else {
        None
    }
}
