impl Window {
    fn new(id: WinId, title: &str, x: i32, y: i32, w: usize, h: usize, z: i32) -> Self {
        let mut tb = [0u8; 24];
        let src = title.as_bytes();
        let n = src.len().min(23);
        tb[..n].copy_from_slice(&src[..n]);
        Self {
            id,
            title: tb,
            title_len: n,
            icon_name: [0u8; 16],
            icon_name_len: 0,
            x,
            y,
            w,
            h,
            z,
            visible: true,
            focused: false,
            warp4_theme: false,
            chrome_visible: true,
            always_on_top: false,
            focusable: true,
            maximized: false,
            minimized: false,
            scroll_y: 0,
            scroll_start_y: 0,
            scroll_target_y: 0,
            scroll_started_ns: None,
            prev_x: x,
            prev_y: y,
            prev_w: w,
            prev_h: h,
            save_x: x,
            save_y: y,
            save_w: w,
            save_h: h,
            dragging: false,
            resizing: false,
            drag_ox: 0,
            drag_oy: 0,
            resize_sx: 0,
            resize_sy: 0,
            resize_sw: 0,
            resize_sh: 0,
            layer: Some(LayerSystem::new_transparent(w, h)),
            shadow_layer: Some(LayerSystem::new_transparent(
                w + shadow_pad() as usize * 2,
                h + shadow_pad() as usize * 2,
            )),
            content_dirty: true,
            content_damage: None,
            shadow_dirty: true,
            open_animating: true,
            motion_started_ns: None,
            render_y_offset: WINDOW_MOTION_OFFSET_Y,
            prev_render_y_offset: WINDOW_MOTION_OFFSET_Y,
            pending_unmaximize: false,
            pending_unmax_ratio: 0.0,
            pending_unmax_mx: 0,
            pending_unmax_my: 0,
        }
    }

    fn title_str(&self) -> &str {
        core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")
    }

    fn icon_str(&self) -> &str {
        core::str::from_utf8(&self.icon_name[..self.icon_name_len]).unwrap_or("")
    }

    pub fn set_icon(&mut self, name: &str) {
        let src = name.as_bytes();
        let n = src.len().min(15);
        self.icon_name[..n].copy_from_slice(&src[..n]);
        self.icon_name_len = n;
    }

    fn ensure_layer(&mut self, screen_w: usize, screen_h: usize) {
        let need_w = self.w.min(screen_w);
        let need_h = self.h.min(screen_h);
        match &self.layer {
            Some(l) if l.width() == need_w && l.height() == need_h => {}
            _ => {
                self.layer = Some(LayerSystem::new_transparent(need_w, need_h));
            }
        }
        let sw = need_w + shadow_pad() as usize * 2;
        let sh = need_h + shadow_pad() as usize * 2;
        match &self.shadow_layer {
            Some(l) if l.width() == sw && l.height() == sh => {}
            _ => {
                self.shadow_layer = Some(LayerSystem::new_transparent(sw, sh));
            }
        }
    }

    fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.w as i32 && py >= self.y && py < self.y + self.h as i32
    }

    fn title_bar_hit(&self, px: i32, py: i32) -> bool {
        self.chrome_visible
            && px >= self.x
            && px < self.x + self.w as i32
            && py >= self.y
            && py < self.y + title_bar_h() as i32
    }

    fn button_hit(&self, px: i32, py: i32) -> char {
        let base_x = self.x + 10;
        let btn_y = self.y + 10;
        let bs = btn_size() as i32;
        if py >= btn_y && py < btn_y + bs {
            if px >= base_x && px < base_x + bs {
                return 'c';
            }
            if px >= base_x + bs + 5 && px < base_x + bs * 2 + 5 {
                return 'i';
            }
            if px >= base_x + bs * 2 + 10 && px < base_x + bs * 3 + 10 {
                return 'm';
            }
        }
        'n'
    }

    fn resize_handle_hit(&self, px: i32, py: i32) -> bool {
        let hw = 12i32;
        px >= self.x + self.w as i32 - hw
            && px < self.x + self.w as i32
            && py >= self.y + self.h as i32 - hw
            && py < self.y + self.h as i32
    }

    pub fn scroll(&mut self, delta: i32) {
        let next = self.scroll_target_y.saturating_add(delta).max(0);
        if self.scroll_target_y != next {
            // Continuous trackpad input extends the active destination. Do
            // not restart its clock for every event or motion can starve.
            if self.scroll_y == self.scroll_target_y {
                self.scroll_start_y = self.scroll_y;
                self.scroll_started_ns = None;
            }
            self.scroll_target_y = next;
        }
    }

    fn tick_scroll(&mut self, now_ns: u64) -> bool {
        if self.scroll_y == self.scroll_target_y {
            self.scroll_started_ns = None;
            return false;
        }
        // Give a newly queued scroll its first 1 ms sample immediately. This
        // avoids a visually stationary first frame under bursty input.
        let started = *self
            .scroll_started_ns
            .get_or_insert(now_ns.saturating_sub(1_000_000));
        let elapsed = now_ns.saturating_sub(started);
        let t = (elapsed as f32 / SCROLL_ANIMATION_NS as f32).clamp(0.0, 1.0);
        let eased = decelerate_scroll(t);
        let distance = self.scroll_target_y - self.scroll_start_y;
        let next = if t >= 1.0 {
            self.scroll_target_y
        } else {
            self.scroll_start_y + (distance as f32 * eased) as i32
        };
        if next == self.scroll_y {
            return false;
        }
        self.scroll_y = next;
        self.content_dirty = true;
        self.content_damage = None;
        if t >= 1.0 {
            self.scroll_started_ns = None;
        }
        true
    }

    pub fn toggle_maximize(&mut self, screen_w: i32, screen_h: i32) {
        if self.maximized {
            self.x = self.save_x;
            self.y = self.save_y;
            self.w = self.save_w;
            self.h = self.save_h;
            self.maximized = false;
        } else {
            self.save_x = self.x;
            self.save_y = self.y;
            self.save_w = self.w;
            self.save_h = self.h;
            self.x = 0;
            self.y = 0;
            self.w = screen_w as usize;
            self.h = (screen_h - taskbar_h() as i32) as usize;
            self.maximized = true;
        }
        self.content_dirty = true;
        self.shadow_dirty = true;
    }

    pub fn toggle_minimize(&mut self) {
        if self.minimized {
            self.minimized = false;
            self.open_animating = true;
        } else {
            self.minimized = true;
            self.open_animating = false;
            self.render_y_offset = 0;
        }
        self.motion_started_ns = None;
        self.content_dirty = true;
    }

    fn is_motion_animating(&self) -> bool {
        self.open_animating
    }

    fn render_y(&self) -> i32 {
        self.y + self.render_y_offset
    }

    fn prev_render_y(&self) -> i32 {
        self.prev_y + self.prev_render_y_offset
    }

    fn tick_motion(&mut self, now_ns: u64) -> bool {
        let was_animating = self.is_motion_animating();
        if !was_animating {
            return false;
        }
        let started = *self.motion_started_ns.get_or_insert(now_ns);
        let t = (now_ns.saturating_sub(started) as f32 / WINDOW_OPEN_DURATION_NS as f32)
            .clamp(0.0, 1.0);
        let old_offset = self.render_y_offset;
        // Ease out: opening starts briskly and settles into place.
        let remaining = 1.0 - t;
        self.render_y_offset =
            (WINDOW_MOTION_OFFSET_Y as f32 * remaining * remaining * remaining) as i32;
        if t >= 1.0 {
            self.open_animating = false;
        }
        old_offset != self.render_y_offset || was_animating
    }

    pub fn start_drag(&mut self, px: i32, py: i32) {
        if self.maximized {
            self.pending_unmaximize = true;
            self.pending_unmax_ratio = px as f64 / self.w as f64;
            self.pending_unmax_mx = px;
            self.pending_unmax_my = py;
        }
        self.dragging = true;
        self.drag_ox = px - self.x;
        self.drag_oy = py - self.y;
    }

    pub fn clamp_scroll(&mut self, content_h: i32, visible_h: i32) {
        let max = (content_h - visible_h).max(0);
        self.scroll_target_y = self.scroll_target_y.min(max);
        if self.scroll_y > max {
            self.scroll_y = max;
            self.scroll_start_y = max;
            self.scroll_started_ns = None;
            self.content_dirty = true;
            self.content_damage = None;
        }
    }
}

