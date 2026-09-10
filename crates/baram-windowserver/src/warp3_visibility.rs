impl Warp3Engine {
    fn hover_amount(&self, idx: usize) -> f32 {
        if let Some((old, new)) = self.hover_transition {
            let started = self.hover_started_ns.unwrap_or(self.animation_now_ns);
            let elapsed = self.animation_now_ns.saturating_sub(started);
            let t = smoothstep(elapsed as f32 / HOVER_DURATION_NS as f32);
            if new == Some(idx) {
                return t;
            }
            if old == Some(idx) {
                return 1.0 - t;
            }
        }
        (self.hovered == Some(idx)) as u8 as f32
    }

    fn switch_amount(&self, idx: usize, on: bool) -> f32 {
        if let Some((animated, target_on)) = self.switch_transition {
            if animated == idx {
                let started = self.switch_started_ns.unwrap_or(self.animation_now_ns);
                let elapsed = self.animation_now_ns.saturating_sub(started);
                let t = ease_out(elapsed as f32 / SWITCH_DURATION_NS as f32);
                return if target_on { t } else { 1.0 - t };
            }
        }
        on as u8 as f32
    }

    fn is_toolbar_tree(&self, idx: usize) -> bool {
        self.nodes.get(idx).map_or(false, |node| node.overlay)
    }

    fn refresh_visibility(&mut self) {
        for node in &mut self.nodes {
            node.hidden = node.manual_hidden;
        }
        let tabs: Vec<(usize, usize, Vec<usize>)> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.is("tab"))
            .map(|(idx, node)| (idx, node.tab, node.children.clone()))
            .collect();
        for (_idx, active, children) in tabs {
            for (tab, child) in children.into_iter().enumerate() {
                if tab != active {
                    // `content` itself is the tab control.  Keep every
                    // control visible and hide only its inactive page body.
                    let page_children = self.nodes[child].children.clone();
                    for page_child in page_children {
                        self.mark_hidden_tree(page_child);
                    }
                }
            }
        }
    }

    fn refresh_text_lines(&mut self) {
        for node in &mut self.nodes {
            if !node.is("content") {
                continue;
            }
            let text = node.prop("text").to_string();
            node.text_lines = if text.is_empty() {
                Vec::new()
            } else if node.is("code") {
                text.split('\n').map(ToString::to_string).collect()
            } else {
                wrap_lines(&text, node.w)
            };
        }
    }

    fn mark_hidden_tree(&mut self, idx: usize) {
        self.nodes[idx].hidden = true;
        let children = self.nodes[idx].children.clone();
        for child in children {
            self.mark_hidden_tree(child);
        }
    }

    fn set_tree_visible(&mut self, idx: usize, visible: bool) {
        self.nodes[idx].manual_hidden = !visible;
        self.nodes[idx].hidden = !visible;
        let children = self.nodes[idx].children.clone();
        for child in children {
            self.set_tree_visible(child, visible);
        }
    }

    /// Built only when layout/tab state changes.  Scroll and hover never walk
    /// the tree: they use these already-resolved paint lists.
    fn rebuild_paint_lists(&mut self) {
        self.document_paint.clear();
        self.toolbar_paint.clear();
        for (idx, node) in self.nodes.iter().enumerate() {
            if node.hidden
                || node.is("config")
                || node.is("space")
                || (node.is("scroll-point") && node.tags.len() == 1)
            {
                continue;
            }
            if node.overlay {
                self.toolbar_paint.push(idx);
            } else {
                self.document_paint.push(idx);
            }
        }
    }

    /// List rows are the dense UI case. Their text is painted normally, but
    /// all separators are emitted as one tight buffer pass instead of making
    /// a LayerSystem draw call per row.
    fn draw_list_dividers(&self, layer: &mut LayerSystem, from: i32, to: i32, origin: i32) {
        let width = layer.width();
        let height = layer.height() as i32;
        let border = html_border().0;
        let buffer = layer.buf_mut();
        for idx in &self.document_paint {
            let node = &self.nodes[*idx];
            if !node.is("list") || node.y + node.h <= from || node.y >= to {
                continue;
            }
            let y = node.y + node.h - 1 - origin;
            if y < 0 || y >= height {
                continue;
            }
            let x0 = node.x.max(0) as usize;
            let x1 = (node.x + node.w).max(0).min(width as i32) as usize;
            if x1 > x0 {
                buffer[y as usize * width + x0..y as usize * width + x1].fill(border);
            }
        }
    }

    fn hit_test(&self, x: i32, y: i32) -> Option<usize> {
        for toolbar_pass in [true, false] {
            if let Some(idx) = (0..self.nodes.len()).rev().find(|idx| {
                self.is_toolbar_tree(*idx) == toolbar_pass
                    && interactive(&self.nodes[*idx])
                    && !self.nodes[*idx].hidden
                    && contains(
                        &self.nodes[*idx],
                        x,
                        if self.is_toolbar_tree(*idx) {
                            y - self.scroll
                        } else {
                            y
                        },
                    )
            }) {
                return Some(idx);
            }
        }
        None
    }

    fn find_parent(&self, child: usize) -> Option<usize> {
        self.nodes
            .iter()
            .position(|node| node.children.contains(&child))
    }
}

