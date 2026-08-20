impl WindowManager {
    pub fn new(screen_w: usize, screen_h: usize) -> Self {
        Self {
            windows: Vec::new(),
            next_z: 0,
            next_id: 1,
            focused_id: None,
            screen_w: screen_w as i32,
            screen_h: screen_h as i32,
            shadow_cache: Vec::new(),
            temp_layer: None,
            order_changed: false,
            pending_damage: None,
            interaction_blocked: None,
            file_dialog: None,
        }
    }

    pub fn add(&mut self, title: &str, x: i32, y: i32, w: usize, h: usize) -> WinId {
        let id = WinId(self.next_id);
        self.next_id += 1;
        self.next_z += 1;
        let win = Window::new(id, title, x, y, w, h, self.next_z);
        self.windows.push(win);
        self.shadow_cache.push((id, None));
        self.focus(id);
        self.order_changed = true;
        id
    }

    pub fn open_file_dialog(&mut self, id: WinId, path: &str) {
        if self.windows.iter().any(|window| window.id == id) {
            let (width, height) = self
                .windows
                .iter()
                .find(|window| window.id == id)
                .map(|window| (window.w, window.h))
                .unwrap_or((560, 620));
            self.file_dialog = Some(NativeFileDialog::new(id, path, width, height));
            self.set_content_dirty(id);
        }
    }

    pub fn is_file_dialog(&self, id: WinId) -> bool {
        self.file_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.win_id() == id)
    }

    pub fn file_dialog_click(&mut self, id: WinId, x: i32, y: i32) -> NativeFileDialogAction {
        if let Some(dialog) = self
            .file_dialog
            .as_mut()
            .filter(|dialog| dialog.win_id() == id)
        {
            let action = dialog.click(x, y);
            self.set_content_dirty(id);
            action
        } else {
            NativeFileDialogAction::None
        }
    }

    pub fn file_dialog_selected_path(&self, id: WinId) -> Option<String> {
        self.file_dialog
            .as_ref()
            .filter(|dialog| dialog.win_id() == id)
            .and_then(NativeFileDialog::selected_path)
    }

    pub fn file_dialog_scroll(&mut self, id: WinId, delta: i32) -> bool {
        if let Some(dialog) = self
            .file_dialog
            .as_mut()
            .filter(|dialog| dialog.win_id() == id)
        {
            let changed = dialog.scroll_by(delta);
            if changed {
                self.set_content_dirty(id);
            }
            changed
        } else {
            false
        }
    }

    pub fn close_file_dialog(&mut self) {
        self.file_dialog = None;
    }

    pub fn set_warp4_theme(&mut self, id: WinId, enabled: bool) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            if w.warp4_theme != enabled {
                w.warp4_theme = enabled;
                w.content_dirty = true;
                w.shadow_dirty = true;
            }
        }
    }

    pub fn configure_special(
        &mut self,
        id: WinId,
        chrome_visible: bool,
        always_on_top: bool,
        focusable: bool,
    ) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.chrome_visible = chrome_visible;
            w.always_on_top = always_on_top;
            w.focusable = focusable;
            w.content_dirty = true;
            w.shadow_dirty = true;
        }
        if !focusable && self.focused_id == Some(id) {
            if let Some(next) = self
                .windows
                .iter()
                .filter(|w| w.focusable && w.visible && !w.minimized && w.id != id)
                .max_by_key(|w| w.z)
                .map(|w| w.id)
            {
                self.focus(next);
            }
        }
        if always_on_top {
            self.next_z += 1;
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
                w.z = self.next_z;
            }
        }
    }

    pub fn set_icon(&mut self, id: WinId, icon_name: &str) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.set_icon(icon_name);
        }
    }

    pub fn set_interaction_blocked(&mut self, id: Option<WinId>) {
        if self.interaction_blocked == id {
            return;
        }
        let old_blocked = self.interaction_blocked;
        if let Some(old_id) = old_blocked {
            self.set_content_dirty(old_id);
        }
        self.interaction_blocked = id;
        if let Some(new_id) = id {
            self.set_content_dirty(new_id);
        } else if let Some(old_id) = old_blocked {
            if self.focused_id == Some(old_id) {
                self.focus(old_id);
            }
        }
    }

    pub fn is_interaction_blocked(&self, id: WinId) -> bool {
        self.interaction_blocked == Some(id)
    }

    pub fn is_focusable(&self, id: WinId) -> bool {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .map_or(true, |w| w.focusable)
    }

    pub fn get_icon_name(&self, id: WinId) -> &str {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.icon_str())
            .unwrap_or("")
    }

    pub fn remove(&mut self, id: WinId) {
        if let Some(pos) = self.windows.iter().position(|w| w.id == id) {
            let w = &self.windows[pos];
            let pad = shadow_pad();
            let rect = (
                (w.x - pad).max(0) as usize,
                (w.y - pad).max(0) as usize,
                (w.x + w.w as i32 + pad).min(self.screen_w).max(0) as usize,
                (w.y + w.h as i32 + pad).min(self.screen_h).max(0) as usize,
            );
            self.pending_damage = Some(match self.pending_damage {
                Some(old) => (
                    old.0.min(rect.0),
                    old.1.min(rect.1),
                    old.2.max(rect.2),
                    old.3.max(rect.3),
                ),
                None => rect,
            });
            self.windows.remove(pos);
            if let Some(pos) = self.shadow_cache.iter().position(|(wid, _)| *wid == id) {
                self.shadow_cache.remove(pos);
            }
            self.order_changed = true;
        }
        if self
            .file_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.win_id() == id)
        {
            self.file_dialog = None;
        }
        if self.focused_id == Some(id) {
            self.focused_id = self.windows.last().map(|w| w.id);
            if let Some(fid) = self.focused_id {
                self.focus(fid);
            }
        }
        if self.interaction_blocked == Some(id) {
            self.interaction_blocked = None;
        }
    }

    pub fn focus(&mut self, id: WinId) {
        if self
            .windows
            .iter()
            .find(|w| w.id == id)
            .map_or(false, |w| !w.focusable)
        {
            return;
        }
        if self.interaction_blocked == Some(id) {
            return;
        }
        for w in &mut self.windows {
            if w.focused != (w.id == id) {
                w.content_dirty = true;
            }
            w.focused = w.id == id;
        }
        self.next_z += 1;
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.z = self.next_z;
        }
        self.focused_id = Some(id);
        self.order_changed = true;
    }

    pub fn scroll_focused(&mut self, delta: i32) {
        if let Some(id) = self.focused_id {
            if self.interaction_blocked == Some(id) {
                return;
            }
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
                w.scroll(delta);
            }
        }
    }

    pub fn scroll_window(&mut self, id: WinId, delta: i32) {
        if self.interaction_blocked == Some(id) {
            return;
        }
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.scroll(delta);
        }
    }

    pub fn clamp_window_scroll(&mut self, id: WinId, content_h: i32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            // Document coordinates include the title-bar offset, while the
            // viewport is clipped below it.  Using the full window height here
            // makes the final document row reachable without scrolling past it.
            w.clamp_scroll(content_h, w.h as i32);
        }
    }

    pub fn tick_scroll_animations(&mut self, now_ns: u64) -> bool {
        let mut changed = false;
        for window in &mut self.windows {
            changed |= window.tick_scroll(now_ns);
        }
        let dialog_changed = self
            .file_dialog
            .as_mut()
            .map(|dialog| dialog.tick_scroll(now_ns))
            .unwrap_or(false);
        if dialog_changed {
            if let Some(id) = self.file_dialog.as_ref().map(NativeFileDialog::win_id) {
                self.set_content_dirty(id);
            }
            changed = true;
        }
        changed
    }

    /// Advance window opening and restoration motion from the shared monotonic
    /// UI clock. Minimization itself is immediate.
    pub fn tick_window_animations(&mut self, now_ns: u64) -> bool {
        let mut changed = false;
        for w in &mut self.windows {
            changed |= w.tick_motion(now_ns);
        }
        changed
    }

    pub fn has_window_animation(&self) -> bool {
        self.windows.iter().any(Window::is_motion_animating)
    }

    pub fn is_scroll_animating(&self, id: WinId) -> bool {
        self.windows
            .iter()
            .find(|window| window.id == id)
            .map_or(false, |window| window.scroll_y != window.scroll_target_y)
    }

    pub fn has_scroll_animation(&self) -> bool {
        self.windows
            .iter()
            .any(|window| window.scroll_y != window.scroll_target_y)
    }

    pub fn window_at(&self, px: i32, py: i32) -> Option<WinId> {
        let mut best: Option<(&Window, i32)> = None;
        for w in &self.windows {
            if w.visible && w.contains(px, py) {
                match best {
                    None => best = Some((w, w.z)),
                    Some((_, best_z)) if w.z > best_z => best = Some((w, w.z)),
                    _ => {}
                }
            }
        }
        best.map(|(w, _)| w.id)
    }

    pub fn sorted_ids(&self) -> Vec<WinId> {
        let mut v: Vec<(WinId, i32)> = self.windows.iter().map(|w| (w.id, w.z)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.into_iter().map(|(id, _)| id).collect()
    }

    pub fn insertion_ids(&self) -> Vec<WinId> {
        self.windows.iter().map(|w| w.id).collect()
    }

    #[inline]
    pub fn insertion_id_at(&self, index: usize) -> Option<WinId> {
        self.windows.get(index).map(|w| w.id)
    }

}

