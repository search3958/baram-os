use super::*;

fn blend_fractional_pixel_up(
    dst: &mut LayerSystem,
    src: &LayerSystem,
    top: usize,
    phase: u8,
) {
    let width = dst.width().min(src.width());
    let height = dst.height().min(src.height());
    let top = top.min(height);
    let dst_buf = dst.buf_mut();
    let src_buf = src.buf_ref();
    for y in top..height {
        let next_y = (y + 1).min(height - 1);
        let row = y * width;
        let next_row = next_y * width;
        for x in 0..width {
            let a = Color(src_buf[row + x]);
            let b = Color(src_buf[next_row + x]);
            let phase = phase as u16;
            let remain = 3 - phase;
            dst_buf[row + x] = Color::rgb(
                ((a.r() as u16 * remain + b.r() as u16 * phase + 1) / 3) as u8,
                ((a.g() as u16 * remain + b.g() as u16 * phase + 1) / 3) as u8,
                ((a.b() as u16 * remain + b.b() as u16 * phase + 1) / 3) as u8,
            )
            .0;
        }
    }
}

impl Warp4Engine {
    pub fn draw_to_layer(&mut self, layer: &mut LayerSystem, ox: i32, oy: i32) {
        if self.dirty {
            self.update(
                layer.width() as i32,
                layer.height() as i32 - self.chrome_height,
            );
        }
        layer.fill_rect(
            0,
            self.chrome_height as usize,
            layer.width(),
            layer.height().saturating_sub(self.chrome_height as usize),
            bg(),
        );
        let chrome_mode = self.has_explicit_scroll();
        // The normal window server already applies its window scroll in `oy`
        // and also mirrors that value into this engine for hit testing. Do
        // not apply the same offset a second time. Xiao calls this with oy=0
        // and therefore keeps the engine-owned document scroll here.
        let flow_oy = if oy != 0 { oy } else { -self.scroll };
        let fractional_phase = if is_xiao() && oy == 0 {
            self.scroll_subpixel.rem_euclid(3) as u8
        } else {
            0
        };
        if fractional_phase != 0 {
            // Render the moving document into a small transparent scratch
            // layer, then translate it by exactly one- or two-thirds of a
            // pixel before the fixed/sticky pass. This keeps tiny motion from
            // becoming a sequence of visibly bad whole-pixel jumps.
            let mut document = LayerSystem::new_transparent(layer.width(), layer.height());
            document.fill_rect(
                0,
                self.chrome_height.max(0) as usize,
                document.width(),
                document.height().saturating_sub(self.chrome_height.max(0) as usize),
                bg(),
            );
            for &root in &self.roots {
                self.paint(
                    &mut document,
                    root,
                    ox,
                    flow_oy,
                    false,
                    chrome_mode,
                    PaintPass::Flow,
                );
            }
            blend_fractional_pixel_up(
                layer,
                &document,
                self.chrome_height.max(0) as usize,
                fractional_phase,
            );
            for &root in &self.roots {
                if self.fixed_subtree.get(root).copied().unwrap_or(true) {
                    self.paint(layer, root, ox, oy, false, chrome_mode, PaintPass::Fixed);
                }
            }
        } else {
            for &root in &self.roots {
                // The compositor supplies the window-manager offset.  The
                // view tree gets two passes: normal content first, then
                // fixed/sticky chrome.
                self.paint(
                    layer,
                    root,
                    ox,
                    flow_oy,
                    false,
                    chrome_mode,
                    PaintPass::Flow,
                );
                if self.fixed_subtree.get(root).copied().unwrap_or(true) {
                    self.paint(layer, root, ox, oy, false, chrome_mode, PaintPass::Fixed);
                }
            }
        }
        if let Some(idx) = self.spinner_open {
            self.paint_spinner_popup(layer, idx, 255);
        } else if let Some(fade) = self.spinner_fade {
            let elapsed = self.now_ns.saturating_sub(fade.started_ns);
            let remaining = 140_000_000u64.saturating_sub(elapsed);
            let opacity = (remaining.saturating_mul(255) / 140_000_000) as u8;
            self.paint_spinner_popup(layer, fade.idx, opacity);
        }
        self.paint_transition(layer);
    }

    pub(crate) fn paint_transition(&self, layer: &mut LayerSystem) {
        let Some(elapsed) = self.transition_elapsed_ns else {
            return;
        };
        let w = layer.width();
        let h = layer.height();
        if w == 0 || h == 0 {
            return;
        }
        // A compact zoom-reveal: the new page is visible through a growing
        // rectangle, while the surrounding classic-Mac gray remains opaque.
        // It is deliberately hard-edged in Xiao, avoiding an extra alpha
        // surface and keeping the effect usable at 128x64.
        let reveal_w = (w as u64 * elapsed / XIAO_TRANSITION_NS) as usize;
        let reveal_h = (h as u64 * elapsed / XIAO_TRANSITION_NS) as usize;
        let left = (w.saturating_sub(reveal_w)) / 2;
        let top = (h.saturating_sub(reveal_h)) / 2;
        let right = left.saturating_add(reveal_w).min(w);
        let bottom = top.saturating_add(reveal_h).min(h);
        let cover = Color::rgb(0xCC, 0xCC, 0xCC);
        if top > 0 {
            layer.fill_rect(0, 0, w, top, cover);
        }
        if bottom < h {
            layer.fill_rect(0, bottom, w, h - bottom, cover);
        }
        if left > 0 && top < bottom {
            layer.fill_rect(0, top, left, bottom - top, cover);
        }
        if right < w && top < bottom {
            layer.fill_rect(right, top, w - right, bottom - top, cover);
        }
        if reveal_w > 1 && reveal_h > 1 {
            layer.rect_outline(left, top, reveal_w, reveal_h, Color::BLACK);
        }
    }

    pub(crate) fn paint(
        &self,
        layer: &mut LayerSystem,
        idx: usize,
        ox: i32,
        oy: i32,
        in_scroll: bool,
        chrome_mode: bool,
        pass: PaintPass,
    ) {
        if pass == PaintPass::Fixed && !self.fixed_subtree.get(idx).copied().unwrap_or(true) {
            return;
        }
        let n = &self.nodes[idx];
        if !n.visible() || !self.active_child(idx) {
            return;
        }
        let fixed = self.node_is_fixed(idx, chrome_mode);
        if pass == PaintPass::Flow && fixed {
            let clipped_scroll = is_scroll_container(n);
            if clipped_scroll {
                // Paint the viewport background before its scrolling
                // descendants. The fixed pass runs after the flow pass, so
                // doing this later would cover a white ScrollView's content.
                let x = n.x + ox;
                let y = n.y;
                if let Some(fill) = parse_color(n.attr("background")) {
                    let radius = parse_dim(n.attr("cornerRadius"), 0).max(0) as usize;
                    if radius > 0 {
                        fill_ui_rounded_rect_at(
                            layer,
                            x,
                            y.max(self.chrome_height),
                            n.w.max(1) as usize,
                            n.h.max(1) as usize,
                            radius
                                .min(n.w.max(1) as usize / 2)
                                .min(n.h.max(1) as usize / 2),
                            fill,
                        );
                    } else {
                        layer.fill_rect_signed(
                            x,
                            y.max(self.chrome_height),
                            n.w.max(1) as usize,
                            n.h.max(1) as usize,
                            fill,
                        );
                    }
                }
                let clip_x0 = n.x + ox;
                let clip_y0 = n.y;
                layer.push_clip(
                    clip_x0.max(0) as usize,
                    clip_y0.max(self.chrome_height).max(0) as usize,
                    (clip_x0 + n.w).max(0) as usize,
                    (clip_y0 + n.h).max(0) as usize,
                );
            }
            let child_scroll = in_scroll || is_scroll_container(n);
            let child_oy = if is_sticky(n) {
                self.node_screen_y(idx).saturating_sub(n.y)
            } else {
                oy
            };
            for &child in &n.children {
                self.paint(layer, child, ox, child_oy, child_scroll, chrome_mode, pass);
            }
            if clipped_scroll {
                layer.pop_clip();
            }
            return;
        }
        if pass == PaintPass::Fixed && !fixed {
            let child_scroll = in_scroll || is_scroll_container(n);
            let child_oy = if is_sticky(n) {
                self.node_screen_y(idx).saturating_sub(n.y)
            } else {
                oy
            };
            for &child in &n.children {
                self.paint(layer, child, ox, child_oy, child_scroll, chrome_mode, pass);
            }
            return;
        }
        let x = n.x + ox;
        let y = if is_sticky(n) {
            self.node_screen_y(idx)
        } else {
            n.y + if fixed { 0 } else { oy }
        };
        let w = n.w.max(1) as usize;
        let h = n.h.max(1) as usize;
        if !(pass == PaintPass::Fixed && fixed && is_scroll_container(n)) {
            if let Some(fill) = parse_color(n.attr("background")) {
                let radius = parse_dim(n.attr("cornerRadius"), 0).max(0) as usize;
                if radius > 0 {
                    fill_ui_rounded_rect_at(
                        layer,
                        x,
                        y,
                        w,
                        h,
                        radius.min(w / 2).min(h / 2),
                        fill,
                    );
                } else {
                    layer.fill_rect_signed(x, y, w, h, fill);
                }
            }
        }
        if is_button_like(n) {
            let active = self.pressed == Some(idx);
            let hover = self.hovered == Some(idx);
            let selected = n.attr("selected") == "true";
            let primary = n.is("PrimaryButton");
            let button_h = h;
            let button_radius = if is_xiao() {
                palette().button_radius
            } else {
                w.min(h) / 2
            };
            draw_ui_button(
                layer,
                x,
                y,
                w,
                button_h,
                button_radius,
                ui_button_face(active, selected, hover, primary),
                active,
                primary,
            );
            if self.keyboard_focus == Some(idx) && w > 6 && h > 6 {
                // Keep the focus indication inside the button's existing
                // one-pixel frame so keyboard navigation is visible without
                // changing XML geometry.
                layer.rect_outline(
                    x.saturating_add(3).max(0) as usize,
                    y.saturating_add(3).max(0) as usize,
                    w.saturating_sub(6),
                    h.saturating_sub(6),
                    Color::BLACK,
                );
            }
        } else if n.is("EditText")
            || n.is("AutoCompleteTextView")
            || n.is("MultiAutoCompleteTextView")
        {
            if is_xiao() {
                draw_ui_input(layer, x, y, w, h);
            } else {
                outline_ui_rounded_rect_at(
                    layer,
                    x,
                    y,
                    w,
                    h,
                    ui_px_usize(WARP4_INPUT_RADIUS).min(w / 2).min(h / 2),
                    palette().warp4_input_border,
                    palette().warp4_input_bg,
                );
            }
        } else if n.is("CheckBox") || n.is("RadioButton") {
            let checked = n.attr("checked") == "true";
            let hover = self.hovered == Some(idx);
            let mark_x = x + ui_px(2);
            let mark_y = y + (n.h - ui_px(if n.is("RadioButton") { 18 } else { 22 })).max(0) / 2;
            if n.is("CheckBox") {
                if is_xiao() {
                    draw_ui_checkbox(
                        layer,
                        mark_x,
                        mark_y,
                        ui_px_usize(22),
                        checked,
                        hover,
                    );
                } else {
                    let border = if checked || hover {
                        palette().warp3_accent
                    } else {
                        palette().warp3_muted
                    };
                    outline_ui_rounded_rect_at(
                        layer,
                        mark_x,
                        mark_y,
                        ui_px_usize(22),
                        ui_px_usize(22),
                        ui_px_usize(4),
                        border,
                        if checked {
                            palette().warp3_accent
                        } else {
                            palette().warp3_surface
                        },
                    );
                    if checked {
                        draw_check_icon(layer, mark_x + ui_px(5), mark_y + ui_px(5));
                    }
                }
            } else {
                let amount = self.control_amount(idx, checked);
                let outer = if self
                    .control_animations
                    .iter()
                    .any(|animation| animation.idx == idx)
                {
                    mix_color(palette().warp4_radio_off, palette().warp4_primary, amount)
                } else if checked {
                    palette().warp4_primary
                } else {
                    palette().warp4_radio_off
                };
                fill_ui_circle_at(
                    layer,
                    mark_x + ui_px(9),
                    mark_y + ui_px(9),
                    ui_px_usize(9),
                    outer,
                );
                let inner_radius = (ui_size(4.0) * amount + ui_size(0.5)) as usize;
                if inner_radius > 0 {
                    fill_ui_circle_at(
                        layer,
                        mark_x + ui_px(9),
                        mark_y + ui_px(9),
                        inner_radius,
                        WARP4_WHITE,
                    );
                }
            }
        } else if n.is("Switch") {
            if is_xiao() {
                draw_ui_switch(layer, x, y, n.h, n.attr("checked") == "true");
            } else {
                let on = n.attr("checked") == "true";
                let amount = self.control_amount(idx, on);
                let track = mix_color(palette().warp3_bg, palette().warp3_accent, amount);
                let sy = y + (n.h - ui_px(22)).max(0) / 2;
                let track_w = ui_px_usize(44);
                outline_ui_rounded_rect_at(
                    layer,
                    x,
                    sy,
                    track_w,
                    ui_px_usize(22),
                    ui_px_usize(11),
                    mix_color(palette().warp3_muted, palette().warp3_accent, amount),
                    track,
                );
                // Warp3 uses a compact 14px knob inside the 22px track.
                let knob_x = x
                    + ui_px(10)
                    + ((track_w as f32 - ui_size(20.0)) * amount + ui_size(0.5)) as i32;
                fill_ui_circle_at(
                    layer,
                    knob_x,
                    sy + ui_px(11),
                    ui_px_usize(7),
                    mix_color(Color::rgb(102, 102, 102), Color::rgb(255, 255, 255), amount),
                );
            }
        } else if n.is("Spinner") || n.is("SearchView") {
            layer.fill_rect_signed(
                x,
                y + h as i32 - ui_px(2),
                w,
                ui_px_usize(2).max(1),
                if self.hovered == Some(idx) {
                    palette().warp3_accent
                } else {
                    palette().warp3_border
                },
            );
            if n.is("Spinner") {
                let selected = parse_i32(n.attr("selectedIndex")).max(0) as usize;
                let value = if !n.attr("text").is_empty() {
                    n.attr("text")
                } else if !n.attr("value").is_empty() {
                    n.attr("value")
                } else if let Some(item) = n
                    .attr("items")
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .nth(selected)
                {
                    item
                } else {
                    match selected % 3 {
                        1 => "Item 2",
                        2 => "Item 3",
                        _ => "Item 1",
                    }
                };
                put_str_size(
                    layer,
                    x + ui_px(7),
                    y + ((h as i32 - ui_px(19)).max(0) / 2),
                    value,
                    palette().warp3_text,
                    ui_size(15.0),
                );
                // Native equivalent of the CSS select arrow.
                let ax = x + n.w - ui_px(14);
                let ay = y + h as i32 / 2 - ui_px(2);
                for row in 0..ui_px_usize(5) {
                    let width = ui_px_usize(2) + row * ui_px_usize(2);
                    let row = row as i32;
                    layer.fill_rect_signed(
                        ax - row,
                        ay + row,
                        width as usize,
                        1,
                        palette().warp3_muted,
                    );
                }
            }
        } else if n.is("SeekBar") {
            let cy = y + h as i32 / 2;
            layer.fill_rect_signed(
                x,
                cy,
                w,
                ui_px_usize(3).max(1),
                if self.hovered == Some(idx) {
                    palette().warp3_accent
                } else {
                    palette().warp3_border
                },
            );
            let max = parse_i32(n.attr("max")).max(1);
            let progress = parse_i32(n.attr("progress")).clamp(0, max);
            let px = x + w as i32 * progress / max;
            if progress > 0 {
                layer.fill_rect_signed(
                    x,
                    cy,
                    (px - x).max(0) as usize,
                    ui_px_usize(3).max(1),
                    palette().warp3_accent,
                );
            }
            fill_ui_circle_at(
                layer,
                px,
                cy,
                ui_px_usize(if self.hovered == Some(idx) { 10 } else { 9 }).max(1),
                palette().warp3_accent,
            );
        } else if n.is("RatingBar") {
            let stars = parse_i32(n.attr("numStars")).max(1).min(10);
            let rating = n.attr("rating").parse::<f32>().unwrap_or(0.0);
            let hover = self.hovered == Some(idx);
            for star in 0..stars {
                let color = if star as f32 + 0.5 <= rating {
                    palette().warp3_accent
                } else if hover {
                    Color::rgb(158, 190, 220)
                } else {
                    Color::rgb(183, 183, 183)
                };
                put_str_size(layer, x + star * ui_px(23), y, "★", color, ui_size(25.0));
            }
        } else if n.is("ProgressBar") {
            if n.attr("style").contains("progressBarStyleHorizontal") || w > ui_px_usize(80) {
                fill_ui_rounded_rect_at(
                    layer,
                    x,
                    y + ui_px(2),
                    w,
                    ui_px_usize(6).max(1),
                    ui_px_usize(3),
                    palette().warp3_border,
                );
                let max = parse_i32(n.attr("max")).max(1);
                let progress = parse_i32(n.attr("progress")).clamp(0, max);
                fill_ui_rounded_rect_at(
                    layer,
                    x,
                    y + ui_px(2),
                    (w as i32 * progress / max) as usize,
                    ui_px_usize(6).max(1),
                    ui_px_usize(3),
                    palette().warp3_accent,
                );
            } else {
                let cx = x + w as i32 / 2;
                let cy = y + h as i32 / 2;
                let outer_radius = ui_px(14);
                let inner_radius = ui_px(9);
                fill_ui_circle_at(
                    layer,
                    cx,
                    cy,
                    outer_radius.max(1) as usize,
                    Color::rgb(207, 207, 207),
                );
                fill_ui_circle_at(
                    layer,
                    cx,
                    cy,
                    inner_radius.max(1) as usize,
                    Color::rgb(255, 255, 255),
                );
                for dy in -outer_radius..=outer_radius {
                    for dx in -outer_radius..=outer_radius {
                        let radius = dx * dx + dy * dy;
                        if radius <= outer_radius * outer_radius
                            && radius >= inner_radius * inner_radius
                            && (dx >= 0 || dy <= 0)
                        {
                            let px = cx + dx;
                            let py = cy + dy;
                            layer.fill_rect_signed(px, py, 1, 1, palette().warp3_accent);
                        }
                    }
                }
            }
        } else if n.is("ImageView") {
            if x < 0 || y < 0 {
                fill_ui_rounded_rect_at(layer, x, y, w, h, 0, palette().warp3_border);
            } else {
                layer.rect_outline(
                    x as usize,
                    y as usize,
                w,
                h,
                palette().warp3_border,
                );
            }
        } else if n.is("ListView") || n.is("ExpandableListView") {
            let row_h = ui_px_usize(44).max(1);
            for row in 0..(h / row_h) {
                let ry = y + row as i32 * row_h as i32;
                layer.fill_rect_signed(
                    x,
                    ry,
                    w,
                    row_h.saturating_sub(1).max(1),
                    palette().warp3_surface,
                );
                layer.fill_rect_signed(
                    x,
                    ry + row_h.saturating_sub(1) as i32,
                    w,
                    1,
                    palette().warp3_border,
                );
                put_str_size(
                    layer,
                    x + ui_px(12),
                    ry + ui_px(12),
                    &format!("Item {}", row + 1),
                    palette().warp3_text,
                    ui_size(14.0),
                );
            }
        }
        let text = if !n.attr("text").is_empty() {
            n.attr("text")
        } else if n.is("ToggleButton") {
            if n.attr("checked") == "true" {
                n.attr("textOn")
            } else {
                n.attr("textOff")
            }
        } else {
            ""
        };
        if !text.is_empty() {
            let color = text_color(n);
            let size = text_size(n);
            let pad = edges(n, "padding");
            let text_w = measure_size(text, size);
            let tx = if is_text_button(n) {
                x + (n.w - text_w).max(0) / 2
            } else if n.is("EditText")
                || n.is("AutoCompleteTextView")
                || n.is("MultiAutoCompleteTextView")
            {
                // Warp3 inputs use a fixed 10px text inset; using the XML
                // padding here made the value appear vertically/horizontally
                // displaced between focused and unfocused states.
                x + ui_px(10)
            } else if ascii_contains_ignore_case(n.attr("gravity"), "right")
                || ascii_contains_ignore_case(n.attr("gravity"), "end")
            {
                x + n.w - text_w - pad.right
            } else if ascii_contains_ignore_case(n.attr("gravity"), "center") {
                x + (n.w - text_w) / 2
            } else {
                x + pad.left
                    + if n.is("CheckBox") {
                        ui_px(32)
                    } else if n.is("RadioButton") {
                        ui_px(28)
                    } else if n.is("Switch") {
                        ui_px(55)
                    } else {
                        0
                    }
            };
            let line_h = if is_xiao() && bdf_font::is_available() {
                // Misaki Gothic is an 8px bitmap font. Use its real line box
                // for centering instead of the 25%-scaled nominal text size.
                8
            } else {
                (size * 1.25) as i32
            };
            let line_count = text.split('\n').count().max(1) as i32;
            let block_h = line_h * line_count;
            let gravity = n.attr("gravity");
            let ty = if is_text_button(n) {
                // Text is centered in the actual XML-resolved button bounds;
                // the compact padding rule must not shift it vertically.
                y + (n.h - block_h).max(0) / 2
            } else if n.is("EditText")
                || n.is("AutoCompleteTextView")
                || n.is("MultiAutoCompleteTextView")
            {
                y + ui_px(8)
            } else if ascii_contains_ignore_case(gravity, "bottom") {
                y + n.h - pad.bottom - block_h
            } else if ascii_contains_ignore_case(gravity, "center_vertical")
                || gravity.eq_ignore_ascii_case("center")
                || ascii_contains_ignore_case(gravity, "center|vertical")
            {
                y + (n.h - block_h).max(0) / 2
            } else {
                y + pad.top
            };
            for (line, part) in text.split('\n').enumerate() {
                let line_y = ty + line as i32 * line_h;
                put_str_size(layer, tx, line_y, part, color, size);
                if text_bold(n) {
                    put_str_size(layer, tx + 1, line_y, part, color, size);
                }
            }
        } else if (n.is("EditText")
            || n.is("AutoCompleteTextView")
            || n.is("MultiAutoCompleteTextView"))
            && !n.attr("hint").is_empty()
        {
            put_str_size(
                layer,
                x + ui_px(10),
                y + ui_px(8),
                n.attr("hint"),
                palette().warp3_muted,
                text_size(n),
            );
        }
        if (n.is("EditText") || n.is("AutoCompleteTextView") || n.is("MultiAutoCompleteTextView"))
            && self.focused == Some(idx)
            && (self.now_ns / 500_000_000) % 2 == 0
        {
            let size = text_size(n);
            let caret_x = x + ui_px(10) + measure_size(text, size);
            let caret_y = y + ui_px(7);
            let caret_h = (size * 1.25).max(1.0) as usize;
            if caret_x >= 0 && caret_y >= 0 {
                layer.fill_rect(caret_x as usize, caret_y as usize, 1, caret_h, WARP4_BLACK);
            }
        }
        let child_scroll = in_scroll || is_scroll_container(n);
        if is_scroll_container(n) {
            layer.push_clip(
                x.max(0) as usize,
                y.max(self.chrome_height).max(0) as usize,
                (x + n.w).max(0) as usize,
                (y + n.h).max(0) as usize,
            );
        }
        let child_oy = if is_sticky(n) {
            self.node_screen_y(idx).saturating_sub(n.y)
        } else {
            oy
        };
        for &child in &n.children {
            self.paint(layer, child, ox, child_oy, child_scroll, chrome_mode, pass);
        }
        if is_scroll_container(n) {
            if n.is("ScrollView") && n.content_h > n.h {
                let track_h = (n.h - ui_px(2)).max(1);
                let thumb_h = (track_h * n.h / n.content_h).max(ui_px(12)).min(track_h);
                let max_thumb_y = track_h - thumb_h;
                let max_scroll = n.content_h.saturating_sub(n.h).max(1);
                let thumb_y = max_thumb_y * self.scroll / max_scroll;
                let bar_x = (x + n.w - ui_px(8)).max(0) as usize;
                fill_ui_rounded_rect(
                    layer,
                    bar_x,
                    (y + ui_px(1)).max(0) as usize,
                    ui_px_usize(6).max(1),
                    track_h as usize,
                    ui_px_usize(SCROLLBAR_RADIUS),
                    palette().scrollbar_track,
                );
                fill_ui_rounded_rect(
                    layer,
                    bar_x,
                    (y + ui_px(1) + thumb_y).max(0) as usize,
                    ui_px_usize(6).max(1),
                    thumb_h as usize,
                    ui_px_usize(SCROLLBAR_RADIUS),
                    palette().scrollbar_thumb,
                );
            } else if n.is("HorizontalScrollView") && n.content_w > n.w {
                let track_w = (n.w - ui_px(2)).max(1);
                let thumb_w = (track_w * n.w / n.content_w).max(ui_px(12)).min(track_w);
                let max_thumb_x = track_w - thumb_w;
                let max_scroll = n.content_w.saturating_sub(n.w).max(1);
                let thumb_x = max_thumb_x * self.scroll / max_scroll;
                fill_ui_rounded_rect(
                    layer,
                    (x + ui_px(1)).max(0) as usize,
                    (y + n.h - ui_px(6)).max(0) as usize,
                    track_w as usize,
                    ui_px_usize(6).max(1),
                    ui_px_usize(SCROLLBAR_RADIUS),
                    palette().scrollbar_track,
                );
                fill_ui_rounded_rect(
                    layer,
                    (x + ui_px(1) + thumb_x).max(0) as usize,
                    (y + n.h - ui_px(6)).max(0) as usize,
                    thumb_w as usize,
                    ui_px_usize(6).max(1),
                    ui_px_usize(SCROLLBAR_RADIUS),
                    palette().scrollbar_thumb,
                );
            }
            layer.pop_clip();
        }
    }

    pub(crate) fn paint_spinner_popup(&self, layer: &mut LayerSystem, idx: usize, opacity: u8) {
        let Some(node) = self.nodes.get(idx) else {
            return;
        };
        if !node.visible() || !node.is("Spinner") {
            return;
        }
        let (x, y, w, h) = self.spinner_popup_rect(idx);
        let popup_w = w.max(1) as usize;
        let popup_h = h.max(1) as usize;
        if is_xiao() {
            let _ = opacity;
            draw_spinner_shadow(
                layer,
                x.max(0) as usize,
                y.max(self.chrome_height) as usize,
                popup_w,
                popup_h,
                ui_px_usize(4),
            );
            self.paint_spinner_popup_content(
                layer,
                x.max(0) as usize,
                y.max(self.chrome_height) as usize,
                popup_w,
                popup_h,
                idx,
            );
            return;
        }
        let radius = ui_px_usize(8);
        let shadow_pad = ui_px_usize(16);
        if opacity < 255 {
            // Seed the temporary layer with the pixels underneath the menu so
            // fading does not turn its transparent margins into black.
            let popup_x = x.max(0) as usize;
            let popup_y = y.max(self.chrome_height) as usize;
            let sx = popup_x.saturating_sub(shadow_pad);
            let sy = popup_y.saturating_sub(shadow_pad);
            let ex = (sx + popup_w + shadow_pad * 2).min(layer.width());
            let ey = (sy + popup_h + shadow_pad * 2).min(layer.height());
            let copy_w = ex.saturating_sub(sx).max(1);
            let copy_h = ey.saturating_sub(sy).max(1);
            let mut popup = LayerSystem::new(copy_w, copy_h);
            for row in 0..copy_h {
                let src = (sy + row) * layer.width() + sx;
                let dst = row * copy_w;
                popup.buf_mut()[dst..dst + copy_w]
                    .copy_from_slice(&layer.buf_ref()[src..src + copy_w]);
            }
            let local_x = popup_x.saturating_sub(sx).min(copy_w.saturating_sub(1));
            let local_y = popup_y.saturating_sub(sy).min(copy_h.saturating_sub(1));
            draw_spinner_shadow(&mut popup, local_x, local_y, popup_w, popup_h, radius);
            self.paint_spinner_popup_content(&mut popup, local_x, local_y, popup_w, popup_h, idx);
            layer.composit_rect_global_alpha(
                popup.buf_ref(),
                popup.width(),
                popup.height(),
                sx,
                sy,
                opacity,
            );
            return;
        }
        let x = x.max(0) as usize;
        let y = y.max(self.chrome_height) as usize;
        draw_spinner_shadow(layer, x, y, popup_w, popup_h, radius);
        self.paint_spinner_popup_content(layer, x, y, popup_w, popup_h, idx);
    }

    pub(crate) fn paint_spinner_popup_content(
        &self,
        layer: &mut LayerSystem,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        idx: usize,
    ) {
        let Some(node) = self.nodes.get(idx) else {
            return;
        };
        fill_ui_rounded_rect(layer, x, y, w, h, ui_px_usize(8), Color::rgb(255, 255, 255));
        let selected = parse_i32(node.attr("selectedIndex")).max(0) as usize;
        let items = node.attr("items");
        for item in 0..self.spinner_item_count(idx) {
            let row_h = ui_px_usize(36).max(1);
            let row_y = y + ui_px_usize(4) + item * row_h;
            let highlighted = item == selected;
            if highlighted {
                fill_ui_rounded_rect(
                    layer,
                    x + ui_px_usize(4),
                    row_y,
                    w.saturating_sub(ui_px_usize(8)),
                    row_h.min(h.saturating_sub(ui_px_usize(4) + item * row_h)),
                    ui_px_usize(6),
                    palette().warp3_accent,
                );
            }
            let label = items
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .nth(item)
                .unwrap_or(match item {
                    1 => "Item 2",
                    2 => "Item 3",
                    _ => "Item 1",
                });
            put_str_size(
                layer,
                x as i32 + ui_px(16),
                row_y as i32 + ui_px(9),
                label,
                if highlighted {
                    Color::rgb(255, 255, 255)
                } else {
                    palette().warp3_text
                },
                ui_size(15.0),
            );
        }
    }

    pub(crate) fn has_explicit_scroll(&self) -> bool {
        self.roots
            .iter()
            .any(|root| contains_scroll(&self.nodes, *root))
    }

    pub(crate) fn active_child(&self, idx: usize) -> bool {
        let Some(parent) = self.nodes[idx].parent else {
            return true;
        };
        let p = &self.nodes[parent];
        if !(p.is("ViewFlipper")
            || p.is("ViewAnimator")
            || p.is("ViewSwitcher")
            || p.is("TextSwitcher"))
        {
            return true;
        }
        let active = parse_i32(p.attr("displayedChild")).max(0) as usize;
        p.children.iter().position(|child| *child == idx) == Some(active)
    }
}
