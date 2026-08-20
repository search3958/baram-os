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


