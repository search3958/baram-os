use alloc::vec;
use alloc::vec::Vec;
use baram_bsd::config;
use baram_core::LayerSystem;
use baram_core::{Color, Screen};
use baram_font::LayerFontExt;
use baram_graphics::svg;

pub fn scroll_speed() -> i32 {
    config::get_i32("ui-theme/window/scroll_speed", 30)
}

pub fn title_bar_h() -> usize {
    config::get_usize("ui-theme/window/title_bar_h", 30)
}

pub fn min_win_w() -> usize {
    config::get_usize("ui-theme/window/min_win_w", 120)
}

pub fn min_win_h() -> usize {
    config::get_usize("ui-theme/window/min_win_h", 60)
}

pub fn btn_size() -> usize {
    config::get_usize("ui-theme/button/size", 20)
}

pub fn btn_area_w() -> usize {
    btn_size() * 3 + 23
}

pub fn win_radius() -> usize {
    config::get_usize("ui-theme/window/win_radius", 16)
}

pub fn taskbar_h() -> usize {
    config::get_usize("ui-theme/taskbar/h", 48)
}

pub fn shadow_pad() -> i32 {
    config::get_i32("ui-theme/window/shadow_pad", 30)
}

pub struct RoundedShadow {
    layer: LayerSystem,
    pad: i32,
}

impl RoundedShadow {
    pub fn new(w: usize, h: usize, radius: usize) -> Option<Self> {
        let pad = shadow_pad().max(0);
        let (alpha, sw, sh) = compute_rounded_shadow_alpha(w, h, radius, pad)?;
        let mut layer = LayerSystem::new_transparent(sw, sh);
        for (dst, a) in layer.buf_mut().iter_mut().zip(alpha.iter()) {
            *dst = *a as u32;
        }
        Some(Self { layer, pad })
    }

    pub fn composite_onto(&self, dst: &mut LayerSystem, x: i32, y: i32) {
        let shadow_x = x - self.pad;
        let shadow_y = y - self.pad;
        let src_x = (-shadow_x).max(0) as usize;
        let src_y = (-shadow_y).max(0) as usize;
        let dst_x = shadow_x.max(0) as usize;
        let dst_y = shadow_y.max(0) as usize;
        let draw_w = self.layer.width().saturating_sub(src_x);
        let draw_h = self.layer.height().saturating_sub(src_y);
        if draw_w > 0 && draw_h > 0 {
            dst.composit_shadow_alpha(
                &self.layer,
                dst_x,
                dst_y,
                src_x,
                src_y,
                draw_w,
                draw_h,
            );
        }
    }
}

pub fn btn_bg_radius() -> usize {
    config::get_usize("ui-theme/button/radius", 8)
}

pub fn btn_bg_color() -> Color {
    config::get_color("ui-theme/color/btn_bg", Color::BTN_BG)
}

const MAX_ICON_SVG: &str = include_str!("../../../data/max.svg");
const MINI_ICON_SVG: &str = include_str!("../../../data/mini.svg");
const CLOSE_ICON_SVG: &str = include_str!("../../../data/close.svg");
const MIN_ICON_SVG: &str = include_str!("../../../data/min.svg");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WinId(pub u32);

pub struct Window {
    pub id: WinId,
    pub title: [u8; 24],
    pub title_len: usize,
    pub icon_name: [u8; 16],
    pub icon_name_len: usize,
    pub x: i32,
    pub y: i32,
    pub w: usize,
    pub h: usize,
    pub z: i32,
    pub visible: bool,
    pub focused: bool,
    pub maximized: bool,
    pub minimized: bool,
    pub scroll_y: i32,
    prev_x: i32,
    prev_y: i32,
    prev_w: usize,
    prev_h: usize,
    save_x: i32,
    save_y: i32,
    save_w: usize,
    save_h: usize,
    dragging: bool,
    pub(crate) resizing: bool,
    drag_ox: i32,
    drag_oy: i32,
    resize_sx: i32,
    resize_sy: i32,
    resize_sw: usize,
    resize_sh: usize,
    pub layer: Option<LayerSystem>,
    pub shadow_layer: Option<LayerSystem>,
    pub content_dirty: bool,
    /// Local window coordinates. `None` means the entire layer must be rebuilt.
    pub content_damage: Option<(usize, usize, usize, usize)>,
    pub shadow_dirty: bool,
    pub open_progress: f32,
    pub open_animating: bool,
    pending_unmaximize: bool,
    pending_unmax_ratio: f64,
    pending_unmax_mx: i32,
    pending_unmax_my: i32,
}

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
            maximized: false,
            minimized: false,
            scroll_y: 0,
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
            open_progress: 0.0,
            open_animating: true,
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
        px >= self.x
            && px < self.x + self.w as i32
            && py >= self.y
            && py < self.y + title_bar_h() as i32
    }

    fn button_hit(&self, px: i32, py: i32) -> char {
        let base_x = self.x + 6;
        let btn_y = self.y + 5;
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
        self.scroll_y = self.scroll_y.saturating_add(delta).max(0);
        self.content_dirty = true;
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
        } else {
            self.minimized = true;
        }
        self.content_dirty = true;
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
        if self.scroll_y > max {
            self.scroll_y = max;
            self.content_dirty = true;
        }
    }
}

struct CachedShadow {
    win_x: i32,
    win_y: i32,
    win_w: usize,
    win_h: usize,
    alpha: Vec<u8>,
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
}

pub struct WindowManager {
    windows: Vec<Window>,
    next_z: i32,
    next_id: u32,
    pub focused_id: Option<WinId>,
    screen_w: i32,
    screen_h: i32,
    shadow_cache: Vec<(WinId, Option<CachedShadow>)>,
    temp_layer: Option<LayerSystem>,
    order_changed: bool,
    pending_damage: Option<(usize, usize, usize, usize)>,
}

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

    pub fn set_icon(&mut self, id: WinId, icon_name: &str) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.set_icon(icon_name);
        }
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
        if self.focused_id == Some(id) {
            self.focused_id = self.windows.last().map(|w| w.id);
            if let Some(fid) = self.focused_id {
                self.focus(fid);
            }
        }
    }

    pub fn focus(&mut self, id: WinId) {
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
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
                w.scroll(delta);
            }
        }
    }

    pub fn scroll_window(&mut self, id: WinId, delta: i32) {
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

    pub fn on_mouse_down(&mut self, px: i32, py: i32) -> Option<char> {
        if let Some(id) = self.window_at(px, py) {
            self.focus(id);
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
            } else {
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

    pub fn draw_all(
        &mut self,
        layer: &mut LayerSystem,
        ui_win: Option<(WinId, &[baram_graphics::uiscript::Command])>,
        warp_engines: &mut alloc::vec::Vec<(WinId, super::warp::WarpEngine)>,
        html_engines: &mut alloc::vec::Vec<(WinId, super::html::HtmlEngine)>,
    ) {
        if self.windows.is_empty() {
            return;
        }

        let n = self.windows.len();
        let screen_w = layer.width();
        let screen_h = layer.height();

        const MAX_WINDOWS: usize = 16;
        let sort_n = n.min(MAX_WINDOWS);
        let mut indices = [0usize; MAX_WINDOWS];
        for i in 0..sort_n {
            indices[i] = i;
        }
        for i in 1..sort_n {
            let mut j = i;
            while j > 0 && self.windows[indices[j - 1]].z > self.windows[indices[j]].z {
                indices.swap(j - 1, j);
                j -= 1;
            }
        }

        for i in 0..sort_n {
            let idx = indices[i];
            let w = &self.windows[idx];
            if !w.visible || w.minimized || w.maximized {
                continue;
            }
            let entry = self.shadow_cache.iter_mut().find(|(wid2, _)| *wid2 == w.id);
            if let Some((_, ref mut cache_opt)) = entry {
                let need_recompute = match cache_opt {
                    Some(c) => c.win_w != w.w || c.win_h != w.h,
                    None => true,
                };
                if need_recompute {
                    *cache_opt = compute_shadow_alpha(w, self.screen_w, self.screen_h);
                }
                if let Some(ref mut c) = cache_opt {
                    c.win_x = w.x;
                    c.win_y = w.y;
                }
            }
        }

        for i in 0..sort_n {
            let idx = indices[i];
            if !self.windows[idx].visible || self.windows[idx].minimized {
                continue;
            }
            self.windows[idx].ensure_layer(screen_w, screen_h);

            let wx = self.windows[idx].x;
            let wy = self.windows[idx].y;
            let ww = self.windows[idx].w;
            let wh = self.windows[idx].h;
            let scroll_y = self.windows[idx].scroll_y;
            let win_id = self.windows[idx].id;
            let is_max = self.windows[idx].maximized;
            let shadow_dirty = self.windows[idx].shadow_dirty;
            let content_dirty = self.windows[idx].content_dirty;
            let open_progress = self.windows[idx].open_progress;
            let open_animating = self.windows[idx].open_animating;
            if open_animating {
                self.windows[idx].open_progress = (open_progress + 0.04).min(1.0);
                if self.windows[idx].open_progress >= 1.0 {
                    self.windows[idx].open_animating = false;
                }
            }

            if !is_max {
                if shadow_dirty {
                    if let Some(entry) = self.shadow_cache.iter().find(|(wid2, _)| *wid2 == win_id)
                    {
                        if let Some(ref cache) = entry.1 {
                            let old_sx = (self.windows[idx].prev_x - shadow_pad()).max(0) as usize;
                            let old_sy = (self.windows[idx].prev_y - shadow_pad()).max(0) as usize;
                            let new_sx = (self.windows[idx].x - shadow_pad()).max(0) as usize;
                            let new_sy = (self.windows[idx].y - shadow_pad()).max(0) as usize;
                            let shadow_layer = self.windows[idx].shadow_layer.as_mut().unwrap();
                            let slw = shadow_layer.width();
                            let slh = shadow_layer.height();
                            let scx0 = old_sx.min(new_sx);
                            let scy0 = old_sy.min(new_sy);
                            let scx1 = (old_sx + cache.w).max(new_sx + cache.w).min(slw);
                            let scy1 = (old_sy + cache.h).max(new_sy + cache.h).min(slh);
                            if scx1 > scx0 && scy1 > scy0 {
                                for row in scy0..scy1 {
                                    let start = row * slw + scx0;
                                    let end = row * slw + scx1;
                                    shadow_layer.buf_mut()[start..end].fill(Color::TRANSPARENT.0);
                                }
                            }
                            let shadow_buf = shadow_layer.buf_mut();
                            for py in 0..cache.h {
                                let alpha_row = py * cache.w;
                                for px in 0..cache.w {
                                    let a = cache.alpha[alpha_row + px];
                                    if a == 0 {
                                        continue;
                                    }
                                    if px >= slw || py >= slh {
                                        continue;
                                    }
                                    shadow_buf[py * slw + px] = 0x0000_0000 | (a as u32);
                                }
                            }
                            self.windows[idx].shadow_dirty = false;
                        }
                    }
                }

                if let Some(entry) = self.shadow_cache.iter().find(|(wid2, _)| *wid2 == win_id) {
                    if entry.1.is_some() {
                        let shadow_ref = self.windows[idx].shadow_layer.as_ref().unwrap();
                        let shadow_size = ww + shadow_pad() as usize * 2;
                        let shadow_h = wh + shadow_pad() as usize * 2;
                        let shadow_x = wx - shadow_pad() as i32;
                        let shadow_y = wy - shadow_pad() as i32;

                        let src_x = if shadow_x < 0 { (-shadow_x) as usize } else { 0 };
                        let src_y = if shadow_y < 0 { (-shadow_y) as usize } else { 0 };
                        let dst_x = shadow_x.max(0) as usize;
                        let dst_y = shadow_y.max(0) as usize;
                        let draw_w = (shadow_size as i32 - src_x as i32).max(0) as usize;
                        let draw_h = (shadow_h as i32 - src_y as i32).max(0) as usize;

                        if draw_w > 0 && draw_h > 0 {
                            layer.composit_shadow_alpha(
                                shadow_ref,
                                dst_x,
                                dst_y,
                                src_x,
                                src_y,
                                draw_w,
                                draw_h,
                            );
                        }
                    }
                }
            }

            if content_dirty {
                let layer_ptr = self.windows[idx].layer.as_mut().unwrap() as *mut LayerSystem;
                let w_ptr = &self.windows[idx] as *const Window;
                let damage = self.windows[idx].content_damage.take();
                unsafe {
                    let lw = (*layer_ptr).width();
                    let lh = (*layer_ptr).height();
                    let (cx0, cy0, cx1, cy1) = damage.unwrap_or((0, 0, lw, lh));
                    if cx1 > cx0 && cy1 > cy0 {
                        for row in cy0..cy1 {
                            let start = row * lw + cx0;
                            let end = row * lw + cx1;
                            (*layer_ptr).buf_mut()[start..end].fill(Color::TRANSPARENT.0);
                        }
                    }

                    (*layer_ptr).push_clip(cx0, cy0, cx1, cy1);

                    // A Warp3 hover patch owns every pixel in its damage rect.
                    // Do not enter generic window chrome/body rendering here:
                    // some SVG/font paths are not damage-clip aware and would
                    // touch title-bar pixels outside the hovered control.
                    if damage.is_none() {
                        if is_max {
                            draw_window(&mut *layer_ptr, &*w_ptr, 0, 0);
                        } else {
                            draw_window_body(&mut *layer_ptr, &*w_ptr, true, 0, 0);
                        }
                    }

                    if let Some((uid, cmds)) = ui_win {
                        if win_id == uid {
                            (*layer_ptr).push_clip(0, title_bar_h(), ww, wh);
                            let card_radius = config::get_usize("ui-theme/card/radius", 12);
                            baram_graphics::uiscript::render(
                                &mut *layer_ptr,
                                cmds,
                                0,
                                0,
                                ww,
                                wh,
                                title_bar_h(),
                                scroll_y,
                                card_radius,
                            );
                            (*layer_ptr).pop_clip();
                        }
                    }
                    for i in 0..warp_engines.len() {
                        if win_id == warp_engines[i].0 {
                            let engine = &mut warp_engines[i].1;
                            (*layer_ptr).push_clip(0, title_bar_h(), ww, wh);
                            engine.draw_to_layer(&mut *layer_ptr, 0, -scroll_y);
                            engine.draw_texts(&mut *layer_ptr, 0, -scroll_y, 1.0);
                            (*layer_ptr).pop_clip();
                            break;
                        }
                    }
                    for i in 0..html_engines.len() {
                        if win_id == html_engines[i].0 {
                            let engine = &mut html_engines[i].1;
                            (*layer_ptr).push_clip(0, title_bar_h(), ww, wh);
                            engine.draw_to_layer(&mut *layer_ptr, 0, -scroll_y);
                            (*layer_ptr).pop_clip();
                            break;
                        }
                    }
                    // Font glyph antialiasing is blended into the destination.
                    // Never redraw the title during a body-only hover patch:
                    // its glyph writer is intentionally not in the body clip.
                    if damage.is_none() {
                        draw_title_bar(&mut *layer_ptr, &*w_ptr, 0, 0);
                    }
                    (*layer_ptr).pop_clip();
                }
                self.windows[idx].prev_x = self.windows[idx].x;
                self.windows[idx].prev_y = self.windows[idx].y;
                self.windows[idx].content_dirty = false;
            }

            let win_layer = self.windows[idx].layer.as_ref().unwrap();
            let screen_w = layer.width() as i32;
            let screen_h = layer.height() as i32;

            let src_x = if wx < 0 { (-wx) as usize } else { 0 };
            let src_y = if wy < 0 { (-wy) as usize } else { 0 };
            let dst_x = wx.max(0) as usize;
            let dst_y = wy.max(0) as usize;
            let draw_w = (ww as i32 - src_x as i32).max(0) as usize;
            let draw_h = (wh as i32 - src_y as i32).max(0) as usize;

            if draw_w == 0 || draw_h == 0 {
                continue;
            }

            if is_max {
                layer.composit_rect(
                    win_layer,
                    dst_x,
                    dst_y,
                    src_x,
                    src_y,
                    draw_w,
                    draw_h,
                );
            } else {
                layer.composit_rounded(win_layer, dst_x, dst_y, src_x, src_y, draw_w, draw_h, win_radius());
                draw_window_border(layer, &self.windows[idx]);
            }
            self.windows[idx].prev_x = self.windows[idx].x;
            self.windows[idx].prev_y = self.windows[idx].y;
            self.windows[idx].prev_w = self.windows[idx].w;
            self.windows[idx].prev_h = self.windows[idx].h;
        }
    }

    pub fn set_content_dirty(&mut self, id: WinId) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.content_dirty = true;
            w.content_damage = None;
        }
    }

    pub fn set_content_damage(&mut self, id: WinId, x0: i32, y0: i32, x1: i32, y1: i32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            let next = (
                x0.max(0).min(w.w as i32) as usize,
                y0.max(0).min(w.h as i32) as usize,
                x1.max(0).min(w.w as i32) as usize,
                y1.max(0).min(w.h as i32) as usize,
            );
            if next.0 >= next.2 || next.1 >= next.3 { return; }
            w.content_damage = Some(match w.content_damage {
                Some(old) => (old.0.min(next.0), old.1.min(next.1), old.2.max(next.2), old.3.max(next.3)),
                None => next,
            });
            w.content_dirty = true;
        }
    }

    pub fn set_window_scroll(&mut self, id: WinId, scroll: i32) {
        if let Some(window) = self.windows.iter_mut().find(|window| window.id == id) {
            window.scroll_y = scroll.max(0);
            window.content_dirty = true;
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
            .map(|w| w.button_hit(px, py))
            .unwrap_or('n')
    }

    pub fn toggle_maximize_at(&mut self, id: WinId) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            let sw = self.screen_w;
            let sh = self.screen_h;
            w.toggle_maximize(sw, sh);
        }
    }

    pub fn toggle_minimize_at(&mut self, id: WinId) {
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
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.resizing = true;
            w.resize_sx = px;
            w.resize_sy = py;
            w.resize_sw = w.w;
            w.resize_sh = w.h;
        }
    }

    pub fn start_drag_at(&mut self, id: WinId, px: i32, py: i32) {
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
                    || w.open_animating
                    || w.x != w.prev_x
                    || w.y != w.prev_y)
            {
                continue;
            }
            let local_damage = w.content_damage.filter(|_| {
                w.content_dirty && !w.shadow_dirty && !w.open_animating
                    && w.x == w.prev_x && w.y == w.prev_y
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
                    (w.y.min(w.prev_y) - shadow_pad).max(0) as usize,
                    (w.x.max(w.prev_x) + w.w.max(w.prev_w) as i32 + shadow_pad)
                        .min(sw as i32).max(0) as usize,
                    (w.y.max(w.prev_y) + w.h.max(w.prev_h) as i32 + shadow_pad)
                        .min(sh as i32).max(0) as usize,
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

fn compute_shadow_alpha(w: &Window, _screen_w: i32, _screen_h: i32) -> Option<CachedShadow> {
    let pad = shadow_pad().max(0);
    let (alpha, sw, sh) = compute_rounded_shadow_alpha(w.w, w.h, win_radius(), pad)?;

    Some(CachedShadow {
        win_x: w.x,
        win_y: w.y,
        win_w: w.w,
        win_h: w.h,
        alpha,
        x0: pad as usize,
        y0: pad as usize,
        w: sw,
        h: sh,
    })
}

fn compute_rounded_shadow_alpha(
    width: usize,
    height: usize,
    radius: usize,
    pad: i32,
) -> Option<(Vec<u8>, usize, usize)> {
    let blur_r = pad;
    let r = radius.min(width / 2).min(height / 2) as i32;
    let ww = width as i32;
    let wh = height as i32;
    let sw = (ww + blur_r * 2).max(0) as usize;
    let sh = (wh + blur_r * 2).max(0) as usize;
    if sw == 0 || sh == 0 {
        return None;
    }

    let mut alpha = alloc::vec![0u8; sw * sh];
    let left = blur_r.max(0) as usize;
    let top = blur_r.max(0) as usize;
    let right = left + width;
    let bottom = top + height;
    let radius = r as usize;
    for py in top..bottom {
        for px in left..right {
            let dx = if px < left + radius { left as i32 + r - px as i32 }
                else if px >= right - radius { px as i32 - (right as i32 - r - 1) } else { 0 };
            let dy = if py < top + radius { top as i32 + r - py as i32 }
                else if py >= bottom - radius { py as i32 - (bottom as i32 - r - 1) } else { 0 };
            if dx == 0 || dy == 0 || dx * dx + dy * dy <= r * r {
                alpha[py * sw + px] = 45;
            }
        }
    }
    let box_radius = (blur_r.max(1) as usize / 3).max(1);
    for _ in 0..3 {
        box_blur_shadow(&mut alpha, sw, sh, box_radius);
    }

    Some((alpha, sw, sh))
}

fn box_blur_shadow(alpha: &mut [u8], width: usize, height: usize, radius: usize) {
    let mut tmp = alloc::vec![0u8; alpha.len()];
    let diameter = radius * 2 + 1;
    for y in 0..height {
        let mut sum = 0u32;
        for x in 0..width + radius {
            if x < width { sum += alpha[y * width + x] as u32; }
            if x > diameter && x - diameter - 1 < width { sum -= alpha[y * width + x - diameter - 1] as u32; }
            if x >= radius && x - radius < width { tmp[y * width + x - radius] = (sum / diameter as u32) as u8; }
        }
    }
    for x in 0..width {
        let mut sum = 0u32;
        for y in 0..height + radius {
            if y < height { sum += tmp[y * width + x] as u32; }
            if y > diameter && y - diameter - 1 < height { sum -= tmp[(y - diameter - 1) * width + x] as u32; }
            if y >= radius && y - radius < height { alpha[(y - radius) * width + x] = (sum / diameter as u32) as u8; }
        }
    }
}

fn draw_title_bar(layer: &mut LayerSystem, w: &Window, ox: i32, oy: i32) {
    let x = ox.max(0) as usize;
    let y = oy.max(0) as usize;
    let sw = layer.width();
    let sh = layer.height();
    if x >= sw || y >= sh {
        return;
    }
    let x1 = (x + w.w).min(sw);
    let y1 = (y + w.h).min(sh);
    let w_draw = x1.saturating_sub(x);
    let h_draw = y1.saturating_sub(y);
    if w_draw == 0 || h_draw == 0 {
        return;
    }

    let (title_bg, _) = if w.focused {
        (
            config::get_color("ui-theme/color/panel", Color::PANEL),
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
        )
    } else {
        (
            config::get_color("ui-theme/color/win_inactive", Color::WIN_INACTIVE),
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
        )
    };

    let tb_h = title_bar_h().min(h_draw);
    layer.fill_rect(x, y, w_draw, tb_h, title_bg);

    let base_x = x as i32 + 6;
    let btn_y = y as i32 + 5;
    let bs = btn_size() as i32;
    let btn_center_x = base_x + bs / 2;
    let btn_center_y = btn_y + bs / 2;

    if btn_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            btn_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    let mini_x = base_x + bs + 5;
    let mini_center_x = mini_x + bs / 2;

    if mini_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            mini_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    let max_x = base_x + bs * 2 + 10;
    let max_center_x = max_x + bs / 2;

    if max_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            max_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    if w.focused {
        if base_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            svg::draw_svg_into_alpha(
                layer,
                CLOSE_ICON_SVG,
                base_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }

        if mini_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            svg::draw_svg_into_alpha(
                layer,
                MIN_ICON_SVG,
                mini_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }

        if max_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            let icon = if w.maximized {
                MINI_ICON_SVG
            } else {
                MAX_ICON_SVG
            };
            svg::draw_svg_into_alpha(
                layer,
                icon,
                max_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }

        let title = w.title_str();
        if !title.is_empty() {
            let title_x = (base_x + bs * 3 + 20) as usize;
            let title_y = (y as i32 + 8) as usize;
            if title_x < sw && title_y < sh {
                layer.put_str(title_x, title_y, title, Color::TEXT);
            }
        }
    }
}

fn draw_window_body(layer: &mut LayerSystem, w: &Window, rounded: bool, ox: i32, oy: i32) {
    let x = ox.max(0) as usize;
    let y = oy.max(0) as usize;
    let sw = layer.width();
    let sh = layer.height();
    if x >= sw || y >= sh {
        return;
    }
    let x1 = (x + w.w).min(sw);
    let y1 = (y + w.h).min(sh);
    let w_draw = x1.saturating_sub(x);
    let h_draw = y1.saturating_sub(y);
    if w_draw == 0 || h_draw == 0 {
        return;
    }

    let (title_bg, body_bg) = if w.focused {
        (
            config::get_color("ui-theme/color/panel", Color::PANEL),
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
        )
    } else {
        (
            config::get_color("ui-theme/color/win_inactive", Color::WIN_INACTIVE),
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
        )
    };
    let title_color = if w.focused {
        config::get_color("ui-theme/color/text", Color::TEXT)
    } else {
        config::get_color("ui-theme/color/win_inactive", Color::WIN_INACTIVE)
    };

    if rounded {
        layer.fill_rounded_rect(x, y, w_draw, h_draw, win_radius(), body_bg);
    } else {
        layer.fill_rect(x, y, w_draw, h_draw, body_bg);
    }

    let tb_h = title_bar_h().min(h_draw);
    layer.fill_rect(x, y, w_draw, tb_h, title_bg);

    let base_x = x as i32 + 6;
    let btn_y = y as i32 + 5;
    let bs = btn_size() as i32;
    let btn_center_x = base_x + bs / 2;
    let btn_center_y = btn_y + bs / 2;

    if btn_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            btn_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    let mini_x = base_x + bs + 5;
    let mini_center_x = mini_x + bs / 2;

    if mini_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            mini_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    let max_x = base_x + bs * 2 + 10;
    let max_center_x = max_x + bs / 2;

    if max_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            max_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    if w.focused {
        if base_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            svg::draw_svg_into_alpha(
                layer,
                CLOSE_ICON_SVG,
                base_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }

        if mini_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            svg::draw_svg_into_alpha(
                layer,
                MIN_ICON_SVG,
                mini_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }

        if max_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            let icon = if w.maximized {
                MINI_ICON_SVG
            } else {
                MAX_ICON_SVG
            };
            svg::draw_svg_into_alpha(
                layer,
                icon,
                max_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }
    }

    layer.put_str(x + btn_area_w(), y + 8, w.title_str(), title_color);
}

fn draw_window_border(_layer: &mut LayerSystem, _w: &Window) {}

fn draw_window(layer: &mut LayerSystem, w: &Window, ox: i32, oy: i32) {
    draw_window_body(layer, w, false, ox, oy);
    draw_window_border(layer, w);
}
