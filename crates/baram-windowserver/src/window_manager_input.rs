impl WindowManager {
    pub fn on_mouse_down(&mut self, px: i32, py: i32) -> Option<char> {
        if let Some(id) = self.window_at(px, py) {
            if self
                .windows
                .iter()
                .find(|w| w.id == id)
                .map_or(true, |w| w.focusable)
            {
                self.focus(id);
            }
            let btn = {
                let win = self.windows.iter().find(|w| w.id == id).unwrap();
                win.button_hit(px, py)
            };
            match btn {
                'c' => {
                    self.remove(id);
                    return Some('c');
                }
                'm' => {
                    let sw = self.screen_w;
                    let sh = self.screen_h;
                    if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
                        w.toggle_maximize(sw, sh);
                    }
                    return Some('m');
                }
                'i' => {
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
                    return Some('i');
                }
                _ => {}
            }
            let resize = {
                let win = self.windows.iter().find(|w| w.id == id).unwrap();
                win.resize_handle_hit(px, py)
            };
            if resize {
                let win = self.windows.iter_mut().find(|w| w.id == id).unwrap();
                win.resizing = true;
                win.resize_sx = px;
                win.resize_sy = py;
                win.resize_sw = win.w;
                win.resize_sh = win.h;
            } else if self
                .windows
                .iter()
                .find(|w| w.id == id)
                .map_or(false, |w| w.title_bar_hit(px, py))
            {
                let win = self.windows.iter_mut().find(|w| w.id == id).unwrap();
                win.start_drag(px, py);
            }
        }
        None
    }

    pub fn on_mouse_up(&mut self) {
        for w in &mut self.windows {
            w.dragging = false;
            w.resizing = false;
            w.pending_unmaximize = false;
        }
    }

    pub fn on_mouse_drag(&mut self, px: i32, py: i32) {
        for w in &mut self.windows {
            if w.dragging {
                if w.pending_unmaximize {
                    let dx = (px - w.pending_unmax_mx).abs();
                    let dy = (py - w.pending_unmax_my).abs();
                    if dx > 2 || dy > 2 {
                        let ratio = w.pending_unmax_ratio;
                        w.w = w.save_w;
                        w.h = w.save_h;
                        w.x = px - (w.w as f64 * ratio) as i32;
                        w.y = py - 10;
                        w.maximized = false;
                        w.content_dirty = true;
                        w.shadow_dirty = true;
                        w.pending_unmaximize = false;
                        w.drag_ox = px - w.x;
                        w.drag_oy = py - w.y;
                    }
                }
                let old_x = w.x;
                let old_y = w.y;
                w.x = px - w.drag_ox;
                w.y = py - w.drag_oy;
                if w.x != old_x || w.y != old_y {
                    w.shadow_dirty = true;
                }
            }
            if w.resizing {
                let dw = px - w.resize_sx;
                let dh = py - w.resize_sy;
                let new_w = (w.resize_sw as i32 + dw).max(min_win_w() as i32) as usize;
                let new_h = (w.resize_sh as i32 + dh).max(min_win_h() as i32) as usize;
                if new_w != w.w || new_h != w.h {
                    w.w = new_w;
                    w.h = new_h;
                    w.content_dirty = true;
                    w.shadow_dirty = true;
                }
            }
        }
    }

}

