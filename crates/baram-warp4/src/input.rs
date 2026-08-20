use super::*;

impl Warp4Engine {
    pub fn set_hover(&mut self, x: i32, y: i32) {
        let _ = self.set_hover_changed(x, y);
    }

    /// Update hover state and report whether the visible control changed.
    /// Xiao uses the result to redraw only when the pointer crosses a
    /// control, keeping ordinary pointer motion on the cheap cursor overlay.
    pub fn set_hover_changed(&mut self, x: i32, y: i32) -> bool {
        let popup_hit = self.spinner_popup_hit(x, y);
        let next = popup_hit.map(|(idx, _)| idx).or_else(|| self.hit(x, y));
        if self.hovered != next {
            self.hovered = next;
            self.dirty = true;
            return true;
        }
        false
    }

    pub(crate) fn close_spinner(&mut self, idx: usize) {
        self.spinner_open = None;
        self.spinner_fade = Some(SpinnerFade {
            idx,
            started_ns: self.now_ns,
        });
        self.dirty = true;
    }

    pub(crate) fn start_control_animation(&mut self, idx: usize, from_on: bool, to_on: bool) {
        if from_on == to_on {
            return;
        }
        self.control_animations
            .retain(|animation| animation.idx != idx);
        self.control_animations.push(ControlAnimation {
            idx,
            to_on,
            started_ns: self.now_ns,
            duration_ns: if self.nodes[idx].is("Switch") {
                SWITCH_DURATION_NS
            } else {
                RADIO_DURATION_NS
            },
        });
        self.dirty = true;
    }

    pub(crate) fn control_amount(&self, idx: usize, on: bool) -> f32 {
        let Some(animation) = self
            .control_animations
            .iter()
            .rev()
            .find(|animation| animation.idx == idx)
        else {
            return if on { 1.0 } else { 0.0 };
        };
        let t = (self.now_ns.saturating_sub(animation.started_ns) as f32
            / animation.duration_ns.max(1) as f32)
            .clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        if animation.to_on {
            eased
        } else {
            1.0 - eased
        }
    }

    pub fn click(&mut self, x: i32, y: i32) {
        if let Some(open_idx) = self.spinner_open {
            if let Some((idx, item)) = self.spinner_popup_hit(x, y) {
                set_attr(&mut self.nodes[idx], "selectedIndex", &item.to_string());
                self.close_spinner(idx);
                self.pressed = Some(idx);
                self.focused = None;
                self.run_click_actions(idx);
                self.dirty = true;
                return;
            }
            // A native dropdown dismisses when the pointer lands outside its
            // menu. Do not leak that same click into a control underneath.
            if self.hit(x, y) != Some(open_idx) {
                self.close_spinner(open_idx);
                self.pressed = None;
                self.dirty = true;
                return;
            }
            self.close_spinner(open_idx);
            self.pressed = Some(open_idx);
            self.dirty = true;
            return;
        }
        let Some(idx) = self.hit(x, y) else {
            self.focused = None;
            self.keyboard_focus = None;
            self.pressed = None;
            self.keyboard_press_until_ns = None;
            self.dirty = true;
            return;
        };
        self.activate_index(idx, false, x);
    }

    pub(crate) fn activate_index(&mut self, idx: usize, keyboard: bool, x: i32) {
        self.pressed = Some(idx);
        self.keyboard_press_until_ns =
            keyboard.then_some(self.now_ns.saturating_add(KEYBOARD_PRESS_NS));
        self.keyboard_focus = Some(idx);
        self.last_clicked_id = Some(self.nodes[idx].id().to_string());
        if is_text_input(&self.nodes[idx]) {
            self.focused = Some(idx);
            self.dirty = true;
            return;
        }
        // Clicking another control follows normal native focus semantics: an
        // EditText loses focus as soon as a different view is activated.
        self.focused = None;
        if self.nodes[idx].is("Switch")
            || self.nodes[idx].is("CheckBox")
            || self.nodes[idx].is("RadioButton")
            || self.nodes[idx].is("ToggleButton")
        {
            if self.nodes[idx].is("RadioButton") {
                if let Some(parent) = self.nodes[idx].parent {
                    for sibling in self.nodes[parent].children.clone() {
                        if self.nodes[sibling].is("RadioButton") {
                            let old = self.nodes[sibling].attr("checked") == "true";
                            let next = sibling == idx;
                            self.start_control_animation(sibling, old, next);
                            set_attr(
                                &mut self.nodes[sibling],
                                "checked",
                                if next { "true" } else { "false" },
                            );
                        }
                    }
                } else {
                    let old = self.nodes[idx].attr("checked") == "true";
                    self.start_control_animation(idx, old, true);
                    set_attr(&mut self.nodes[idx], "checked", "true");
                }
            } else {
                let old = self.nodes[idx].attr("checked") == "true";
                let value = !old;
                if self.nodes[idx].is("Switch") {
                    self.start_control_animation(idx, old, value);
                }
                set_attr(
                    &mut self.nodes[idx],
                    "checked",
                    if value { "true" } else { "false" },
                );
            }
        } else if self.nodes[idx].is("SeekBar") {
            self.set_seek_progress(idx, x);
        } else if self.nodes[idx].is("RatingBar") {
            self.set_rating(idx, x);
        } else if self.nodes[idx].is("Spinner") {
            self.spinner_open = Some(idx);
            self.spinner_fade = None;
        }
        self.run_click_actions(idx);
        self.dirty = true;
    }

    pub fn take_clicked_id(&mut self) -> Option<String> {
        self.last_clicked_id.take()
    }

    pub fn set_text(&mut self, id: &str, text: &str) {
        if let Some(idx) = self.find(id) {
            set_attr(&mut self.nodes[idx], "text", text);
            bdf_font::preload_text(text);
            self.dirty = true;
            self.layout_dirty = true;
        }
    }

    pub fn set_visible(&mut self, id: &str, visible: bool) {
        if let Some(idx) = self.find(id) {
            set_attr(
                &mut self.nodes[idx],
                "visibility",
                if visible { "visible" } else { "gone" },
            );
            self.nodes[idx].hidden = !visible;
            self.dirty = true;
            self.layout_dirty = true;
        }
    }

    pub fn set_selected(&mut self, id: &str, selected: bool) {
        if let Some(idx) = self.find(id) {
            set_attr(
                &mut self.nodes[idx],
                "selected",
                if selected { "true" } else { "false" },
            );
            self.dirty = true;
        }
    }

    pub fn set_state_value(&mut self, key: &str, value: &str) {
        self.set_state(key, value);
        self.dirty = true;
    }

    /// Update a pointer-controlled widget while the primary pointer is held.
    /// The caller keeps the window drag state separate from this capture, just
    /// like the browser range/rating controls do.
    pub fn pointer_move(&mut self, x: i32, y: i32) -> bool {
        let Some(idx) = self.pressed else {
            return false;
        };
        if self.nodes.get(idx).map_or(true, |n| {
            !n.visible() || (!n.is("SeekBar") && !n.is("RatingBar"))
        }) {
            return false;
        }
        let changed = if self.nodes[idx].is("SeekBar") {
            self.set_seek_progress(idx, x)
        } else {
            self.set_rating(idx, x)
        };
        if changed {
            self.dirty = true;
        }
        let _ = y;
        true
    }

    pub fn has_pointer_capture(&self) -> bool {
        self.pressed.is_some_and(|idx| {
            self.nodes
                .get(idx)
                .is_some_and(|n| n.is("SeekBar") || n.is("RatingBar"))
        })
    }

    pub fn has_pressed(&self) -> bool {
        self.pressed.is_some()
    }

    pub fn release(&mut self) {
        if self.pressed.take().is_some() {
            self.keyboard_press_until_ns = None;
            self.dirty = true;
        }
    }

    pub fn handle_key(&mut self, key: u8) {
        if key == 8 || key == 127 {
            self.handle_text("", 1);
        } else if key >= 0x20 {
            let byte = [key];
            let text = unsafe { core::str::from_utf8_unchecked(&byte) };
            self.handle_text(text, 0);
        }
    }

    pub fn handle_text(&mut self, text: &str, replace_chars: usize) {
        let Some(idx) = self.focused else {
            return;
        };
        let mut value = self.nodes[idx].attr("text").to_string();
        for _ in 0..replace_chars {
            value.pop();
        }
        value.push_str(text);
        set_attr(&mut self.nodes[idx], "text", &value);
        bdf_font::preload_text(&value);
        self.dirty = true;
        self.layout_dirty = true;
    }

    pub(crate) fn hit(&self, x: i32, y: i32) -> Option<usize> {
        (0..self.nodes.len()).rev().find(|idx| {
            let n = &self.nodes[*idx];
            n.visible()
                && self.active_child(*idx)
                && (interactive(n) || self.script.clicks.iter().any(|(id, _)| id == n.id()))
                && self.hit_visible(*idx, x, y)
        })
    }

    pub(crate) fn ensure_keyboard_focus_visible(&mut self, idx: usize) {
        if idx >= self.nodes.len() {
            return;
        }
        let node = &self.nodes[idx];
        let screen_y = self.node_screen_y(idx);
        let top = self.chrome_height;
        let bottom = self.chrome_height + self.height.max(1);
        let target = if screen_y < top {
            self.scroll.saturating_add(screen_y - top)
        } else if screen_y + node.h > bottom {
            self.scroll.saturating_add(screen_y + node.h - bottom)
        } else {
            return;
        };
        self.set_scroll(target);
    }

    pub(crate) fn hit_visible(&self, idx: usize, x: i32, y: i32) -> bool {
        // The compositor gives us document coordinates (`y` already includes
        // the window scroll). Convert once to the viewport coordinate used by
        // paint, then test the control itself.
        let screen_y = y - self.scroll;
        let node = &self.nodes[idx];
        let node_y = self.node_screen_y(idx);
        if x < node.x || screen_y < node_y || x >= node.x + node.w || screen_y >= node_y + node.h {
            return false;
        }

        // Ordinary layout parents do not clip in the native painter. Only a
        // real scroll viewport is a hit-test boundary. Checking every parent
        // here makes a fixed child of a scrolled root appear unclickable once
        // the root's own document rectangle has moved off-screen.
        let mut current = node.parent;
        while let Some(i) = current {
            let viewport = &self.nodes[i];
            if is_scroll_container(viewport) {
                let viewport_y = self.node_screen_y(i);
                if x < viewport.x
                    || screen_y < viewport_y
                    || x >= viewport.x + viewport.w
                    || screen_y >= viewport_y + viewport.h
                {
                    return false;
                }
            }
            current = viewport.parent;
        }
        true
    }

    pub(crate) fn node_is_fixed(&self, idx: usize, chrome_mode: bool) -> bool {
        let node = &self.nodes[idx];
        // A screen can contain both a fixed toolbar and normal flow content.
        // Treating every non-root node outside a ScrollView as fixed makes
        // that normal content get painted twice; the second pass can cover
        // the scrolled region with the viewport background. Explicit fixed
        // controls and every declared scroll viewport belong in the fixed
        // pass. The latter is important for nested ScrollViews: their frame
        // must remain a clip window while their document moves underneath it.
        is_fixed(node) || (chrome_mode && is_scroll_container(node))
    }

    pub(crate) fn node_screen_y(&self, idx: usize) -> i32 {
        let chrome_mode = self.has_explicit_scroll();
        self.nodes[idx].y
            + if self.node_is_fixed(idx, chrome_mode) {
                0
            } else {
                -self.scroll
            }
    }

    pub(crate) fn set_seek_progress(&mut self, idx: usize, x: i32) -> bool {
        let n = &self.nodes[idx];
        let max = parse_i32(n.attr("max")).max(1);
        let next = ((x - n.x).clamp(0, n.w.max(1)) * max / n.w.max(1)).clamp(0, max);
        let current = parse_i32(n.attr("progress")).clamp(0, max);
        if current == next {
            return false;
        }
        set_attr(&mut self.nodes[idx], "progress", &next.to_string());
        true
    }

    pub(crate) fn set_rating(&mut self, idx: usize, x: i32) -> bool {
        let n = &self.nodes[idx];
        let stars = parse_i32(n.attr("numStars")).clamp(1, 10);
        let step = n.attr("stepSize").parse::<f32>().unwrap_or(1.0).max(0.1);
        let raw =
            ((x - n.x).max(0) as f32 / n.w.max(1) as f32 * stars as f32).clamp(0.0, stars as f32);
        let next = ((raw / step + 0.5) as i32).max(0) as f32 * step;
        let integral = (next + 0.5) as i32;
        let value = if (next - integral as f32).abs() < 0.001 {
            integral.to_string()
        } else {
            next.to_string()
        };
        if n.attr("rating") == value {
            return false;
        }
        set_attr(&mut self.nodes[idx], "rating", &value);
        true
    }
}
