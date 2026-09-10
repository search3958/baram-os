impl Warp3Engine {
    fn layout(&mut self, idx: usize, x: i32, y: i32, width: i32) -> i32 {
        let width = width.max(1);
        self.nodes[idx].x = x;
        self.nodes[idx].y = y;
        self.nodes[idx].w = width;
        let tag = self.nodes[idx].tags.first().cloned().unwrap_or_default();
        match tag.as_str() {
            "text" | "detail" | "head" | "code" | "scroll-point" => {
                let lines = if tag == "code" {
                    self.nodes[idx].prop("text").split('\n').count().max(1)
                } else {
                    wrap_lines(self.nodes[idx].prop("text"), width).len().max(1)
                } as i32;
                self.nodes[idx].h = if tag == "head" {
                    38
                } else {
                    lines * 22 + if tag == "code" { 22 } else { 0 }
                };
            }
            "button" => {
                // `i32::clamp` panics when the window is narrower than the
                // button's normal 44 px minimum because min > max.  A tiny
                // window must shrink the control instead of crashing.
                self.nodes[idx].w = self.nodes[idx]
                    .prop("key-width")
                    .parse::<i32>()
                    .ok()
                    .map(|requested| requested.clamp(1, width))
                    .unwrap_or_else(|| {
                        fit_button_width(measure(self.nodes[idx].prop("text")) + 28, width)
                    });
                self.nodes[idx].h = 34;
            }
            "input" => {
                self.nodes[idx].w = width.min(420);
                self.nodes[idx].h = 34;
            }
            "textarea" => {
                self.nodes[idx].w = width.min(420);
                self.nodes[idx].h = 100;
            }
            "switch" => {
                self.nodes[idx].w = 40;
                self.nodes[idx].h = 20;
            }
            "space" => {
                self.nodes[idx].h = 1;
                self.nodes[idx].w = width;
            }
            "keyboard" => {
                let children = self.nodes[idx].children.clone();
                let rows: Vec<usize> = children
                    .iter()
                    .copied()
                    .filter(|child| self.nodes[*child].is("keyboard-row"))
                    .collect();
                let grid_w = rows
                    .iter()
                    .map(|row| keyboard_row_natural_width(&self.nodes, *row))
                    .max()
                    .unwrap_or(width)
                    .min(width);
                let grid_x = x + (width - grid_w) / 2;
                let mut cy = y;
                for child in children {
                    if self.nodes[child].hidden {
                        continue;
                    }
                    let h = self.layout(child, grid_x, cy, grid_w);
                    cy += h + 6;
                }
                self.nodes[idx].h = (cy - y - 6).max(1);
            }
            "candidate-row" => {
                let children = self.nodes[idx].children.clone();
                let gap = 6i32;
                let mut cx = x;
                for child in children {
                    if self.nodes[child].hidden {
                        continue;
                    }
                    let item_w = fit_button_width(
                        measure(self.nodes[child].prop("text")) + 28,
                        width.saturating_sub(cx - x).max(1),
                    );
                    set_prop(&mut self.nodes[child], "key-width", &item_w.to_string());
                    set_prop(&mut self.nodes[child], "key-center", "true");
                    self.layout(child, cx, y, item_w);
                    self.nodes[child].h = 30;
                    cx += item_w + gap;
                }
                self.nodes[idx].h = 30;
            }
            "keyboard-row" => {
                // This is a compact fit-content grid, not a generic toolbar.
                // Each key starts at its measured minimum, action keys impose
                // a sensible minimum, and the space key absorbs only the
                // remainder. Every row is then centred on the same grid.
                let children = self.nodes[idx].children.clone();
                let gap = 6i32;
                let requested: Vec<i32> = children
                    .iter()
                    .map(|child| keyboard_key_width(&self.nodes[*child]))
                    .collect();
                let gaps = gap * children.len().saturating_sub(1) as i32;
                let space_count = children
                    .iter()
                    .filter(|child| {
                        self.nodes[**child]
                            .classes
                            .iter()
                            .any(|class| class == "space")
                    })
                    .count() as i32;
                let fixed = children
                    .iter()
                    .zip(requested.iter())
                    .filter(|(child, _)| {
                        !self.nodes[**child]
                            .classes
                            .iter()
                            .any(|class| class == "space")
                    })
                    .map(|(_, key_w)| *key_w)
                    .sum::<i32>()
                    + gaps;
                let available_space = if space_count > 0 {
                    ((width - fixed) / space_count)
                        .max(requested.iter().copied().min().unwrap_or(44))
                } else {
                    0
                };
                let total = if space_count > 0 {
                    fixed + available_space * space_count
                } else {
                    fixed
                };
                let mut cx = x + ((width - total).max(0) / 2);
                let mut max_h = 30;
                for (child, mut key_w) in children.into_iter().zip(requested) {
                    if self.nodes[child]
                        .classes
                        .iter()
                        .any(|class| class == "space")
                    {
                        key_w = available_space;
                    }
                    set_prop(&mut self.nodes[child], "key-width", &key_w.to_string());
                    set_prop(&mut self.nodes[child], "key-center", "true");
                    let h = self.layout(child, cx, y, key_w);
                    self.nodes[child].h = 30;
                    cx += self.nodes[child].w + gap;
                    max_h = max_h.max(h);
                }
                self.nodes[idx].h = max_h.min(30);
            }
            "flex" | "toolbar" => {
                let children = self.nodes[idx].children.clone();
                let mut cx = x + if tag == "toolbar" { 10 } else { 0 };
                let mut max_h = 34;
                let available = width - if tag == "toolbar" { 20 } else { 0 };
                let fixed: i32 = children
                    .iter()
                    .filter(|child| !self.nodes[**child].is("space"))
                    .map(|child| (measure(self.nodes[*child].prop("text")) + 36).max(44))
                    .sum();
                let gaps = (children.len().saturating_sub(1) as i32) * 8;
                let spaces = children
                    .iter()
                    .filter(|child| self.nodes[**child].is("space"))
                    .count() as i32;
                let space_w = if spaces > 0 {
                    (available - fixed - gaps).max(8) / spaces
                } else {
                    0
                };
                for child in children {
                    let child_w = if self.nodes[child].is("space") {
                        space_w
                    } else {
                        available
                    };
                    let h =
                        self.layout(child, cx, y + if tag == "toolbar" { 9 } else { 0 }, child_w);
                    cx += self.nodes[child].w + 8;
                    max_h = max_h.max(h);
                }
                self.nodes[idx].h = if tag == "toolbar" { 54 } else { max_h };
            }
            "tab" => {
                let children = self.nodes[idx].children.clone();
                let control = self.nodes[idx].prop("control").to_string();
                let vertical = control == "left" || control == "right";
                let controls_w = if vertical { 104 } else { width };
                let mut control_x = if control == "right" {
                    x + width - controls_w + 8
                } else {
                    x + 8
                };
                let mut control_y = y + 8;
                let mut page_h = 0;
                for (tab, child) in children.iter().copied().enumerate() {
                    let label_w = (measure(self.nodes[child].prop("text")) + 24).max(64);
                    self.nodes[child].x = control_x;
                    self.nodes[child].y = control_y;
                    self.nodes[child].w = if vertical { controls_w - 16 } else { label_w };
                    self.nodes[child].h = 34;
                    if vertical {
                        control_y += 37;
                    } else {
                        control_x += label_w + 4;
                    }
                    if tab == self.nodes[idx].tab {
                        let page_x = if control == "left" {
                            x + controls_w + 18
                        } else {
                            x + 18
                        };
                        let page_width = if vertical {
                            width - controls_w - 36
                        } else {
                            width - 36
                        };
                        let mut cy = if vertical { y + 18 } else { y + 56 };
                        for grandchild in self.nodes[child].children.clone() {
                            let h = self.layout(grandchild, page_x, cy, page_width);
                            cy += h + 8;
                        }
                        page_h = cy - y + 10;
                    }
                }
                self.nodes[idx].h = page_h.max(if vertical { control_y - y + 8 } else { 100 });
            }
            "content" => {}
            "card" => {
                let mut cy = y + 18;
                if !self.nodes[idx].prop("text").is_empty() {
                    cy += 30;
                }
                for child in self.nodes[idx].children.clone() {
                    let h = self.layout(child, x + 16, cy, width - 32);
                    cy += h + 8;
                }
                self.nodes[idx].h = cy - y + 10;
            }
            "list-box" => {
                let title_height = if self.nodes[idx].prop("text").is_empty() {
                    0
                } else {
                    36
                };
                let mut cy = y + title_height;
                for child in self.nodes[idx].children.clone() {
                    let h = self.layout(child, x, cy, width);
                    cy += h;
                }
                self.nodes[idx].h = cy - y;
            }
            "list" => {
                let detail_width = self.nodes[idx]
                    .children
                    .iter()
                    .find(|child| self.nodes[**child].is("detail"))
                    .map(|child| (measure(self.nodes[*child].prop("text")) + 8).min(width / 2))
                    .unwrap_or(0);
                let mut max_h = 22;
                for child in self.nodes[idx].children.clone() {
                    if self.nodes[child].is("detail") {
                        let h =
                            self.layout(child, x + width - detail_width - 8, y + 12, detail_width);
                        max_h = max_h.max(h);
                    } else {
                        let text_width =
                            (width - detail_width - if detail_width > 0 { 24 } else { 16 }).max(24);
                        let h = self.layout(child, x + 8, y + 12, text_width);
                        max_h = max_h.max(h);
                    }
                }
                self.nodes[idx].h = (max_h + 24).max(46);
            }
            _ => {
                let mut cy = y;
                for child in self.nodes[idx].children.clone() {
                    let h = self.layout(child, x, cy, width);
                    cy += h + 8;
                }
                self.nodes[idx].h = (cy - y).max(1);
            }
        }
        let text = self.nodes[idx].prop("text").to_string();
        self.nodes[idx].text_lines = if text.is_empty() {
            Vec::new()
        } else if self.nodes[idx].is("code") {
            text.split('\n').map(ToString::to_string).collect()
        } else {
            wrap_lines(&text, self.nodes[idx].w)
        };
        self.nodes[idx].h
    }

    fn invalidate_from(&mut self, y: i32) {
        self.repaint_from = self.repaint_from.min(y.max(0));
        self.repaint_to = i32::MAX;
        self.dirty = true;
    }

    fn invalidate_all(&mut self) {
        self.repaint_from = 0;
        self.repaint_to = i32::MAX;
        self.dirty = true;
        self.toolbar_dirty = true;
        self.window_damage = None;
        self.full_window_redraw = true;
    }

    fn invalidate_range(&mut self, from: i32, to: i32) {
        if self.repaint_to <= self.repaint_from {
            self.repaint_from = from;
            self.repaint_to = to;
        } else {
            self.repaint_from = self.repaint_from.min(from);
            self.repaint_to = self.repaint_to.max(to);
        }
    }

    fn invalidate_nodes(&mut self, old: Option<usize>, new: Option<usize>) {
        let mut document_y = None;
        let mut document_bottom = None;
        for idx in [old, new].into_iter().flatten() {
            // A screen replacement invalidates node IDs. `load_screen`
            // clears transitions, but ignore a stale ID defensively so a
            // future tree mutation cannot turn hover repaint into a panic.
            if idx >= self.nodes.len() {
                continue;
            }
            let node = &self.nodes[idx];
            let screen_y = if node.overlay {
                node.y
            } else {
                node.y - self.scroll
            };
            // Document patches may never clear title-bar pixels.  Those are
            // outside Warp3's ownership and would otherwise expose wallpaper.
            let y0 = if node.overlay {
                screen_y - HOVER_DAMAGE_PAD
            } else {
                (screen_y - HOVER_DAMAGE_PAD).max(crate::window::title_bar_h() as i32)
            };
            let next = (
                node.x - HOVER_DAMAGE_PAD,
                y0,
                node.x + node.w + HOVER_DAMAGE_PAD,
                screen_y + node.h + HOVER_DAMAGE_PAD,
            );
            self.window_damage = Some(match self.window_damage {
                Some(old) => (
                    old.0.min(next.0),
                    old.1.min(next.1),
                    old.2.max(next.2),
                    old.3.max(next.3),
                ),
                None => next,
            });
            if self.is_toolbar_tree(idx) {
                // Fixed controls are painted directly over the final document;
                // their hover state therefore needs no cached-layer rebuild.
            } else {
                document_y =
                    Some(document_y.map_or(self.nodes[idx].y, |y: i32| y.min(self.nodes[idx].y)));
                document_bottom = Some(
                    document_bottom.map_or(self.nodes[idx].y + self.nodes[idx].h, |bottom: i32| {
                        bottom.max(self.nodes[idx].y + self.nodes[idx].h)
                    }),
                );
            }
        }
        if let Some(y) = document_y {
            // Shadows extend outside the node; repaint their full footprint.
            self.invalidate_range(
                (y - HOVER_DAMAGE_PAD).max(0),
                document_bottom.unwrap_or(y) + HOVER_DAMAGE_PAD,
            );
        }
    }

}

