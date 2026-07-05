use alloc::vec;
use alloc::vec::Vec;
use crate::gop::{Color, Screen};

const TITLE_BAR_H: usize = 30;
const MIN_WIN_W: usize = 120;
const MIN_WIN_H: usize = 60;
const BTN_SIZE: usize = 20;

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
    // saved position/size before maximize
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
        px >= self.x && px < self.x + self.w as i32 && py >= self.y && py < self.y + TITLE_BAR_H as i32
    }

    /// Returns which button was hit: 'c'lose, 'm'aximize, 'n'one.
    fn button_hit(&self, px: i32, py: i32) -> char {
        let base_x = self.x + self.w as i32 - (BTN_SIZE as i32) * 3;
        let btn_y = self.y + 5;
        if py >= btn_y && py < btn_y + BTN_SIZE as i32 {
            if px >= base_x && px < base_x + BTN_SIZE as i32 {
                return 'm'; // maximize
            }
            if px >= base_x + BTN_SIZE as i32 * 2 && px < base_x + BTN_SIZE as i32 * 3 {
                return 'c'; // close
            }
        }
        'n'
    }

    fn resize_handle_hit(&self, px: i32, py: i32) -> bool {
        px >= self.x + self.w as i32 - 6
            && py >= self.y + self.h as i32 - 6
    }

    pub fn toggle_maximize(&mut self, screen_w: i32, screen_h: i32) {
        if self.maximized {
            // Restore
            self.x = self.save_x;
            self.y = self.save_y;
            self.w = self.save_w;
            self.h = self.save_h;
            self.maximized = false;
        } else {
            // Save current position
            self.save_x = self.x;
            self.save_y = self.y;
            self.save_w = self.w;
            self.save_h = self.h;
            // Maximize: fill screen (leave taskbar space)
            self.x = 0;
            self.y = 0;
            self.w = screen_w as usize;
            self.h = (screen_h - TITLE_BAR_H as i32 - 32) as usize; // 32 = taskbar
            self.maximized = true;
        }
    }

    pub fn start_drag(&mut self, px: i32, py: i32) {
        // If maximized, restore first then start drag
        if self.maximized {
            // Map mouse to restored position proportionally
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

    /// Returns Some('c') if close was clicked, Some('m') if maximize toggled.
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
            // Start drag or resize
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

    pub fn draw_all(&self, layer: &mut LayerSystem) {
        let mut sorted: Vec<&Window> = self.windows.iter().collect();
        sorted.sort_by(|a, b| a.z.cmp(&b.z));
        for w in &sorted {
            if w.visible {
                draw_window(layer, w);
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

    // --- Colors (Windows 10 dark) ---
    let (title_bg, body_bg, border) = if w.focused {
        (Color::ACCENT, Color::WIN_BG, Color::ACCENT)
    } else {
        (Color::WIN_INACTIVE, Color::WIN_BG, Color::BORDER)
    };

    // Shadow (subtle)
    if !w.maximized {
        let sx = (x + 3).min(sw);
        let sy = (y + 3).min(sh);
        let sw2 = w_draw.min(sw.saturating_sub(sx));
        let sh2 = h_draw.min(sh.saturating_sub(sy));
        layer.fill_rect(sx, sy, sw2, sh2, Color::rgb(0, 0, 0));
    }

    // Window body
    layer.fill_rect(x, y, w_draw, h_draw, body_bg);

    // Title bar
    let tb_h = TITLE_BAR_H.min(h_draw);
    layer.fill_rect(x, y, w_draw, tb_h, title_bg);

    // Title text (white on accent)
    layer.put_str(x + 10, y + 7, w.title_str(), Color::TEXT);

    // --- Title bar buttons (right to left: close, maximize, minimize) ---
    let base_x = x + w_draw - BTN_SIZE * 3;
    let btn_y = y + 5;

    // Maximize/Restore button
    if base_x + BTN_SIZE <= sw && btn_y + BTN_SIZE <= sh {
        let mx_color = title_bg;
        layer.fill_rect(base_x, btn_y, BTN_SIZE, BTN_SIZE, mx_color);
        // Draw a small square icon (maximize) or overlapping squares (restore)
        let ix = base_x + 5;
        let iy = btn_y + 5;
        if w.maximized {
            // Restore icon: two overlapping rectangles
            layer.rect_outline(ix + 3, iy, 9, 9, Color::TEXT);
            layer.fill_rect(ix + 3, iy, 9, 1, Color::TEXT); // top line
            layer.fill_rect(ix + 3, iy, 1, 9, Color::TEXT); // left line
            layer.rect_outline(ix, iy + 3, 9, 9, Color::TEXT);
            layer.fill_rect(ix, iy + 3, 9, 1, Color::TEXT);
            layer.fill_rect(ix, iy + 3, 1, 9, Color::TEXT);
        } else {
            // Maximize icon: single rectangle
            layer.rect_outline(ix, iy, 10, 10, Color::TEXT);
            layer.fill_rect(ix, iy, 10, 1, Color::TEXT);
            layer.fill_rect(ix, iy, 1, 10, Color::TEXT);
        }
    }

    // Close button (no background, just X icon)
    let close_x = x + w_draw - BTN_SIZE;
    if close_x <= sw && btn_y + BTN_SIZE <= sh {
        // X icon only
        let cx = close_x + 5;
        let cy = btn_y + 5;
        for i in 0..10 {
            if cx + i < sw && cy + i < sh {
                layer.put_pixel(cx + i, cy + i, Color::TEXT);
            }
            if cx + 10 - 1 - i < sw && cy + i < sh {
                layer.put_pixel(cx + 10 - 1 - i, cy + i, Color::TEXT);
            }
        }
    }

    // Border (1px)
    if !w.maximized {
        layer.rect_outline(x, y, w_draw, h_draw, border);
    }

    // Resize grip (bottom-right corner, only if not maximized)
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
// LayerSystem — full-frame off-screen renderer
//
// Every frame the entire scene is drawn into `buf` (a Vec<u32> in
// Color.0 format).  `flush()` then copies the whole buffer to the
// framebuffer in a single pass, converting pixel format on the fly.
// This is simple, correct, and fast enough at 640×480 / 1024×768.
// ---------------------------------------------------------------------------

pub struct LayerSystem {
    width: usize,
    height: usize,
    buf: Vec<u32>,
    frame_count: u64,
}

impl LayerSystem {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            width: w,
            height: h,
            buf: vec![Color::BG.0; w * h],
            frame_count: 0,
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
        if x < self.width && y < self.height {
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
                    if px < self.width {
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

    /// Write the entire buffer to the framebuffer via Screen.
    /// Every pixel is written every frame — no differential rendering.
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
