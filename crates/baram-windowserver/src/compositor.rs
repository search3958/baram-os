use super::cursor::{self};
use crate::html::HtmlEngine;
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
use baram_graphics::uiscript;
use uefi::runtime;

pub const TASKBAR_H: usize = 48;
pub const TASKBAR_BLUR_R: i32 = 30;
pub const IME_BUTTON_W: usize = 40;
const TASKBAR_STATUS_SIZE: f32 = 32.0;

pub struct TaskbarSurface {
    layer: LayerSystem,
    blurred: Vec<u32>,
    blur_scratch: Vec<u32>,
    base: Vec<u32>,
    base_valid: bool,
    valid: bool,
    search_dirty: bool,
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
            170,
        );
        self.base_valid = true;
    }

    fn composite_onto(&self, scene: &mut LayerSystem, y: usize) {
        scene.composit_rect(&self.layer, 0, y, 0, 0, self.layer.width(), TASKBAR_H);
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
    let x1 = (x + width).min(layer.width());
    let y1 = (y + height).min(layer.height());
    for py in y..y1 {
        for px in x..x1 {
            let coverage = rounded_rect_coverage(px, py, x, y, width, height, radius);
            if coverage == 0 {
                continue;
            }
            let idx = py * layer.width() + px;
            let bg = layer.buf_ref()[idx];
            let a = alpha.min(255) * coverage / 16;
            let inv = 255 - a;
            let r = (color.r() as u32 * a + ((bg >> 16) & 0xff) * inv) / 255;
            let g = (color.g() as u32 * a + ((bg >> 8) & 0xff) * inv) / 255;
            let b = (color.b() as u32 * a + (bg & 0xff) * inv) / 255;
            layer.buf_mut()[idx] = Color::rgb(r as u8, g as u8, b as u8).0;
        }
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
    let x1 = (x + width).min(layer.width());
    let y1 = (y + height).min(layer.height());
    for py in y..y1 {
        for px in x..x1 {
            let coverage = rounded_rect_coverage(px, py, x, y, width, height, radius);
            if coverage == 0 {
                continue;
            }
            let idx = py * layer.width() + px;
            let fg = source[(py - source_y) * source_w + px - source_x];
            if coverage == 16 {
                layer.buf_mut()[idx] = fg;
            } else {
                let bg = layer.buf_ref()[idx];
                let a = coverage * 255 / 16;
                let inv = 255 - a;
                let r = (((fg >> 16) & 0xff) * a + ((bg >> 16) & 0xff) * inv) / 255;
                let g = (((fg >> 8) & 0xff) * a + ((bg >> 8) & 0xff) * inv) / 255;
                let b = ((fg & 0xff) * a + (bg & 0xff) * inv) / 255;
                layer.buf_mut()[idx] = (r << 16) | (g << 8) | b;
            }
        }
    }
}

fn rounded_rect_coverage(
    px: usize,
    py: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    radius: usize,
) -> u32 {
    if radius == 0 {
        return 16;
    }
    let left = x as f32;
    let top = y as f32;
    let right = (x + width) as f32;
    let bottom = (y + height) as f32;
    let r = radius as f32;
    let mut inside = 0u32;
    for sy in 0..4 {
        for sx in 0..4 {
            let sample_x = px as f32 + (sx as f32 + 0.5) * 0.25;
            let sample_y = py as f32 + (sy as f32 + 0.5) * 0.25;
            let corner_x = sample_x.clamp(left + r, right - r);
            let corner_y = sample_y.clamp(top + r, bottom - r);
            let dx = sample_x - corner_x;
            let dy = sample_y - corner_y;
            if dx * dx + dy * dy <= r * r {
                inside += 1;
            }
        }
    }
    inside
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
    for py in 0..height {
        for px in 0..width {
            let dx = if px < r {
                r - px
            } else if px >= width - r {
                px + r + 1 - width
            } else {
                0
            };
            let dy = if py < r {
                r - py
            } else if py >= height - r {
                py + r + 1 - height
            } else {
                0
            };
            if dx * dx + dy * dy <= r * r {
                alpha[(py + PAD) * sw + px + PAD] = 24;
            }
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
    for py in 0..height {
        for px in 0..width {
            let dx = if px < r {
                r - px
            } else if px >= width - r {
                px + r + 1 - width
            } else {
                0
            };
            let dy = if py < r {
                r - py
            } else if py >= height - r {
                py + r + 1 - height
            } else {
                0
            };
            if dx * dx + dy * dy <= r * r {
                alpha[(py + pad + offset_y) * sw + px + pad] = opacity;
            }
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
        for sx in source_x..sw {
            let dx = ox + sx - source_x;
            if dx >= layer.width() {
                continue;
            }
            if rounded_rect_coverage(dx, dy, x, y, width, height, radius) != 0 {
                continue;
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

pub const APPS_SVG: &str = include_str!("../../../data/apps.svg");

pub struct IconBitmap {
    pub pixels: Vec<[u8; 4]>,
    pub w: usize,
    pub h: usize,
}

pub fn decode_icon(bytes: &[u8], size: usize) -> Option<IconBitmap> {
    let (header, pixels) = png_decoder::decode(bytes).ok()?;
    let src_w = header.width as usize;
    let src_h = header.height as usize;
    let mut buf = alloc::vec![[0u8; 4]; size * size];
    for y in 0..size {
        let sy = y * src_h / size;
        for x in 0..size {
            let sx = x * src_w / size;
            buf[y * size + x] = pixels[sy * src_w + sx];
        }
    }
    Some(IconBitmap {
        pixels: buf,
        w: size,
        h: size,
    })
}

pub struct AppEntry {
    pub name: alloc::string::String,
    pub app_type: alloc::string::String,
    pub title: alloc::string::String,
    pub icon: alloc::string::String,
    pub tags: Vec<alloc::string::String>,
}

pub fn parse_index_yaml(yaml: &str) -> (Vec<alloc::string::String>, Vec<AppEntry>) {
    let mut autostart = Vec::new();
    let mut apps = Vec::new();
    let mut in_autostart = false;
    let mut in_apps = false;
    let mut current_name = alloc::string::String::new();
    let mut current_type = alloc::string::String::from("warp-2");
    let mut current_title = alloc::string::String::new();
    let mut current_icon = alloc::string::String::new();
    let mut current_tags: Vec<alloc::string::String> = Vec::new();
    let mut in_tags = false;
    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if trimmed == "autostart:" {
            in_autostart = true;
            in_apps = false;
            continue;
        }
        if trimmed == "apps:" {
            in_apps = true;
            in_autostart = false;
            if !current_name.is_empty() {
                let title = if current_title.is_empty() {
                    current_name.clone()
                } else {
                    current_title.clone()
                };
                apps.push(AppEntry {
                    name: current_name.clone(),
                    app_type: current_type.clone(),
                    title,
                    icon: current_icon.clone(),
                    tags: current_tags.clone(),
                });
                current_name.clear();
                current_type = alloc::string::String::from("warp-2");
                current_title.clear();
                current_icon.clear();
                current_tags.clear();
            }
            continue;
        }
        if in_autostart {
            if trimmed.starts_with("- ") {
                let name = alloc::string::String::from(trimmed[2..].trim());
                if !name.is_empty() {
                    autostart.push(name);
                }
            } else if !trimmed.starts_with(' ') && !trimmed.starts_with('\t') {
                in_autostart = false;
            }
        }
        if in_apps {
            if !line.starts_with(' ') && !line.starts_with('\t') {
                if !current_name.is_empty() {
                    let title = if current_title.is_empty() {
                        current_name.clone()
                    } else {
                        current_title.clone()
                    };
                    apps.push(AppEntry {
                        name: current_name.clone(),
                        app_type: current_type.clone(),
                        title,
                        icon: current_icon.clone(),
                        tags: current_tags.clone(),
                    });
                    current_name.clear();
                    current_type = alloc::string::String::from("warp-2");
                    current_title.clear();
                    current_icon.clear();
                    current_tags.clear();
                }
                in_apps = false;
                continue;
            }
            if trimmed.ends_with(':')
                && !trimmed.contains("icon")
                && !trimmed.contains("type")
                && !trimmed.contains("title")
                && !trimmed.starts_with("tag")
            {
                if !current_name.is_empty() {
                    let title = if current_title.is_empty() {
                        current_name.clone()
                    } else {
                        current_title.clone()
                    };
                    apps.push(AppEntry {
                        name: current_name.clone(),
                        app_type: current_type.clone(),
                        title,
                        icon: current_icon.clone(),
                        tags: current_tags.clone(),
                    });
                }
                current_name = alloc::string::String::from(trimmed.trim_end_matches(':'));
                current_type = alloc::string::String::from("warp-2");
                current_title.clear();
                current_icon.clear();
                current_tags.clear();
                in_tags = false;
            } else if let Some(v) = trimmed.strip_prefix("type:") {
                current_type = alloc::string::String::from(v.trim().trim_matches('"'));
            } else if let Some(v) = trimmed.strip_prefix("title:") {
                current_title = alloc::string::String::from(v.trim().trim_matches('"'));
            } else if let Some(v) = trimmed.strip_prefix("icon:") {
                let val = v.trim().trim_matches('"');
                if val.is_empty() || val == "null" {
                    current_icon = alloc::string::String::from("noname.png");
                } else {
                    current_icon = alloc::string::String::from(val);
                }
                in_tags = false;
            } else if let Some(v) = trimmed.strip_prefix("tag:") {
                current_tags.clear();
                let val = v.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    current_tags.push(alloc::string::String::from(val));
                }
                in_tags = true;
            } else if in_tags && trimmed.starts_with("- ") {
                let val = trimmed[2..].trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    current_tags.push(alloc::string::String::from(val));
                }
            }
        }
    }
    if in_apps && !current_name.is_empty() {
        let title = if current_title.is_empty() {
            current_name.clone()
        } else {
            current_title
        };
        apps.push(AppEntry {
            name: current_name,
            app_type: current_type,
            title,
            icon: current_icon,
            tags: current_tags,
        });
    }
    (autostart, apps)
}

pub const WALLPAPER_baram_PNG: &[u8] = include_bytes!("../../../data/wallpaper/baram.png");
pub const WALLPAPER_HANUL_PNG: &[u8] = include_bytes!("../../../data/wallpaper/hanul.png");
pub const WALLPAPER_REFLECT_PNG: &[u8] = include_bytes!("../../../data/wallpaper/reflect.png");
pub const WALLPAPERS: &[&[u8]] = &[
    WALLPAPER_baram_PNG,
    WALLPAPER_HANUL_PNG,
    WALLPAPER_REFLECT_PNG,
];

pub fn decode_wallpaper(bytes: &[u8], screen_w: usize, screen_h: usize) -> Option<Vec<u32>> {
    let (header, pixels) = png_decoder::decode(bytes).ok()?;
    let img_w = header.width as usize;
    let img_h = header.height as usize;
    let mut buf = alloc::vec![0u32; screen_w * screen_h];
    let scale = if screen_w * img_h > screen_h * img_w {
        screen_w as f64 / img_w as f64
    } else {
        screen_h as f64 / img_h as f64
    };
    let src_w = (screen_w as f64 / scale) as usize;
    let src_h = (screen_h as f64 / scale) as usize;
    let src_x = (img_w.saturating_sub(src_w)) / 2;
    let src_y = (img_h.saturating_sub(src_h)) / 2;
    for y in 0..screen_h {
        let sy = (y * src_h / screen_h).min(src_h - 1) + src_y;
        let src_row = sy * img_w;
        let dst_row = y * screen_w;
        for x in 0..screen_w {
            let sx = (x * src_w / screen_w).min(src_w - 1) + src_x;
            let px = pixels[src_row + sx];
            buf[dst_row + x] = Color::rgb(px[0], px[1], px[2]).0;
        }
    }
    Some(buf)
}

pub fn make_solid_wallpaper(color: u32, screen_w: usize, screen_h: usize) -> Vec<u32> {
    alloc::vec![color; screen_w * screen_h]
}

static mut TB_BTN_CACHE: [Option<(usize, Vec<u32>)>; 4] = [None, None, None, None];

fn get_or_render_tb_btn(size: usize, ca: u32) -> &'static [u32] {
    let slot_idx = match ca {
        255 => 0,
        100 => 1,
        128 => 2,
        _ => 3,
    };
    unsafe {
        if let Some((cached_size, ref pixels)) = TB_BTN_CACHE[slot_idx] {
            if cached_size == size {
                return pixels;
            }
        }
        let mut pixels = alloc::vec![0u32; size * size];
        let r_f = size as f32 / 2.0;
        for py in 0..size {
            for px in 0..size {
                let dx = px as f32 + 0.5 - r_f;
                let dy = py as f32 + 0.5 - r_f;
                let dist_sq = dx * dx + dy * dy;
                let alpha = if dist_sq < (r_f - 1.0) * (r_f - 1.0) {
                    1.0f32
                } else if dist_sq > (r_f + 0.5) * (r_f + 0.5) {
                    0.0
                } else {
                    let dist = libm::sqrtf(dist_sq);
                    (r_f + 0.5 - dist).clamp(0.0, 1.0)
                };
                if alpha <= 0.0 {
                    continue;
                }
                let a = (alpha * ca as f32) as u32;
                pixels[py * size + px] = (a << 24) | 0x00FF_FFFF;
            }
        }
        TB_BTN_CACHE[slot_idx] = Some((size, pixels));
        TB_BTN_CACHE[slot_idx].as_ref().unwrap().1.as_slice()
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}

fn draw_taskbar_glyph(
    layer: &mut LayerSystem,
    data: &[u8],
    glyph_w: i32,
    glyph_h: i32,
    x: usize,
    top: i32,
    color: Color,
) {
    let w = layer.width();
    let h = layer.height();
    let buf = layer.buf_mut();
    for row in 0..glyph_h {
        let py = top + row;
        if py < 0 || py >= h as i32 {
            continue;
        }
        for col in 0..glyph_w {
            let px = x + col as usize;
            if px >= w {
                continue;
            }
            let a = data[(row * glyph_w + col) as usize] as u32;
            if a == 0 {
                continue;
            }
            let idx = py as usize * w + px;
            let bg = buf[idx];
            let inv = 255 - a;
            let r = (((color.0 >> 16) & 0xff) * a + ((bg >> 16) & 0xff) * inv) / 255;
            let g = (((color.0 >> 8) & 0xff) * a + ((bg >> 8) & 0xff) * inv) / 255;
            let b = ((color.0 & 0xff) * a + (bg & 0xff) * inv) / 255;
            buf[idx] = (r << 16) | (g << 8) | b;
        }
    }
}

fn draw_taskbar_text(
    layer: &mut LayerSystem,
    text: &str,
    mut x: usize,
    baseline_y: i32,
    color: Color,
    size: f32,
) {
    for ch in text.chars() {
        let glyph = baram_font::ttf_font_hud::glyph_at_size(ch, size);
        if glyph.w > 0 && glyph.h > 0 {
            draw_taskbar_glyph(
                layer,
                &glyph.data,
                glyph.w,
                glyph.h,
                x,
                baseline_y + glyph.y_off,
                color,
            );
            x += glyph.advance.max(0) as usize;
            continue;
        }
        // Google Sans used by the taskbar has no Japanese glyphs. Fall back
        // to the regular UI font so placeholder and typed Japanese stay visible.
        let fallback = baram_font::ttf_font::glyph_at_size(ch, size);
        if fallback.w > 0 && fallback.h > 0 {
            draw_taskbar_glyph(
                layer,
                &fallback.data,
                fallback.w,
                fallback.h,
                x,
                baseline_y + fallback.y_off,
                color,
            );
            x += fallback.advance.max(0) as usize;
        } else {
            x += 8;
        }
    }
}

fn draw_taskbar_search(layer: &mut LayerSystem, search_focused: bool, search_query: &str) {
    let search_x = 12usize;
    let search_h = 32usize;
    let search_y = (TASKBAR_H - search_h) / 2;
    let search_w = 190usize;
    let search_bg = config::get_color("ui-theme/color/panel", Color::PANEL);
    let search_alpha = if search_focused { 255 } else { 128 };
    draw_control_shadow(
        layer,
        search_x,
        search_y,
        search_w,
        search_h,
        search_h / 2,
        2,
        0x33,
    );
    blend_rounded_rect(
        layer,
        search_x,
        search_y,
        search_w,
        search_h,
        search_h / 2,
        search_bg,
        search_alpha,
    );
    let text = if search_query.is_empty() {
        "アプリを検索..."
    } else {
        search_query
    };
    let text_color = if search_query.is_empty() {
        config::get_color("ui-theme/color/muted", Color::MUTED)
    } else {
        config::get_color("ui-theme/color/text", Color::TEXT)
    };
    draw_taskbar_text(
        layer,
        text,
        search_x + 12,
        search_y as i32 + 22,
        text_color,
        18.0,
    );
}

fn redraw_taskbar_search(surface: &mut TaskbarSurface, search_focused: bool, search_query: &str) {
    const SEARCH_DAMAGE_W: usize = 226;
    let w = surface.layer.width();
    let copy_w = SEARCH_DAMAGE_W.min(w);
    if surface.base_valid {
        for y in 0..TASKBAR_H {
            let start = y * w;
            surface.layer.buf_mut()[start..start + copy_w]
                .copy_from_slice(&surface.base[start..start + copy_w]);
        }
    }
    draw_taskbar_search(&mut surface.layer, search_focused, search_query);
    surface.search_dirty = false;
}

fn redraw_taskbar(
    surface: &mut TaskbarSurface,
    wm: &WindowManager,
    add_progress: f32,
    shift_x: f32,
    _hover_apps_icon: bool,
    search_focused: bool,
    search_query: &str,
    clock_hh: u8,
    clock_mm: u8,
    battery_pct: Option<u8>,
    ime_hiragana: bool,
) {
    let layer = &mut surface.layer;
    let w = layer.width();
    if surface.base_valid {
        layer.buf_mut().copy_from_slice(&surface.base);
    } else {
        layer.clear(config::get_color("ui-theme/color/taskbar", Color::TASKBAR));
    }

    let count = wm.count();
    let btn_d = 40usize;
    let btn_gap = 12i32;
    let total_w = count as i32 * (btn_d as i32 + btn_gap) - btn_gap;
    let base_bx = ((w as i32 - total_w) / 2).max(0);
    let btn_y = (TASKBAR_H - btn_d) / 2;
    let add_offset_y = if add_progress >= 0.0 {
        ((1.0 - ease_out_cubic(add_progress)) * (TASKBAR_H + 8) as f32) as usize
    } else {
        0
    };

    for i in 0..count {
        let Some(id) = wm.insertion_id_at(i) else {
            continue;
        };
        let icon_name = wm.get_icon_name(id);
        let is_focused = wm.focused_id == Some(id);
        let is_minimized = wm.is_minimized(id);
        let scaled_d = btn_d;
        let offset = if add_progress >= 0.0 && i == count - 1 {
            add_offset_y
        } else {
            0
        };
        let bx = base_bx + shift_x as i32 + i as i32 * (btn_d as i32 + btn_gap);
        let cached_btn = get_or_render_tb_btn(scaled_d, if is_focused { 255 } else { 100 });
        for py in 0..scaled_d {
            let dst_y = btn_y + offset + py;
            if dst_y >= TASKBAR_H {
                continue;
            }
            for px in 0..scaled_d {
                let a = (cached_btn[py * scaled_d + px] >> 24) & 0xff;
                if a == 0 {
                    continue;
                }
                let dst_x = bx + px as i32;
                if dst_x < 0 || dst_x >= w as i32 {
                    continue;
                }
                let idx = dst_y * w + dst_x as usize;
                let bg = layer.buf_ref()[idx];
                let inv = 255 - a;
                let r = (255 * a + ((bg >> 16) & 0xff) * inv) / 255;
                let g = (255 * a + ((bg >> 8) & 0xff) * inv) / 255;
                let b = (255 * a + (bg & 0xff) * inv) / 255;
                layer.buf_mut()[idx] = (r << 16) | (g << 8) | b;
            }
        }

        let resolved_icon = if icon_name.is_empty() {
            "noname.png"
        } else {
            icon_name
        };
        if let Some(icon) = get_or_decode_icon(resolved_icon, 40) {
            let icon_draw = scaled_d;
            let icon_offset = offset;
            for py in 0..icon_draw {
                let sy = py * icon.h / icon_draw;
                let dst_y = btn_y + icon_offset + py;
                if dst_y >= TASKBAR_H {
                    continue;
                }
                for px in 0..icon_draw {
                    let sx = px * icon.w / icon_draw;
                    let src = icon.pixels[sy * icon.w + sx];
                    let a = src[3] as u32 * if is_minimized { 128 } else { 255 } / 255;
                    if a == 0 {
                        continue;
                    }
                    let dst_x = bx + px as i32;
                    if dst_x < 0 || dst_x >= w as i32 {
                        continue;
                    }
                    let idx = dst_y * w + dst_x as usize;
                    let bg = layer.buf_ref()[idx];
                    let inv = 255 - a;
                    let r = (src[0] as u32 * a + ((bg >> 16) & 0xff) * inv) / 255;
                    let g = (src[1] as u32 * a + ((bg >> 8) & 0xff) * inv) / 255;
                    let b = (src[2] as u32 * a + (bg & 0xff) * inv) / 255;
                    layer.buf_mut()[idx] = (r << 16) | (g << 8) | b;
                }
            }
        }
    }

    let time_bytes = [
        b'0' + clock_hh / 10,
        b'0' + clock_hh % 10,
        b':',
        b'0' + clock_mm / 10,
        b'0' + clock_mm % 10,
    ];
    let time = unsafe { core::str::from_utf8_unchecked(&time_bytes) };
    let mut battery_bytes = [0u8; 4];
    let battery = battery_pct.map(|pct| {
        let len;
        if pct >= 100 {
            battery_bytes.copy_from_slice(b"100%");
            len = 4;
        } else if pct >= 10 {
            battery_bytes[0] = b'0' + pct / 10;
            battery_bytes[1] = b'0' + pct % 10;
            battery_bytes[2] = b'%';
            len = 3;
        } else {
            battery_bytes[0] = b'0' + pct % 10;
            battery_bytes[1] = b'%';
            len = 2;
        }
        unsafe { core::str::from_utf8_unchecked(&battery_bytes[..len]) }
    });

    let size = TASKBAR_STATUS_SIZE;
    let measure = |text: &str| -> usize {
        text.chars()
            .map(|ch| {
                let g = baram_font::ttf_font_hud::glyph_at_size(ch, size);
                if g.w > 0 {
                    g.advance.max(0) as usize
                } else {
                    8
                }
            })
            .sum()
    };
    let gap = 12usize;
    let battery_width = battery.map_or(0, |text| gap + measure(text));
    let status_x = w.saturating_sub(measure(time) + battery_width + 16);
    let baseline = TASKBAR_H as i32 - baram_font::ttf_font_hud::ascent_at_size(size) + 9;
    let status_color = config::get_color("ui-theme/color/text", Color::TEXT);
    let ime_x = status_x.saturating_sub(IME_BUTTON_W + gap);
    let ime_y = (TASKBAR_H - 34) / 2;
    draw_control_shadow(layer, ime_x, ime_y, IME_BUTTON_W, 34, 17, 1, 0x22);
    blend_rounded_rect(
        layer,
        ime_x,
        ime_y,
        IME_BUTTON_W,
        34,
        17,
        config::get_color("ui-theme/color/panel", Color::PANEL),
        176,
    );
    let ime_label = if ime_hiragana { "あ" } else { "A" };
    let ime_width = measure(ime_label);
    draw_taskbar_text(
        layer,
        ime_label,
        ime_x + (IME_BUTTON_W.saturating_sub(ime_width)) / 2,
        baseline,
        status_color,
        size,
    );
    draw_taskbar_text(layer, time, status_x, baseline, status_color, size);
    if let Some(battery) = battery {
        draw_taskbar_text(
            layer,
            battery,
            status_x + measure(time) + gap,
            baseline,
            status_color,
            size,
        );
    }

    draw_taskbar_search(layer, search_focused, search_query);
    layer.mark_all_dirty();
    surface.valid = true;
    surface.search_dirty = false;
}

fn draw_ime_candidates(
    layer: &mut LayerSystem,
    taskbar_y: usize,
    reading: &str,
    candidates: &[alloc::string::String],
    selected: usize,
) {
    if candidates.is_empty() || taskbar_y < 76 {
        return;
    }
    let x = 24usize;
    let y = taskbar_y - 76;
    let width = 440usize.min(layer.width().saturating_sub(x * 2));
    let height = 64usize;
    draw_control_shadow(layer, x, y, width, height, 12, 2, 0x36);
    blend_rounded_rect(
        layer,
        x,
        y,
        width,
        height,
        12,
        config::get_color("ui-theme/color/panel", Color::PANEL),
        242,
    );
    let text_color = config::get_color("ui-theme/color/text", Color::TEXT);
    let accent = config::get_color("ui-theme/color/btn_primary", Color::BTN_PRIMARY);
    let mut heading = alloc::string::String::from("変換中: ");
    heading.push_str(reading);
    layer.put_str(x + 14, y + 8, &heading, text_color);

    let mut cx = x + 14;
    for (index, candidate) in candidates.iter().enumerate() {
        let candidate_width = candidate.chars().count() * 16 + 18;
        if cx + candidate_width > x + width - 10 {
            break;
        }
        if index == selected {
            blend_rounded_rect(layer, cx, y + 30, candidate_width, 25, 6, accent, 48);
        }
        layer.put_str(cx + 8, y + 34, candidate, text_color);
        if index == selected {
            // The underline marks the candidate currently composing in the
            // target input. Enter commits this underlined candidate.
            layer.fill_rect(cx + 7, y + 52, candidate_width.saturating_sub(14), 2, accent);
        }
        cx += candidate_width + 6;
    }
}

/// Bounds of the taskbar IME toggle, kept in sync with the status layout.
pub fn ime_button_bounds(width: usize, battery_pct: Option<u8>) -> (i32, i32, i32, i32) {
    let measure = |text: &str| -> usize {
        text.chars()
            .map(|ch| {
                let g = baram_font::ttf_font_hud::glyph_at_size(ch, TASKBAR_STATUS_SIZE);
                if g.w > 0 {
                    g.advance.max(0) as usize
                } else {
                    8
                }
            })
            .sum()
    };
    let battery_len = battery_pct.map_or(0, |pct| {
        if pct >= 100 {
            4
        } else if pct >= 10 {
            3
        } else {
            2
        }
    });
    let battery_width = if battery_len == 0 {
        0
    } else {
        12 + measure(&"0000"[..battery_len])
    };
    let status_x = width.saturating_sub(measure("00:00") + battery_width + 16);
    let x = status_x.saturating_sub(IME_BUTTON_W + 12) as i32;
    (x, (TASKBAR_H as i32 - 34) / 2, IME_BUTTON_W as i32, 34)
}

pub fn render_scene(
    layer: &mut LayerSystem,
    taskbar: &mut TaskbarSurface,
    wm: &mut WindowManager,
    _mouse_ev: u32,
    key_ev: u32,
    fps: u32,
    _mouse_mode: &str,
    ui_commands: &[uiscript::Command],
    ui_win_id: Option<WinId>,
    warp_engines: &mut alloc::vec::Vec<(WinId, WarpEngine)>,
    html_engines: &mut alloc::vec::Vec<(WinId, HtmlEngine)>,
    wallpaper: Option<&[u32]>,
    cached_launcher_layer: &mut Option<Vec<u32>>,
    taskbar_dirty: bool,
    add_progress: f32,
    remove_progress: f32,
    shift_x: f32,
    hud_enabled: bool,
    bg_cache: &mut Option<Vec<u32>>,
    bg_cache_valid: bool,
    show_app_launcher: bool,
    app_list: &[alloc::string::String],
    app_icon_list: &[alloc::string::String],
    hover_apps_icon: bool,
    search_focused: bool,
    search_query: &str,
    launcher_scroll_y: usize,
    launcher_anim_phase: i8,
    launcher_anim_elapsed_ms: u32,
    launcher_scroll_changed: bool,
    launcher_only_redraw: bool,
    taskbar_only: bool,
    clock_hh: u8,
    clock_mm: u8,
    battery_pct: Option<u8>,
    ime_hiragana: bool,
    ime_reading: Option<&str>,
    ime_candidates: &[alloc::string::String],
    ime_selected: usize,
) {
    let w = layer.width();
    let h = layer.height();
    let tb_y = h.saturating_sub(TASKBAR_H);

    // During launcher-only animation frames, restore the small captured
    // underlay instead of rebuilding the wallpaper, HUD, and every window.
    if launcher_only_redraw {
        let grid_h = 3 * (52 + 20 + 16);
        let grid_y = h.saturating_sub(TASKBAR_H + grid_h + 16);
        let panel_x = 12usize;
        let panel_y = grid_y.saturating_sub(8);
        let panel_w = 4 * (52 + 16) + 16;
        let panel_h = grid_h + 16;
        const CACHE_PAD: usize = 54;
        let cache_x = panel_x.saturating_sub(CACHE_PAD);
        let cache_y = panel_y.saturating_sub(CACHE_PAD);
        let cache_x1 = (panel_x + panel_w + CACHE_PAD).min(w);
        let cache_y1 = (panel_y + panel_h + CACHE_PAD).min(h);
        let cache_w = cache_x1.saturating_sub(cache_x);
        let cache_h = cache_y1.saturating_sub(cache_y);
        let cache_len = cache_w * cache_h;
        if let Some(cache) = cached_launcher_layer.as_deref() {
            if cache.len() == cache_len * 4 {
                layer.copy_rect_buffer(
                    &cache[cache_len * 2..cache_len * 3],
                    cache_w,
                    cache_h,
                    cache_x,
                    cache_y,
                );
            }
        }
    }

    if !taskbar_only && !launcher_only_redraw {
        if bg_cache_valid {
            if let Some(ref cached) = bg_cache {
                layer.copy_from_screen_buffer(cached);
            }
        } else if let Some(pixels) = wallpaper {
            layer.copy_from_screen_buffer(pixels);
        } else {
            layer.clear(config::get_color("ui-theme/color/bg", Color::BG));
        }

        if !bg_cache_valid {
            let mut bg = alloc::vec![0u32; w * h];
            bg.copy_from_slice(layer.buf_ref());
            *bg_cache = Some(bg);
        }

        if !bg_cache_valid || !taskbar.base_valid {
            taskbar.refresh_wallpaper_blur(layer, tb_y);
        }
    }

    let mut fb = FmtBuf::new();
    fb.push_str("Key:");
    fb.push_u32(key_ev);
    fb.push_str(" Window:");
    fb.push_u32(wm.count() as u32);
    fb.push_str(" ");
    fb.push_u32(fps);
    fb.push_str("FPS");

    if hud_enabled && !taskbar_only && !launcher_only_redraw {
        if let Some(ref bg) = bg_cache {
            let hud_y0 = (tb_y as i32 - 44).max(0) as usize;
            let hud_y1 = tb_y;
            for y in hud_y0..hud_y1 {
                let s = y * w;
                let e = s + w;
                if e <= bg.len() {
                    layer.buf_mut()[s..e].copy_from_slice(&bg[s..e]);
                }
            }
        }

        let hud_text1 = "Baram OS (1.2)";
        let mut hw1 = 0usize;
        for ch in hud_text1.chars() {
            if baram_font::ttf_font_hud::is_available() {
                let g = baram_font::ttf_font_hud::glyph(ch);
                hw1 += if g.w > 0 {
                    g.advance.max(0) as usize
                } else {
                    8
                };
            } else {
                hw1 += 8;
            }
        }
        layer.put_str_hud(
            w - hw1 - 16,
            tb_y - 28,
            hud_text1,
            config::get_color("ui-theme/color/muted", Color::MUTED),
        );

        let s2 = fb.as_str();
        let mut hw2 = 0usize;
        for ch in s2.chars() {
            if baram_font::ttf_font_hud::is_available() {
                let g = baram_font::ttf_font_hud::glyph(ch);
                hw2 += if g.w > 0 {
                    g.advance.max(0) as usize
                } else {
                    8
                };
            } else {
                hw2 += 8;
            }
        }
        layer.put_str_hud(
            w - hw2 - 16,
            tb_y - 12,
            s2,
            config::get_color("ui-theme/color/muted", Color::MUTED),
        );
    }

    // Stable z-order: background -> HUD -> windows/launcher -> taskbar.
    if !taskbar_only && !launcher_only_redraw {
        wm.draw_all(
            layer,
            ui_win_id.map(|id| (id, ui_commands)),
            warp_engines,
            html_engines,
        );
    }

    if show_app_launcher {
        let cols = 4usize;
        let icon_size = 52usize;
        let icon_gap = 16usize;
        let label_h = 20usize;
        let cell_w = icon_size + icon_gap;
        let cell_h = icon_size + label_h + icon_gap;
        let grid_w = cols * cell_w;
        let visible_rows = 3usize;
        let grid_h = visible_rows * cell_h;
        let grid_x = 20usize;
        let grid_y = h.saturating_sub(TASKBAR_H + grid_h + 16);
        let content_y = grid_y + 4;
        let panel_h = grid_h.max(40) + 16;
        let panel_x = 12usize;
        let panel_y = grid_y.saturating_sub(8);
        let panel_w = grid_w + 16;
        let panel_radius = 26usize;
        let launcher_alpha = if launcher_anim_phase > 0 {
            let t = (launcher_anim_elapsed_ms as f32 / 200.0).clamp(0.0, 1.0);
            (ease_out_cubic(t) * 255.0) as u32
        } else if launcher_anim_phase < 0 {
            let t = (launcher_anim_elapsed_ms as f32 / 200.0).clamp(0.0, 1.0);
            ((1.0 - t * t * t) * 255.0) as u32
        } else {
            255
        };
        // The complete launcher is cached as one layer. Animation only
        // changes this layer's position and global opacity.
        let launcher_offset_y = if launcher_anim_phase > 0 {
            let t = (launcher_anim_elapsed_ms as f32 / 200.0).clamp(0.0, 1.0);
            ((1.0 - ease_out_cubic(t)) * 16.0) as usize
        } else {
            0
        };

        let building_launcher_cache = cached_launcher_layer.is_none();
        let rebuild_launcher_content = building_launcher_cache || launcher_scroll_changed;
        // Cache the complete glass-and-shadow base once per opening.
        if building_launcher_cache {
            const CACHE_PAD: usize = 54;
            let cache_x = panel_x.saturating_sub(CACHE_PAD);
            let cache_y = panel_y.saturating_sub(CACHE_PAD);
            let cache_x1 = (panel_x + panel_w + CACHE_PAD).min(w);
            let cache_y1 = (panel_y + panel_h + CACHE_PAD).min(h);
            let cache_w = cache_x1.saturating_sub(cache_x);
            let cache_h = cache_y1.saturating_sub(cache_y);
            let mut panel_base = LayerSystem::new(cache_w, cache_h);
            for py in 0..cache_h {
                let src_start = (cache_y + py) * w + cache_x;
                let dst_start = py * cache_w;
                panel_base.buf_mut()[dst_start..dst_start + cache_w]
                    .copy_from_slice(&layer.buf_ref()[src_start..src_start + cache_w]);
            }
            // Two box-blur passes with r=13 can read up to 26px beyond
            // the result. Keep that margin around the panel, rather than
            // filtering the entire screen.
            const BLUR_RADIUS: usize = 26;
            let blur_x0 = panel_x.saturating_sub(BLUR_RADIUS);
            let blur_y0 = panel_y.saturating_sub(BLUR_RADIUS);
            let blur_x1 = (panel_x + panel_w + BLUR_RADIUS).min(w);
            let blur_y1 = (panel_y + panel_h + BLUR_RADIUS).min(h);
            let blur_w = blur_x1.saturating_sub(blur_x0);
            let blur_h = blur_y1.saturating_sub(blur_y0);
            let mut blur_source = alloc::vec![0u32; blur_w * blur_h];
            for py in 0..blur_h {
                let src_start = (blur_y0 + py) * w + blur_x0;
                let dst_start = py * blur_w;
                blur_source[dst_start..dst_start + blur_w]
                    .copy_from_slice(&layer.buf_ref()[src_start..src_start + blur_w]);
            }
            let mut blurred = alloc::vec![0u32; blur_source.len()];
            blur::blur_region_to(&blur_source, &mut blurred, blur_w, 0, blur_h, 26);
            let underlay = panel_base.buf_ref().to_vec();
            draw_soft_box_shadow(
                &mut panel_base,
                panel_x - cache_x,
                panel_y - cache_y,
                panel_w,
                panel_h,
                panel_radius,
            );
            copy_rounded_region_from_crop(
                &mut panel_base,
                &blurred,
                blur_w,
                blur_x0 - cache_x,
                blur_y0 - cache_y,
                panel_x - cache_x,
                panel_y - cache_y,
                panel_w,
                panel_h,
                panel_radius,
            );
            blend_rounded_rect(
                &mut panel_base,
                panel_x - cache_x,
                panel_y - cache_y,
                panel_w,
                panel_h,
                panel_radius,
                Color::rgb(0xf5, 0xf5, 0xf5),
                150,
            );
            let panel = panel_base.buf_ref();
            let mut cache = Vec::with_capacity(panel.len() * 4);
            cache.extend_from_slice(panel); // fully composed launcher
            cache.extend_from_slice(panel); // fixed glass background
            cache.extend_from_slice(&underlay); // captured screen below it
            cache.extend_from_slice(panel); // per-frame layer scratch
            *cached_launcher_layer = Some(cache);
        }
        let (clip_x0, clip_y0, clip_x1, clip_y1) = layer.clip_bounds();
        if rebuild_launcher_content {
            if let Some(panel_base) = cached_launcher_layer.as_deref() {
                const CACHE_PAD: usize = 54;
                let cache_x = panel_x.saturating_sub(CACHE_PAD);
                let cache_y = panel_y.saturating_sub(CACHE_PAD);
                let cache_x1 = (panel_x + panel_w + CACHE_PAD).min(w);
                let cache_y1 = (panel_y + panel_h + CACHE_PAD).min(h);
                let cache_w = cache_x1.saturating_sub(cache_x);
                let cache_h = cache_y1.saturating_sub(cache_y);
                if panel_base.len() == cache_w * cache_h * 4 {
                    let panel_start = cache_w * cache_h;
                    for py in 0..cache_h {
                        let dst_y = cache_y + py;
                        if dst_y < clip_y0 || dst_y >= clip_y1 {
                            continue;
                        }
                        let src_start = panel_start + py * cache_w;
                        let draw_x0 = cache_x.max(clip_x0);
                        let draw_x1 = cache_x1.min(clip_x1);
                        if draw_x0 >= draw_x1 {
                            continue;
                        }
                        let src_start = src_start + draw_x0 - cache_x;
                        let dst_start = dst_y * w + draw_x0;
                        let draw_w = draw_x1 - draw_x0;
                        layer.buf_mut()[dst_start..dst_start + draw_w]
                            .copy_from_slice(&panel_base[src_start..src_start + draw_w]);
                    }
                }
            }

            if app_list.is_empty() {
                layer.put_str(
                    28,
                    content_y + 8,
                    "該当するアプリはありません",
                    Color::BLACK,
                );
            }

            let content_rows = ((app_list.len() + cols - 1) / cols).max(visible_rows);
            let content_h = content_rows * cell_h;
            let scroll_y = launcher_scroll_y.min(content_h.saturating_sub(grid_h));
            // Keep the scratch surface bounded to the viewport. The old code
            // allocated and cleared a surface as tall as the complete app
            // list for every animation frame.
            let first_scratch_row = (scroll_y / cell_h).saturating_sub(1);
            let scratch_y = first_scratch_row * cell_h;
            let viewport_src_y = scroll_y - scratch_y;
            let scratch_h = (grid_h + cell_h * 2).min(content_h.saturating_sub(scratch_y));
            let mut content = LayerSystem::new_transparent(grid_w, scratch_h);
            // Antialiased pixels must be blended against the actual panel
            // background, not transparent black. Seed the visible viewport
            // before rendering its icons and labels.
            for py in 0..scratch_h {
                let screen_y = content_y.saturating_add(py).saturating_sub(viewport_src_y);
                if screen_y >= h {
                    continue;
                }
                let src_start = screen_y * w + grid_x;
                let dst_start = py * grid_w;
                content.buf_mut()[dst_start..dst_start + grid_w]
                    .copy_from_slice(&layer.buf_ref()[src_start..src_start + grid_w]);
            }
            for (i, name) in app_list.iter().enumerate() {
                let col = i % cols;
                let row = i / cols;
                let cx = col * cell_w + icon_gap / 2;
                let item_y = row * cell_h;
                if item_y + cell_h <= scratch_y || item_y >= scratch_y + scratch_h {
                    continue;
                }
                let cy = item_y - scratch_y;

                content.fill_circle(
                    cx + icon_size / 2,
                    cy + icon_size / 2,
                    icon_size / 2,
                    Color::rgb(0xff, 0xff, 0xff),
                );

                let icon_name = app_icon_list.get(i).map(|s| s.as_str()).unwrap_or("");
                let resolved_icon = if icon_name.is_empty() || icon_name == "null" {
                    "noname.png"
                } else {
                    icon_name
                };
                {
                    if let Some(icon) = get_or_decode_icon(resolved_icon, icon_size) {
                        let pad = (icon_size - icon.w) / 2;
                        for py in 0..icon.h {
                            for px in 0..icon.w {
                                let src_px = icon.pixels[py * icon.w + px];
                                let a = src_px[3] as u32;
                                if a == 0 {
                                    continue;
                                }
                                let sx = cx + pad + px;
                                let sy = cy + pad + py;
                                if sx >= grid_w || sy >= scratch_h {
                                    continue;
                                }
                                let idx = sy * grid_w + sx;
                                let bg = Color(content.buf_ref()[idx]);
                                let inv = 255 - a;
                                let r = (src_px[0] as u32 * a + bg.r() as u32 * inv) / 255;
                                let g = (src_px[1] as u32 * a + bg.g() as u32 * inv) / 255;
                                let b = (src_px[2] as u32 * a + bg.b() as u32 * inv) / 255;
                                content.buf_mut()[idx] = Color::rgb(r as u8, g as u8, b as u8).0;
                            }
                        }
                    }
                }

                let char_w = 8usize;
                let max_chars = icon_size / char_w;
                let char_count = name.chars().count();
                let display_name = if char_count > max_chars {
                    let truncated_len = max_chars.saturating_sub(3);
                    let mut s = alloc::string::String::with_capacity(max_chars * 4);
                    for ch in name.chars().take(truncated_len) {
                        s.push(ch);
                    }
                    s.push_str("...");
                    s
                } else {
                    name.clone()
                };
                let mut tw = 0usize;
                for ch in display_name.chars() {
                    if baram_font::ttf_font::is_available() {
                        let g = baram_font::ttf_font::glyph(ch);
                        if g.w > 0 {
                            tw += g.advance.max(0) as usize;
                        } else {
                            tw += char_w;
                        }
                    } else {
                        tw += char_w;
                    }
                }
                let tx = cx + (icon_size.saturating_sub(tw)) / 2;
                let ty = cy + icon_size + 4;
                let label_color = Color::BLACK;
                content.put_str(tx, ty, &display_name, label_color);
            }
            layer.composit_rect_opaque(
                &content,
                grid_x,
                content_y,
                0,
                viewport_src_y,
                grid_w,
                grid_h,
            );

            // Replace the first half with the fully composed launcher
            // (glass, icons, and labels), then put the captured underlay back.
            const CACHE_PAD: usize = 54;
            let cache_x = panel_x.saturating_sub(CACHE_PAD);
            let cache_y = panel_y.saturating_sub(CACHE_PAD);
            let cache_x1 = (panel_x + panel_w + CACHE_PAD).min(w);
            let cache_y1 = (panel_y + panel_h + CACHE_PAD).min(h);
            let cache_w = cache_x1.saturating_sub(cache_x);
            let cache_h = cache_y1.saturating_sub(cache_y);
            let cache_len = cache_w * cache_h;
            if let Some(cache) = cached_launcher_layer.as_mut() {
                if cache.len() == cache_len * 4 {
                    for py in 0..cache_h {
                        let src_start = (cache_y + py) * w + cache_x;
                        let dst_start = py * cache_w;
                        cache[dst_start..dst_start + cache_w]
                            .copy_from_slice(&layer.buf_ref()[src_start..src_start + cache_w]);
                    }
                }
            }
            if let Some(cache) = cached_launcher_layer.as_deref() {
                if cache.len() == cache_len * 4 {
                    layer.copy_rect_buffer(
                        &cache[cache_len * 2..cache_len * 3],
                        cache_w,
                        cache_h,
                        cache_x,
                        cache_y,
                    );
                }
            }
        }

        // Build one launcher layer from a fixed glass background plus the
        // cached app pixels shifted inside it. Then apply opacity once to
        // that entire layer through the SIMD compositor.
        if let Some(cache) = cached_launcher_layer.as_mut() {
            const CACHE_PAD: usize = 54;
            let cache_x = panel_x.saturating_sub(CACHE_PAD);
            let cache_y = panel_y.saturating_sub(CACHE_PAD);
            let cache_x1 = (panel_x + panel_w + CACHE_PAD).min(w);
            let cache_y1 = (panel_y + panel_h + CACHE_PAD).min(h);
            let cache_w = cache_x1.saturating_sub(cache_x);
            let cache_h = cache_y1.saturating_sub(cache_y);
            let cache_len = cache_w * cache_h;
            if cache.len() == cache_len * 4 && launcher_alpha != 0 {
                if launcher_anim_phase == 0 {
                    layer.copy_rect_buffer(&cache[..cache_len], cache_w, cache_h, cache_x, cache_y);
                    // No opacity or internal motion remains in steady and
                    // scroll frames, so the finished cache is final.
                } else if launcher_anim_phase < 0 {
                    // Closing has no internal translation. Feed the final
                    // cached launcher straight into the SIMD alpha pass.
                    layer.composit_rect_global_alpha(
                        &cache[..cache_len],
                        cache_w,
                        cache_h,
                        cache_x,
                        cache_y,
                        launcher_alpha as u8,
                    );
                } else {
                    cache.copy_within(cache_len..cache_len * 2, cache_len * 3);

                    let content_x = grid_x - cache_x;
                    let content_base_y = content_y - cache_y;
                    for py in 0..grid_h {
                        let dst_py = content_base_y + py + launcher_offset_y;
                        if dst_py >= cache_h {
                            continue;
                        }
                        let src_row = (content_base_y + py) * cache_w + content_x;
                        let dst_row = dst_py * cache_w + content_x;
                        for px in 0..grid_w {
                            let src = src_row + px;
                            if cache[src] != cache[cache_len + src] {
                                cache[cache_len * 3 + dst_row + px] = cache[src];
                            }
                        }
                    }

                    layer.composit_rect_global_alpha(
                        &cache[cache_len * 3..cache_len * 4],
                        cache_w,
                        cache_h,
                        cache_x,
                        cache_y,
                        launcher_alpha as u8,
                    );
                }
            }
        }
    }

    if taskbar_dirty || !taskbar.is_valid() {
        redraw_taskbar(
            taskbar,
            wm,
            add_progress,
            shift_x,
            hover_apps_icon,
            search_focused,
            search_query,
            clock_hh,
            clock_mm,
            battery_pct,
            ime_hiragana,
        );
    } else if taskbar.is_search_dirty() {
        redraw_taskbar_search(taskbar, search_focused, search_query);
    }
    if let Some(reading) = ime_reading {
        draw_ime_candidates(layer, tb_y, reading, ime_candidates, ime_selected);
    }
    taskbar.composite_onto(layer, tb_y);
}

pub fn render_frame(
    layer: &mut LayerSystem,
    taskbar: &mut TaskbarSurface,
    wm: &mut WindowManager,
    _last_keys: &[&'static str],
    _mouse_ev: u32,
    key_ev: u32,
    fps: u32,
    mouse_mode: &str,
    cursor_x: i32,
    cursor_y: i32,
    ui_commands: &[uiscript::Command],
    ui_win_id: Option<WinId>,
    warp_engines: &mut alloc::vec::Vec<(WinId, WarpEngine)>,
    html_engines: &mut alloc::vec::Vec<(WinId, HtmlEngine)>,
    wallpaper: Option<&[u32]>,
    cached_launcher_layer: &mut Option<Vec<u32>>,
    taskbar_dirty: bool,
    add_progress: f32,
    remove_progress: f32,
    shift_x: f32,
    pointer_size: f32,
    hud_enabled: bool,
    bg_cache: &mut Option<Vec<u32>>,
    bg_cache_valid: bool,
    show_app_launcher: bool,
    app_list: &[alloc::string::String],
    app_icon_list: &[alloc::string::String],
    hover_apps_icon: bool,
    search_focused: bool,
    search_query: &str,
    launcher_scroll_y: usize,
    launcher_anim_phase: i8,
    launcher_anim_elapsed_ms: u32,
    launcher_scroll_changed: bool,
    launcher_only_redraw: bool,
    taskbar_only: bool,
    clock_hh: u8,
    clock_mm: u8,
    battery_pct: Option<u8>,
    ime_hiragana: bool,
    ime_reading: Option<&str>,
    ime_candidates: &[alloc::string::String],
    ime_selected: usize,
) {
    render_scene(
        layer,
        taskbar,
        wm,
        _mouse_ev,
        key_ev,
        fps,
        mouse_mode,
        ui_commands,
        ui_win_id,
        warp_engines,
        html_engines,
        wallpaper,
        cached_launcher_layer,
        taskbar_dirty,
        add_progress,
        remove_progress,
        shift_x,
        hud_enabled,
        bg_cache,
        bg_cache_valid,
        show_app_launcher,
        app_list,
        app_icon_list,
        hover_apps_icon,
        search_focused,
        search_query,
        launcher_scroll_y,
        launcher_anim_phase,
        launcher_anim_elapsed_ms,
        launcher_scroll_changed,
        launcher_only_redraw,
        taskbar_only,
        clock_hh,
        clock_mm,
        battery_pct,
        ime_hiragana,
        ime_reading,
        ime_candidates,
        ime_selected,
    );
    let is_resizing = wm.is_any_resizing();
    cursor::draw_cursor_into_layer(layer, cursor_x, cursor_y, is_resizing, pointer_size);
}

pub fn time_diff_ns(a: &runtime::Time, b: &runtime::Time) -> u64 {
    let a_s = (a.hour() as u64) * 3600 + (a.minute() as u64) * 60 + a.second() as u64;
    let b_s = (b.hour() as u64) * 3600 + (b.minute() as u64) * 60 + b.second() as u64;
    let diff_s = b_s.saturating_sub(a_s);
    diff_s * 1_000_000_000 + (b.nanosecond() as u64).saturating_sub(a.nanosecond() as u64)
}
