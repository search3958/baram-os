use super::cursor::{self};
use crate::warp::WarpEngine;
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
            current_name.clone()
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

pub fn render_scene(
    layer: &mut LayerSystem,
    wm: &mut WindowManager,
    _mouse_ev: u32,
    key_ev: u32,
    fps: u32,
    _mouse_mode: &str,
    ui_commands: &[uiscript::Command],
    ui_win_id: Option<WinId>,
    warp_engines: &mut alloc::vec::Vec<(WinId, WarpEngine)>,
    wallpaper: Option<&[u32]>,
    cached_taskbar: &mut Option<Vec<u32>>,
    cached_taskbar_strip: &mut Option<Vec<u32>>,
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
) {
    let w = layer.width();
    let h = layer.height();

    if bg_cache_valid {
        if let Some(ref cached) = bg_cache {
            layer.buf_mut()[..w * h].copy_from_slice(cached);
        }
    } else if let Some(pixels) = wallpaper {
        layer.buf_mut()[..w * h].copy_from_slice(pixels);
    } else {
        layer.clear(config::get_color("ui-theme/color/bg", Color::BG));
    }

    let tb_y = h.saturating_sub(TASKBAR_H);

    if bg_cache_valid {
    } else if let Some(ref cached) = cached_taskbar {
        layer.buf_mut()[tb_y * w..h * w].copy_from_slice(cached);
    } else {
        let mut blurred = alloc::vec![0u32; w * TASKBAR_H];
        blur::blur_region_to(layer.buf_ref(), &mut blurred, w, tb_y, h, TASKBAR_BLUR_R);
        let tb_alpha = 170u32;
        let tb_inv = 255 - tb_alpha;
        let tb_color = config::get_color("ui-theme/color/taskbar", Color::TASKBAR);
        for y in 0..TASKBAR_H {
            let row_start = y * w;
            for x in 0..w {
                let idx = row_start + x;
                let bg = Color(blurred[idx]);
                let r = (tb_color.r() as u32 * tb_alpha + bg.r() as u32 * tb_inv) / 255;
                let g = (tb_color.g() as u32 * tb_alpha + bg.g() as u32 * tb_inv) / 255;
                let b = (tb_color.b() as u32 * tb_alpha + bg.b() as u32 * tb_inv) / 255;
                layer.buf_mut()[(tb_y + y) * w + x] = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }

        let mut bar = alloc::vec![0u32; w * TASKBAR_H];
        bar.copy_from_slice(&layer.buf_ref()[tb_y * w..h * w]);
        *cached_taskbar = Some(bar);
    }

    if !bg_cache_valid {
        let mut bg = alloc::vec![0u32; w * h];
        bg.copy_from_slice(layer.buf_ref());
        *bg_cache = Some(bg);
    }

    wm.draw_all(layer, ui_win_id.map(|id| (id, ui_commands)), warp_engines);

    if !taskbar_dirty {
        if let Some(ref strip) = cached_taskbar_strip {
            layer.buf_mut()[tb_y * w..h * w].copy_from_slice(strip);
        }
    } else {
        let ids = wm.insertion_ids();
        let count = ids.len();
        let btn_d = 40usize;
        let btn_gap = 12i32;
        let total_w = count as i32 * (btn_d as i32 + btn_gap) - btn_gap;
        let base_bx = ((w as i32 - total_w) / 2).max(0);
        let btn_y = tb_y + (TASKBAR_H - btn_d) / 2;

        let add_scale = if add_progress >= 0.0 {
            ease_out_back(add_progress)
        } else {
            1.0
        };

        for (i, id) in ids.iter().enumerate() {
            let title = wm.get_title(*id).unwrap_or("???");
            let icon_name = wm.get_icon_name(*id);
            let is_focused = wm.focused_id == Some(*id);
            let is_minimized = wm.is_minimized(*id);

            let scale = if add_progress >= 0.0 && i == count - 1 {
                add_scale
            } else {
                1.0
            };

            let ca = if is_focused { 255u32 } else { 100u32 };
            let bx = base_bx + shift_x as i32 + i as i32 * (btn_d as i32 + btn_gap);
            let scaled_d = (btn_d as f32 * scale) as usize;
            if scaled_d == 0 {
                continue;
            }
            let offset = if scaled_d > btn_d {
                0
            } else {
                (btn_d - scaled_d) / 2
            };
            let cached_btn = get_or_render_tb_btn(scaled_d, ca);
            for py in 0..scaled_d {
                let src_row = py * scaled_d;
                let dst_y = btn_y + offset + py;
                if dst_y >= h {
                    continue;
                }
                let dst_row = dst_y * w;
                for px in 0..scaled_d {
                    let sp = cached_btn[src_row + px];
                    let pre_a = (sp >> 24) & 0xFF;
                    if pre_a == 0 {
                        continue;
                    }
                    let sx = bx as usize + offset + px;
                    if sx >= w {
                        continue;
                    }
                    let idx = dst_row + sx;
                    let inv = 255 - pre_a;
                    let bg = Color(layer.buf_ref()[idx]);
                    let r = (255 * pre_a + bg.r() as u32 * inv) / 255;
                    let g = (255 * pre_a + bg.g() as u32 * inv) / 255;
                    let b = (255 * pre_a + bg.b() as u32 * inv) / 255;
                    layer.buf_mut()[idx] = Color::rgb(r as u8, g as u8, b as u8).0;
                }
            }

            let resolved_icon = if icon_name.is_empty() {
                "noname.png"
            } else {
                icon_name
            };
            {
                let icon_path = alloc::format!("apps/icon/{}", resolved_icon);
                let icon_data = baram_bsd::vfs::read_file(&icon_path);
                if !icon_data.is_empty() {
                    if let Some(icon) = decode_icon(&icon_data, 40) {
                        let icon_draw = (btn_d as f32 * scale) as usize;
                        if icon_draw > 0 {
                            let icon_offset = if icon_draw > btn_d {
                                0
                            } else {
                                (btn_d - icon_draw) / 2
                            };
                            let ix = bx as usize + icon_offset;
                            let iy = btn_y + icon_offset;
                            let icon_alpha = if is_minimized { 128u32 } else { 255u32 };
                            let src_w = icon.w as f32;
                            let src_h = icon.h as f32;
                            let dst_w = icon_draw as f32;
                            let dst_h = icon_draw as f32;
                            for py in 0..icon_draw {
                                let sy_f = (py as f32 + 0.5) * src_h / dst_h - 0.5;
                                let sy_floor = libm::floorf(sy_f);
                                let sy0 = sy_floor.max(0.0) as usize;
                                let sy1 = (sy0 + 1).min(icon.h - 1);
                                let fy = sy_f - sy_floor;
                                let fy_inv = 1.0 - fy;
                                for px in 0..icon_draw {
                                    let sx_f = (px as f32 + 0.5) * src_w / dst_w - 0.5;
                                    let sx_floor = libm::floorf(sx_f);
                                    let sx0 = sx_floor.max(0.0) as usize;
                                    let sx1 = (sx0 + 1).min(icon.w - 1);
                                    let fx = sx_f - sx_floor;
                                    let fx_inv = 1.0 - fx;
                                    let p00 = &icon.pixels[sy0 * icon.w + sx0];
                                    let p10 = &icon.pixels[sy0 * icon.w + sx1];
                                    let p01 = &icon.pixels[sy1 * icon.w + sx0];
                                    let p11 = &icon.pixels[sy1 * icon.w + sx1];
                                    let r = ((p00[0] as f32 * fx_inv + p10[0] as f32 * fx) * fy_inv
                                        + (p01[0] as f32 * fx_inv + p11[0] as f32 * fx) * fy)
                                        as u32;
                                    let g = ((p00[1] as f32 * fx_inv + p10[1] as f32 * fx) * fy_inv
                                        + (p01[1] as f32 * fx_inv + p11[1] as f32 * fx) * fy)
                                        as u32;
                                    let b = ((p00[2] as f32 * fx_inv + p10[2] as f32 * fx) * fy_inv
                                        + (p01[2] as f32 * fx_inv + p11[2] as f32 * fx) * fy)
                                        as u32;
                                    let a = (((p00[3] as f32 * fx_inv + p10[3] as f32 * fx)
                                        * fy_inv
                                        + (p01[3] as f32 * fx_inv + p11[3] as f32 * fx) * fy)
                                        as u32
                                        * icon_alpha
                                        / 255) as u32;
                                    if a == 0 {
                                        continue;
                                    }
                                    let sx = ix + px;
                                    let sy = iy + py;
                                    if sx >= w || sy >= h {
                                        continue;
                                    }
                                    let idx = sy * w + sx;
                                    let bg = Color(layer.buf_ref()[idx]);
                                    let inv = 255 - a;
                                    let out_r = (r * a + bg.r() as u32 * inv) / 255;
                                    let out_g = (g * a + bg.g() as u32 * inv) / 255;
                                    let out_b = (b * a + bg.b() as u32 * inv) / 255;
                                    layer.buf_mut()[idx] =
                                        Color::rgb(out_r as u8, out_g as u8, out_b as u8).0;
                                }
                            }
                        }
                    }
                }
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

        if hud_enabled {
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
                tb_y + 6,
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
                tb_y + 26,
                s2,
                config::get_color("ui-theme/color/muted", Color::MUTED),
            );
        }

        let mut strip = alloc::vec![0u32; w * TASKBAR_H];
        strip.copy_from_slice(&layer.buf_ref()[tb_y * w..h * w]);
        *cached_taskbar_strip = Some(strip);
    }

    {
        let apps_icon_size = 24usize;
        let apps_icon_x = 16usize;
        let apps_icon_y = tb_y + (TASKBAR_H - apps_icon_size) / 2;
        let apps_icon_alpha = if hover_apps_icon { 153u32 } else { 255u32 };
        svg::draw_svg_into_alpha(
            layer,
            APPS_SVG,
            apps_icon_x as i32,
            apps_icon_y as i32,
            apps_icon_size as f32,
            apps_icon_size as f32,
            apps_icon_alpha,
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
                    let icon_path = alloc::format!("apps/icon/{}", resolved_icon);
                    let icon_data = baram_bsd::vfs::read_file(&icon_path);
                    if !icon_data.is_empty() {
                        if let Some(icon) = decode_icon(&icon_data, icon_size) {
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
    }
}

pub fn render_frame(
    layer: &mut LayerSystem,
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
    wallpaper: Option<&[u32]>,
    cached_taskbar: &mut Option<Vec<u32>>,
    cached_taskbar_strip: &mut Option<Vec<u32>>,
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
) {
    render_scene(
        layer,
        wm,
        _mouse_ev,
        key_ev,
        fps,
        mouse_mode,
        ui_commands,
        ui_win_id,
        warp_engines,
        wallpaper,
        cached_taskbar,
        cached_taskbar_strip,
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
