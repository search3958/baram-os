impl Warp3Engine {
    fn paint_cached_layers(&mut self) {
        let width = self.width.max(1) as usize;
        let document_h = self.content_height.max(self.height).max(1) as usize;
        let recreate_document = !matches!(&self.document_layer, Some(layer) if layer.width() == width && layer.height() == document_h);
        if recreate_document {
            self.document_layer = Some(LayerSystem::new_transparent(width, document_h));
            self.repaint_from = 0;
            self.repaint_to = i32::MAX;
        }
        let from = self.repaint_from.max(0) as usize;
        let to = self.repaint_to.min(document_h as i32).max(from as i32) as usize;
        if let Some(mut layer) = self.document_layer.take() {
            layer.fill_rect(0, from, width, to.saturating_sub(from), html_bg());
            layer.push_clip(0, from, width, to);
            for paint_index in 0..self.document_paint.len() {
                let idx = self.document_paint[paint_index];
                let node = &self.nodes[idx];
                if node.y + node.h + 12 <= from as i32 || node.y - 12 >= to as i32 {
                    continue;
                }
                self.draw_node(&mut layer, idx, node.x, node.y);
            }
            self.draw_list_dividers(&mut layer, from as i32, to as i32, 0);
            layer.pop_clip();
            self.document_layer = Some(layer);
        }
        self.repaint_from = self.content_height;
        self.repaint_to = self.content_height;

        let toolbar_h = (self.height + crate::window::title_bar_h() as i32).max(1) as usize;
        let recreate_toolbar = !matches!(&self.toolbar_layer, Some(layer) if layer.width() == width && layer.height() == toolbar_h);
        if recreate_toolbar {
            self.toolbar_layer = Some(LayerSystem::new_transparent(width, toolbar_h));
        }
        if self.toolbar_dirty || recreate_toolbar {
            if let Some(mut layer) = self.toolbar_layer.take() {
                layer.clear(Color::TRANSPARENT);
                for paint_index in 0..self.toolbar_paint.len() {
                    let idx = self.toolbar_paint[paint_index];
                    let node = &self.nodes[idx];
                    if node.is("toolbar") {
                        self.composite_shadow_transparent(&mut layer, idx);
                    }
                }
                self.toolbar_layer = Some(layer);
            }
            self.toolbar_dirty = false;
        }
    }

    fn prepare_shadows(&mut self) {
        let mut wanted: Vec<(usize, usize, usize)> = Vec::new();
        for node in &self.nodes {
            let dimensions = if node.is("list-box") {
                let title_height = if node.prop("text").is_empty() { 0 } else { 36 };
                Some((
                    node.w.max(1) as usize,
                    (node.h - title_height).max(1) as usize,
                    8,
                ))
            } else if node.is("toolbar") || node.is("card") || node.is("tab") {
                Some((node.w.max(1) as usize, node.h.max(1) as usize, 8))
            } else {
                None
            };
            if let Some(key) = dimensions {
                if !wanted.contains(&key) {
                    wanted.push(key);
                }
            }
        }
        self.shadows
            .retain(|mask| wanted.contains(&(mask.w, mask.h, mask.radius)));
        for (w, h, radius) in wanted {
            if !self
                .shadows
                .iter()
                .any(|mask| mask.w == w && mask.h == h && mask.radius == radius)
            {
                self.shadows.push(ShadowMask::new(w, h, radius));
            }
        }
    }

    fn composite_shadow(
        &self,
        layer: &mut LayerSystem,
        x: i32,
        y: i32,
        w: usize,
        h: usize,
        radius: usize,
    ) {
        if let Some(mask) = self
            .shadows
            .iter()
            .find(|mask| mask.w == w && mask.h == h && mask.radius == radius)
        {
            mask.composite(layer, x, y);
        }
    }

    fn composite_shadow_transparent(&self, layer: &mut LayerSystem, idx: usize) {
        let node = &self.nodes[idx];
        if let Some(mask) = self.shadows.iter().find(|mask| {
            mask.w == node.w.max(1) as usize && mask.h == node.h.max(1) as usize && mask.radius == 8
        }) {
            mask.composite_transparent(layer, node.x, node.y);
        }
    }

    fn draw_node(&self, layer: &mut LayerSystem, idx: usize, x: i32, y: i32) {
        let node = &self.nodes[idx];
        let text = node.prop("text");
        let bg = html_bg();
        let panel = html_layer();
        let solid = html_layer_solid();
        let border = html_border();
        let fg = html_text();
        let muted = html_muted();
        let accent = html_accent();
        let xu = x.max(0) as usize;
        let yu = y.max(0) as usize;
        let wu = node.w.max(1) as usize;
        let hu = node.h.max(1) as usize;

        if node.is("list-box") {
            let title_height = if text.is_empty() { 0 } else { 36 };
            let box_y = y + title_height;
            let box_h = (node.h - title_height).max(1) as usize;
            self.composite_shadow(layer, x, box_y, wu, box_h, 8);
            rounded_fill(layer, x, box_y, wu, box_h, 8, panel);
        } else if node.is("toolbar") || node.is("card") || node.is("tab") {
            if !node.is("toolbar") {
                self.composite_shadow(layer, x, y, wu, hu, 8);
            }
            rounded_fill(layer, x, y, wu, hu, 8, panel);
        } else if node.is("button") || node.is("content") {
            let primary = node.prop("type") == "primary";
            let text_only = node.prop("type") == "text";
            let active_tab = node.is("content") && self.is_active_tab(idx);
            let hover = self.hover_amount(idx);
            let color = if primary {
                blend_color(accent, html_accent_hover(), hover)
            } else if hover > 0.0 {
                blend_color(solid, Color::rgb(255, 255, 255), hover)
            } else if primary {
                accent
            } else if active_tab {
                solid
            } else if text_only {
                bg
            } else {
                solid
            };
            let animated_border = if primary {
                blend_color(border, Color::rgb(0, 78, 150), hover)
            } else {
                blend_color(border, Color::rgb(158, 158, 158), hover)
            };
            if !text_only && (!node.is("content") || active_tab || hover > 0.0) {
                rounded_box(layer, x, y, wu, hu, 4, animated_border, color);
            }
        } else if node.is("input") || node.is("textarea") {
            rounded_box(layer, x, y, wu, hu, 4, border, solid);
            let bottom = if self.focused_input == Some(idx) {
                accent
            } else {
                Color::rgb(119, 119, 119)
            };
            layer.fill_rect(
                xu + 4,
                yu + hu.saturating_sub(2),
                wu.saturating_sub(8),
                2,
                bottom,
            );
        } else if node.is("code") {
            layer.fill_rounded_rect(xu, yu, wu, hu, 6, Color::rgb(32, 32, 32));
        } else if node.is("switch") {
            let on = node.prop("default") == "true";
            let progress = self.switch_amount(idx, on);
            let switch_bg = blend_color(bg, accent, progress);
            rounded_box(
                layer,
                x,
                y,
                wu,
                hu,
                10,
                blend_color(Color::rgb(119, 119, 119), accent, progress),
                switch_bg,
            );
            let knob_x = x + 3 + ((node.w - 21).max(0) as f32 * progress) as i32;
            layer.fill_circle(
                knob_x.max(0) as usize + 7,
                yu + 10,
                7,
                blend_color(
                    Color::rgb(102, 102, 102),
                    Color::rgb(255, 255, 255),
                    progress,
                ),
            );
        }

        if text.is_empty() {
            return;
        }
        let (tx, ty, color) = if node.is("button") || node.is("content") {
            let color = if node.prop("type") == "primary" {
                Color::rgb(255, 255, 255)
            } else if node.is("content") && self.is_active_tab(idx) {
                accent
            } else {
                fg
            };
            let tx = if node.prop("key-center") == "true" {
                x + ((node.w - measure(text)).max(0) / 2)
            } else {
                x + 14
            };
            (
                tx,
                y + if node.prop("key-center") == "true" {
                    6
                } else {
                    8
                },
                color,
            )
        } else if node.is("input") || node.is("textarea") {
            (x + 10, y + 8, fg)
        } else if node.is("card") {
            (x + 16, y + 14, fg)
        } else if node.is("list-box") {
            (x, y + 8, fg)
        } else if node.is("list") {
            (x + 8, y + 13, fg)
        } else if node.is("code") {
            (x + 12, y + 11, Color::rgb(246, 246, 246))
        } else {
            (
                x,
                y + if node.is("head") { 8 } else { 2 },
                if node.is("detail") { muted } else { fg },
            )
        };
        for (line, value) in node.text_lines.iter().enumerate() {
            let line_y = ty + line as i32 * 22;
            if line_y >= 0 && line_y < layer.height() as i32 {
                if node.is("head") {
                    put_str_size(layer, tx, line_y - 5, value, color, 28.0);
                } else if node.is("list-box") {
                    put_str_size(layer, tx, line_y - 3, value, color, 20.0);
                } else {
                    layer.put_str(tx.max(0) as usize, line_y as usize, value, color);
                }
            }
        }
    }

    fn is_active_tab(&self, content_idx: usize) -> bool {
        self.nodes
            .iter()
            .any(|node| node.is("tab") && node.children.get(node.tab).copied() == Some(content_idx))
    }

}

