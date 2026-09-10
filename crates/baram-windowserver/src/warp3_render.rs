impl Warp3Engine {
    pub fn draw_to_layer(&mut self, layer: &mut LayerSystem, ox: i32, _oy: i32) {
        // The window may be drawn before its next regular update tick.  Apply
        // hover-only damage here as well, without paying for a layout pass.
        if !self.dirty && (self.repaint_from < self.content_height || self.toolbar_dirty) {
            self.paint_cached_layers();
        }
        layer.fill_rect(0, 0, layer.width(), layer.height(), html_bg());
        let target_y = self.screen_transition_offset_y.max(0.0) as usize;
        let subpixel_y = self.screen_transition_offset_y - target_y as f32;
        if let Some(document) = &self.document_layer {
            let source_y = self.scroll.max(0) as usize;
            let visible_h = layer.height().saturating_sub(target_y);
            if source_y < document.height() && visible_h > 0 {
                let draw_h = visible_h.min(document.height() - source_y);
                if subpixel_y > 0.0 {
                    layer.composit_rect_opaque_subpixel_y(
                        document,
                        ox.max(0) as usize,
                        target_y,
                        0,
                        source_y,
                        document.width(),
                        draw_h,
                        subpixel_y,
                    );
                } else {
                    layer.composit_rect_opaque(
                        document,
                        ox.max(0) as usize,
                        target_y,
                        0,
                        source_y,
                        document.width(),
                        draw_h,
                    );
                }
            }
        }
        // This is a separate transparent layer, composited after the scrolling
        // document.  It keeps both its pixels and its hit targets fixed.
        if let Some(toolbar) = &self.toolbar_layer {
            layer.composit_rect_alpha(
                toolbar,
                ox.max(0) as usize,
                target_y,
                0,
                0,
                toolbar.width(),
                toolbar.height(),
            );
        }
        // Paint fixed controls only for a full frame or a toolbar-local hover.
        // Repainting TTF text outside the local damage would blend its edge
        // pixels again even though the document did not change.
        let paint_toolbar = self.window_damage.map_or(true, |(_, y0, _, y1)| {
            let toolbar_top = crate::window::title_bar_h() as i32 + self.height - 9 - 54 - 14;
            y1 >= toolbar_top && y0 < self.height + crate::window::title_bar_h() as i32
        });
        // Paint the fixed controls over the already-composited document. This
        // gives text and rounded corners their final-background antialiasing;
        // only the shadow itself needs transparent alpha storage.
        if paint_toolbar {
            for paint_index in 0..self.toolbar_paint.len() {
                let idx = self.toolbar_paint[paint_index];
                let node = &self.nodes[idx];
                self.draw_node(layer, idx, node.x + ox, node.y + target_y as i32);
            }
        }
        if let Some(idx) = self.focused_input {
            if self.caret_visible && self.nodes.get(idx).is_some_and(|node| !node.hidden) {
                let node = &self.nodes[idx];
                let text = node.prop("text");
                let caret_x = node.x + ox + 10 + measure(text);
                let caret_y = if self.is_toolbar_tree(idx) {
                    node.y + target_y as i32 + 7
                } else {
                    node.y - self.scroll + target_y as i32 + 7
                };
                text_cursor::draw(layer, caret_x, caret_y, 20, html_text());
            }
        }
        self.window_damage = None;
        // This draw is reached only after WindowManager selected the full
        // content path while `full_window_redraw` was set.
        self.full_window_redraw = false;
    }

    fn load_screen(&mut self) {
        let ui_path = alloc::format!("{}.w3u", self.screen);
        let source = self.archive.read_text(&ui_path);
        self.nodes = Parser::new(&source).parse();
        self.roots = root_indices(&self.nodes);
        let toolbar_roots: Vec<usize> = self
            .roots
            .iter()
            .copied()
            .filter(|idx| self.nodes[*idx].is("toolbar"))
            .collect();
        for root in toolbar_roots {
            mark_overlay_tree(&mut self.nodes, root);
        }
        self.scripts.clear();
        self.script_frames.clear();
        self.script_wait_until_ns = None;
        self.command_queue.clear();
        let mut script_names = Vec::new();
        for node in &self.nodes {
            if node.is("config") {
                for (key, value) in &node.props {
                    if key == "title" && !value.is_empty() {
                        self.app_title = value.clone();
                    } else if key == "script" {
                        script_names.push(value.clone());
                    }
                }
            }
        }
        for name in script_names {
            self.scripts
                .extend(parse_script(&self.archive.read_text(&name)));
        }
        self.hovered = None;
        self.hover_transition = None;
        self.hover_started_ns = None;
        self.focused_input = None;
        self.scroll = 0;
        self.scroll_request = Some(0);
        self.dirty = true;
        self.repaint_from = 0;
        self.repaint_to = i32::MAX;
        self.document_layer = None;
        self.toolbar_layer = None;
        self.toolbar_dirty = true;
        self.document_paint.clear();
        self.toolbar_paint.clear();
        // Loading a screen invalidates every cached document pixel.  The
        // caller's normal `set_content_dirty` path therefore performs a full
        // window redraw, never a hover-sized patch.
        self.window_damage = None;
        self.full_window_redraw = true;
    }

}

