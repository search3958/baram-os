use super::cursor::{self};
use crate::warp::WarpEngine;
use crate::html::HtmlEngine;
use crate::window::{WinId, WindowManager};
use alloc::vec;
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

pub struct TaskbarSurface {
    layer: LayerSystem,
    backdrop: Vec<u32>,
    blurred: Vec<u32>,
    blur_scratch: Vec<u32>,
    backdrop_valid: bool,
    valid: bool,
}

impl TaskbarSurface {
    pub fn new(width: usize) -> Self {
        let sample_h = TASKBAR_H + TASKBAR_BLUR_R.max(0) as usize;
        Self {
            layer: LayerSystem::new_transparent(width, TASKBAR_H),
            backdrop: alloc::vec![0; width * sample_h],
            blurred: alloc::vec![0; width * sample_h],
            blur_scratch: alloc::vec![0; width * sample_h],
            backdrop_valid: false,
            valid: false,
        }
    }

    #[inline]
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    #[inline]
    pub fn has_backdrop(&self) -> bool {
        self.backdrop_valid
    }

    fn capture_backdrop(&mut self, scene: &LayerSystem, y: usize) {
        let width = self.layer.width();
        let pad = TASKBAR_BLUR_R.max(0) as usize;
        let start_y = y.saturating_sub(pad);
        let sample_h = TASKBAR_H + pad;
        if scene.width() != width || scene.height() < start_y.saturating_add(sample_h) {
            return;
        }
        self.backdrop.copy_from_slice(
            &scene.buf_ref()[start_y * width..(start_y + sample_h) * width],
        );
        self.backdrop_valid = true;
    }

    fn composite_onto(&self, scene: &mut LayerSystem, y: usize) {
        scene.composit_rect(
            &self.layer,
            0,
            y,
            0,
            0,
            self.layer.width(),
            TASKBAR_H,
        );
    }
}

#[inline]
fn tint_taskbar_scalar(pixels: &mut [u32], color: u32, alpha: u16) {
    let inv = 255u32 - alpha as u32;
    let tr = (color >> 16) & 0xff;
    let tg = (color >> 8) & 0xff;
    let tb = color & 0xff;
    for px in pixels {
        let r = (tr * alpha as u32 + ((*px >> 16) & 0xff) * inv) / 255;
        let g = (tg * alpha as u32 + ((*px >> 8) & 0xff) * inv) / 255;
        let b = (tb * alpha as u32 + (*px & 0xff) * inv) / 255;
        *px = (r << 16) | (g << 8) | b;
    }
}

fn tint_taskbar(pixels: &mut [u32], color: u32, alpha: u16) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::*;
        let zero = _mm_setzero_si128();
        let va = _mm_set1_epi16(alpha as i16);
        let vi = _mm_set1_epi16((255 - alpha) as i16);
        let bias = _mm_set1_epi16(128);
        let vc = _mm_set1_epi32((color & 0x00ff_ffff) as i32);
        let clo = _mm_unpacklo_epi8(vc, zero);
        let chi = _mm_unpackhi_epi8(vc, zero);
        let mut i = 0usize;
        while i + 4 <= pixels.len() {
            let p = _mm_loadu_si128(pixels.as_ptr().add(i) as *const __m128i);
            let plo = _mm_unpacklo_epi8(p, zero);
            let phi = _mm_unpackhi_epi8(p, zero);
            let lo_sum = _mm_add_epi16(_mm_mullo_epi16(plo, vi), _mm_mullo_epi16(clo, va));
            let hi_sum = _mm_add_epi16(_mm_mullo_epi16(phi, vi), _mm_mullo_epi16(chi, va));
            let lo_t = _mm_add_epi16(lo_sum, bias);
            let hi_t = _mm_add_epi16(hi_sum, bias);
            let lo = _mm_srli_epi16(_mm_add_epi16(lo_t, _mm_srli_epi16(lo_t, 8)), 8);
            let hi = _mm_srli_epi16(_mm_add_epi16(hi_t, _mm_srli_epi16(hi_t, 8)), 8);
            let out = _mm_packus_epi16(lo, hi);
            _mm_storeu_si128(pixels.as_mut_ptr().add(i) as *mut __m128i, out);
            i += 4;
        }
        tint_taskbar_scalar(&mut pixels[i..], color, alpha);
        return;
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use core::arch::aarch64::*;
        let color_bytes = vreinterpretq_u8_u32(vdupq_n_u32(color & 0x00ff_ffff));
        let clo = vmovl_u8(vget_low_u8(color_bytes));
        let chi = vmovl_u8(vget_high_u8(color_bytes));
        let va = vdupq_n_u16(alpha);
        let vi = vdupq_n_u16(255 - alpha);
        let bias = vdupq_n_u16(128);
        let mut i = 0usize;
        while i + 4 <= pixels.len() {
            let p = vreinterpretq_u8_u32(vld1q_u32(pixels.as_ptr().add(i)));
            let plo = vmovl_u8(vget_low_u8(p));
            let phi = vmovl_u8(vget_high_u8(p));
            let lo_sum = vaddq_u16(vmulq_u16(plo, vi), vmulq_u16(clo, va));
            let hi_sum = vaddq_u16(vmulq_u16(phi, vi), vmulq_u16(chi, va));
            let lo_t = vaddq_u16(lo_sum, bias);
            let hi_t = vaddq_u16(hi_sum, bias);
            let lo = vshrq_n_u16(vaddq_u16(lo_t, vshrq_n_u16(lo_t, 8)), 8);
            let hi = vshrq_n_u16(vaddq_u16(hi_t, vshrq_n_u16(hi_t, 8)), 8);
            vst1q_u32(
                pixels.as_mut_ptr().add(i),
                vreinterpretq_u32_u8(vcombine_u8(vqmovn_u16(lo), vqmovn_u16(hi))),
            );
            i += 4;
        }
        tint_taskbar_scalar(&mut pixels[i..], color, alpha);
        return;
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    tint_taskbar_scalar(pixels, color, alpha);
}

const ICON_CACHE_CAP: usize = 32;
static mut ICON_CACHE: [Option<(alloc::string::String, usize, IconBitmap)>; ICON_CACHE_CAP] = [
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
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
                        if n == icon_name && *s == size { Some(b) } else { None }
                    } else { None }
                });
            }
        }
        ICON_CACHE[0] = Some((alloc::string::String::from(icon_name), size, bitmap));
        ICON_CACHE.iter().find_map(|e| {
            if let Some((ref n, s, ref b)) = e {
                if n == icon_name && *s == size { Some(b) } else { None }
            } else { None }
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
                });
                current_name.clear();
                current_type = alloc::string::String::from("warp-2");
                current_title.clear();
                current_icon.clear();
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
                    });
                    current_name.clear();
                    current_type = alloc::string::String::from("warp-2");
                    current_title.clear();
                    current_icon.clear();
                }
                in_apps = false;
                continue;
            }
            if trimmed.ends_with(':')
                && !trimmed.contains("icon")
                && !trimmed.contains("type")
                && !trimmed.contains("title")
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
                    });
                }
                current_name = alloc::string::String::from(trimmed.trim_end_matches(':'));
                current_type = alloc::string::String::from("warp-2");
                current_title.clear();
                current_icon.clear();
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

pub fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158f32;
    let c3 = c1 + 1.0;
    let t = t.min(1.0);
    1.0 + c3 * libm::powf(t - 1.0, 3.0) + c1 * libm::powf(t - 1.0, 2.0)
}

fn draw_taskbar_text(
    layer: &mut LayerSystem,
    text: &str,
    mut x: usize,
    baseline_y: i32,
    color: Color,
    size: f32,
) {
    let w = layer.width();
    let h = layer.height();
    for ch in text.chars() {
        let glyph = baram_font::ttf_font_hud::glyph_at_size(ch, size);
        if glyph.w == 0 || glyph.h == 0 {
            x += 8;
            continue;
        }
        let top = baseline_y + glyph.y_off;
        let buf = layer.buf_mut();
        for row in 0..glyph.h {
            let py = top + row;
            if py < 0 || py >= h as i32 {
                continue;
            }
            for col in 0..glyph.w {
                let px = x + col as usize;
                if px >= w {
                    continue;
                }
                let a = glyph.data[(row * glyph.w + col) as usize] as u32;
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
        x += glyph.advance.max(0) as usize;
    }
}

fn redraw_taskbar(
    surface: &mut TaskbarSurface,
    wm: &WindowManager,
    add_progress: f32,
    shift_x: f32,
    hover_apps_icon: bool,
    clock_hh: u8,
    clock_mm: u8,
    battery_pct: u8,
) {
    let layer = &mut surface.layer;
    let w = layer.width();
    let pad = TASKBAR_BLUR_R.max(0) as usize;
    let sample_h = TASKBAR_H + pad;
    blur::blur_region_to_with_scratch(
        &surface.backdrop,
        &mut surface.blurred,
        &mut surface.blur_scratch,
        w,
        0,
        sample_h,
        TASKBAR_BLUR_R,
    );
    layer.buf_mut().copy_from_slice(
        &surface.blurred[pad * w..(pad + TASKBAR_H) * w],
    );
    tint_taskbar(
        layer.buf_mut(),
        config::get_color("ui-theme/color/taskbar", Color::TASKBAR).0,
        170,
    );

    let count = wm.count();
    let btn_d = 40usize;
    let btn_gap = 12i32;
    let total_w = count as i32 * (btn_d as i32 + btn_gap) - btn_gap;
    let base_bx = ((w as i32 - total_w) / 2).max(0);
    let btn_y = (TASKBAR_H - btn_d) / 2;
    let add_scale = if add_progress >= 0.0 {
        ease_out_back(add_progress)
    } else {
        1.0
    };

    for i in 0..count {
        let Some(id) = wm.insertion_id_at(i) else { continue };
        let icon_name = wm.get_icon_name(id);
        let is_focused = wm.focused_id == Some(id);
        let is_minimized = wm.is_minimized(id);
        let scale = if add_progress >= 0.0 && i == count - 1 {
            add_scale
        } else {
            1.0
        };
        let scaled_d = (btn_d as f32 * scale) as usize;
        if scaled_d == 0 {
            continue;
        }
        let offset = btn_d.saturating_sub(scaled_d) / 2;
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
                let dst_x = bx + (offset + px) as i32;
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

        let resolved_icon = if icon_name.is_empty() { "noname.png" } else { icon_name };
        if let Some(icon) = get_or_decode_icon(resolved_icon, 40) {
            let icon_draw = scaled_d;
            let icon_offset = btn_d.saturating_sub(icon_draw) / 2;
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
                    let dst_x = bx + (icon_offset + px) as i32;
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
    let battery_len;
    if battery_pct >= 100 {
        battery_bytes.copy_from_slice(b"100%");
        battery_len = 4;
    } else if battery_pct >= 10 {
        battery_bytes[0] = b'0' + battery_pct / 10;
        battery_bytes[1] = b'0' + battery_pct % 10;
        battery_bytes[2] = b'%';
        battery_len = 3;
    } else {
        battery_bytes[0] = b'0' + battery_pct % 10;
        battery_bytes[1] = b'%';
        battery_len = 2;
    }
    let battery = unsafe {
        core::str::from_utf8_unchecked(&battery_bytes[..battery_len])
    };

    let size = 32.0;
    let measure = |text: &str| -> usize {
        text.chars()
            .map(|ch| {
                let g = baram_font::ttf_font_hud::glyph_at_size(ch, size);
                if g.w > 0 { g.advance.max(0) as usize } else { 8 }
            })
            .sum()
    };
    let gap = 12usize;
    let status_x = w.saturating_sub(measure(time) + gap + measure(battery) + 16);
    let baseline = TASKBAR_H as i32
        - baram_font::ttf_font_hud::ascent_at_size(size)
        + 9;
    let status_color = config::get_color("ui-theme/color/text", Color::TEXT);
    draw_taskbar_text(layer, time, status_x, baseline, status_color, size);
    draw_taskbar_text(
        layer,
        battery,
        status_x + measure(time) + gap,
        baseline,
        status_color,
        size,
    );

    svg::draw_svg_into_alpha(
        layer,
        APPS_SVG,
        16,
        ((TASKBAR_H - 24) / 2) as i32,
        24.0,
        24.0,
        if hover_apps_icon { 153 } else { 255 },
    );
    layer.mark_all_dirty();
    surface.valid = true;
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
    taskbar_only: bool,
    clock_hh: u8,
    clock_mm: u8,
    battery_pct: u8,
) {
    let w = layer.width();
    let h = layer.height();
    let tb_y = h.saturating_sub(TASKBAR_H);

    if !taskbar_only {
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
    }

    let mut fb = FmtBuf::new();
    fb.push_str("Key:");
    fb.push_u32(key_ev);
    fb.push_str(" Window:");
    fb.push_u32(wm.count() as u32);
    fb.push_str(" ");
    fb.push_u32(fps);
    fb.push_str("FPS");

    if hud_enabled && !taskbar_only {
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

        let hud_text1 = "Baram OS (1.1.0)";
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
    if !taskbar_only {
        wm.draw_all(
            layer,
            ui_win_id.map(|id| (id, ui_commands)),
            warp_engines,
            html_engines,
        );
    }

    if show_app_launcher {
        if cached_launcher_layer.is_none() {
            let mut lsys = LayerSystem::new(w, h);
            lsys.clear(Color::TRANSPARENT);
            let src: &[u32] = if let Some(ref cached) = bg_cache {
                cached
            } else if let Some(pixels) = wallpaper {
                pixels
            } else {
                &[]
            };
            if !src.is_empty() {
                blur::blur_region_darkened_to(src, lsys.buf_mut(), w, 0, tb_y, 60, 200);
            }

            let cols = 5usize;
            let icon_size = 64usize;
            let icon_gap = 24usize;
            let label_h = 20usize;
            let cell_w = icon_size + icon_gap;
            let cell_h = icon_size + label_h + icon_gap;
            let grid_w = cols * cell_w;
            let rows = (app_list.len() + cols - 1) / cols;
            let grid_h = rows * cell_h;
            let grid_x = (w.saturating_sub(grid_w)) / 2;
            let grid_y = ((h - TASKBAR_H).saturating_sub(grid_h)) / 2;

            for (i, name) in app_list.iter().enumerate() {
                let col = i % cols;
                let row = i / cols;
                let cx = grid_x + col * cell_w + icon_gap / 2;
                let cy = grid_y + row * cell_h;

                lsys.fill_circle(
                    cx + icon_size / 2,
                    cy + icon_size / 2,
                    icon_size / 2,
                    config::get_color("ui-theme/color/panel", Color::PANEL),
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
                                if sx >= w || sy >= tb_y {
                                    continue;
                                }
                                let idx = sy * w + sx;
                                let bg = Color(lsys.buf_ref()[idx]);
                                let inv = 255 - a;
                                let r = (src_px[0] as u32 * a + bg.r() as u32 * inv) / 255;
                                let g = (src_px[1] as u32 * a + bg.g() as u32 * inv) / 255;
                                let b = (src_px[2] as u32 * a + bg.b() as u32 * inv) / 255;
                                lsys.buf_mut()[idx] = Color::rgb(r as u8, g as u8, b as u8).0;
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
                let label_color = config::get_color("ui-theme/color/btn_tonal", Color::BTN_TONAL);
                lsys.put_str(tx, ty, &display_name, label_color);
            }
            *cached_launcher_layer = Some(lsys.buf_ref().to_vec());
        }
        if let Some(launcher) = cached_launcher_layer.as_ref() {
            layer.copy_rect_buffer(launcher, w, tb_y, 0, 0);
        }
    }

    if !taskbar_only && taskbar_dirty {
        taskbar.capture_backdrop(layer, tb_y);
    }
    if taskbar_dirty || !taskbar.is_valid() {
        redraw_taskbar(
            taskbar,
            wm,
            add_progress,
            shift_x,
            hover_apps_icon,
            clock_hh,
            clock_mm,
            battery_pct,
        );
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
    taskbar_only: bool,
    clock_hh: u8,
    clock_mm: u8,
    battery_pct: u8,
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
        taskbar_only,
        clock_hh,
        clock_mm,
        battery_pct,
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
