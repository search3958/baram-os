use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use baram_bsd::{config, vfs};
use baram_core::Color;
use baram_core::LayerSystem;

const SCROLL_ANIMATION_NS: u64 = 180_000_000;
const WINDOW_OPEN_DURATION_NS: u64 = 400_000_000;
const WINDOW_MOTION_OFFSET_Y: i32 = 30;
use baram_font::LayerFontExt;
use baram_graphics::blur;
use baram_graphics::svg;

pub fn scroll_speed() -> i32 {
    config::get_i32("ui-theme/window/scroll_speed", 30)
}

/// Shared smooth-scroll state for windows and non-window scroll viewports.
pub struct SmoothScroll {
    pub position: i32,
    target: i32,
    start: i32,
    started_ns: Option<u64>,
    max: i32,
}

impl SmoothScroll {
    pub const fn new() -> Self {
        Self {
            position: 0,
            target: 0,
            start: 0,
            started_ns: None,
            max: 0,
        }
    }

    pub fn reset(&mut self) {
        self.position = 0;
        self.target = 0;
        self.start = 0;
        self.started_ns = None;
    }

    pub fn set_max(&mut self, max: i32) {
        self.max = max.max(0);
        self.target = self.target.min(self.max);
        self.position = self.position.min(self.max);
        self.start = self.start.min(self.max);
    }

    pub fn scroll(&mut self, delta: i32) -> bool {
        let next = self.target.saturating_add(delta).clamp(0, self.max);
        if next == self.target {
            return false;
        }
        // Match Window/Warp3 scrolling: extend the current target without
        // restarting an active animation for every wheel event.
        if self.position == self.target {
            self.start = self.position;
            self.started_ns = None;
        }
        self.target = next;
        true
    }

    pub fn tick(&mut self, now_ns: u64) -> bool {
        if self.position == self.target {
            self.started_ns = None;
            return false;
        }
        let started = *self
            .started_ns
            .get_or_insert(now_ns.saturating_sub(1_000_000));
        let elapsed = now_ns.saturating_sub(started);
        let t = (elapsed as f32 / SCROLL_ANIMATION_NS as f32).clamp(0.0, 1.0);
        let eased = decelerate_scroll(t);
        let distance = self.target - self.start;
        let next = if t >= 1.0 {
            self.target
        } else {
            self.start + (distance as f32 * eased) as i32
        };
        if next == self.position {
            return false;
        }
        self.position = next;
        if t >= 1.0 {
            self.started_ns = None;
        }
        true
    }

    pub fn is_animating(&self) -> bool {
        self.position != self.target
    }
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
    // Keep the title clear of the window controls.
    btn_size() * 3 + 27
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
            dst.composit_shadow_alpha(&self.layer, dst_x, dst_y, src_x, src_y, draw_w, draw_h);
        }
    }
}

pub fn btn_bg_radius() -> usize {
    config::get_usize("ui-theme/button/radius", 8)
}

pub fn btn_bg_color() -> Color {
    config::get_color("ui-theme/color/btn_bg", Color::BTN_BG)
}

struct WindowBaseRedraw {
    layer: *mut LayerSystem,
    width: usize,
    height: usize,
    damage: Option<(usize, usize, usize, usize)>,
    maximized: bool,
    body_bg: Color,
    title_height: usize,
    radius: usize,
    polygon: *const (f32, f32),
    polygon_len: usize,
}

unsafe impl Sync for WindowBaseRedraw {}

fn redraw_window_base(jobs: &Vec<WindowBaseRedraw>, index: usize) {
    let job = &jobs[index];
    let layer = unsafe { &mut *job.layer };
    let (x0, y0, x1, y1) = job.damage.unwrap_or((0, 0, job.width, job.height));
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for row in y0..y1 {
        let start = row * job.width + x0;
        let end = row * job.width + x1;
        layer.buf_mut()[start..end].fill(Color::TRANSPARENT.0);
    }
    if job.damage.is_some() {
        let body_y = y0.max(job.title_height);
        if y1 > body_y {
            layer.fill_rect(x0, body_y, x1 - x0, y1 - body_y, job.body_bg);
        }
    } else if job.maximized {
        layer.fill_rect(0, 0, job.width, job.height, job.body_bg);
    } else {
        let polygon = unsafe { core::slice::from_raw_parts(job.polygon, job.polygon_len) };
        layer.fill_rounded_rect_with_polygon(
            0,
            0,
            job.width,
            job.height,
            job.radius,
            job.body_bg,
            polygon,
        );
    }
}

/// Build a separate progressive-blur layer from the title bar's top 40px.
/// The upper 12px is a fixed 16px blur; below it, neighbouring integer blur
/// radii are crossfaded so the 16px-to-0px falloff has no visible seams.
fn draw_title_bar_blur_layer(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) {
    if width == 0 || height == 0 {
        return;
    }
    const BLUR_RADIUS: usize = 16;
    const FIXED_BLUR_HEIGHT: usize = 12;
    const CAPTURE_HEIGHT: usize = 40;
    let sample_y0 = y;
    let sample_y1 = (y + CAPTURE_HEIGHT).min(layer.height());
    let sample_h = sample_y1.saturating_sub(sample_y0);
    if sample_h == 0 {
        return;
    }
    let mut backdrop = alloc::vec![0u32; width * sample_h];
    for row in 0..sample_h {
        let src = (sample_y0 + row) * layer.width() + x;
        let dst = row * width;
        backdrop[dst..dst + width].copy_from_slice(&layer.buf_ref()[src..src + width]);
    }
    let mut blurred = alloc::vec![0u32; backdrop.len()];
    let mut blur_layer = LayerSystem::new(width, height);
    for row in 0..height {
        let target_row = row * width;
        if row < sample_h {
            let source_row = row * width;
            blur_layer.buf_mut()[target_row..target_row + width]
                .copy_from_slice(&backdrop[source_row..source_row + width]);
        } else if y + row < layer.height() {
            // The capture is deliberately limited to the top 40 px.  Keep
            // the remainder of the title bar untouched instead of stretching
            // the final captured row, which previously formed a bright seam.
            let source_row = (y + row) * layer.width() + x;
            blur_layer.buf_mut()[target_row..target_row + width]
                .copy_from_slice(&layer.buf_ref()[source_row..source_row + width]);
        }
    }
    let fade_height = height.saturating_sub(FIXED_BLUR_HEIGHT + 1).max(1);
    for radius in 1..=BLUR_RADIUS {
        // Preserve every radius step.  For the wider (box-blurred) steps the
        // title bar uses one H→V sweep rather than the normal two sweeps.
        blur::blur_region_to_single_box(&backdrop, &mut blurred, width, 0, sample_h, radius as i32);
        for row in 0..height {
            let scaled_radius = if row < FIXED_BLUR_HEIGHT {
                (BLUR_RADIUS * 256) as u32
            } else {
                (height.saturating_sub(1 + row) * BLUR_RADIUS * 256 / fade_height) as u32
            };
            let lower_radius = (scaled_radius / 256) as usize;
            let upper_radius = ((scaled_radius + 255) / 256) as usize;
            let upper_alpha = (scaled_radius & 0xff) as u8;
            if row >= sample_h {
                continue;
            }
            let source_row = row * width;
            if radius == lower_radius && lower_radius != 0 {
                blur_layer.composit_rect_global_alpha(
                    &blurred[source_row..source_row + width],
                    width,
                    1,
                    0,
                    row,
                    255,
                );
            }
            if radius == upper_radius && upper_radius != lower_radius {
                blur_layer.composit_rect_global_alpha(
                    &blurred[source_row..source_row + width],
                    width,
                    1,
                    0,
                    row,
                    upper_alpha,
                );
            }
        }
    }
    layer.composit_rect_global_alpha(blur_layer.buf_ref(), width, height, x, y, 255);
}

/// Overlay the white transparency gradient after the independent blur layer.
fn draw_title_bar_overlay(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) {
    if width == 0 || height == 0 {
        return;
    }
    let color = config::get_color("ui-theme/color/win_bg", Color::WIN_BG);
    let solid_row = alloc::vec![color.0; width];
    let denominator = height.saturating_sub(1).max(1) as u32;
    for row in 0..height {
        let alpha = 255 - (row as u32 * 255 / denominator) as u8;
        layer.composit_rect_global_alpha(&solid_row, width, 1, x, y + row, alpha);
    }
}

fn draw_title_bar_background(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    _skip_blur: bool,
    warp4_theme: bool,
) {
    // Title bars are intentionally opaque.  Progressive backdrop blur made
    // the title surface change while the window moved and differed from the
    // Warp3/Warp4 native reference, so use a stable solid fill instead.
    layer.fill_rect(
        x,
        y,
        width,
        height,
        if warp4_theme {
            Color::rgb(250, 250, 252)
        } else {
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG)
        },
    );
}

const MAX_ICON_SVG: &str = include_str!("../../../files/data/ui/max.svg");
const MINI_ICON_SVG: &str = include_str!("../../../files/data/ui/mini.svg");
const CLOSE_ICON_SVG: &str = include_str!("../../../files/data/ui/close.svg");
const MIN_ICON_SVG: &str = include_str!("../../../files/data/ui/min.svg");

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
    /// Warp4 owns a per-window palette; Warp3 and system windows keep the
    /// global window-server theme.
    pub warp4_theme: bool,
    pub chrome_visible: bool,
    pub always_on_top: bool,
    pub focusable: bool,
    pub maximized: bool,
    pub minimized: bool,
    pub scroll_y: i32,
    scroll_start_y: i32,
    scroll_target_y: i32,
    scroll_started_ns: Option<u64>,
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
    pub open_animating: bool,
    motion_started_ns: Option<u64>,
    render_y_offset: i32,
    prev_render_y_offset: i32,
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
        px >= self.x
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

/// CSS `cubic-bezier(0, 0, 0, 1)`. Since both x control points are zero,
/// x(s) = s^3; a short binary solve is sufficient for the 1 ms UI clock.
fn decelerate_scroll(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let mut low = 0.0f32;
    let mut high = 1.0f32;
    for _ in 0..10 {
        let s = (low + high) * 0.5;
        if s * s * s < t {
            low = s;
        } else {
            high = s;
        }
    }
    let s = (low + high) * 0.5;
    s * s * (3.0 - 2.0 * s)
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

const FILE_DIALOG_LIST_Y: i32 = 96;
const FILE_DIALOG_FOOTER_H: i32 = 56;
const FILE_DIALOG_SMALL_CELL_H: i32 = 42;
const FILE_DIALOG_LARGE_CELL_H: i32 = 100;
const FILE_DIALOG_GRID_COLUMNS: usize = 5;

macro_rules! file_icon_bytes {
    ($size:literal, $name:literal) => {
        include_bytes!(concat!(
            "../../../files/data/file/",
            $size,
            "/",
            $name,
            ".png"
        ))
    };
}

fn file_icon_key(entry: &vfs::FileEntry) -> &'static str {
    if entry.is_dir {
        return match entry.name.as_str() {
            "app" => "files-folder-app",
            "data" => "files-folder-data",
            "os" => "files-folder-os",
            "user" => "files-folder-user",
            _ => "files-folder",
        };
    }
    let name = entry.name.as_str();
    if name.ends_with(".ini") {
        "files-file-appini"
    } else if name.ends_with(".w3s")
        || name.ends_with(".w4s")
        || name.ends_with(".warp")
        || name.ends_with(".sh")
    {
        "files-file-appscript"
    } else if name.ends_with(".w3u") || name.ends_with(".w4u") || name.ends_with(".xml") {
        "files-file-appxml"
    } else if name.ends_with(".w3a") || name.ends_with(".w4a") || name.ends_with(".s4a") {
        "files-folder-app"
    } else if name.ends_with(".svg") {
        "files-file-svg"
    } else if name.ends_with(".png")
        || name.ends_with(".jpg")
        || name.ends_with(".jpeg")
        || name.ends_with(".gif")
    {
        "files-file-image"
    } else if name.ends_with(".yaml") || name.ends_with(".yml") || name.ends_with(".md"){
        "files-file-yaml"
    } else if name.ends_with(".bin") || name.ends_with(".o") {
        "files-file-bin"
    } else if name.ends_with(".txt")
        || name.ends_with(".rs")
        || name.ends_with(".c")
        || name.ends_with(".h")
    {
        "files-file-text"
    } else {
        "files-file"
    }
}

fn file_icon(entry: &vfs::FileEntry, large: bool) -> &'static [u8] {
    let key = file_icon_key(entry);
    if large {
        match key {
            "files-folder-app" => file_icon_bytes!(64, "files-folder-app"),
            "files-folder-data" => file_icon_bytes!(64, "files-folder-data"),
            "files-folder-os" => file_icon_bytes!(64, "files-folder-os"),
            "files-folder-user" => file_icon_bytes!(64, "files-folder-user"),
            "files-folder" => file_icon_bytes!(64, "files-folder"),
            "files-file-appini" => file_icon_bytes!(64, "files-file-appini"),
            "files-file-appscript" => file_icon_bytes!(64, "files-file-appscript"),
            "files-file-appxml" => file_icon_bytes!(64, "files-file-appxml"),
            "files-file-bin" => file_icon_bytes!(64, "files-file-bin"),
            "files-file-image" => file_icon_bytes!(64, "files-file-image"),
            "files-file-redflag" => file_icon_bytes!(64, "files-file-redflag"),
            "files-file-svg" => file_icon_bytes!(64, "files-file-svg"),
            "files-file-text-1" => file_icon_bytes!(64, "files-file-text-1"),
            "files-file-text" => file_icon_bytes!(64, "files-file-text"),
            "files-file-warpfile" => file_icon_bytes!(64, "files-file-warpfile"),
            "files-file-yaml" => file_icon_bytes!(64, "files-file-yaml"),
            _ => file_icon_bytes!(64, "files-file"),
        }
    } else {
        match key {
            "files-folder-app" => file_icon_bytes!(24, "files-folder-app"),
            "files-folder-data" => file_icon_bytes!(24, "files-folder-data"),
            "files-folder-os" => file_icon_bytes!(24, "files-folder-os"),
            "files-folder-user" => file_icon_bytes!(24, "files-folder-user"),
            "files-folder" => file_icon_bytes!(24, "files-folder"),
            "files-file-appini" => file_icon_bytes!(24, "files-file-appini"),
            "files-file-appscript" => file_icon_bytes!(24, "files-file-appscript"),
            "files-file-appxml" => file_icon_bytes!(24, "files-file-appxml"),
            "files-file-bin" => file_icon_bytes!(24, "files-file-bin"),
            "files-file-image" => file_icon_bytes!(24, "files-file-image"),
            "files-file-redflag" => file_icon_bytes!(24, "files-file-redflag"),
            "files-file-svg" => file_icon_bytes!(24, "files-file-svg"),
            "files-file-text-1" => file_icon_bytes!(24, "files-file-text-1"),
            "files-file-text" => file_icon_bytes!(24, "files-file-text"),
            "files-file-warpfile" => file_icon_bytes!(24, "files-file-warpfile"),
            "files-file-yaml" => file_icon_bytes!(24, "files-file-yaml"),
            _ => file_icon_bytes!(24, "files-file"),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum NativeFileDialogAction {
    None,
    Changed,
    Cancel,
    Confirm,
}

/// OS-owned, read-only file chooser. It deliberately has no text editor or
/// app script behind it: the OS owns the path, selection, and buttons.
pub struct NativeFileDialog {
    win_id: WinId,
    content_width: i32,
    content_height: i32,
    path: String,
    entries: Vec<vfs::FileEntry>,
    selected: Option<usize>,
    scroll: SmoothScroll,
    large_view: bool,
}

impl NativeFileDialog {
    pub fn new(win_id: WinId, path: &str, window_width: usize, window_height: usize) -> Self {
        let mut dialog = Self {
            win_id,
            content_width: window_width as i32,
            content_height: window_height.saturating_sub(title_bar_h()) as i32,
            path: path.into(),
            entries: Vec::new(),
            selected: None,
            scroll: SmoothScroll::new(),
            large_view: false,
        };
        dialog.reload();
        dialog
    }

    pub fn win_id(&self) -> WinId {
        self.win_id
    }

    pub fn selected_path(&self) -> Option<String> {
        let index = self.selected?;
        let entry = self.entries.get(index)?;
        if entry.is_dir {
            return None;
        }
        Some(format!(
            "{}/{}",
            self.path.trim_end_matches('/'),
            entry.name
        ))
    }

    fn reload(&mut self) {
        self.entries = vfs::list_files(&self.path);
        self.selected = None;
        self.scroll.reset();
        self.update_scroll_limit();
    }

    fn display_path(&self) -> String {
        if self.path.trim_end_matches('/') == "files" {
            "files://".into()
        } else {
            format!(
                "files://{}/",
                self.path.trim_start_matches("files/").trim_end_matches('/')
            )
        }
    }

    fn footer_top(&self) -> i32 {
        self.content_height - FILE_DIALOG_FOOTER_H
    }

    fn cell_height(&self) -> i32 {
        if self.large_view {
            FILE_DIALOG_LARGE_CELL_H
        } else {
            FILE_DIALOG_SMALL_CELL_H
        }
    }

    fn columns(&self) -> usize {
        if self.large_view {
            FILE_DIALOG_GRID_COLUMNS
        } else {
            1
        }
    }

    fn visible_rows(&self) -> usize {
        self.footer_top().saturating_sub(FILE_DIALOG_LIST_Y).max(0) as usize
            / self.cell_height() as usize
    }

    fn update_scroll_limit(&mut self) {
        let rows = (self.entries.len() + self.columns() - 1) / self.columns();
        let content_height = rows as i32 * self.cell_height();
        let viewport_height = self.footer_top().saturating_sub(FILE_DIALOG_LIST_Y);
        self.scroll
            .set_max(content_height.saturating_sub(viewport_height));
    }

    pub fn click(&mut self, x: i32, y: i32) -> NativeFileDialogAction {
        if x >= 12 && x < 108 && y >= 46 && y < 82 {
            let path = self.path.trim_end_matches('/');
            self.path = path
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_string())
                .unwrap_or_else(|| "files".into());
            self.reload();
            return NativeFileDialogAction::Changed;
        }

        let view_button_y = 46;
        if y >= view_button_y && y < view_button_y + 36 {
            let large_x = self.content_width - 84;
            let small_x = self.content_width - 156;
            if x >= small_x && x < small_x + 64 {
                if self.large_view {
                    self.large_view = false;
                    self.update_scroll_limit();
                    return NativeFileDialogAction::Changed;
                }
            } else if x >= large_x && x < large_x + 64 {
                if !self.large_view {
                    self.large_view = true;
                    self.update_scroll_limit();
                    return NativeFileDialogAction::Changed;
                }
            }
        }

        let footer_top = self.footer_top();
        if y >= footer_top {
            let button_w = (560 - 32) / 2;
            let second_x = 20 + button_w;
            if x >= 12 && x < 12 + button_w {
                return NativeFileDialogAction::Cancel;
            }
            if x >= second_x && x < second_x + button_w {
                return NativeFileDialogAction::Confirm;
            }
            return NativeFileDialogAction::None;
        }

        let list_bottom = FILE_DIALOG_LIST_Y + self.visible_rows() as i32 * self.cell_height();
        if y < FILE_DIALOG_LIST_Y || y >= list_bottom {
            return NativeFileDialogAction::None;
        }
        let available_width = self.content_width.saturating_sub(24);
        let gap = 8;
        let columns = self.columns();
        let cell_width = (available_width - gap * (columns as i32 - 1)) / columns as i32;
        let local_x = x - 12;
        if local_x < 0 {
            return NativeFileDialogAction::None;
        }
        let column = (local_x / (cell_width + gap)) as usize;
        if column >= columns {
            return NativeFileDialogAction::None;
        }
        let local_y = y - FILE_DIALOG_LIST_Y + self.scroll.position.max(0);
        let row = (local_y / self.cell_height()) as usize;
        let index = row * columns + column;
        let Some(entry) = self.entries.get(index).cloned() else {
            return NativeFileDialogAction::None;
        };
        if entry.is_dir {
            self.path = format!("{}/{}", self.path.trim_end_matches('/'), entry.name);
            self.reload();
        } else {
            self.selected = Some(index);
        }
        NativeFileDialogAction::Changed
    }

    pub fn scroll_by(&mut self, delta: i32) -> bool {
        self.scroll.scroll(delta)
    }

    pub fn tick_scroll(&mut self, now_ns: u64) -> bool {
        self.scroll.tick(now_ns)
    }

    pub fn draw_to_layer(&self, layer: &mut LayerSystem, body_y: i32) {
        let width = layer.width();
        let height = layer.height();
        let body_top = body_y.max(0) as usize;
        if body_top >= height {
            return;
        }
        layer.fill_rect(
            0,
            body_top,
            width,
            height - body_top,
            Color::rgb(250, 250, 252),
        );

        layer.put_str(16, body_top + 14, &self.display_path(), Color::TEXT);
        draw_native_button(layer, 12, body_top + 46, 96, 36, "戻る", false);
        let selected = self
            .selected
            .and_then(|index| self.entries.get(index))
            .map(|entry| format!("選択中: {}", entry.name))
            .unwrap_or_else(|| "ファイルを選択してください".into());
        layer.put_str(120, body_top + 59, &selected, Color::MUTED);

        let small_x = width.saturating_sub(156);
        let large_x = width.saturating_sub(84);
        draw_native_button(
            layer,
            small_x,
            body_top + 46,
            64,
            36,
            "小",
            !self.large_view,
        );
        draw_native_button(layer, large_x, body_top + 46, 64, 36, "大", self.large_view);

        let footer_top = self.footer_top().max(0) as usize + body_top;
        let list_y = body_top + FILE_DIALOG_LIST_Y as usize;
        let gap = 8usize;
        let columns = self.columns();
        let cell_width = (width.saturating_sub(24 + gap * (columns - 1))) / columns;
        let offset = self.scroll.position.max(0) as usize;
        let first_row = offset / self.cell_height() as usize;
        let offset_in_row = offset % self.cell_height() as usize;
        let rows_to_draw = self.visible_rows() + 2;
        for row_offset in 0..rows_to_draw {
            let row = first_row + row_offset;
            for column in 0..columns {
                let index = row * columns + column;
                let x = 12 + column * (cell_width + gap);
                let y = list_y + row_offset * self.cell_height() as usize - offset_in_row;
                let cell_h = self.cell_height().saturating_sub(4) as usize;
                let Some(entry) = self.entries.get(index) else {
                    continue;
                };
                let selected = self.selected == Some(index);
                if !self.large_view {
                    let row_bg = if selected {
                        Color::BTN_PRIMARY
                    } else {
                        Color::rgb(242, 242, 245)
                    };
                    layer.fill_rounded_rect(x, y, cell_width, cell_h, 6, row_bg);
                }
                let icon_size = if self.large_view { 64 } else { 24 };
                let icon_x = if self.large_view {
                    x + cell_width.saturating_sub(icon_size) / 2
                } else {
                    x + 10
                };
                let icon_y = if self.large_view {
                    y + 4
                } else {
                    y + (cell_h.saturating_sub(icon_size)) / 2
                };
                draw_native_file_icon(
                    layer,
                    file_icon(entry, self.large_view),
                    icon_x,
                    icon_y,
                    icon_size,
                );
                let text_color = if !self.large_view && selected {
                    Color::BTN_TEXT
                } else {
                    Color::TEXT
                };
                let label_x = if self.large_view {
                    x + cell_width.saturating_sub(native_text_width(&entry.name)) / 2
                } else {
                    x + 42
                };
                let label_y = if self.large_view {
                    y + cell_h.saturating_sub(16)
                } else {
                    y + (cell_h.saturating_sub(16)) / 2
                };
                if self.large_view && selected {
                    let label_width = native_text_width(&entry.name).saturating_add(10);
                    let label_bg_x = x + cell_width.saturating_sub(label_width) / 2;
                    layer.fill_rounded_rect(
                        label_bg_x,
                        label_y.saturating_sub(4),
                        label_width,
                        22,
                        5,
                        Color::BTN_PRIMARY,
                    );
                    layer.put_str(label_bg_x + 5, label_y, &entry.name, Color::BTN_TEXT);
                } else {
                    layer.put_str(label_x, label_y, &entry.name, text_color);
                }
            }
        }

        let button_y = footer_top.saturating_sub(4);
        let button_w = width.saturating_sub(32) / 2;
        draw_native_button(layer, 12, button_y, button_w, 40, "キャンセル", false);
        draw_native_button(
            layer,
            20 + button_w,
            button_y,
            button_w,
            40,
            "アップロード",
            true,
        );
    }
}

fn draw_native_button(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    label: &str,
    primary: bool,
) {
    let bg = if primary {
        Color::rgb(0, 106, 255)
    } else {
        Color::rgb(232, 232, 235)
    };
    let fg = if primary {
        Color::BTN_TEXT
    } else {
        Color::TEXT
    };
    layer.fill_rounded_rect(x, y, width, height, 8, bg);
    layer.put_str(x + 12, y + 12, label, fg);
}

fn native_text_width(text: &str) -> usize {
    text.chars()
        .map(|ch| {
            let glyph = baram_font::ttf_font::glyph(ch);
            if glyph.w > 0 {
                glyph.advance.max(0) as usize
            } else {
                8
            }
        })
        .sum()
}

fn draw_native_file_icon(layer: &mut LayerSystem, bytes: &[u8], x: usize, y: usize, size: usize) {
    let Ok((header, pixels)) = png_decoder::decode(bytes) else {
        return;
    };
    let src_w = header.width as usize;
    let src_h = header.height as usize;
    let width = layer.width();
    let height = layer.height();
    let buffer = layer.buf_mut();
    for py in 0..size {
        let dst_y = y + py;
        if dst_y >= height {
            continue;
        }
        let src_y = py * src_h / size.max(1);
        for px in 0..size {
            let dst_x = x + px;
            if dst_x >= width {
                continue;
            }
            let src_x = px * src_w / size.max(1);
            let [sr, sg, sb, alpha] = pixels[src_y * src_w + src_x];
            if alpha == 0 {
                continue;
            }
            let index = dst_y * width + dst_x;
            let dst = Color(buffer[index]);
            let inverse = 255u32.saturating_sub(alpha as u32);
            let r = (sr as u32 * alpha as u32 + dst.r() as u32 * inverse) / 255;
            let g = (sg as u32 * alpha as u32 + dst.g() as u32 * inverse) / 255;
            let b = (sb as u32 * alpha as u32 + dst.b() as u32 * inverse) / 255;
            buffer[index] = Color::rgb(r as u8, g as u8, b as u8).0;
        }
    }
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
    interaction_blocked: Option<WinId>,
    file_dialog: Option<NativeFileDialog>,
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

    pub fn draw_all(
        &mut self,
        layer: &mut LayerSystem,
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
        let top_z = self.windows.iter().map(|w| w.z).max().unwrap_or(0) + 1;
        for w in &mut self.windows {
            if w.always_on_top {
                w.z = top_z;
            }
        }
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
            if !w.visible || (w.minimized && !w.is_motion_animating()) || w.maximized {
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

        // Allocate on the BSP; AP jobs below only touch disjoint window layers.
        for i in 0..sort_n {
            let idx = indices[i];
            if self.windows[idx].visible
                && (!self.windows[idx].minimized || self.windows[idx].is_motion_animating())
            {
                self.windows[idx].ensure_layer(screen_w, screen_h);
            }
        }
        let body_bg = config::get_color("ui-theme/color/win_bg", Color::WIN_BG);
        let radius = win_radius();
        let title_height = title_bar_h();
        let mut redraw_polygons: Vec<Vec<(f32, f32)>> = Vec::new();
        for i in 0..sort_n {
            let w = &self.windows[indices[i]];
            if w.visible
                && (!w.minimized || w.is_motion_animating())
                && w.content_dirty
                && w.content_damage.is_none()
                && !w.maximized
            {
                redraw_polygons.push(LayerSystem::squircle_polygon(
                    w.w as f32,
                    w.h as f32,
                    radius.min(w.w / 2).min(w.h / 2) as f32,
                ));
            }
        }
        let mut polygon_index = 0usize;
        let mut redraw_jobs: Vec<WindowBaseRedraw> = Vec::new();
        for i in 0..sort_n {
            let w = &mut self.windows[indices[i]];
            if !w.visible || (w.minimized && !w.is_motion_animating()) || !w.content_dirty {
                continue;
            }
            let (polygon, polygon_len) = if w.content_damage.is_none() && !w.maximized {
                let poly = &redraw_polygons[polygon_index];
                polygon_index += 1;
                (poly.as_ptr(), poly.len())
            } else {
                (core::ptr::null(), 0)
            };
            redraw_jobs.push(WindowBaseRedraw {
                layer: w.layer.as_mut().unwrap() as *mut LayerSystem,
                width: w.w,
                height: w.h,
                damage: w.content_damage,
                maximized: w.maximized,
                body_bg,
                title_height,
                radius,
                polygon,
                polygon_len,
            });
        }
        baram_core::parallel::for_each(redraw_jobs.len(), &redraw_jobs, redraw_window_base);

        for i in 0..sort_n {
            let idx = indices[i];
            if !self.windows[idx].visible
                || (self.windows[idx].minimized && !self.windows[idx].is_motion_animating())
            {
                continue;
            }
            self.windows[idx].ensure_layer(screen_w, screen_h);

            let wx = self.windows[idx].x;
            let wy = self.windows[idx].render_y();
            let ww = self.windows[idx].w;
            let wh = self.windows[idx].h;
            let scroll_y = self.windows[idx].scroll_y;
            let win_id = self.windows[idx].id;
            let is_max = self.windows[idx].maximized;
            let shadow_dirty = self.windows[idx].shadow_dirty;
            let content_dirty = self.windows[idx].content_dirty;
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

                        let src_x = if shadow_x < 0 {
                            (-shadow_x) as usize
                        } else {
                            0
                        };
                        let src_y = if shadow_y < 0 {
                            (-shadow_y) as usize
                        } else {
                            0
                        };
                        let dst_x = shadow_x.max(0) as usize;
                        let dst_y = shadow_y.max(0) as usize;
                        let draw_w = (shadow_size as i32 - src_x as i32).max(0) as usize;
                        let draw_h = (shadow_h as i32 - src_y as i32).max(0) as usize;

                        if draw_w > 0 && draw_h > 0 {
                            layer.composit_shadow_alpha(
                                shadow_ref, dst_x, dst_y, src_x, src_y, draw_w, draw_h,
                            );
                        }
                    }
                }
            }

            if content_dirty {
                let chrome_h = if self.windows[idx].chrome_visible {
                    title_bar_h()
                } else {
                    0
                };
                let skip_title_blur = self.windows[idx].is_motion_animating()
                    || html_engines
                        .iter()
                        .any(|(id, engine)| *id == win_id && engine.is_animating());
                let layer_ptr = self.windows[idx].layer.as_mut().unwrap() as *mut LayerSystem;
                let w_ptr = &self.windows[idx] as *const Window;
                let damage = self.windows[idx].content_damage.take();
                unsafe {
                    let lw = (*layer_ptr).width();
                    let lh = (*layer_ptr).height();
                    let (cx0, cy0, cx1, cy1) = damage.unwrap_or((0, 0, lw, lh));
                    (*layer_ptr).push_clip(cx0, cy0, cx1, cy1);

                    // A Warp3 hover patch owns every pixel in its damage rect.
                    // Do not enter generic window chrome/body rendering here:
                    // some SVG/font paths are not damage-clip aware and would
                    // touch title-bar pixels outside the hovered control.
                    // Base clearing/fill ran in parallel. SVG, font and engine
                    // caches remain on the BSP because they can allocate.

                    for i in 0..warp_engines.len() {
                        if win_id == warp_engines[i].0 {
                            let engine = &mut warp_engines[i].1;
                            (*layer_ptr).push_clip(0, chrome_h, ww, wh);
                            engine.draw_to_layer(&mut *layer_ptr, 0, -scroll_y);
                            engine.draw_texts(&mut *layer_ptr, 0, -scroll_y, 1.0);
                            (*layer_ptr).pop_clip();
                            break;
                        }
                    }
                    for i in 0..html_engines.len() {
                        if win_id == html_engines[i].0 {
                            let engine = &mut html_engines[i].1;
                            let content_top = if engine.is_warp3() { 0 } else { chrome_h };
                            (*layer_ptr).push_clip(0, content_top, ww, wh);
                            engine.draw_to_layer(&mut *layer_ptr, 0, -scroll_y);
                            (*layer_ptr).pop_clip();
                            break;
                        }
                    }
                    if let Some(dialog) = self
                        .file_dialog
                        .as_ref()
                        .filter(|dialog| dialog.win_id() == win_id)
                    {
                        (*layer_ptr).push_clip(0, chrome_h, ww, wh);
                        dialog.draw_to_layer(&mut *layer_ptr, chrome_h as i32);
                        (*layer_ptr).pop_clip();
                    }
                    if self.interaction_blocked == Some(win_id) {
                        draw_settings_permission_overlay(&mut *layer_ptr, ww, wh);
                    }
                    // The Warp3 document reaches the top of the window and
                    // therefore sits behind the title bar. Repaint the full
                    // chrome only when this damage touches it; body-only
                    // hover patches retain the cheap, clipped path.
                    let repaint_title = self.windows[idx].chrome_visible
                        && damage.map_or(true, |(_, y0, _, _)| y0 < chrome_h);
                    (*layer_ptr).pop_clip();
                    if repaint_title {
                        draw_title_bar(&mut *layer_ptr, &*w_ptr, 0, 0, skip_title_blur);
                    }
                }
                self.windows[idx].prev_x = self.windows[idx].x;
                self.windows[idx].prev_y = self.windows[idx].y;
                self.windows[idx].prev_render_y_offset = self.windows[idx].render_y_offset;
                self.windows[idx].content_dirty = false;
            }

            let win_layer = self.windows[idx].layer.as_ref().unwrap();
            let _screen_w = layer.width() as i32;
            let _screen_h = layer.height() as i32;

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
                layer.composit_rect(win_layer, dst_x, dst_y, src_x, src_y, draw_w, draw_h);
            } else {
                layer.composit_rounded(
                    win_layer,
                    dst_x,
                    dst_y,
                    src_x,
                    src_y,
                    draw_w,
                    draw_h,
                    win_radius(),
                );
                draw_window_border(layer, &self.windows[idx]);
            }
            self.windows[idx].prev_x = self.windows[idx].x;
            self.windows[idx].prev_y = self.windows[idx].y;
            self.windows[idx].prev_render_y_offset = self.windows[idx].render_y_offset;
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
    let mask = ShadowMaskPass {
        alpha: alpha.as_mut_ptr(),
        stride: sw,
        left,
        right,
        top,
        bottom,
        radius,
    };
    baram_core::parallel::for_each(height, &mask, fill_shadow_mask_row);
    let box_radius = (blur_r.max(1) as usize / 2).max(1);
    for _ in 0..2 {
        box_blur_shadow(&mut alpha, sw, sh, box_radius);
    }

    Some((alpha, sw, sh))
}

struct ShadowMaskPass {
    alpha: *mut u8,
    stride: usize,
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
    radius: usize,
}

unsafe impl Sync for ShadowMaskPass {}

fn fill_shadow_mask_row(pass: &ShadowMaskPass, local_y: usize) {
    let py = pass.top + local_y;
    if py >= pass.bottom {
        return;
    }
    let r = pass.radius as i32;
    for px in pass.left..pass.right {
        let dx = if px < pass.left + pass.radius {
            pass.left as i32 + r - px as i32
        } else if px >= pass.right - pass.radius {
            px as i32 - (pass.right as i32 - r - 1)
        } else {
            0
        };
        let dy = if py < pass.top + pass.radius {
            pass.top as i32 + r - py as i32
        } else if py >= pass.bottom - pass.radius {
            py as i32 - (pass.bottom as i32 - r - 1)
        } else {
            0
        };
        if dx == 0 || dy == 0 || dx * dx + dy * dy <= r * r {
            unsafe {
                *pass.alpha.add(py * pass.stride + px) = 45;
            }
        }
    }
}

struct ShadowHorizontalPass {
    src: *const u8,
    dst: *mut u8,
    width: usize,
    radius: usize,
}
unsafe impl Sync for ShadowHorizontalPass {}

fn blur_shadow_row(pass: &ShadowHorizontalPass, y: usize) {
    let diameter = pass.radius * 2 + 1;
    let mut sum = 0u32;
    for x in 0..pass.width + pass.radius {
        unsafe {
            if x < pass.width {
                sum += *pass.src.add(y * pass.width + x) as u32;
            }
            if x >= diameter && x - diameter < pass.width {
                sum -= *pass.src.add(y * pass.width + x - diameter) as u32;
            }
            if x >= pass.radius && x - pass.radius < pass.width {
                *pass.dst.add(y * pass.width + x - pass.radius) = (sum / diameter as u32) as u8;
            }
        }
    }
}

struct ShadowVerticalPass {
    src: *const u8,
    dst: *mut u8,
    width: usize,
    height: usize,
    radius: usize,
}
unsafe impl Sync for ShadowVerticalPass {}

fn blur_shadow_column(pass: &ShadowVerticalPass, x: usize) {
    let diameter = pass.radius * 2 + 1;
    let mut sum = 0u32;
    for y in 0..pass.height + pass.radius {
        unsafe {
            if y < pass.height {
                sum += *pass.src.add(y * pass.width + x) as u32;
            }
            if y >= diameter && y - diameter < pass.height {
                sum -= *pass.src.add((y - diameter) * pass.width + x) as u32;
            }
            if y >= pass.radius && y - pass.radius < pass.height {
                *pass.dst.add((y - pass.radius) * pass.width + x) = (sum / diameter as u32) as u8;
            }
        }
    }
}

fn box_blur_shadow(alpha: &mut [u8], width: usize, height: usize, radius: usize) {
    let mut tmp = alloc::vec![0u8; alpha.len()];
    let horizontal = ShadowHorizontalPass {
        src: alpha.as_ptr(),
        dst: tmp.as_mut_ptr(),
        width,
        radius,
    };
    baram_core::parallel::for_each(height, &horizontal, blur_shadow_row);
    let vertical = ShadowVerticalPass {
        src: tmp.as_ptr(),
        dst: alpha.as_mut_ptr(),
        width,
        height,
        radius,
    };
    baram_core::parallel::for_each(width, &vertical, blur_shadow_column);
}

fn draw_title_bar(layer: &mut LayerSystem, w: &Window, ox: i32, oy: i32, skip_blur: bool) {
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
    if !w.chrome_visible {
        layer.fill_rect(
            x,
            y,
            w_draw,
            h_draw,
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
        );
        return;
    }

    let tb_h = title_bar_h().min(h_draw);
    draw_title_bar_background(layer, x, y, w_draw, tb_h, skip_blur, w.warp4_theme);

    let base_x = x as i32 + 10;
    let btn_y = y as i32 + 10;
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
            let title_y = (y as i32 + 13) as usize;
            if title_x < sw && title_y < sh {
                layer.put_str(title_x, title_y, title, Color::TEXT);
            }
        }
    }
}

fn draw_settings_permission_overlay(layer: &mut LayerSystem, width: usize, height: usize) {
    let content_top = title_bar_h().min(height);
    let buffer_width = layer.width();
    let buffer_height = layer.height();
    let buffer = layer.buf_mut();
    for y in content_top..height.min(buffer_height) {
        let row = y * buffer_width;
        for x in 0..width.min(buffer_width) {
            let index = row + x;
            let color = Color(buffer[index]);
            let blend = |channel: u8| ((channel as u32 * 70 + 255 * 185) / 255) as u8;
            buffer[index] = Color::rgb(blend(color.r()), blend(color.g()), blend(color.b())).0;
        }
    }

    let lines = [
        "操作体系の設定変更を要求しています",
        "確認ウィンドウでアクションを選択してください",
    ];
    let line_height = 24usize;
    let block_height = line_height * lines.len();
    let content_height = height.saturating_sub(content_top);
    let start_y = content_top + content_height.saturating_sub(block_height) / 2;
    for (line_index, text) in lines.iter().enumerate() {
        let text_width = text
            .chars()
            .map(|ch| {
                if baram_font::ttf_font::is_available() {
                    let glyph = baram_font::ttf_font::glyph(ch);
                    if glyph.w > 0 {
                        glyph.advance.max(0) as usize
                    } else {
                        8
                    }
                } else {
                    8
                }
            })
            .sum::<usize>();
        let x = width.saturating_sub(text_width) / 2;
        layer.put_str(
            x,
            start_y + line_index * line_height,
            text,
            Color::rgb(40, 40, 40),
        );
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
    if !w.chrome_visible {
        layer.fill_rect(
            x,
            y,
            w_draw,
            h_draw,
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
        );
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

    let base_x = x as i32 + 10;
    let btn_y = y as i32 + 10;
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

    layer.put_str(x + btn_area_w(), y + 13, w.title_str(), title_color);
}

fn draw_window_border(_layer: &mut LayerSystem, _w: &Window) {}

fn draw_window(layer: &mut LayerSystem, w: &Window, ox: i32, oy: i32) {
    draw_window_body(layer, w, false, ox, oy);
    draw_window_border(layer, w);
}
