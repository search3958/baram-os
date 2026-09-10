impl WindowManager {
    pub fn set_content_dirty(&mut self, id: WinId) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.content_dirty = true;
            w.content_damage = None;
        }
    }

    pub fn set_content_damage(&mut self, id: WinId, x0: i32, y0: i32, x1: i32, y1: i32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            if self.interaction_blocked == Some(id) {
                w.content_dirty = true;
                w.content_damage = None;
                return;
            }
            // `content_dirty && content_damage.is_none()` means a full content
            // redraw is already pending (for example after scrolling). Never
            // downgrade it to a hover-sized patch later in the same frame.
            if w.content_dirty && w.content_damage.is_none() {
                return;
            }
            let next = (
                x0.max(0).min(w.w as i32) as usize,
                y0.max(0).min(w.h as i32) as usize,
                x1.max(0).min(w.w as i32) as usize,
                y1.max(0).min(w.h as i32) as usize,
            );
            if next.0 >= next.2 || next.1 >= next.3 {
                return;
            }
            w.content_damage = Some(match w.content_damage {
                Some(old) => (
                    old.0.min(next.0),
                    old.1.min(next.1),
                    old.2.max(next.2),
                    old.3.max(next.3),
                ),
                None => next,
            });
            w.content_dirty = true;
        }
    }

    pub fn set_window_scroll(&mut self, id: WinId, scroll: i32) {
        if let Some(window) = self.windows.iter_mut().find(|window| window.id == id) {
            let next = scroll.max(0);
            if window.scroll_target_y != next {
                if window.scroll_y == window.scroll_target_y {
                    window.scroll_start_y = window.scroll_y;
                    window.scroll_started_ns = None;
                }
                window.scroll_target_y = next;
            }
        }
    }

    pub fn set_all_dirty(&mut self) {
        for w in &mut self.windows {
            w.content_dirty = true;
            w.shadow_dirty = true;
        }
    }

    pub fn is_any_resizing(&self) -> bool {
        self.windows.iter().any(|w| w.resizing)
    }

    pub fn is_over_resize_handle(&self, px: i32, py: i32) -> bool {
        self.windows
            .iter()
            .any(|w| w.visible && w.resize_handle_hit(px, py))
    }

    pub fn count(&self) -> usize {
        self.windows.len()
    }

    pub fn take_order_changed(&mut self) -> bool {
        let v = self.order_changed;
        self.order_changed = false;
        v
    }

    pub fn get_title(&self, id: WinId) -> Option<&str> {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.title_str())
    }

    pub fn is_minimized(&self, id: WinId) -> bool {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .map_or(false, |w| w.minimized)
    }

    pub fn restore_minimized(&mut self, id: WinId) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.minimized = false;
            w.open_animating = true;
            w.motion_started_ns = None;
            w.content_dirty = true;
        }
    }

    pub fn get_window_rect(&self, id: WinId) -> Option<(i32, i32, usize, usize, i32)> {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .map(|w| (w.x, w.y, w.w, w.h, w.scroll_y))
    }

    pub fn button_hit_at(&self, id: WinId, px: i32, py: i32) -> char {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .filter(|w| w.chrome_visible)
            .map(|w| w.button_hit(px, py))
            .unwrap_or('n')
    }

    pub fn title_bar_hit_at(&self, id: WinId, px: i32, py: i32) -> bool {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.title_bar_hit(px, py))
            .unwrap_or(false)
    }

    pub fn has_pointer_capture(&self) -> bool {
        self.windows.iter().any(|w| w.dragging || w.resizing)
    }

    pub fn toggle_maximize_at(&mut self, id: WinId) {
        if self.interaction_blocked == Some(id) {
            return;
        }
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            let sw = self.screen_w;
            let sh = self.screen_h;
            w.toggle_maximize(sw, sh);
        }
    }

    pub fn toggle_minimize_at(&mut self, id: WinId) {
        if self.interaction_blocked == Some(id) {
            return;
        }
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.toggle_minimize();
        }
        if let Some(w) = self.windows.iter().find(|w| w.id == id) {
            if w.minimized {
                if let Some(next) = self
                    .windows
                    .iter()
                    .filter(|w| w.visible && !w.minimized && w.id != id)
                    .max_by_key(|w| w.z)
                {
                    self.focus(next.id);
                }
            }
        }
    }

    pub fn resize_hit_at(&self, id: WinId, px: i32, py: i32) -> bool {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.resize_handle_hit(px, py))
            .unwrap_or(false)
    }

    pub fn start_resize_at(&mut self, id: WinId, px: i32, py: i32) {
        if self.interaction_blocked == Some(id) {
            return;
        }
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.resizing = true;
            w.resize_sx = px;
            w.resize_sy = py;
            w.resize_sw = w.w;
            w.resize_sh = w.h;
        }
    }

    pub fn start_drag_at(&mut self, id: WinId, px: i32, py: i32) {
        if self.interaction_blocked == Some(id) {
            return;
        }
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.start_drag(px, py);
        }
    }

    pub fn all_window_rects(&self) -> alloc::vec::Vec<(i32, i32, usize, usize)> {
        self.windows
            .iter()
            .filter(|w| w.visible)
            .map(|w| (w.x, w.y, w.w, w.h))
            .collect()
    }

    pub fn dirty_bbox(&self, shadow_pad: i32) -> (usize, usize, usize, usize) {
        let sw = self.screen_w as usize;
        let sh = self.screen_h as usize;
        let (mut min_x, mut min_y, mut max_x, mut max_y) =
            self.pending_damage.unwrap_or((sw, sh, 0, 0));
        for w in &self.windows {
            if !w.visible
                || !(w.content_dirty
                    || w.shadow_dirty
                    || w.is_motion_animating()
                    || w.render_y_offset != w.prev_render_y_offset
                    || w.x != w.prev_x
                    || w.y != w.prev_y)
            {
                continue;
            }
            let local_damage = w.content_damage.filter(|_| {
                w.content_dirty
                    && !w.shadow_dirty
                    && !w.is_motion_animating()
                    && w.x == w.prev_x
                    && w.render_y() == w.prev_render_y()
            });
            let (x0, y0, x1, y1) = if let Some((dx0, dy0, dx1, dy1)) = local_damage {
                // Hover-only changes must not force the compositor to redraw
                // the whole window (or its shadow) on the display surface.
                (
                    (w.x + dx0 as i32).max(0) as usize,
                    (w.y + dy0 as i32).max(0) as usize,
                    (w.x + dx1 as i32).min(sw as i32).max(0) as usize,
                    (w.y + dy1 as i32).min(sh as i32).max(0) as usize,
                )
            } else {
                (
                    (w.x.min(w.prev_x) - shadow_pad).max(0) as usize,
                    (w.render_y().min(w.prev_render_y()) - shadow_pad).max(0) as usize,
                    (w.x.max(w.prev_x) + w.w.max(w.prev_w) as i32 + shadow_pad)
                        .min(sw as i32)
                        .max(0) as usize,
                    (w.render_y().max(w.prev_render_y()) + w.h.max(w.prev_h) as i32 + shadow_pad)
                        .min(sh as i32)
                        .max(0) as usize,
                )
            };
            if x0 < min_x {
                min_x = x0;
            }
            if y0 < min_y {
                min_y = y0;
            }
            if x1 > max_x {
                max_x = x1;
            }
            if y1 > max_y {
                max_y = y1;
            }
        }
        if max_x <= min_x || max_y <= min_y {
            return (0, 0, 0, 0);
        }
        (min_x, min_y, max_x, max_y)
    }

    pub fn clear_pending_damage(&mut self) {
        self.pending_damage = None;
    }
}

