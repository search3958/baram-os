use super::cursor::{self};
use crate::html::HtmlEngine;
use crate::soft_keyboard::SoftKeyboard;
use crate::text_cursor;
use crate::warp::WarpEngine;
use crate::window::{WinId, WindowManager};
use alloc::vec::Vec;
use baram_bsd::config;
use baram_core::Color;
use baram_core::LayerSystem;
use baram_font::LayerFontExt;
use baram_graphics::blur;
use baram_graphics::svg;
use baram_graphics::ui::FmtBuf;
use uefi::runtime;

pub const TASKBAR_H: usize = 48;
pub const TASKBAR_BLUR_R: i32 = 30;
// The hit box deliberately matches the bare SVG, keeping the click target and
// popup anchor aligned with the visible input-source icon.
pub const IME_BUTTON_W: usize = 20;
const IME_STATUS_STRIP_W: usize = 160;
const IME_MENU_W: usize = 210;
const IME_MENU_H: usize = 264;
const TASKBAR_STATUS_SIZE: f32 = 32.0;
const KEYBOARD_ENGLISH_SVG: &str =
    include_str!("../../../files/data/keyboard/keyboard-english.svg");
const KEYBOARD_JAPANESE_SVG: &str =
    include_str!("../../../files/data/keyboard/keyboard-japanese.svg");
const KEYBOARD_KP2_SVG: &str = include_str!("../../../files/data/keyboard/keyboard-kp2.svg");
const KEYBOARD_KR2_SVG: &str = include_str!("../../../files/data/keyboard/keyboard-kr2.svg");
const KEYBOARD_KRCOM_SVG: &str = include_str!("../../../files/data/keyboard/keyboard-krcom.svg");
const KEYBOARD_PINYIN_SVG: &str = include_str!("../../../files/data/keyboard/keyboard-pinyin.svg");
const KEYBOARD_ICON_SVG: &str = include_str!("../../../files/data/keyboard/keyboard-icon.svg");

fn ime_icon_svg(selection: usize) -> &'static str {
    match selection {
        1 => KEYBOARD_JAPANESE_SVG,
        2 => KEYBOARD_KR2_SVG,
        3 => KEYBOARD_KRCOM_SVG,
        4 => KEYBOARD_KP2_SVG,
        5 => KEYBOARD_PINYIN_SVG,
        _ => KEYBOARD_ENGLISH_SVG,
    }
}

fn draw_ime_icon(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    size: usize,
    selection: usize,
    alpha: u32,
) {
    draw_taskbar_svg_icon(layer, ime_icon_svg(selection), x, y, size, alpha);
}

fn draw_taskbar_svg_icon(
    layer: &mut LayerSystem,
    source: &str,
    x: usize,
    y: usize,
    size: usize,
    alpha: u32,
) {
    svg::draw_svg_into_alpha(
        layer,
        source,
        x as i32,
        y as i32,
        size as f32,
        size as f32,
        alpha,
    );
}

fn draw_keyboard_icon(layer: &mut LayerSystem, x: usize, y: usize, size: usize, alpha: u32) {
    draw_taskbar_svg_icon(layer, KEYBOARD_ICON_SVG, x, y, size, alpha);
}

fn taskbar_status_text_width(text: &str) -> usize {
    text.chars()
        .map(|ch| {
            let g = baram_font::ttf_font_hud::glyph_at_size(ch, TASKBAR_STATUS_SIZE);
            if g.w > 0 {
                g.advance.max(0) as usize
            } else {
                let fallback = baram_font::ttf_font::glyph_at_size(ch, TASKBAR_STATUS_SIZE);
                if fallback.w > 0 {
                    fallback.advance.max(0) as usize
                } else {
                    8
                }
            }
        })
        .sum()
}

fn taskbar_text_width(text: &str, size: f32) -> usize {
    text.chars()
        .map(|ch| {
            let glyph = baram_font::ttf_font_hud::glyph_at_size(ch, size);
            if glyph.w > 0 {
                glyph.advance.max(0) as usize
            } else {
                let fallback = baram_font::ttf_font::glyph_at_size(ch, size);
                if fallback.w > 0 {
                    fallback.advance.max(0) as usize
                } else {
                    8
                }
            }
        })
        .sum()
}

fn taskbar_status_x(width: usize, battery_pct: Option<u8>) -> usize {
    let battery_width = match battery_pct {
        Some(pct) if pct >= 100 => 12 + taskbar_status_text_width("100%"),
        Some(pct) if pct >= 10 => 12 + taskbar_status_text_width("00%"),
        Some(_) => 12 + taskbar_status_text_width("0%"),
        None => 0,
    };
    width.saturating_sub(taskbar_status_text_width("00:00") + battery_width + 16)
}

pub struct TaskbarSurface {
    layer: LayerSystem,
    blurred: Vec<u32>,
    blur_scratch: Vec<u32>,
    base: Vec<u32>,
    base_valid: bool,
    valid: bool,
    search_dirty: bool,
    ime_status_strip: Vec<u32>,
    ime_status_strip_x: usize,
    ime_status_strip_w: usize,
}

impl TaskbarSurface {
    pub fn new(width: usize) -> Self {
        let sample_h = TASKBAR_H + TASKBAR_BLUR_R.max(0) as usize;
        Self {
            layer: LayerSystem::new_transparent(width, TASKBAR_H),
            blurred: alloc::vec![0; width * sample_h],
            blur_scratch: alloc::vec![0; width * sample_h],
            base: alloc::vec![0; width * TASKBAR_H],
            base_valid: false,
            valid: false,
            search_dirty: false,
            ime_status_strip: Vec::new(),
            ime_status_strip_x: usize::MAX,
            ime_status_strip_w: 0,
        }
    }

    #[inline]
    pub fn invalidate(&mut self) {
        self.valid = false;
        self.search_dirty = false;
    }

    #[inline]
    pub fn invalidate_search(&mut self) {
        if self.valid {
            self.search_dirty = true;
        }
    }

    #[inline]
    pub fn is_search_dirty(&self) -> bool {
        self.search_dirty
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Rebuild the cached taskbar background from the wallpaper layer.  This
    /// is called only when the wallpaper cache is initially created or
    /// invalidated, never for ordinary taskbar/window animation frames.
    fn refresh_wallpaper_blur(&mut self, wallpaper: &LayerSystem, y: usize) {
        let width = self.layer.width();
        let pad = TASKBAR_BLUR_R.max(0) as usize;
        let start_y = y.saturating_sub(pad);
        let sample_h = TASKBAR_H + pad;
        let end_y = start_y.saturating_add(sample_h);
        if wallpaper.width() != width || wallpaper.height() < end_y {
            return;
        }

        blur::blur_region_to_with_scratch(
            wallpaper.buf_ref(),
            &mut self.blurred,
            &mut self.blur_scratch,
            width,
            start_y,
            end_y,
            TASKBAR_BLUR_R,
        );
        self.base
            .copy_from_slice(&self.blurred[pad * width..(pad + TASKBAR_H) * width]);
        tint_taskbar(
            &mut self.base,
            config::get_color("ui-theme/color/taskbar", Color::TASKBAR).0,
            200,
        );
        self.base_valid = true;
    }

    fn composite_onto(&self, scene: &mut LayerSystem, y: usize) {
        // A valid taskbar always starts from a fully opaque base and all
        // controls are blended into it. Skip composit_rect's per-row scan for
        // transparent pixels; the active scene clip still limits the copy to
        // the current damage rectangle.
        scene.composit_rect_opaque(&self.layer, 0, y, 0, 0, self.layer.width(), TASKBAR_H);
    }

    /// Restores the cached right-hand status strip (clock/battery/IME area)
    /// and paints only the new IME opacity. No generic taskbar background is
    /// ever copied during this fast path.
    fn redraw_ime_status_strip(
        &mut self,
        battery_pct: Option<u8>,
        selection: usize,
        hovered_keyboard: bool,
        hovered_ime: bool,
    ) -> bool {
        if !self.valid
            || self.ime_status_strip_w == 0
            || self.ime_status_strip.len() != self.ime_status_strip_w * TASKBAR_H
        {
            return false;
        }
        let (icon_x, icon_y, _, _) = ime_button_bounds(self.layer.width(), battery_pct);
        let icon_x = icon_x.max(0) as usize;
        let keyboard_x = icon_x.saturating_sub(IME_BUTTON_W + 12);
        if keyboard_x < self.ime_status_strip_x
            || icon_x + IME_BUTTON_W > self.ime_status_strip_x + self.ime_status_strip_w
        {
            return false;
        }
        self.layer.copy_rect_buffer(
            &self.ime_status_strip,
            self.ime_status_strip_w,
            TASKBAR_H,
            self.ime_status_strip_x,
            0,
        );
        draw_keyboard_icon(
            &mut self.layer,
            keyboard_x,
            icon_y.max(0) as usize,
            IME_BUTTON_W,
            if hovered_keyboard { 128 } else { 255 },
        );
        draw_ime_icon(
            &mut self.layer,
            icon_x,
            icon_y.max(0) as usize,
            IME_BUTTON_W,
            selection,
            if hovered_ime { 128 } else { 255 },
        );
        true
    }
}

fn tint_taskbar(pixels: &mut [u32], color: u32, alpha: u32) {
    let inv = 255 - alpha;
    let tr = (color >> 16) & 0xff;
    let tg = (color >> 8) & 0xff;
    let tb = color & 0xff;
    for pixel in pixels {
        let r = (tr * alpha + ((*pixel >> 16) & 0xff) * inv) / 255;
        let g = (tg * alpha + ((*pixel >> 8) & 0xff) * inv) / 255;
        let b = (tb * alpha + (*pixel & 0xff) * inv) / 255;
        *pixel = (r << 16) | (g << 8) | b;
    }
}

fn blend_rounded_rect(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    radius: usize,
    color: Color,
    alpha: u32,
) {
    if width == 0 || height == 0 || alpha == 0 {
        return;
    }
    let radius = radius.min(width / 2).min(height / 2);
    // Keep compositor-owned translucent controls on the same smooth
    // superellipse used by Warp windows.  `LayerSystem::fill_rounded_rect`
    // already uses this geometry, but these controls need per-pixel alpha.
    let squircle = LayerSystem::squircle_polygon(width as f32, height as f32, radius as f32);
    let x1 = (x + width).min(layer.width());
    let y1 = (y + height).min(layer.height());
    for py in y..y1 {
        let Some((span_l, span_r)) = squircle_row_pixel_span(&squircle, py - y, width) else {
            continue;
        };
        let fill_l = (x + span_l).min(x1);
        let fill_r = (x + span_r).min(x1);
        if fill_l < fill_r {
            blend_solid_span(layer, py, fill_l, fill_r, color, alpha.min(255));
        }
        blend_squircle_edge(
            layer,
            x + span_l.saturating_sub(1),
            py,
            x,
            y,
            &squircle,
            color,
            alpha,
        );
        blend_squircle_edge(layer, x + span_r, py, x, y, &squircle, color, alpha);
    }
}

/// Copy a rounded region from a cropped source image. The crop is retained
/// only while building the launcher cache, avoiding a full-screen temporary.
fn copy_rounded_region_from_crop(
    layer: &mut LayerSystem,
    source: &[u32],
    source_w: usize,
    source_x: usize,
    source_y: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    radius: usize,
) {
    if source_w == 0 || source.len() % source_w != 0 {
        return;
    }
    let source_h = source.len() / source_w;
    if x < source_x
        || y < source_y
        || x.saturating_add(width) > source_x.saturating_add(source_w)
        || y.saturating_add(height) > source_y.saturating_add(source_h)
    {
        return;
    }
    let radius = radius.min(width / 2).min(height / 2);
    let squircle = LayerSystem::squircle_polygon(width as f32, height as f32, radius as f32);
    let x1 = (x + width).min(layer.width());
    let y1 = (y + height).min(layer.height());
    for py in y..y1 {
        let Some((span_l, span_r)) = squircle_row_pixel_span(&squircle, py - y, width) else {
            continue;
        };
        let fill_l = (x + span_l).min(x1);
        let fill_r = (x + span_r).min(x1);
        if fill_l < fill_r {
            let dst = py * layer.width() + fill_l;
            let src = (py - source_y) * source_w + fill_l - source_x;
            layer.buf_mut()[dst..dst + fill_r - fill_l]
                .copy_from_slice(&source[src..src + fill_r - fill_l]);
        }
        copy_squircle_edge(
            layer,
            source,
            source_w,
            source_x,
            source_y,
            x + span_l.saturating_sub(1),
            py,
            x,
            y,
            &squircle,
        );
        copy_squircle_edge(
            layer,
            source,
            source_w,
            source_x,
            source_y,
            x + span_r,
            py,
            x,
            y,
            &squircle,
        );
    }
}

/// Four-by-four anti-aliased coverage for the Warp squircle polygon.
/// Coordinates are screen-relative while the geometry is local to the rect.
fn squircle_coverage(px: usize, py: usize, x: usize, y: usize, polygon: &[(f32, f32)]) -> u32 {
    let mut inside = 0u32;
    for sy in 0..4 {
        for sx in 0..4 {
            let sample_x = px as f32 - x as f32 + (sx as f32 + 0.5) * 0.25;
            let sample_y = py as f32 - y as f32 + (sy as f32 + 0.5) * 0.25;
            if LayerSystem::point_in_polygon(sample_x, sample_y, polygon) {
                inside += 1;
            }
        }
    }
    inside
}

#[inline]
fn squircle_row_bounds(polygon: &[(f32, f32)], py: f32) -> Option<(f32, f32)> {
    let mut left = f32::MAX;
    let mut right = f32::MIN;
    let mut hits = 0usize;
    let mut prev = polygon.len().checked_sub(1)?;
    for current in 0..polygon.len() {
        let (x0, y0) = polygon[prev];
        let (x1, y1) = polygon[current];
        if (y0 > py) != (y1 > py) {
            let edge_x = x0 + (py - y0) * (x1 - x0) / (y1 - y0);
            left = left.min(edge_x);
            right = right.max(edge_x);
            hits += 1;
        }
        prev = current;
    }
    (hits >= 2).then_some((left, right))
}

/// Returns the fully covered pixel span for a scanline.  Only the two
/// neighbouring pixels need expensive anti-alias coverage tests.
#[inline]
fn squircle_row_pixel_span(
    polygon: &[(f32, f32)],
    local_y: usize,
    width: usize,
) -> Option<(usize, usize)> {
    let (left, right) = squircle_row_bounds(polygon, local_y as f32 + 0.5)?;
    let span_l = libm::ceilf(left).max(0.0) as usize;
    let span_r = libm::floorf(right).max(0.0) as usize;
    let span_l = span_l.min(width);
    let span_r = span_r.min(width);
    (span_l <= span_r).then_some((span_l, span_r))
}

#[inline]
fn blend_solid_span(
    layer: &mut LayerSystem,
    py: usize,
    left: usize,
    right: usize,
    color: Color,
    alpha: u32,
) {
    let range = py * layer.width() + left..py * layer.width() + right;
    if alpha == 255 {
        layer.buf_mut()[range].fill(color.0);
        return;
    }
    let inv = 255 - alpha;
    for bg in &mut layer.buf_mut()[range] {
        let r = (color.r() as u32 * alpha + ((*bg >> 16) & 0xff) * inv) / 255;
        let g = (color.g() as u32 * alpha + ((*bg >> 8) & 0xff) * inv) / 255;
        let b = (color.b() as u32 * alpha + (*bg & 0xff) * inv) / 255;
        *bg = (r << 16) | (g << 8) | b;
    }
}

#[inline]
fn blend_squircle_edge(
    layer: &mut LayerSystem,
    px: usize,
    py: usize,
    x: usize,
    y: usize,
    polygon: &[(f32, f32)],
    color: Color,
    alpha: u32,
) {
    if px >= layer.width() || py >= layer.height() {
        return;
    }
    let coverage = squircle_coverage(px, py, x, y, polygon);
    if coverage == 0 {
        return;
    }
    let a = alpha.min(255) * coverage / 16;
    let idx = py * layer.width() + px;
    let bg = layer.buf_ref()[idx];
    let inv = 255 - a;
    layer.buf_mut()[idx] = Color::rgb(
        ((color.r() as u32 * a + ((bg >> 16) & 0xff) * inv) / 255) as u8,
        ((color.g() as u32 * a + ((bg >> 8) & 0xff) * inv) / 255) as u8,
        ((color.b() as u32 * a + (bg & 0xff) * inv) / 255) as u8,
    )
    .0;
}

#[inline]
fn copy_squircle_edge(
    layer: &mut LayerSystem,
    source: &[u32],
    source_w: usize,
    source_x: usize,
    source_y: usize,
    px: usize,
    py: usize,
    x: usize,
    y: usize,
    polygon: &[(f32, f32)],
) {
    if px >= layer.width() || py >= layer.height() {
        return;
    }
    let coverage = squircle_coverage(px, py, x, y, polygon);
    if coverage == 0 {
        return;
    }
    let idx = py * layer.width() + px;
    let fg = source[(py - source_y) * source_w + px - source_x];
    if coverage == 16 {
        layer.buf_mut()[idx] = fg;
        return;
    }
    let bg = layer.buf_ref()[idx];
    let a = coverage * 255 / 16;
    let inv = 255 - a;
    let r = (((fg >> 16) & 0xff) * a + ((bg >> 16) & 0xff) * inv) / 255;
    let g = (((fg >> 8) & 0xff) * a + ((bg >> 8) & 0xff) * inv) / 255;
    let b = ((fg & 0xff) * a + (bg & 0xff) * inv) / 255;
    layer.buf_mut()[idx] = (r << 16) | (g << 8) | b;
}

fn box_blur_alpha_2x(alpha: &mut [u8], width: usize, height: usize, radius: usize) {
    if width == 0 || height == 0 || radius == 0 {
        return;
    }
    let diameter = radius * 2 + 1;
    let mut scratch = alloc::vec![0u8; alpha.len()];
    for _ in 0..2 {
        for y in 0..height {
            let mut sum = 0u32;
            for x in 0..width + radius {
                if x < width {
                    sum += alpha[y * width + x] as u32;
                }
                if x >= diameter && x - diameter < width {
                    sum -= alpha[y * width + x - diameter] as u32;
                }
                if x >= radius && x - radius < width {
                    scratch[y * width + x - radius] = (sum / diameter as u32) as u8;
                }
            }
        }
        for x in 0..width {
            let mut sum = 0u32;
            for y in 0..height + radius {
                if y < height {
                    sum += scratch[y * width + x] as u32;
                }
                if y >= diameter && y - diameter < height {
                    sum -= scratch[(y - diameter) * width + x] as u32;
                }
                if y >= radius && y - radius < height {
                    alpha[(y - radius) * width + x] = (sum / diameter as u32) as u8;
                }
            }
        }
    }
}

fn draw_soft_box_shadow(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    radius: usize,
) {
    const PAD: usize = 54;
    let sw = width + PAD * 2;
    let sh = height + PAD * 2;
    let mut alpha = alloc::vec![0u8; sw * sh];
    let r = radius.min(width / 2).min(height / 2);
    let squircle = LayerSystem::squircle_polygon(width as f32, height as f32, r as f32);
    for py in 0..height {
        if let Some((span_l, span_r)) = squircle_row_pixel_span(&squircle, py, width) {
            alpha[(py + PAD) * sw + span_l + PAD..(py + PAD) * sw + span_r + PAD].fill(24);
        }
    }
    box_blur_alpha_2x(&mut alpha, sw, sh, 18);
    let ox = x.saturating_sub(PAD);
    let oy = y.saturating_sub(PAD);
    let source_x = PAD.saturating_sub(x);
    let source_y = PAD.saturating_sub(y);
    for sy in source_y..sh {
        let dy = oy + sy - source_y;
        if dy >= layer.height() {
            continue;
        }
        for sx in source_x..sw {
            let dx = ox + sx - source_x;
            if dx >= layer.width() {
                continue;
            }
            let a = alpha[sy * sw + sx] as u32;
            if a == 0 {
                continue;
            }
            let idx = dy * layer.width() + dx;
            let bg = layer.buf_ref()[idx];
            let inv = 255 - a;
            let rr = (((bg >> 16) & 0xff) * inv) / 255;
            let gg = (((bg >> 8) & 0xff) * inv) / 255;
            let bb = ((bg & 0xff) * inv) / 255;
            layer.buf_mut()[idx] = (rr << 16) | (gg << 8) | bb;
        }
    }
}

/// Draw a compact, CSS-like black box shadow behind a rounded control.
/// The rounded control itself is masked out so its fill never reveals the
/// shadow when that fill is translucent.
fn draw_control_shadow(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    radius: usize,
    offset_y: usize,
    opacity: u8,
) {
    if width == 0 || height == 0 || opacity == 0 {
        return;
    }
    let blur_radius = 4usize; // Two passes approximate an 8px CSS blur.
    let pad = 24usize;
    let sw = width + pad * 2;
    let sh = height + pad * 2 + offset_y;
    let mut alpha = alloc::vec![0u8; sw * sh];
    let r = radius.min(width / 2).min(height / 2);
    let squircle = LayerSystem::squircle_polygon(width as f32, height as f32, r as f32);
    for py in 0..height {
        if let Some((span_l, span_r)) = squircle_row_pixel_span(&squircle, py, width) {
            let row = (py + pad + offset_y) * sw + pad;
            alpha[row + span_l..row + span_r].fill(opacity);
        }
    }
    box_blur_alpha_2x(&mut alpha, sw, sh, blur_radius);

    let ox = x.saturating_sub(pad);
    let oy = y.saturating_sub(pad);
    let source_x = pad.saturating_sub(x);
    let source_y = pad.saturating_sub(y);
    for sy in source_y..sh {
        let dy = oy + sy - source_y;
        if dy >= layer.height() {
            continue;
        }
        let inside_span = dy
            .checked_sub(y)
            .filter(|local_y| *local_y < height)
            .and_then(|local_y| squircle_row_pixel_span(&squircle, local_y, width));
        for sx in source_x..sw {
            let dx = ox + sx - source_x;
            if dx >= layer.width() {
                continue;
            }
            if let Some((span_l, span_r)) = inside_span {
                if dx >= x + span_l.saturating_sub(1) && dx <= x + span_r {
                    continue;
                }
            }
            let a = alpha[sy * sw + sx] as u32;
            if a == 0 {
                continue;
            }
            let idx = dy * layer.width() + dx;
            let bg = layer.buf_ref()[idx];
            let inv = 255 - a;
            layer.buf_mut()[idx] = Color::rgb(
                (((bg >> 16) & 0xff) * inv / 255) as u8,
                (((bg >> 8) & 0xff) * inv / 255) as u8,
                ((bg & 0xff) * inv / 255) as u8,
            )
            .0;
        }
    }
}

const ICON_CACHE_CAP: usize = 32;
static mut ICON_CACHE: [Option<(alloc::string::String, usize, IconBitmap)>; ICON_CACHE_CAP] = [
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
];

fn get_or_decode_icon(icon_name: &str, size: usize) -> Option<&'static IconBitmap> {
    unsafe {
        for entry in ICON_CACHE.iter() {
            if let Some((ref name, cached_size, ref bitmap)) = entry {
                if name == icon_name && *cached_size == size {
                    return Some(bitmap);
                }
            }
        }
        let icon_path = alloc::format!("apps/icon/{}", icon_name);
        let icon_data = baram_bsd::vfs::read_file(&icon_path);
        if icon_data.is_empty() {
            return None;
        }
        let bitmap = decode_icon(&icon_data, size)?;
        for entry in ICON_CACHE.iter_mut() {
            if entry.is_none() {
                *entry = Some((alloc::string::String::from(icon_name), size, bitmap));
                return ICON_CACHE.iter().find_map(|e| {
                    if let Some((ref n, s, ref b)) = e {
                        if n == icon_name && *s == size {
                            Some(b)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });
            }
        }
        ICON_CACHE[0] = Some((alloc::string::String::from(icon_name), size, bitmap));
        ICON_CACHE.iter().find_map(|e| {
            if let Some((ref n, s, ref b)) = e {
                if n == icon_name && *s == size {
                    Some(b)
                } else {
                    None
                }
            } else {
                None
            }
        })
    }
}


