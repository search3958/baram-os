use alloc::vec;
use alloc::vec::Vec;
use crate::gop::{Color, Screen};
use crate::svg;

const TITLE_BAR_H: usize = 30;
const MIN_WIN_W: usize = 120;
const MIN_WIN_H: usize = 60;
const BTN_SIZE: usize = 20;

const MAX_ICON_SVG: &str = include_str!("data/max.svg");
const MINI_ICON_SVG: &str = include_str!("data/mini.svg");
const CLOSE_ICON_SVG: &str = include_str!("data/close.svg");

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
            visible: true, focused: false, maximized: false,
            scroll_y: 0,
            save_x: x, save_y: y, save_w: w, save_h: h,
            dragging: false, resizing: false,
            drag_ox: 0, drag_oy: 0,
            resize_sx: 0, resize_sy: 0, resize_sw: 0, resize_sh: 0,
        }
    }

    fn title_str(&self) -> &str {
        core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")
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
        let base_x = self.x + self.w as i32 - (BTN_SIZE as i32) * 3;
        let btn_y = self.y + 5;
        if py >= btn_y && py < btn_y + BTN_SIZE as i32 {
            if px >= base_x && px < base_x + BTN_SIZE as i32 {
                return 'm';
            }
            if px >= base_x + BTN_SIZE as i32 * 2 && px < base_x + BTN_SIZE as i32 * 3 {
                return 'c';
            }
        }
        'n'
    }

    fn resize_handle_hit(&self, px: i32, py: i32) -> bool {
        px >= self.x + self.w as i32 - 6
            && py >= self.y + self.h as i32 - 6
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
            self.h = (screen_h - TITLE_BAR_H as i32 - 32) as usize;
            self.maximized = true;
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

pub struct WindowManager {
    windows: Vec<Window>,
    next_z: i32,
    next_id: u32,
    pub focused_id: Option<WinId>,
    screen_w: i32,
    screen_h: i32,
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
        }
    }

    pub fn add(&mut self, title: &str, x: i32, y: i32, w: usize, h: usize) -> WinId {
        let id = WinId(self.next_id);
        self.next_id += 1;
        self.next_z += 1;
        let win = Window::new(id, title, x, y, w, h, self.next_z);
        self.windows.push(win);
        self.focus(id);
        id
    }

    pub fn remove(&mut self, id: WinId) {
        self.windows.retain(|w| w.id != id);
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
        let mut sorted: Vec<&Window> = self.windows.iter().collect();
        sorted.sort_by(|a, b| b.z.cmp(&a.z));
        sorted.into_iter()
            .find(|w| w.visible && w.contains(px, py))
            .map(|w| w.id)
    }

    pub fn sorted_ids(&self) -> Vec<WinId> {
        let mut v: Vec<WinId> = self.windows.iter().map(|w| w.id).collect();
        v.sort_by(|&a, &b| {
            let za = self.windows.iter().find(|w| w.id == a).map(|w| w.z).unwrap_or(0);
            let zb = self.windows.iter().find(|w| w.id == b).map(|w| w.z).unwrap_or(0);
            zb.cmp(&za)
        });
        v
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

    /// Draw all windows in z-order. `draw_window_content` is called between
    /// filling the body and drawing the frame, so content is clipped by the
    /// window borders.
    pub fn draw_all(
        &self,
        layer: &mut LayerSystem,
        ui_win: Option<(WinId, &[super::uiscript::Command])>,
    ) {
        let mut sorted: Vec<&Window> = self.windows.iter().collect();
        sorted.sort_by(|a, b| a.z.cmp(&b.z));
        for w in &sorted {
            if w.visible {
                draw_window(layer, w);
                if let Some((uid, cmds)) = ui_win {
                    if w.id == uid {
                        layer.push_clip(
                            w.x.max(0) as usize,
                            (w.y + TITLE_BAR_H as i32).max(0) as usize,
                            (w.x + w.w as i32).max(0) as usize,
                            (w.y + w.h as i32).max(0) as usize,
                        );
                        super::uiscript::render(
                            layer, cmds,
                            w.x, w.y, w.w, w.h,
                            TITLE_BAR_H, w.scroll_y,
                        );
                        layer.pop_clip();
                    }
                }
            }
        }
    }

    pub fn count(&self) -> usize {
        self.windows.len()
    }

    pub fn get_title(&self, id: WinId) -> Option<&str> {
        self.windows.iter().find(|w| w.id == id).map(|w| w.title_str())
    }

    pub fn get_window_rect(&self, id: WinId) -> Option<(i32, i32, usize, usize, i32)> {
        self.windows.iter().find(|w| w.id == id).map(|w| (w.x, w.y, w.w, w.h, w.scroll_y))
    }
}

/// Draw a single window frame. After filling the body, the caller should draw
/// content (UI Script etc.), then this function draws the title bar and borders
/// on top to naturally clip the content.
fn draw_window(layer: &mut LayerSystem, w: &Window) {
    let x = w.x.max(0) as usize;
    let y = w.y.max(0) as usize;
    let sw = layer.width();
    let sh = layer.height();
    if x >= sw || y >= sh { return; }
    let x1 = (x + w.w).min(sw);
    let y1 = (y + w.h).min(sh);
    let w_draw = x1.saturating_sub(x);
    let h_draw = y1.saturating_sub(y);
    if w_draw == 0 || h_draw == 0 { return; }

    let (title_bg, body_bg, border) = if w.focused {
        (Color::ACCENT, Color::WIN_BG, Color::ACCENT)
    } else {
        (Color::WIN_INACTIVE, Color::WIN_BG, Color::BORDER)
    };

    // Shadow — smooth blurred shadow, 2px down offset
    if !w.maximized {
        let blur_r: i32 = 30;
        let offset_y: i32 = 2;
        let win_x0 = x as i32;
        let win_y0 = y as i32 + offset_y;
        let win_x1 = x as i32 + w_draw as i32;
        let win_y1 = y as i32 + h_draw as i32 + offset_y;
        let sx = (win_x0 - blur_r).max(0) as usize;
        let sy = (win_y0 - blur_r).max(0) as usize;
        let ex = (win_x1 + blur_r).min(sw as i32) as usize;
        let ey = (win_y1 + blur_r).min(sh as i32) as usize;
        if ex > sx && ey > sy {
            for py in sy..ey {
                let py_i = py as i32;
                let edge_y = if py_i >= win_y0 && py_i < win_y1 {
                    0
                } else if py_i < win_y0 {
                    win_y0 - py_i
                } else {
                    py_i - (win_y1 - 1)
                };
                for px in sx..ex {
                    let px_i = px as i32;
                    let edge_x = if px_i >= win_x0 && px_i < win_x1 {
                        0
                    } else if px_i < win_x0 {
                        win_x0 - px_i
                    } else {
                        px_i - (win_x1 - 1)
                    };
                    let edge = edge_x.max(edge_y);
                    if edge >= blur_r { continue; }
                    let t = (blur_r - edge) as f32 / blur_r as f32;
                    let alpha_f = t * t * (3.0 - 2.0 * t) * 0.175;
                    let a = (alpha_f * 255.0) as u32;
                    if a == 0 { continue; }
                    let inv = 255 - a;
                    let bg = layer.get_pixel(px, py);
                    let r = (bg.r() as u32 * inv) / 255;
                    let g = (bg.g() as u32 * inv) / 255;
                    let b = (bg.b() as u32 * inv) / 255;
                    layer.put_pixel(px, py, Color::rgb(r as u8, g as u8, b as u8));
                }
            }
        }
    }

    // 1. Fill entire window body (content will be drawn on top by caller)
    layer.fill_rect(x, y, w_draw, h_draw, body_bg);

    // 2. Title bar
    let tb_h = TITLE_BAR_H.min(h_draw);
    layer.fill_rect(x, y, w_draw, tb_h, title_bg);
    layer.put_str(x + 10, y + 7, w.title_str(), Color::TEXT);

    // Title bar buttons
    let base_x = x + w_draw - BTN_SIZE * 3;
    let btn_y = y + 5;

    if base_x + BTN_SIZE <= sw && btn_y + BTN_SIZE <= sh {
        layer.fill_rect(base_x, btn_y, BTN_SIZE, BTN_SIZE, title_bg);
        let icon = if w.maximized { MINI_ICON_SVG } else { MAX_ICON_SVG };
        svg::draw_svg_into(layer, icon,
            base_x as i32 + 4, btn_y as i32 + 4,
            (BTN_SIZE - 8) as f32, (BTN_SIZE - 8) as f32);
    }

    let close_x = x + w_draw - BTN_SIZE;
    if close_x <= sw && btn_y + BTN_SIZE <= sh {
        svg::draw_svg_into(layer, CLOSE_ICON_SVG,
            close_x as i32 + 4, btn_y as i32 + 4,
            (BTN_SIZE - 8) as f32, (BTN_SIZE - 8) as f32);
    }

    // 3. Borders (drawn LAST to clip content)
    if !w.maximized {
        // Top border (1px line just below title bar)
        let border_y = y + tb_h;
        if border_y < y1 {
            layer.fill_rect(x, border_y, w_draw, 1, border);
        }
        // Left border
        layer.fill_rect(x, y + tb_h, 1, h_draw.saturating_sub(tb_h), border);
        // Right border
        if w_draw > 1 {
            layer.fill_rect(x + w_draw - 1, y + tb_h, 1, h_draw.saturating_sub(tb_h), border);
        }
        // Bottom border
        if h_draw > 1 {
            layer.fill_rect(x, y + h_draw - 1, w_draw, 1, border);
        }
    }

    // Resize grip
    if !w.maximized {
        let rx = x + w_draw - 6;
        let ry = y + h_draw - 6;
        for i in 0..3 {
            let gx = rx + i * 2;
            let gy = ry + i * 2;
            if gx < sw && gy < sh {
                layer.put_pixel(gx, gy, Color::MUTED);
                if gx + 1 < sw { layer.put_pixel(gx + 1, gy, Color::MUTED); }
                if gy + 1 < sh { layer.put_pixel(gx, gy + 1, Color::MUTED); }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// LayerSystem — full-frame off-screen renderer with clip rect support
// ---------------------------------------------------------------------------

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

    pub fn push_clip(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        let x0 = x0.min(self.width);
        let y0 = y0.min(self.height);
        let x1 = x1.min(self.width);
        let y1 = y1.min(self.height);
        // Save current clip to stack
        if let Some(cur) = self.clip {
            self.clip_stack.push(cur);
            // Intersect with current clip
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
        let v = c.0;
        for p in self.buf.iter_mut() {
            *p = v;
        }
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

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: Color) {
        if let Some((cx0, cy0, cx1, cy1)) = self.clip {
            let x0 = x.max(cx0).min(self.width);
            let y0 = y.max(cy0).min(self.height);
            let x1 = (x + w).min(cx1).min(self.width);
            let y1 = (y + h).min(cy1).min(self.height);
            if x0 >= x1 || y0 >= y1 { return; }
            let v = c.0;
            let stride = self.width;
            for yy in y0..y1 {
                let row = yy * stride;
                for xx in x0..x1 {
                    self.buf[row + xx] = v;
                }
            }
        } else {
            let x0 = x.min(self.width);
            let y0 = y.min(self.height);
            let x1 = (x + w).min(self.width);
            let y1 = (y + h).min(self.height);
            if x0 >= x1 || y0 >= y1 { return; }
            let v = c.0;
            let stride = self.width;
            for yy in y0..y1 {
                let row = yy * stride;
                for xx in x0..x1 {
                    self.buf[row + xx] = v;
                }
            }
        }
    }

    /// Draw a filled rectangle with rounded corners and anti-aliasing.
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

        for py in y0..y1 {
            for px in x0..x1 {
                let (cx_f, cy_f): (f32, f32) = if px < x + r && py < y + r {
                    ((x + r) as f32, (y + r) as f32)
                } else if px >= x + w.saturating_sub(r) && py < y + r && r > 0 {
                    ((x + w - r - 1) as f32, (y + r) as f32)
                } else if px < x + r && py >= y + h.saturating_sub(r) && r > 0 {
                    ((x + r) as f32, (y + h - r - 1) as f32)
                } else if px >= x + w.saturating_sub(r) && py >= y + h.saturating_sub(r) && r > 0 {
                    ((x + w - r - 1) as f32, (y + h - r - 1) as f32)
                } else {
                    self.put_pixel(px, py, c);
                    continue;
                };

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

    /// Draw a rounded rectangle outline with anti-aliasing.
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
                    let cx_f = (x + w - r - 1) as f32;
                    let cy_f = (y + r) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else if px < x + r && py >= y + h.saturating_sub(r) && r > 0 {
                    let cx_f = (x + r) as f32;
                    let cy_f = (y + h - r - 1) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else if px >= x + w.saturating_sub(r) && py >= y + h.saturating_sub(r) && r > 0 {
                    let cx_f = (x + w - r - 1) as f32;
                    let cy_f = (y + h - r - 1) as f32;
                    let dx = px as f32 + 0.5 - cx_f;
                    let dy = py as f32 + 0.5 - cy_f;
                    libm::sqrtf(dx * dx + dy * dy) - rf
                } else {
                    // Straight edge
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

    pub fn flush(&mut self, screen: &mut Screen) {
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let c = Color(self.buf[y * w + x]);
                screen.put_pixel(x, y, c);
            }
        }
        self.frame_count += 1;
    }

    pub fn frame_count(&self) -> u64 { self.frame_count }
    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }
}
