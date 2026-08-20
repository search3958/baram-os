use super::*;

/// Match the normal window server's cubic-bezier(0, 0, 0, 1) scroll curve.
/// Solving x=s^3 with a short binary search keeps this identical without
/// adding a curve table to the Xiao image.
fn decelerate_scroll(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let mut low = 0.0f32;
    let mut high = 1.0f32;
    for _ in 0..10 {
        let s = (low + high) * 0.5;
        if s * s * s < t {
            low = s;
        } else {
            high = s;
        }
    }
    let s = (low + high) * 0.5;
    s * s * (3.0 - 2.0 * s)
}

impl Warp4Engine {
    pub fn new(app_name: &str) -> Self {
        Self::from_archive(Warp4Archive::open(app_name))
    }

    pub fn new_embedded(name: &str, sources: &[(&str, &str)]) -> Self {
        Self::from_archive(Warp4Archive::from_embedded(name, sources))
    }

    pub(crate) fn from_archive(archive: Warp4Archive) -> Self {
        let cfg = archive.read_text("config.ini");
        let screen = ini(&cfg, "screen").unwrap_or_else(|| "main".into());
        let title = ini(&cfg, "name").unwrap_or_else(|| "Warp 4".into());
        let mut this = Self {
            origin: archive.app_name().to_string(),
            archive,
            title,
            screen,
            nodes: Vec::new(),
            roots: Vec::new(),
            fixed_subtree: Vec::new(),
            script: Script::default(),
            state: Vec::new(),
            focused: None,
            keyboard_focus: None,
            hovered: None,
            pressed: None,
            keyboard_press_until_ns: None,
            spinner_open: None,
            spinner_fade: None,
            width: 0,
            height: 0,
            chrome_height: title_bar_h(),
            scroll: 0,
            scroll_target: 0,
            scroll_start: 0,
            scroll_subpixel: 0,
            scroll_target_subpixel: 0,
            scroll_start_subpixel: 0,
            scroll_started_ns: None,
            content_height: 0,
            last_command: None,
            dirty: true,
            layout_dirty: true,
            now_ns: 0,
            wait_until_ns: None,
            pending: Vec::new(),
            break_requested: false,
            flip_elapsed_ns: 0,
            last_tick_ns: None,
            control_animations: Vec::new(),
            transition_elapsed_ns: None,
            last_clicked_id: None,
            runtime_fps: 0,
            runtime_windows: 0,
            runtime_keys: 0,
            runtime_mouse: 0,
        };
        this.load_screen();
        this
    }

    pub(crate) fn preload_text_cache(&self) {
        if !bdf_font::is_available() {
            return;
        }
        let mut texts = Vec::new();
        for node in &self.nodes {
            for key in ["text", "hint", "textOn", "textOff"] {
                let text = node.attr(key);
                if !text.is_empty() {
                    texts.push(text);
                }
            }
        }
        bdf_font::preload_texts(&texts);
    }

    pub fn set_origin(&mut self, name: &str) {
        self.origin = name.into();
    }

    /// Removes the window-manager title-bar coordinate space for surfaces
    /// embedded directly into an OS-owned card or overlay.
    pub fn set_chrome_visible(&mut self, visible: bool) {
        let next = if visible { title_bar_h() } else { 0 };
        if self.chrome_height != next {
            self.chrome_height = next;
            self.scroll = 0;
            self.scroll_target = 0;
            self.scroll_start = 0;
            self.scroll_subpixel = 0;
            self.scroll_target_subpixel = 0;
            self.scroll_start_subpixel = 0;
            self.scroll_started_ns = None;
            self.dirty = true;
            self.layout_dirty = true;
        }
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn set_scroll(&mut self, scroll: i32) {
        let max = self
            .content_height
            .saturating_sub(self.height + self.chrome_height);
        let next = scroll.max(0).min(max.max(0));
        if is_xiao() {
            let next_subpixel = next.saturating_mul(3);
            let changed = self.scroll_target_subpixel != next_subpixel
                || self.scroll_subpixel != next_subpixel;
            if self.scroll_target_subpixel != next_subpixel {
                // Queue a new destination while an animation is in flight.
                // Restarting from the current frame made repeated arrow keys
                // visibly stutter and differed from the normal window server.
                if self.scroll_subpixel == self.scroll_target_subpixel {
                    self.scroll_start = self.scroll;
                    self.scroll_start_subpixel = self.scroll_subpixel;
                    self.scroll_started_ns = None;
                }
                self.scroll_target = next;
                self.scroll_target_subpixel = next_subpixel;
            }
            if changed {
                if let Some(idx) = self.spinner_open {
                    self.close_spinner(idx);
                }
                self.hovered = None;
                self.dirty = true;
            }
        } else if self.scroll != next {
            self.scroll = next;
            self.scroll_target = next;
            self.scroll_start = next;
            self.scroll_subpixel = next.saturating_mul(3);
            self.scroll_target_subpixel = self.scroll_subpixel;
            self.scroll_start_subpixel = self.scroll_subpixel;
            self.scroll_started_ns = None;
            if let Some(idx) = self.spinner_open {
                self.close_spinner(idx);
            }
            self.hovered = None;
            self.dirty = true;
        }
    }

    /// Move the active document viewport by a fixed keyboard step.  Xiao uses
    /// this directly for UEFI arrow-key events because those events have no
    /// printable byte to pass through `handle_key`.
    pub fn scroll_step(&self) -> i32 {
        config::get_i32("ui-theme/window/scroll_speed", 30).max(1)
    }

    pub fn scroll_by(&mut self, delta: i32) -> bool {
        let before = if is_xiao() {
            self.scroll_target
        } else {
            self.scroll
        };
        let base = if is_xiao() {
            self.scroll_target
        } else {
            self.scroll
        };
        self.set_scroll(base.saturating_add(delta));
        self.scroll_target != before || self.scroll != before
    }

    /// Return the active document offset in layer pixels.
    ///
    /// Xiao feeds pointer coordinates in viewport space, while the normal
    /// window server feeds Warp4 document coordinates. The kiosk uses this
    /// value to convert its pointer event without duplicating scroll state.
    pub fn scroll_position(&self) -> i32 {
        self.scroll
    }
    pub fn is_animating(&self) -> bool {
        !self.control_animations.is_empty()
            || (is_xiao() && self.scroll_subpixel != self.scroll_target_subpixel)
            || self.transition_elapsed_ns.is_some()
    }
    pub fn window_damage(&self) -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn has_focused_input(&self) -> bool {
        self.focused.is_some()
    }

    /// Move keyboard focus between interactive controls without requiring
    /// every Warp application to implement its own focus script.  Horizontal
    /// arrows are intentionally used here by Xiao; vertical arrows remain
    /// document scrolling keys.
    pub fn focus_direction(&mut self, direction: i32) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        let mut focusable = Vec::new();
        for idx in 0..self.nodes.len() {
            let node = &self.nodes[idx];
            if node.visible() && self.active_child(idx) && is_keyboard_focusable(node) {
                focusable.push(idx);
            }
        }
        if focusable.is_empty() {
            return false;
        }
        let next = match self
            .keyboard_focus
            .and_then(|current| focusable.iter().position(|idx| *idx == current))
        {
            Some(position) => {
                let next = position as i32 + if direction < 0 { -1 } else { 1 };
                if next < 0 {
                    focusable[focusable.len() - 1]
                } else {
                    focusable[next as usize % focusable.len()]
                }
            }
            None if direction < 0 => focusable[focusable.len() - 1],
            None => focusable[0],
        };
        let changed = self.keyboard_focus != Some(next);
        self.keyboard_focus = Some(next);
        self.ensure_keyboard_focus_visible(next);
        self.dirty = true;
        changed
            || if is_xiao() {
                self.scroll_subpixel != self.scroll_target_subpixel
            } else {
                self.scroll != self.scroll_target
            }
    }

    /// Activate the currently focused button as if Space had clicked it.
    /// Text fields are left to the normal text-input path.
    pub fn activate_focused(&mut self) -> bool {
        let Some(idx) = self.keyboard_focus else {
            return false;
        };
        if !self
            .nodes
            .get(idx)
            .is_some_and(|node| is_keyboard_focusable(node) && !is_text_input(node))
        {
            return false;
        }
        let center_x = self.nodes[idx].x + self.nodes[idx].w.max(1) / 2;
        self.activate_index(idx, true, center_x);
        true
    }

    /// Begin the shared classic-Mac-style zoom reveal used when Xiao enters a
    /// new screen.  The overlay is painted by this engine, so applications do
    /// not need transition elements in their XML.
    pub fn start_transition(&mut self) {
        if is_xiao() {
            self.transition_elapsed_ns = Some(0);
            self.dirty = true;
        }
    }
    pub fn set_runtime_metrics(&mut self, fps: u32, windows: usize, keys: u32, mouse: u32) {
        self.runtime_fps = fps;
        self.runtime_windows = windows;
        self.runtime_keys = keys;
        self.runtime_mouse = mouse;
    }
    pub fn hovered_node(&self) -> Option<usize> {
        self.hovered
    }
    pub fn clear_hover(&mut self) {
        if self.hovered.take().is_some() {
            self.dirty = true;
        }
    }
    pub fn refresh_config(&mut self) {
        self.dirty = true;
    }
    pub fn take_command(&mut self) -> Option<String> {
        self.last_command.take()
    }
    pub fn take_scroll_request(&mut self) -> Option<i32> {
        None
    }
    pub fn tick(&mut self, now_ns: u64) -> bool {
        self.now_ns = now_ns;
        let delta = self
            .last_tick_ns
            .replace(now_ns)
            .map(|previous| now_ns.saturating_sub(previous).min(100_000_000))
            .unwrap_or(0);
        self.flip_elapsed_ns = self.flip_elapsed_ns.saturating_add(delta);

        let mut changed = false;
        let mut transition_finished = false;
        if let Some(elapsed) = self.transition_elapsed_ns.as_mut() {
            *elapsed = elapsed.saturating_add(delta.max(1_000_000));
            transition_finished = *elapsed >= XIAO_TRANSITION_NS;
            changed = true;
            self.dirty = true;
        }
        if transition_finished {
            self.transition_elapsed_ns = None;
        }
        if is_xiao() && self.scroll_subpixel != self.scroll_target_subpixel {
            // Use the same timestamp-origin model as the normal window
            // server. Accumulating frame deltas made input bursts and missed
            // timer wakeups change the effective scroll speed.
            let started = *self
                .scroll_started_ns
                .get_or_insert(self.now_ns.saturating_sub(1_000_000));
            let elapsed = self.now_ns.saturating_sub(started);
            let t = (elapsed as f32 / XIAO_SCROLL_ANIMATION_NS as f32).clamp(0.0, 1.0);
            let eased = decelerate_scroll(t);
            let distance = self.scroll_target_subpixel - self.scroll_start_subpixel;
            let next_subpixel = if t >= 1.0 {
                self.scroll_target_subpixel
            } else {
                let amount = distance as f32 * eased;
                // Round to the nearest third-pixel. Truncation left a final
                // fraction pending and produced a visible last-pixel jump.
                let rounded = if amount >= 0.0 {
                    (amount + 0.5) as i32
                } else {
                    (amount - 0.5) as i32
                };
                self.scroll_start_subpixel.saturating_add(rounded)
            };
            if self.scroll_subpixel != next_subpixel {
                self.scroll_subpixel = next_subpixel;
                self.scroll = next_subpixel.div_euclid(3);
                self.dirty = true;
                changed = true;
            }
            if t >= 1.0 {
                self.scroll_start = self.scroll_target;
                self.scroll_start_subpixel = self.scroll_target_subpixel;
                self.scroll_started_ns = None;
            }
        }
        let mut flip_interval_ns: Option<u64> = None;
        for node in &self.nodes {
            if (node.is("ViewFlipper") || node.is("ViewAnimator"))
                && truth(node.attr("autoStart"))
                && node.children.len() > 1
            {
                let interval_ms = parse_i32(node.attr("flipInterval")).max(16) as u64;
                flip_interval_ns =
                    Some(flip_interval_ns.map_or(interval_ms * 1_000_000, |current| {
                        current.min(interval_ms * 1_000_000)
                    }));
            }
        }
        if let Some(interval_ns) = flip_interval_ns {
            if self.flip_elapsed_ns >= interval_ns {
                self.flip_elapsed_ns %= interval_ns;
                if let Some(idx) = self.nodes.iter().position(|node| {
                    (node.is("ViewFlipper") || node.is("ViewAnimator"))
                        && truth(node.attr("autoStart"))
                        && node.children.len() > 1
                }) {
                    let count = self.nodes[idx].children.len() as i32;
                    let next =
                        (parse_i32(self.nodes[idx].attr("displayedChild")).max(0) + 1) % count;
                    set_attr(&mut self.nodes[idx], "displayedChild", &next.to_string());
                    self.hovered = None;
                    self.pressed = None;
                    self.dirty = true;
                    changed = true;
                }
            }
        }

        if let Some(until) = self.wait_until_ns {
            if now_ns >= until {
                self.wait_until_ns = None;
                if !self.pending.is_empty() {
                    let pending = core::mem::take(&mut self.pending);
                    self.execute(&pending);
                    changed = true;
                }
            }
        }
        if let Some(until) = self.keyboard_press_until_ns {
            if now_ns >= until {
                self.keyboard_press_until_ns = None;
                self.pressed = None;
                self.dirty = true;
                changed = true;
            }
        }
        let mut controls_animating = false;
        self.control_animations.retain(|animation| {
            let active = now_ns.saturating_sub(animation.started_ns) < animation.duration_ns;
            controls_animating |= active;
            active
        });
        if controls_animating {
            self.dirty = true;
            changed = true;
        }
        // Keep a focused text field alive while its native caret blinks.  The
        // caret is painted independently from the field's text value.
        if self.focused.is_some() {
            self.dirty = true;
            changed = true;
        }
        if let Some(fade) = self.spinner_fade {
            if now_ns.saturating_sub(fade.started_ns) >= 140_000_000 {
                self.spinner_fade = None;
                // The last translucent frame must be invalidated as well so
                // the underlying content is painted back over the popup.
                self.dirty = true;
                changed = true;
            } else {
                self.dirty = true;
                changed = true;
            }
        }
        changed
    }

    pub fn set_screen(&mut self, screen: &str) {
        let screen = screen
            .trim()
            .trim_end_matches(".w4u")
            .trim_end_matches(".w3u");
        if self.screen != screen {
            self.screen = screen.into();
            self.load_screen();
            self.start_transition();
        }
    }

    pub(crate) fn load_screen(&mut self) {
        // Rebuilding the native view tree must not reset a running app. Keep
        // script variables and user-editable view values across navigation or
        // any other screen reload.
        let previous_state = self.state.clone();
        let mut previous_attrs: Vec<(String, String, String)> = Vec::new();
        for node in &self.nodes {
            let id = node.id();
            if id.is_empty() {
                continue;
            }
            for key in [
                "text",
                "checked",
                "progress",
                "rating",
                "selectedIndex",
                "value",
                "visibility",
                "enabled",
            ] {
                let value = node.attr(key);
                if !value.is_empty() {
                    previous_attrs.push((id.to_string(), key.into(), value.into()));
                }
            }
        }

        self.nodes.clear();
        self.roots.clear();
        self.fixed_subtree.clear();
        self.focused = None;
        self.keyboard_focus = None;
        self.hovered = None;
        self.pressed = None;
        self.keyboard_press_until_ns = None;
        self.spinner_open = None;
        self.spinner_fade = None;
        self.control_animations.clear();
        self.flip_elapsed_ns = 0;
        self.last_tick_ns = None;
        self.transition_elapsed_ns = None;
        self.scroll = 0;
        self.scroll_target = 0;
        self.scroll_start = 0;
        self.scroll_subpixel = 0;
        self.scroll_target_subpixel = 0;
        self.scroll_start_subpixel = 0;
        self.scroll_started_ns = None;
        self.wait_until_ns = None;
        self.pending.clear();
        let mut layout = self.archive.read_text(&format!("{}.w4u", self.screen));
        if layout.is_empty() {
            layout = self.archive.read_text(&format!("{}.w3u", self.screen));
        }
        // A few early Warp 4 examples were distributed as plain XML instead
        // of using the `.w4u` member name. They are the same native view-tree
        // format; accepting the suffix here keeps those archives native while
        // avoiding an HTML conversion path.
        if layout.is_empty() {
            layout = self.archive.read_text(&format!("{}.xml", self.screen));
        }
        if layout.is_empty() && self.screen != "main" {
            layout = self.archive.read_text("main.xml");
        }
        let mut parser = XmlParser::new(&layout);
        if let Some(root) = parser.parse_element(None, &mut self.nodes) {
            self.roots.push(root);
        }
        let mut source = self.archive.read_text(&format!("{}.w4s", self.screen));
        if source.is_empty() {
            source = self.archive.read_text(&format!("{}.w3s", self.screen));
        }
        self.script = parse_script(&source);
        let init = self.script.init.clone();
        self.execute(&init);
        // Init provides defaults on the first load. When this is a reload,
        // restore the values owned by the already-running application.
        for (key, value) in previous_state {
            self.set_state(&key, &value);
        }
        for (id, key, value) in previous_attrs {
            if let Some(idx) = self.find(&id) {
                set_attr(&mut self.nodes[idx], &key, &value);
            }
        }
        let chrome_mode = self.has_explicit_scroll();
        self.rebuild_fixed_subtree(chrome_mode);
        self.refresh_visibility();
        self.dirty = true;
        self.layout_dirty = true;
        self.preload_text_cache();
    }

    pub(crate) fn rebuild_fixed_subtree(&mut self, chrome_mode: bool) {
        self.fixed_subtree.clear();
        self.fixed_subtree.resize(self.nodes.len(), false);
        let roots = self.roots.clone();
        for root in roots {
            self.mark_fixed_subtree(root, chrome_mode);
        }
    }

    pub(crate) fn mark_fixed_subtree(&mut self, idx: usize, chrome_mode: bool) -> bool {
        let mut has_fixed = self.node_is_fixed(idx, chrome_mode);
        let children = self.nodes[idx].children.clone();
        for child in children {
            has_fixed |= self.mark_fixed_subtree(child, chrome_mode);
        }
        self.fixed_subtree[idx] = has_fixed;
        has_fixed
    }

    pub fn update(&mut self, width: i32, height: i32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width != width || self.height != height {
            self.layout_dirty = true;
        }
        self.width = width;
        self.height = height;
        // Scrolling and hover only invalidate pixels. They do not change the
        // XML layout, so avoid measuring the whole view tree for every frame.
        if !self.layout_dirty {
            self.dirty = false;
            return;
        }
        self.refresh_visibility();
        let roots = self.roots.clone();
        // `height` is the viewport below the window title bar.  Coordinates
        // remain full-layer coordinates so the compositor can apply its
        // window scroll offset without a second layout coordinate system.
        // The generated HTML has no implicit document margin, so the native
        // root starts at the content edge and uses the complete width.
        let mut y = self.chrome_height;
        let usable = self.height;
        for root in roots {
            let forced = if is_match_parent(self.nodes[root].attr("layout_height")) {
                Some(usable)
            } else {
                None
            };
            let h = self.layout(root, 0, y, self.width, forced, "");
            y += h;
        }
        // Scroll extent is deliberately owned by explicit XML scroll
        // containers.  Use each container's actual document bottom instead
        // of adding the largest overflow to the last root: nested
        // ScrollViews otherwise produce an incorrect end position and leave
        // a blank strip after the viewport has moved.
        let mut document_bottom = y;
        for idx in 0..self.nodes.len() {
            if is_scroll_container(&self.nodes[idx]) {
                document_bottom = document_bottom
                    .max(self.nodes[idx].y.saturating_add(self.nodes[idx].content_h));
            }
        }
        self.content_height = document_bottom.max(self.height + self.chrome_height);
        let max_scroll = self
            .content_height
            .saturating_sub(self.height + self.chrome_height)
            .max(0);
        self.scroll = self.scroll.min(max_scroll);
        self.scroll_target = self.scroll_target.min(max_scroll);
        self.scroll_start = self.scroll_start.min(max_scroll);
        self.scroll_subpixel = self.scroll_subpixel.min(max_scroll.saturating_mul(3));
        self.scroll_target_subpixel = self.scroll_target_subpixel.min(max_scroll.saturating_mul(3));
        self.scroll_start_subpixel = self.scroll_start_subpixel.min(max_scroll.saturating_mul(3));
        self.dirty = false;
        self.layout_dirty = false;
    }
}
