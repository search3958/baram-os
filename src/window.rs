use alloc::vec;
use alloc::vec::Vec;
use crate::gop::{Color, Screen};
use crate::ui::put_str;

const TITLE_BAR_H: usize = 24;
const MIN_WIN_W: usize = 120;
const MIN_WIN_H: usize = 60;

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
            visible: true, focused: false,
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
        px >= self.x && px < self.x + self.w as i32 && py == self.y
    }

    fn close_hit(&self, px: i32, py: i32) -> bool {
        let cx = self.x + self.w as i32 - 18;
        let cy = self.y + 4;
        px >= cx && px < cx + 14 && py >= cy && py < cy + 16
    }

    fn resize_hit(&self, px: i32, py: i32) -> bool {
        px >= self.x + self.w as i32 - 14
            && py >= self.y + self.h as i32 - 14
    }
}

pub struct WindowManager {
    windows: Vec<Window>,
    next_z: i32,
    next_id: u32,
    pub focused_id: Option<WinId>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            next_z: 0,
            next_id: 1,
            focused_id: None,
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

    pub fn on_mouse_down(&mut self, px: i32, py: i32) {
        if let Some(id) = self.window_at(px, py) {
            self.focus(id);
            let win = self.windows.iter_mut().find(|w| w.id == id).unwrap();
            if win.close_hit(px, py) {
                let _ = win;
                self.remove(id);
                return;
            }
            if win.resize_hit(px, py) {
                win.resizing = true;
                win.resize_sx = px;
                win.resize_sy = py;
                win.resize_sw = win.w;
                win.resize_sh = win.h;
            } else if win.title_bar_hit(px, py) {
                win.dragging = true;
                win.drag_ox = px - win.x;
                win.drag_oy = py - win.y;
            }
        }
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

    pub fn draw_all(&self, screen: &mut Screen) {
        let mut sorted: Vec<&Window> = self.windows.iter().collect();
        sorted.sort_by(|a, b| a.z.cmp(&b.z));
        for w in &sorted {
            if w.visible {
                draw_window(screen, w);
            }
        }
    }

    pub fn count(&self) -> usize {
        self.windows.len()
    }

    pub fn get_title(&self, id: WinId) -> Option<&str> {
        self.windows.iter().find(|w| w.id == id).map(|w| w.title_str())
    }
}

fn draw_window(screen: &mut Screen, w: &Window) {
    let x = w.x.max(0) as usize;
    let y = w.y.max(0) as usize;
    let sw = screen.width();
    let sh = screen.height();
    if x >= sw || y >= sh { return; }
    let x1 = (x + w.w).min(sw);
    let y1 = (y + w.h).min(sh);
    let w_draw = x1.saturating_sub(x);
    let h_draw = y1.saturating_sub(y);
    if w_draw == 0 || h_draw == 0 { return; }

    let title_bg = if w.focused { Color::ACCENT } else { Color::PANEL };
    let border = if w.focused { Color::ACCENT } else { Color::MUTED };

    // Shadow
    if x + 4 < sw && y + 4 < sh {
        let sh_x = (x + 4).min(sw);
        let sh_y = (y + 4).min(sh);
        let sh_w = w_draw.min(sw.saturating_sub(sh_x));
        let sh_h = h_draw.min(sh.saturating_sub(sh_y));
        screen.fill_rect(sh_x, sh_y, sh_w, sh_h, Color::BLACK);
    }

    // Window body
    screen.fill_rect(x, y, w_draw, h_draw, Color::PANEL);

    // Title bar
    let tb_h = TITLE_BAR_H.min(h_draw);
    screen.fill_rect(x, y, w_draw, tb_h, title_bg);

    // Title text
    put_str(screen, x + 8, y + 4, w.title_str(), Color::TEXT, title_bg);

    // Close button
    let cbx = x + w.w - 18;
    let cby = y + 4;
    if cbx + 14 <= sw && cby + 16 <= sh {
        screen.fill_rect(cbx, cby, 14, 16, Color::rgb(0xCC, 0x44, 0x44));
        put_str(screen, cbx + 3, cby + 1, "X", Color::TEXT, Color::rgb(0xCC, 0x44, 0x44));
    }

    // Resize grip
    let rx = x + w.w - 12;
    let ry = y + h_draw - 12;
    if rx + 10 <= sw && ry + 10 <= sh {
        for i in 0..4 {
            let gx = rx + i * 2;
            let gy = ry + i * 2;
            if gx < sw && gy < sh {
                screen.fill_rect(gx, gy, 2, 2, Color::MUTED);
            }
        }
    }

    // Border
    screen.rect_outline(x, y, w_draw, h_draw, border);
}

pub struct LayerSystem {
    #[allow(dead_code)]
    last_fb: Vec<u32>,
    frame_count: u64,
}

impl LayerSystem {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            last_fb: vec![u32::MAX; w * h],
            frame_count: 0,
        }
    }

    #[allow(dead_code)]
    pub fn frame_count(&self) -> u64 { self.frame_count }

    pub fn tick(&mut self) { self.frame_count += 1; }
}
