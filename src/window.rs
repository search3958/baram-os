use alloc::vec;
use alloc::vec::Vec;
use crate::gop::{Color, Screen};
use crate::svg;

const TITLE_BAR_H: usize = 30;
const MIN_WIN_W: usize = 120;
const MIN_WIN_H: usize = 60;
const BTN_SIZE: usize = 20;
const BTN_AREA_W: usize = BTN_SIZE * 3 + 23;
const WIN_RADIUS: usize = 16;
const TASKBAR_H: usize = 48;
const SHADOW_PAD: i32 = 30;
const BTN_BG_RADIUS: usize = 8;
const BTN_BG_COLOR: Color = Color::rgb(216, 216, 216);

const MAX_ICON_SVG: &str = include_str!("data/max.svg");
const MINI_ICON_SVG: &str = include_str!("data/mini.svg");
const CLOSE_ICON_SVG: &str = include_str!("data/close.svg");
const MIN_ICON_SVG: &str = include_str!("data/min.svg");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WinId(pub u32);

pub struct Window {
    pub id: WinId,
    pub title: [u8; 24],
    pub title_len: usize,
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
    save_x: i32,
    save_y: i32,
    save_w: usize,
    save_h: usize,
    dragging: bool,
    resizing: bool,
    drag_ox: i32,
    drag_oy: i32,
    resize_sx: i32,
    resize_sy: i32,
    resize_sw: usize,
    resize_sh: usize,
    pub layer: Option<LayerSystem>,
    pub shadow_layer: Option<LayerSystem>,
}

impl Window {
    fn new(id: WinId, title: &str, x: i32, y: i32, w: usize, h: usize, z: i32) -> Self {
        let mut tb = [0u8; 24];
        let src = title.as_bytes();
        let n = src.len().min(23);
        tb[..n].copy_from_slice(&src[..n]);
        Self {
            id, title: tb, title_len: n,
            x, y, w, h, z,
            visible: true, focused: false, maximized: false, minimized: false,
            scroll_y: 0,
            save_x: x, save_y: y, save_w: w, save_h: h,
            dragging: false, resizing: false,
            drag_ox: 0, drag_oy: 0,
            resize_sx: 0, resize_sy: 0, resize_sw: 0, resize_sh: 0,
            layer: Some(LayerSystem::new_transparent(w, h)),
            shadow_layer: Some(LayerSystem::new_transparent(w + SHADOW_PAD as usize * 2, h + SHADOW_PAD as usize * 2)),
        }
    }

    fn title_str(&self) -> &str {
        core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")
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
        let sw = need_w + SHADOW_PAD as usize * 2;
        let sh = need_h + SHADOW_PAD as usize * 2;
        match &self.shadow_layer {
            Some(l) if l.width() == sw && l.height() == sh => {}
            _ => {
                self.shadow_layer = Some(LayerSystem::new_transparent(sw, sh));
            }
        }
    }

    fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.w as i32
            && py >= self.y && py < self.y + self.h as i32
    }

    fn title_bar_hit(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.w as i32
            && py >= self.y && py < self.y + TITLE_BAR_H as i32
    }

    fn button_hit(&self, px: i32, py: i32) -> char {
        let base_x = self.x + 6;
        let btn_y = self.y + 5;
        let bs = BTN_SIZE as i32;
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
            self.h = (screen_h - TASKBAR_H as i32) as usize;
            self.maximized = true;
        }
    }

    pub fn toggle_minimize(&mut self) {
        if self.minimized {
            self.minimized = false;
        } else {
            self.minimized = true;
        }
    }

    pub fn start_drag(&mut self, px: i32, py: i32) {
        if self.maximized {
            let ratio = px as f64 / self.w as f64;
            self.w = self.save_w;
            self.h = self.save_h;
            self.x = px - (self.w as f64 * ratio) as i32;
            self.y = py - 10;
            self.maximized = false;
        }
        self.dragging = true;
        self.drag_ox = px - self.x;
        self.drag_oy = py - self.y;
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

    pub fn remove(&mut self, id: WinId) {
        if let Some(pos) = self.windows.iter().position(|w| w.id == id) {
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
        let mut v: Vec<(WinId, i32)> = self.windows.iter()
            .map(|w| (w.id, w.z))
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.into_iter().map(|(id, _)| id).collect()
    }

    pub fn insertion_ids(&self) -> Vec<WinId> {
        self.windows.iter().map(|w| w.id).collect()
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
                            if let Some(next) = self.windows.iter()
                                .filter(|w| w.visible && !w.minimized && w.id != id)
                                .max_by_key(|w| w.z) {
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
        }
    }

    pub fn on_mouse_drag(&mut self, px: i32, py: i32) {
        for w in &mut self.windows {
            if w.dragging {
                w.x = px - w.drag_ox;
                w.y = py - w.drag_oy;
            }
            if w.resizing {
                let dw = px - w.resize_sx;
                let dh = py - w.resize_sy;
                w.w = (w.resize_sw as i32 + dw).max(MIN_WIN_W as i32) as usize;
                w.h = (w.resize_sh as i32 + dh).max(MIN_WIN_H as i32) as usize;
            }
        }
    }

    
    
    
        pub fn draw_all(
        &mut self,
        layer: &mut LayerSystem,
        ui_win: Option<(WinId, &[super::uiscript::Command])>,
        warp_win: Option<(WinId, &mut super::warp::WarpEngine)>,
    ) {
        if self.windows.is_empty() {
            return;
        }

        let n = self.windows.len();
        let screen_w = layer.width();
        let screen_h = layer.height();

        // ---- z-order 昇順（下→上）のインデックスリストを作成 ----
        let mut indices: Vec<usize> = (0..n).collect();
        indices.sort_by_key(|&i| self.windows[i].z);

        // ---- シャドウキャッシュの更新 ----
        for &idx in &indices {
            let w = &self.windows[idx];
            if !w.visible || w.minimized || w.maximized {
                continue;
            }
            let entry = self
                .shadow_cache
                .iter_mut()
                .find(|(wid2, _)| *wid2 == w.id);
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

        // ---- z-order 順（下→上）で描画 ----
        for &idx in &indices {
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

            // ===== シャドウ描画（各ウィンドウの shadow_layer に独立描画 → main layer へ合成）=====
            if !is_max {
                if let Some(entry) = self
                    .shadow_cache
                    .iter()
                    .find(|(wid2, _)| *wid2 == win_id)
                {
                    if let Some(ref cache) = entry.1 {
                        // shadow_layer へシャドウアルファ値を書き込む
                        {
                            let shadow_layer =
                                self.windows[idx].shadow_layer.as_mut().unwrap();
                            let slw = shadow_layer.width();
                            let slh = shadow_layer.height();
                            shadow_layer.buf_mut()[..slw * slh].fill(Color::TRANSPARENT.0);
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
                        }

                        // shadow_layer を main layer に合成（暗くする）
                        let shadow_ref =
                            self.windows[idx].shadow_layer.as_ref().unwrap();
                        // 画面外にはみ出す部分をクリップした座標を計算
                        let src_x = (SHADOW_PAD - wx).max(0) as usize;
                        let src_y = (SHADOW_PAD - wy).max(0) as usize;
                        let dst_x = (wx - SHADOW_PAD).max(0) as usize;
                        let dst_y = (wy - SHADOW_PAD).max(0) as usize;
                        layer.composit_shadow_alpha(
                            shadow_ref,
                            dst_x,
                            dst_y,
                            src_x,
                            src_y,
                            ww + SHADOW_PAD as usize * 2,
                            wh + SHADOW_PAD as usize * 2,
                        );
                    }
                }
            }

            // ===== ウィンドウ本体描画（各ウィンドウの layer に独立描画）=====
            {
                let layer_ptr =
                    self.windows[idx].layer.as_mut().unwrap() as *mut LayerSystem;
                let w_ptr = &self.windows[idx] as *const Window;
                unsafe {
                    let lw = (*layer_ptr).width();
                    let lh = (*layer_ptr).height();
                    (*layer_ptr).buf_mut()[..lw * lh].fill(Color::TRANSPARENT.0);

                    if is_max {
                        draw_window(&mut *layer_ptr, &*w_ptr, 0, 0);
                    } else {
                        draw_window_body(&mut *layer_ptr, &*w_ptr, true, 0, 0);
                    }

                    if let Some((uid, cmds)) = ui_win {
                        if win_id == uid {
                            (*layer_ptr).push_clip(0, TITLE_BAR_H, ww, wh);
                            super::uiscript::render(
                                &mut *layer_ptr,
                                cmds,
                                0,
                                0,
                                ww,
                                wh,
                                TITLE_BAR_H,
                                scroll_y,
                            );
                            (*layer_ptr).pop_clip();
                        }
                    }
                    if let Some((wid, ref engine)) = warp_win {
                        if win_id == wid {
                            (*layer_ptr).push_clip(0, TITLE_BAR_H, ww, wh);
                            engine.draw_to_layer(&mut *layer_ptr, 0, -scroll_y);
                            engine.draw_texts(&mut *layer_ptr, 0, -scroll_y, 1.0);
                            (*layer_ptr).pop_clip();
                        }
                    }
                }
            }

            // ===== ウィンドウ本体を main layer に合成 =====
            let win_layer = self.windows[idx].layer.as_ref().unwrap();
            if is_max {
                layer.composit_rect(
                    win_layer,
                    wx.max(0) as usize,
                    wy.max(0) as usize,
                    0,
                    0,
                    ww,
                    wh,
                );
            } else {
                let wx_usize = wx.max(0) as usize;
                let wy_usize = wy.max(0) as usize;
                layer.composit_rounded(
                    win_layer,
                    wx_usize,
                    wy_usize,
                    0,
                    0,
                    ww,
                    wh,
                    WIN_RADIUS,
                );
                draw_window_border(layer, &self.windows[idx]);
            }
        }
    }

    pub fn is_any_resizing(&self) -> bool {
        self.windows.iter().any(|w| w.resizing)
    }

    pub fn is_over_resize_handle(&self, px: i32, py: i32) -> bool {
        self.windows.iter().any(|w| w.visible && w.resize_handle_hit(px, py))
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
        self.windows.iter().find(|w| w.id == id).map(|w| w.title_str())
    }

    pub fn is_minimized(&self, id: WinId) -> bool {
        self.windows.iter().find(|w| w.id == id).map_or(false, |w| w.minimized)
    }

    pub fn restore_minimized(&mut self, id: WinId) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.minimized = false;
        }
    }

    pub fn get_window_rect(&self, id: WinId) -> Option<(i32, i32, usize, usize, i32)> {
        self.windows.iter().find(|w| w.id == id).map(|w| (w.x, w.y, w.w, w.h, w.scroll_y))
    }

    pub fn all_window_rects(&self) -> alloc::vec::Vec<(i32, i32, usize, usize)> {
        self.windows.iter().filter(|w| w.visible).map(|w| (w.x, w.y, w.w, w.h)).collect()
    }

    pub fn dirty_bbox(&self, shadow_pad: i32) -> (usize, usize, usize, usize) {
        let sw = self.screen_w as usize;
        let sh = self.screen_h as usize;
        if self.windows.is_empty() {
            return (0, 0, sw, sh);
        }
        let mut min_x = sw;
        let mut min_y = sh;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        for w in &self.windows {
            if !w.visible { continue; }
            let x0 = (w.x - shadow_pad).max(0) as usize;
            let y0 = (w.y - shadow_pad).max(0) as usize;
            let x1 = (w.x + w.w as i32 + shadow_pad).min(sw as i32) as usize;
            let y1 = (w.y + w.h as i32 + shadow_pad).min(sh as i32) as usize;
            if x0 < min_x { min_x = x0; }
            if y0 < min_y { min_y = y0; }
            if x1 > max_x { max_x = x1; }
            if y1 > max_y { max_y = y1; }
        }
        if max_x <= min_x || max_y <= min_y {
            return (0, 0, sw, sh);
        }
        (min_x, min_y, max_x, max_y)
    }
}


fn compute_shadow_alpha(w: &Window, _screen_w: i32, _screen_h: i32) -> Option<CachedShadow> {
    let blur_r: i32 = 30;
    let r = WIN_RADIUS as f32;
    let ww = w.w as i32;
    let wh = w.h as i32;
    let pad = blur_r as usize;
    let sw = (ww + blur_r * 2) as usize;
    let sh = (wh + blur_r * 2) as usize;
    if sw == 0 || sh == 0 { return None; }

    let blur_r_f = blur_r as f32;
    let mut alpha = Vec::with_capacity(sw * sh);

    // SDF計算用の事前準備
    let hww = ww as f32 / 2.0;
    let hwh = wh as f32 / 2.0;
    let center_x = hww;
    let center_y = hwh;
    // コーナーの円の起点となる内部矩形のサイズ
    let box_w = hww - r;
    let box_h = hwh - r;

    for py_i in 0..sh as i32 {
        let py_f = (py_i - blur_r) as f32 + 0.5;
        for px_i in 0..sw as i32 {
            let px_f = (px_i - blur_r) as f32 + 0.5;

            // 中心からの絶対値（第1象限に折りたたむ）
            let qx = (px_f - center_x).abs();
            let qy = (py_f - center_y).abs();

            // 内部矩形からの距離
            let dx = qx - box_w;
            let dy = qy - box_h;

            // 角丸矩形のSDFによる正確な距離計算
            let dist = if dx > 0.0 && dy > 0.0 {
                libm::sqrtf(dx * dx + dy * dy)
            } else {
                dx.max(dy)
            };

            // 最終的なエッジからの距離（0.0が境界、> 0.0が外側）
            let edge_dist = dist - r;

            // ウィンドウの内部（edge_dist <= 0.0）、またはブラー半径の外側
            if edge_dist <= 0.0 || edge_dist >= blur_r_f {
                alpha.push(0u8);
            } else {
                let t = (blur_r_f - edge_dist) / blur_r_f;
                let alpha_f = t * t * 0.175;
                alpha.push((alpha_f * 255.0) as u8);
            }
        }
    }

    Some(CachedShadow {
        win_x: w.x, win_y: w.y, win_w: w.w, win_h: w.h,
        alpha, x0: pad, y0: pad, w: sw, h: sh,
    })
}

fn blit_cached_shadow(layer: &mut LayerSystem, cache: &CachedShadow) {
    let buf = &mut layer.buf;
    let lw = layer.width;
    let lh = layer.height;
    let blur_r = 30i32;
    let screen_x0 = (cache.win_x - blur_r).max(0) as usize;
    let screen_y0 = (cache.win_y - blur_r).max(0) as usize;
    let x_off = (blur_r - cache.win_x).max(0) as usize;
    let y_off = (blur_r - cache.win_y).max(0) as usize;
    for py in y_off..cache.h {
        let dst_y = screen_y0 + (py - y_off);
        if dst_y >= lh { break; }
        let row_start = dst_y * lw + screen_x0;
        let alpha_row = py * cache.w;
        for px in x_off..cache.w {
            let dst_x = screen_x0 + (px - x_off);
            if dst_x >= lw { break; }
            let a = cache.alpha[alpha_row + px] as u32;
            if a == 0 { continue; }
            let inv = 255 - a;
            let bg = Color(buf[row_start + (px - x_off)]);
            let cr = (bg.r() as u32 * inv) / 255;
            let cg = (bg.g() as u32 * inv) / 255;
            let cb = (bg.b() as u32 * inv) / 255;
            buf[row_start + (px - x_off)] = Color::rgb(cr as u8, cg as u8, cb as u8).0;
        }
    }
}

fn draw_shadow(layer: &mut LayerSystem, w: &Window) {
    let blur_r: i32 = 30;
    let r = WIN_RADIUS as f32;
    let sw = layer.width() as i32;
    let sh = layer.height() as i32;
    
    // クリップ前の実際のウィンドウ座標とサイズを使用する
    let win_x0 = w.x;
    let win_y0 = w.y;
    let ww = w.w as i32;
    let wh = w.h as i32;
    let win_x1 = win_x0 + ww;
    let win_y1 = win_y0 + wh;

    // 描画範囲（画面外に出ないようにクリップ）
    let sx = (win_x0 - blur_r).max(0).min(sw) as usize;
    let sy = (win_y0 - blur_r).max(0).min(sh) as usize;
    let ex = (win_x1 + blur_r).max(0).min(sw) as usize;
    let ey = (win_y1 + blur_r).max(0).min(sh) as usize;
    if ex <= sx || ey <= sy { return; }

    let buf = &mut layer.buf;
    let width = layer.width;
    let blur_r_f = blur_r as f32;
    
    // SDF計算用の事前準備
    let hww = ww as f32 / 2.0;
    let hwh = wh as f32 / 2.0;
    let center_x = win_x0 as f32 + hww;
    let center_y = win_y0 as f32 + hwh;
    let box_w = hww - r;
    let box_h = hwh - r;

    for py in sy..ey {
        let py_f = py as f32 + 0.5;
        let row_start = py * width;
        for px in sx..ex {
            let px_f = px as f32 + 0.5;
            
            let qx = (px_f - center_x).abs();
            let qy = (py_f - center_y).abs();
            
            let dx = qx - box_w;
            let dy = qy - box_h;
            
            let dist = if dx > 0.0 && dy > 0.0 {
                libm::sqrtf(dx * dx + dy * dy)
            } else {
                dx.max(dy)
            };
            
            let edge_dist = dist - r;
            
            if edge_dist <= 0.0 || edge_dist >= blur_r_f {
                continue;
            }
            
            let t = (blur_r_f - edge_dist) / blur_r_f;
            let alpha_f = t * t * 0.175;
            let a = (alpha_f * 255.0) as u32;
            if a == 0 { continue; }
            
            let inv = 255 - a;
            let idx = row_start + px;
            let bg = Color(buf[idx]);
            let cr = (bg.r() as u32 * inv) / 255;
            let cg = (bg.g() as u32 * inv) / 255;
            let cb = (bg.b() as u32 * inv) / 255;
            buf[idx] = Color::rgb(cr as u8, cg as u8, cb as u8).0;
        }
    }
}

fn draw_window_body(layer: &mut LayerSystem, w: &Window, rounded: bool, ox: i32, oy: i32) {
    let x = ox.max(0) as usize;
    let y = oy.max(0) as usize;
    let sw = layer.width();
    let sh = layer.height();
    if x >= sw || y >= sh { return; }
    let x1 = (x + w.w).min(sw);
    let y1 = (y + w.h).min(sh);
    let w_draw = x1.saturating_sub(x);
    let h_draw = y1.saturating_sub(y);
    if w_draw == 0 || h_draw == 0 { return; }

    let (title_bg, body_bg) = if w.focused {
        (Color::ACCENT, Color::WIN_BG)
    } else {
        (Color::WIN_INACTIVE, Color::WIN_BG)
    };
    let title_color = if w.focused { Color::TEXT } else { Color::TITLE_INACTIVE };

    
    if rounded {
        layer.fill_rounded_rect(x, y, w_draw, h_draw, WIN_RADIUS, body_bg);
    } else {
        layer.fill_rect(x, y, w_draw, h_draw, body_bg);
    }

    
    let tb_h = TITLE_BAR_H.min(h_draw);
    layer.fill_rect(x, y, w_draw, tb_h, title_bg);

    
    let base_x = x as i32 + 6;
    let btn_y = y as i32 + 5;
    let bs = BTN_SIZE as i32;
    let btn_center_x = base_x + bs / 2;
    let btn_center_y = btn_y + bs / 2;

    if btn_center_x + BTN_BG_RADIUS as i32 <= sw as i32 && btn_center_y + BTN_BG_RADIUS as i32 <= sh as i32 {
        layer.fill_circle(btn_center_x as usize, btn_center_y as usize, BTN_BG_RADIUS, BTN_BG_COLOR);
    }

    let mini_x = base_x + bs + 5;
    let mini_center_x = mini_x + bs / 2;

    if mini_center_x + BTN_BG_RADIUS as i32 <= sw as i32 && btn_center_y + BTN_BG_RADIUS as i32 <= sh as i32 {
        layer.fill_circle(mini_center_x as usize, btn_center_y as usize, BTN_BG_RADIUS, BTN_BG_COLOR);
    }

    let max_x = base_x + bs * 2 + 10;
    let max_center_x = max_x + bs / 2;

    if max_center_x + BTN_BG_RADIUS as i32 <= sw as i32 && btn_center_y + BTN_BG_RADIUS as i32 <= sh as i32 {
        layer.fill_circle(max_center_x as usize, btn_center_y as usize, BTN_BG_RADIUS, BTN_BG_COLOR);
    }

    if w.focused {
        if base_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            svg::draw_svg_into_alpha(layer, CLOSE_ICON_SVG,
                base_x + 4, btn_y + 4,
                (BTN_SIZE - 8) as f32, (BTN_SIZE - 8) as f32,
                77u32);
        }

        if mini_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            svg::draw_svg_into_alpha(layer, MIN_ICON_SVG,
                mini_x + 4, btn_y + 4,
                (BTN_SIZE - 8) as f32, (BTN_SIZE - 8) as f32,
                77u32);
        }

        if max_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            let icon = if w.maximized { MINI_ICON_SVG } else { MAX_ICON_SVG };
            svg::draw_svg_into_alpha(layer, icon,
                max_x + 4, btn_y + 4,
                (BTN_SIZE - 8) as f32, (BTN_SIZE - 8) as f32,
                77u32);
        }
    }

    
    layer.put_str(x + BTN_AREA_W, y + 8, w.title_str(), title_color);
}

fn draw_window_border(layer: &mut LayerSystem, w: &Window) {
}

fn draw_window(layer: &mut LayerSystem, w: &Window, ox: i32, oy: i32) {
    draw_window_body(layer, w, false, ox, oy);
    draw_window_border(layer, w);
}





pub struct LayerSystem {
    width: usize,
    height: usize,
    buf: Vec<u32>,
    frame_count: u64,
    clip_stack: Vec<(usize, usize, usize, usize)>,
    clip: Option<(usize, usize, usize, usize)>,
}

impl LayerSystem {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            width: w,
            height: h,
            buf: vec![Color::BG.0; w * h],
            frame_count: 0,
            clip_stack: Vec::new(),
            clip: None,
        }
    }

    pub fn new_transparent(w: usize, h: usize) -> Self {
        Self {
            width: w,
            height: h,
            buf: vec![Color::TRANSPARENT.0; w * h],
            frame_count: 0,
            clip_stack: Vec::new(),
            clip: None,
        }
    }

    pub fn push_clip(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        let x0 = x0.min(self.width);
        let y0 = y0.min(self.height);
        let x1 = x1.min(self.width);
        let y1 = y1.min(self.height);
        
        if let Some(cur) = self.clip {
            self.clip_stack.push(cur);
            
            self.clip = Some((
                x0.max(cur.0),
                y0.max(cur.1),
                x1.min(cur.2),
                y1.min(cur.3),
            ));
        } else {
            self.clip = Some((x0, y0, x1, y1));
        }
    }

    pub fn pop_clip(&mut self) {
        if let Some(prev) = self.clip_stack.pop() {
            self.clip = Some(prev);
        } else {
            self.clip = None;
        }
    }

    #[inline]
    fn clip_test(&self, x: usize, y: usize) -> bool {
        if let Some((cx0, cy0, cx1, cy1)) = self.clip {
            x >= cx0 && x < cx1 && y >= cy0 && y < cy1
        } else {
            true
        }
    }

    pub fn clear(&mut self, c: Color) {
        self.buf.fill(c.0);
    }

    #[inline]
    pub fn put_pixel(&mut self, x: usize, y: usize, c: Color) {
        if x < self.width && y < self.height && self.clip_test(x, y) {
            self.buf[y * self.width + x] = c.0;
        }
    }

    #[allow(dead_code)]
    pub fn get_pixel(&self, x: usize, y: usize) -> Color {
        if x < self.width && y < self.height {
            Color(self.buf[y * self.width + x])
        } else {
            Color::BLACK
        }
    }

    
    #[inline]
    pub fn buf_mut(&mut self) -> &mut [u32] {
        &mut self.buf
    }

    #[inline]
    pub fn buf_ref(&self) -> &[u32] {
        &self.buf
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: Color) {
        let v = c.0;
        let stride = self.width;
        if let Some((cx0, cy0, cx1, cy1)) = self.clip {
            let x0 = x.max(cx0).min(stride);
            let y0 = y.max(cy0).min(self.height);
            let x1 = (x + w).min(cx1).min(stride);
            let y1 = (y + h).min(cy1).min(self.height);
            if x0 >= x1 || y0 >= y1 { return; }
            for yy in y0..y1 {
                self.buf[yy * stride + x0..yy * stride + x1].fill(v);
            }
        } else {
            let x0 = x.min(stride);
            let y0 = y.min(self.height);
            let x1 = (x + w).min(stride);
            let y1 = (y + h).min(self.height);
            if x0 >= x1 || y0 >= y1 { return; }
            for yy in y0..y1 {
                self.buf[yy * stride + x0..yy * stride + x1].fill(v);
            }
        }
    }

    
    pub fn fill_rounded_rect(&mut self, x: usize, y: usize, w: usize, h: usize, r: usize, c: Color) {
        if w == 0 || h == 0 { return; }
        let r = r.min(w / 2).min(h / 2);
        let rf = r as f32;
        let cr = c.r() as f32;
        let cg = c.g() as f32;
        let cb = c.b() as f32;
        let y0 = y.min(self.height);
        let y1 = (y + h).min(self.height);
        let x0 = x.min(self.width);
        let x1 = (x + w).min(self.width);
        let v = c.0;
        let stride = self.width;

        for py in y0..y1 {
            let row = py * stride;
            if r == 0 {
                self.buf[row + x0..row + x1].fill(v);
                continue;
            }
            let corner_top = py < y + r;
            let corner_bot = py >= y + h.saturating_sub(r);
            if !corner_top && !corner_bot {
                self.buf[row + x0..row + x1].fill(v);
                continue;
            }
            for px in x0..x1 {
                let in_corner = (px < x + r && corner_top)
                    || (px >= x + w.saturating_sub(r) && corner_top)
                    || (px < x + r && corner_bot)
                    || (px >= x + w.saturating_sub(r) && corner_bot);
                if !in_corner {
                    self.buf[row + px] = v;
                    continue;
                }

                let cx_f = if px < x + r { x + r } else { x + w - r } as f32;
                let cy_f = if corner_top { y + r } else { y + h - r } as f32;
                let dx = px as f32 + 0.5 - cx_f;
                let dy = py as f32 + 0.5 - cy_f;
                let dist_sq = dx * dx + dy * dy;
                let alpha = if dist_sq < (rf - 0.5) * (rf - 0.5) {
                    1.0
                } else if dist_sq > (rf + 0.5) * (rf + 0.5) {
                    0.0
                } else {
                    let dist = libm::sqrtf(dist_sq);
                    (rf + 0.5 - dist).clamp(0.0, 1.0)
                };

                if alpha > 0.0 {
                    if alpha >= 1.0 {
                        self.buf[row + px] = v;
                    } else {
                        let bg = self.buf[row + px];
                        let br = ((bg >> 16) & 0xFF) as f32;
                        let bg2 = ((bg >> 8) & 0xFF) as f32;
                        let bb = (bg & 0xFF) as f32;
                        let r2 = (cr * alpha + br * (1.0 - alpha)) as u32;
                        let g = (cg * alpha + bg2 * (1.0 - alpha)) as u32;
                        let b = (cb * alpha + bb * (1.0 - alpha)) as u32;
                        self.buf[row + px] = Color::rgb(r2 as u8, g as u8, b as u8).0;
                    }
                }
            }
        }
    }

    pub fn fill_circle(&mut self, cx: usize, cy: usize, r: usize, c: Color) {
        if r == 0 { return; }
        let rf = r as f32;
        let cr = c.r() as f32;
        let cg = c.g() as f32;
        let cb = c.b() as f32;
        let x0 = cx.saturating_sub(r).min(self.width);
        let y0 = cy.saturating_sub(r).min(self.height);
        let x1 = (cx + r + 1).min(self.width);
        let y1 = (cy + r + 1).min(self.height);
        for py in y0..y1 {
            let row = py * self.width;
            for px in x0..x1 {
                let dx = px as f32 + 0.5 - cx as f32;
                let dy = py as f32 + 0.5 - cy as f32;
                let dist_sq = dx * dx + dy * dy;
                let alpha = if dist_sq < (rf - 0.5) * (rf - 0.5) {
                    1.0
                } else if dist_sq > (rf + 0.5) * (rf + 0.5) {
                    continue;
                } else {
                    let dist = libm::sqrtf(dist_sq);
                    (rf + 0.5 - dist).clamp(0.0, 1.0)
                };
                if alpha >= 1.0 {
                    self.buf[row + px] = c.0;
                } else {
                    let bg = self.buf[row + px];
                    let br = ((bg >> 16) & 0xFF) as f32;
                    let bg2 = ((bg >> 8) & 0xFF) as f32;
                    let bb = (bg & 0xFF) as f32;
                    let r2 = (cr * alpha + br * (1.0 - alpha)) as u32;
                    let g = (cg * alpha + bg2 * (1.0 - alpha)) as u32;
                    let b = (cb * alpha + bb * (1.0 - alpha)) as u32;
                    self.buf[row + px] = Color::rgb(r2 as u8, g as u8, b as u8).0;
                }
            }
        }
    }

    
    pub fn rounded_rect_outline(&mut self, x: usize, y: usize, w: usize, h: usize, r: usize, c: Color) {
        if w == 0 || h == 0 { return; }
        let r = r.min(w / 2).min(h / 2);
        let rf = r as f32;
        let cr = c.r() as f32;
        let cg = c.g() as f32;
        let cb = c.b() as f32;
        let y0 = y.min(self.height);
        let y1 = (y + h).min(self.height);
        let x0 = x.min(self.width);
        let x1 = (x + w).min(self.width);

        for py in y0..y1 {
            for px in x0..x1 {
                let on_edge = px == x || px == x + w - 1 || py == y || py == y + h - 1;
                if !on_edge { continue; }

                let dist_to_edge = if px < x + r && py < y + r {
                    let cx_f = (x + r) as f32;
                    let cy_f = (y + r) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else if px >= x + w.saturating_sub(r) && py < y + r && r > 0 {
                    let cx_f = (x + w - r) as f32;
                    let cy_f = (y + r) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else if px < x + r && py >= y + h.saturating_sub(r) && r > 0 {
                    let cx_f = (x + r) as f32;
                    let cy_f = (y + h - r) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else if px >= x + w.saturating_sub(r) && py >= y + h.saturating_sub(r) && r > 0 {
                    let cx_f = (x + w - r) as f32;
                    let cy_f = (y + h - r) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else {
                    
                    self.put_pixel(px, py, c);
                    continue;
                };

                let alpha = if dist_to_edge < -0.5 {
                    0.0
                } else if dist_to_edge > 0.5 {
                    0.0
                } else {
                    (0.5 - dist_to_edge.abs()).clamp(0.0, 1.0)
                };

                if alpha > 0.0 {
                    let bg = self.buf[py * self.width + px];
                    let br = ((bg >> 16) & 0xFF) as f32;
                    let bg2 = ((bg >> 8) & 0xFF) as f32;
                    let bb = (bg & 0xFF) as f32;
                    let r2 = (cr * alpha + br * (1.0 - alpha)) as u32;
                    let g = (cg * alpha + bg2 * (1.0 - alpha)) as u32;
                    let b = (cb * alpha + bb * (1.0 - alpha)) as u32;
                    self.put_pixel(px, py, Color::rgb(r2 as u8, g as u8, b as u8));
                }
            }
        }
    }

    pub fn rect_outline(&mut self, x: usize, y: usize, w: usize, h: usize, c: Color) {
        if w == 0 || h == 0 { return; }
        self.fill_rect(x, y, w, 1, c);
        self.fill_rect(x, y + h - 1, w, 1, c);
        self.fill_rect(x, y, 1, h, c);
        self.fill_rect(x + w - 1, y, 1, h, c);
    }

    pub fn put_char(&mut self, x: usize, y: usize, ch: char, fg: Color) {
        if crate::ttf_font::is_available() && ch as u32 >= 0x20 {
            let glyph = crate::ttf_font::glyph(ch);
            if glyph.w > 0 && glyph.h > 0 {
                let baseline = y as i32 + crate::ttf_font::ascent();
                for row in 0..glyph.h {
                    let py = baseline + glyph.y_off + row;
                    if py < 0 || py >= self.height as i32 { continue; }
                    for col in 0..glyph.w {
                        let px = x as i32 + col;
                        if px < 0 || px >= self.width as i32 { continue; }
                        if !self.clip_test(px as usize, py as usize) { continue; }
                        let alpha = glyph.data[(row * glyph.w + col) as usize];
                        if alpha > 0 {
                            let a = alpha as u32;
                            let bg = self.buf[py as usize * self.width + px as usize];
                            let br = (bg >> 16) & 0xFF;
                            let bg2 = (bg >> 8) & 0xFF;
                            let bb = bg & 0xFF;
                            let fr = (fg.0 >> 16) & 0xFF;
                            let fg2 = (fg.0 >> 8) & 0xFF;
                            let fb = fg.0 & 0xFF;
                            let r = (fr * a + br * (255 - a)) / 255;
                            let g = (fg2 * a + bg2 * (255 - a)) / 255;
                            let b = (fb * a + bb * (255 - a)) / 255;
                            self.buf[py as usize * self.width + px as usize] = (r << 16) | (g << 8) | b;
                        }
                    }
                }
                return;
            }
        }
        if (ch as u32) < 0x20 || (ch as u32) > 0x7E { return; }
        use crate::font::{self, GLYPH_W, GLYPH_H};
        let glyph = font::glyph(ch as u8);
        for row in 0..GLYPH_H {
            let bits = glyph[row];
            let py = y + row;
            if py >= self.height { break; }
            for col in 0..GLYPH_W {
                if (bits >> (7 - col)) & 1 == 1 {
                    let px = x + col;
                    if px < self.width && self.clip_test(px, py) {
                        self.buf[py * self.width + px] = fg.0;
                    }
                }
            }
        }
    }

    pub fn put_str(&mut self, mut x: usize, y: usize, s: &str, fg: Color) {
        if crate::ttf_font::is_available() {
            for ch in s.chars() {
                let glyph = crate::ttf_font::glyph(ch);
                if glyph.w > 0 && glyph.h > 0 {
                    self.put_char(x, y, ch, fg);
                    x += glyph.advance.max(0) as usize;
                } else if (ch as u32) < 0x80 {
                    self.put_char(x, y, ch, fg);
                    x += crate::font::GLYPH_W;
                }
            }
            return;
        }
        use crate::font::GLYPH_W;
        for &b in s.as_bytes() {
            if b >= 0x80 { break; }
            self.put_char(x, y, b as char, fg);
            x += GLYPH_W;
        }
    }

    pub fn put_str_hud(&mut self, mut x: usize, y: usize, s: &str, fg: Color) {
        if crate::ttf_font_hud::is_available() {
            for ch in s.chars() {
                let glyph = crate::ttf_font_hud::glyph(ch);
                if glyph.w > 0 && glyph.h > 0 {
                    let baseline = y as i32 + crate::ttf_font_hud::ascent();
                    for row in 0..glyph.h {
                        let py = baseline + glyph.y_off + row;
                        if py < 0 || py >= self.height as i32 { continue; }
                        for col in 0..glyph.w {
                            let px = x as i32 + col;
                            if px < 0 || px >= self.width as i32 { continue; }
                            if !self.clip_test(px as usize, py as usize) { continue; }
                            let alpha = glyph.data[(row * glyph.w + col) as usize];
                            if alpha > 0 {
                                let a = alpha as u32;
                                let bg = self.buf[py as usize * self.width + px as usize];
                                let br = (bg >> 16) & 0xFF;
                                let bg2 = (bg >> 8) & 0xFF;
                                let bb = bg & 0xFF;
                                let fr = (fg.0 >> 16) & 0xFF;
                                let fg2 = (fg.0 >> 8) & 0xFF;
                                let fb = fg.0 & 0xFF;
                                let r = (fr * a + br * (255 - a)) / 255;
                                let g = (fg2 * a + bg2 * (255 - a)) / 255;
                                let b = (fb * a + bb * (255 - a)) / 255;
                                self.buf[py as usize * self.width + px as usize] = (r << 16) | (g << 8) | b;
                            }
                        }
                    }
                    x += glyph.advance.max(0) as usize;
                } else if (ch as u32) < 0x80 {
                    self.put_char(x, y, ch, fg);
                    x += crate::font::GLYPH_W;
                }
            }
            return;
        }
        self.put_str(x, y, s, fg);
    }

    pub fn flush(&mut self, screen: &mut Screen) {
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            let row = &self.buf[y * w..(y + 1) * w];
            screen.flush_layer_row(y, row);
        }
        self.frame_count += 1;
    }

    pub fn flush_rect(&self, screen: &mut Screen, x0: usize, y0: usize, x1: usize, y1: usize) {
        let w = self.width;
        let y0 = y0.min(self.height);
        let y1 = y1.min(self.height);
        let x0 = x0.min(w);
        let x1 = x1.min(w);
        for y in y0..y1 {
            let row = &self.buf[y * w + x0..y * w + x1];
            screen.flush_layer_row_range(y, x0, row);
        }
    }

    pub fn composit_rounded(
        &mut self,
        src: &LayerSystem,
        dx: usize, dy: usize,
        sx: usize, sy: usize,
        w: usize, h: usize,
        r: usize,
    ) {
        let r = r.min(w / 2).min(h / 2);
        let rf = r as f32;
        let sw = src.width;
        let sh = src.height;
        let dw = self.width;
        let dh = self.height;

        if r == 0 {
            for py in 0..h {
                let src_y = sy + py;
                let dst_y = dy + py;
                if src_y >= sh || dst_y >= dh { continue; }
                let src_row_start = src_y * sw + sx;
                let dst_row_start = dst_y * dw + dx;
                let copy_w = w.min(sw - sx).min(dw - dx);
                for px in 0..copy_w {
                    let sp = src.buf[src_row_start + px];
                    if sp != Color::TRANSPARENT.0 {
                        self.buf[dst_row_start + px] = sp;
                    }
                }
            }
            return;
        }

        let corner_end = r;
        let straight_start = r;
        let straight_end = h.saturating_sub(r);
        let corner_row_start = h.saturating_sub(r);

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh { continue; }

            let src_row = src_y * sw + sx;
            let dst_row = dst_y * dw + dx;

            let in_top_corner = py < corner_end;
            let in_bot_corner = py >= corner_row_start;

            if !in_top_corner && !in_bot_corner {
                let copy_w = w.min(sw - sx).min(dw - dx);
                for px in 0..copy_w {
                    let sp = src.buf[src_row + px];
                    if sp != Color::TRANSPARENT.0 {
                        self.buf[dst_row + px] = sp;
                    }
                }
                continue;
            }

            let end_x = w.min(sw - sx).min(dw - dx);
            for px in 0..end_x {
                let src_pixel = Color(src.buf[src_row + px]);

                let alpha = {
                    let in_corner = (px < r && py < r)
                        || (px >= w.saturating_sub(r) && py < r)
                        || (px < r && py >= h.saturating_sub(r))
                        || (px >= w.saturating_sub(r) && py >= h.saturating_sub(r));
                    if !in_corner {
                        1.0
                    } else {
                        let cx_f = if px < r { r } else { w - r } as f32;
                        let cy_f = if py < r { r } else { h - r } as f32;
                        let dx_f = px as f32 + 0.5 - cx_f;
                        let dy_f = py as f32 + 0.5 - cy_f;
                        let dist_sq = dx_f * dx_f + dy_f * dy_f;
                        if dist_sq < (rf - 0.5) * (rf - 0.5) {
                            1.0
                        } else if dist_sq > (rf + 0.5) * (rf + 0.5) {
                            0.0
                        } else {
                            let dist = libm::sqrtf(dist_sq);
                            (rf + 0.5 - dist).clamp(0.0, 1.0)
                        }
                    }
                };

                if alpha <= 0.0 { continue; }
                if src_pixel.0 == Color::TRANSPARENT.0 { continue; }

                if alpha >= 1.0 {
                    self.buf[dst_row + px] = src_pixel.0;
                } else {
                    let dst_idx = dst_row + px;
                    let dst_pixel = Color(self.buf[dst_idx]);
                    let sr = src_pixel.r() as f32;
                    let sg = src_pixel.g() as f32;
                    let sb = src_pixel.b() as f32;
                    let dr = dst_pixel.r() as f32;
                    let dg = dst_pixel.g() as f32;
                    let db = dst_pixel.b() as f32;
                    let out_r = (sr * alpha + dr * (1.0 - alpha)) as u32;
                    let out_g = (sg * alpha + dg * (1.0 - alpha)) as u32;
                    let out_b = (sb * alpha + db * (1.0 - alpha)) as u32;
                    self.buf[dst_idx] = Color::rgb(out_r as u8, out_g as u8, out_b as u8).0;
                }
            }
        }
    }

        /// Shadow layer を合成する。src の各ピクセルの下位バイトを
    /// アルファ値として読み取り、dst（self）を暗くする。
    pub fn composit_shadow_alpha(
        &mut self,
        src: &LayerSystem,
        dx: usize,
        dy: usize,
        sx: usize,
        sy: usize,
        w: usize,
        h: usize,
    ) {
        let sw = src.width;
        let sh = src.height;
        let dw = self.width;
        let dh = self.height;

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh {
                continue;
            }

            let src_row = src_y * sw + sx;
            let dst_row = dst_y * dw + dx;
            let max_px = w.min(sw.saturating_sub(sx)).min(dw.saturating_sub(dx));

            for px in 0..max_px {
                let a = src.buf[src_row + px] & 0xFF;
                if a == 0 {
                    continue;
                }
                let inv = 255 - a;
                let idx = dst_row + px;
                let bg = self.buf[idx];
                let br = (bg >> 16) & 0xFF;
                let bg2 = (bg >> 8) & 0xFF;
                let bb = bg & 0xFF;
                let r = (br * inv) / 255;
                let g = (bg2 * inv) / 255;
                let b = (bb * inv) / 255;
                self.buf[idx] = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }
    }

    pub fn composit_rect(
        &mut self,
        src: &LayerSystem,
        dx: usize, dy: usize,
        sx: usize, sy: usize,
        w: usize, h: usize,
    ) {
        let sw = src.width;
        let sh = src.height;
        let dw = self.width;
        let dh = self.height;

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh { continue; }

            for px in 0..w {
                let src_x = sx + px;
                let dst_x = dx + px;
                if src_x >= sw || dst_x >= dw { continue; }

                let src_pixel = Color(src.buf[src_y * sw + src_x]);
                if src_pixel.0 == Color::TRANSPARENT.0 { continue; }

                self.buf[dst_y * dw + dst_x] = src_pixel.0;
            }
        }
    }

    pub fn composit_rect_alpha(
        &mut self,
        src: &LayerSystem,
        dx: usize, dy: usize,
        sx: usize, sy: usize,
        w: usize, h: usize,
    ) {
        let sw = src.width;
        let sh = src.height;
        let dw = self.width;
        let dh = self.height;

        for py in 0..h {
            let src_y = sy + py;
            let dst_y = dy + py;
            if src_y >= sh || dst_y >= dh { continue; }

            for px in 0..w {
                let src_x = sx + px;
                let dst_x = dx + px;
                if src_x >= sw || dst_x >= dw { continue; }

                let sp = src.buf[src_y * sw + src_x];
                let src_a = ((sp >> 24) & 0xFF) as u32;
                if src_a == 0 { continue; }
                if src_a >= 255 {
                    self.buf[dst_y * dw + dst_x] = sp;
                } else {
                    let inv = 255 - src_a;
                    let sr = (sp >> 16) & 0xFF;
                    let sg = (sp >> 8) & 0xFF;
                    let sb = sp & 0xFF;
                    let dp = self.buf[dst_y * dw + dst_x];
                    let dr = (dp >> 16) & 0xFF;
                    let dg = (dp >> 8) & 0xFF;
                    let db = dp & 0xFF;
                    let r = (sr * src_a + dr * inv) / 255;
                    let g = (sg * src_a + dg * inv) / 255;
                    let b = (sb * src_a + db * inv) / 255;
                    self.buf[dst_y * dw + dst_x] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
                }
            }
        }
    }

    pub fn frame_count(&self) -> u64 { self.frame_count }
    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }
}
