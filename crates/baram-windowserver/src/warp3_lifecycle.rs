impl Warp3Engine {
    pub fn new(app_name: &str) -> Self {
        Self::from_archive(Warp3Archive::open(app_name))
    }

    /// Creates Warp 3 UI for an OS surface. The resources never enter app
    /// discovery or the VFS, so this is not an independently launchable app.
    pub fn new_embedded(name: &str, sources: &[(&str, &str)]) -> Self {
        Self::from_archive(Warp3Archive::from_embedded(name, sources))
    }

    fn from_archive(archive: Warp3Archive) -> Self {
        let config_source = archive.read_text("config.ini");
        let screen = ini_value(&config_source, "screen").unwrap_or_else(|| "main".to_string());
        let title = ini_value(&config_source, "name").unwrap_or_else(|| "Warp 3".to_string());
        let mut engine = Self {
            archive,
            screen,
            app_title: title,
            nodes: Vec::new(),
            roots: Vec::new(),
            scripts: Vec::new(),
            script_frames: Vec::new(),
            script_wait_until_ns: None,
            command_queue: Vec::new(),
            state: Vec::new(),
            hovered: None,
            focused_input: None,
            width: 0,
            height: 0,
            scroll: 0,
            content_height: 0,
            scroll_request: None,
            dirty: true,
            repaint_from: 0,
            repaint_to: i32::MAX,
            document_layer: None,
            toolbar_layer: None,
            toolbar_dirty: true,
            document_paint: Vec::new(),
            toolbar_paint: Vec::new(),
            window_damage: None,
            full_window_redraw: false,
            hover_transition: None,
            switch_transition: None,
            hover_started_ns: None,
            switch_started_ns: None,
            screen_transition_active: false,
            screen_transition_started_ns: None,
            screen_transition_offset_y: 0.0,
            animation_now_ns: 0,
            caret_visible: true,
            shadows: Vec::new(),
            now: NowValues::default(),
            last_clicked_class: None,
            candidate_nodes: Vec::new(),
        };
        engine.load_screen();
        engine
    }

    pub fn title(&self) -> &str {
        &self.app_title
    }

    pub fn set_screen(&mut self, screen: &str) {
        if self.screen != screen {
            self.screen = screen.to_string();
            self.begin_screen_transition();
            self.load_screen();
        }
    }

    fn begin_screen_transition(&mut self) {
        self.cancel_hover();
        self.screen_transition_active = true;
        self.screen_transition_started_ns = None;
        self.screen_transition_offset_y = 15.0;
    }

    pub fn set_text(&mut self, class: &str, value: &str) {
        self.set_element_text(class, value);
    }

    pub fn set_visible(&mut self, class: &str, visible: bool) {
        if let Some(idx) = self
            .nodes
            .iter()
            .position(|node| node.classes.iter().any(|item| item == class))
        {
            if self.nodes[idx].manual_hidden == !visible {
                return;
            }
            self.set_tree_visible(idx, visible);
            self.invalidate_all();
        }
    }

    /// Reuses a grow-only pool of candidate buttons so an OS surface can
    /// present any number of suggestions without fixed empty placeholders.
    pub fn set_candidate_items(&mut self, mode: &str, candidates: &[String]) {
        self.set_element_text("candidate-mode", mode);
        let Some(row) = self.nodes.iter().position(|node| node.is("candidate-row")) else {
            return;
        };
        while self.candidate_nodes.len() < candidates.len() {
            let index = self.candidate_nodes.len();
            let node = self.nodes.len();
            self.nodes.push(Node {
                tags: vec!["button".to_string()],
                classes: vec![alloc::format!("candidate-{index}")],
                props: vec![
                    ("text".to_string(), String::new()),
                    ("type".to_string(), "tonal".to_string()),
                ],
                ..Node::default()
            });
            self.candidate_nodes.push(node);
        }
        let mode_node = self.nodes[row].children.first().copied();
        self.nodes[row].children.clear();
        if let Some(mode_node) = mode_node {
            self.nodes[row].children.push(mode_node);
        }
        for index in 0..self.candidate_nodes.len() {
            let node = self.candidate_nodes[index];
            let visible = index < candidates.len();
            self.nodes[node].manual_hidden = !visible;
            self.nodes[node].hidden = !visible;
            if visible {
                set_prop(&mut self.nodes[node], "text", &candidates[index]);
                self.nodes[row].children.push(node);
            }
        }
        self.invalidate_all();
    }

    pub fn hold_command(&mut self) {
        self.script_wait_until_ns = Some(u64::MAX);
    }

    pub fn complete_command(&mut self) {
        if self.script_wait_until_ns == Some(u64::MAX) {
            self.script_wait_until_ns = None;
            self.resume_script();
        }
    }

    pub fn update(&mut self, width: i32, height: i32) {
        if !self.dirty && self.width == width && self.height == height {
            // Hover changes do not affect geometry.  Repaint only the cached
            // damage rectangle and skip the document layout walk.
            if self.repaint_from < self.content_height || self.toolbar_dirty {
                self.paint_cached_layers();
            }
            return;
        }
        if self.width != width || self.height != height {
            self.toolbar_dirty = true;
        }
        self.refresh_visibility();
        self.width = width.max(1);
        self.height = height.max(1);
        let document_width = self.width.min(960);
        let document_x = (self.width - document_width) / 2;
        let content_x = document_x + 14;
        let content_width = document_width - 28;
        let title_bar = crate::window::title_bar_h() as i32;
        // Keep the first document element clear of the title-bar overlay.
        let mut y = title_bar + 16;
        let roots = self.roots.clone();
        for idx in roots {
            if self.nodes[idx].is("config") || self.nodes[idx].is("toolbar") {
                continue;
            }
            let h = self.layout(idx, content_x, y, content_width);
            y += h + 12;
        }
        self.content_height = (y + 48).max(title_bar + self.height);
        let toolbar_width = (self.width - 16).min(900);
        let toolbar_x = (self.width - toolbar_width) / 2;
        // Toolbars live in viewport coordinates.  They are deliberately not
        // part of the document, so scrolling never moves or reflows them.
        let toolbar_y = title_bar + self.height - 9 - 54;
        let toolbars: Vec<usize> = self
            .roots
            .iter()
            .copied()
            .filter(|idx| self.nodes[*idx].is("toolbar"))
            .collect();
        for idx in toolbars {
            self.layout(idx, toolbar_x, toolbar_y, toolbar_width);
        }
        self.refresh_text_lines();
        self.rebuild_paint_lists();
        self.prepare_shadows();
        self.paint_cached_layers();
        self.dirty = false;
    }

    pub fn set_scroll(&mut self, scroll: i32) {
        if self.scroll != scroll {
            self.scroll = scroll.max(0);
        }
    }

    pub fn hovered_node(&self) -> Option<usize> {
        self.hovered
    }

    pub fn has_focused_input(&self) -> bool {
        self.focused_input.is_some()
    }

    pub fn is_screen_transition_active(&self) -> bool {
        self.screen_transition_active
    }

    pub fn window_damage(&self) -> Option<(i32, i32, i32, i32)> {
        (!self.full_window_redraw)
            .then_some(self.window_damage)
            .flatten()
    }

    pub fn take_scroll_request(&mut self) -> Option<i32> {
        self.scroll_request.take()
    }

    pub fn take_command(&mut self) -> Option<String> {
        if self.command_queue.is_empty() {
            None
        } else {
            Some(self.command_queue.remove(0))
        }
    }

    pub fn set_runtime_metrics(&mut self, fps: u32, windows: usize, keys: u32, mouse: u32) {
        self.now = NowValues {
            fps,
            windows,
            keys,
            mouse,
        };
    }

    pub fn set_hover(&mut self, x: i32, y: i32) {
        if self.screen_transition_active {
            self.cancel_hover();
            return;
        }
        let next = self.hit_test(x, y);
        if self.hovered != next {
            let old = self.hovered;
            // A rapid A -> B -> C move can replace a transition before A has
            // returned to its base pixels. Keep the superseded pair dirty so
            // the next paint restores both nodes before drawing B -> C.
            if let Some((previous_old, previous_new)) = self.hover_transition {
                self.invalidate_nodes(previous_old, previous_new);
            }
            self.hover_transition = Some((old, next));
            self.hover_started_ns = None;
            self.invalidate_nodes(old, next);
            self.hovered = next;
        }
    }

    pub fn clear_hover(&mut self) {
        if self.screen_transition_active {
            self.cancel_hover();
            return;
        }
        if let Some(old) = self.hovered.take() {
            if let Some((previous_old, previous_new)) = self.hover_transition {
                self.invalidate_nodes(previous_old, previous_new);
            }
            self.hover_transition = Some((Some(old), None));
            self.hover_started_ns = None;
            self.invalidate_nodes(Some(old), None);
        }
    }

    pub fn hover_token(&self) -> Option<usize> {
        self.hovered
    }

    pub fn cancel_hover(&mut self) {
        if let Some((old, new)) = self.hover_transition.take() {
            self.invalidate_nodes(old, new);
        }
        if let Some(old) = self.hovered.take() {
            self.invalidate_nodes(Some(old), None);
        }
        self.hover_started_ns = None;
    }

    pub fn click(&mut self, x: i32, y: i32) {
        self.last_clicked_class = None;
        let hit = self.hit_test(x, y);
        let Some(idx) = hit else {
            self.focused_input = None;
            self.invalidate_from(0);
            return;
        };
        let hit_y = self.nodes[idx].y;
        let mut tab_changed = false;
        if self.nodes[idx].is("input") || self.nodes[idx].is("textarea") {
            self.focused_input = Some(idx);
        } else if self.nodes[idx].is("switch") {
            let current = self.nodes[idx].prop("default") == "true";
            let next = !current;
            set_prop(
                &mut self.nodes[idx],
                "default",
                if next { "true" } else { "false" },
            );
            self.switch_transition = Some((idx, next));
            self.switch_started_ns = None;
            self.run_click(idx);
        } else if self.nodes[idx].is("content") {
            if let Some(parent) = self.find_parent(idx) {
                let siblings = self.nodes[parent].children.clone();
                if let Some(tab) = siblings.iter().position(|item| *item == idx) {
                    if self.nodes[parent].tab != tab {
                        self.nodes[parent].tab = tab;
                        tab_changed = true;
                    }
                }
            }
        } else {
            self.focused_input = None;
            self.last_clicked_class = self.nodes[idx].classes.first().cloned();
            self.run_click(idx);
        }
        if tab_changed || self.full_window_redraw {
            self.invalidate_all();
        } else {
            // A script may have replaced the complete node tree, so never
            // index self.nodes with the old hit after run_click.
            self.invalidate_from(hit_y);
        }
    }

    pub fn take_clicked_class(&mut self) -> Option<String> {
        self.last_clicked_class.take()
    }

    pub fn handle_key(&mut self, key: u8) {
        if key == 0x08 || key == 0x7f {
            self.handle_text("", 1);
        } else if (0x20..0x7f).contains(&key) {
            let text = [key];
            let text = unsafe { core::str::from_utf8_unchecked(&text) };
            self.handle_text(text, 0);
        }
    }

    /// Replaces the active IME composition and accepts UTF-8 text.
    pub fn handle_text(&mut self, text: &str, replace_chars: usize) {
        let Some(idx) = self.focused_input else {
            return;
        };
        let mut value = self.nodes[idx].prop("text").to_string();
        for _ in 0..replace_chars {
            value.pop();
        }
        value.push_str(text);
        set_prop(&mut self.nodes[idx], "text", &value);
        self.invalidate_from(self.nodes[idx].y);
    }

    /// Sample control transitions from absolute monotonic time. No layout is
    /// involved; only the old/new control damage rectangle is requested.
    pub fn tick(&mut self, now_ns: u64) -> bool {
        let next_caret_visible = self
            .focused_input
            .map_or(true, |_| text_cursor::visible(now_ns));
        let caret_changed = self.caret_visible != next_caret_visible;
        self.caret_visible = next_caret_visible;
        if self.animation_now_ns == now_ns {
            return caret_changed;
        }
        self.animation_now_ns = now_ns;
        let mut changed = caret_changed;
        if caret_changed {
            self.invalidate_nodes(self.focused_input, None);
        }
        if self
            .script_wait_until_ns
            .is_some_and(|deadline| now_ns >= deadline)
        {
            self.script_wait_until_ns = None;
            self.resume_script();
            changed = true;
        }
        if let Some((old, new)) = self.hover_transition {
            let started = self.hover_started_ns.get_or_insert(now_ns);
            let complete = now_ns.saturating_sub(*started) >= HOVER_DURATION_NS;
            self.invalidate_nodes(old, new);
            changed = true;
            if complete {
                self.hover_transition = None;
                self.hover_started_ns = None;
            }
        }
        if let Some((idx, _on)) = self.switch_transition {
            let started = self.switch_started_ns.get_or_insert(now_ns);
            let complete = now_ns.saturating_sub(*started) >= SWITCH_DURATION_NS;
            self.invalidate_nodes(Some(idx), None);
            changed = true;
            if complete {
                self.switch_transition = None;
                self.switch_started_ns = None;
            }
        }
        if self.screen_transition_active {
            let started = *self.screen_transition_started_ns.get_or_insert(now_ns);
            let t = (now_ns.saturating_sub(started) as f32 / 250_000_000.0).min(1.0);
            let remaining = 1.0 - t;
            let raw_offset = 15.0 * remaining * remaining * remaining;
            // The first 90% stays on the integer-pixel fast path. During the
            // final settle, retain the fractional position for smoother text.
            let next_offset = if t < 0.9 {
                raw_offset as i32 as f32
            } else {
                raw_offset
            };
            changed |= self.screen_transition_offset_y != next_offset;
            self.screen_transition_offset_y = next_offset;
            if t >= 1.0 {
                self.screen_transition_active = false;
                self.screen_transition_offset_y = 0.0;
                changed = true;
            }
        }
        changed
    }

}

