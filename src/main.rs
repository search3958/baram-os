#![no_std]
#![no_main]

extern crate alloc;

mod absolute_pointer;
mod cursor;
mod font;
mod gop;
mod keyboard;

mod mouse;
mod svg;
mod ttf_font;
mod ttf_font_hud;
mod ui;
mod uiscript;
mod usb_hid;
mod window;

use alloc::vec::Vec;

use uefi::prelude::*;
use uefi::runtime;

use crate::gop::{Color, Screen};
use crate::keyboard::Keyboard;
use crate::mouse::{Mouse, MouseEvent};
use crate::ui::FmtBuf;
use crate::window::{WindowManager, LayerSystem};

const TASKBAR_H: usize = 48;
const TASKBAR_BLUR_R: i32 = 30;
const SCROLL_SPEED: i32 = 30;

const CURSOR_SVG: &str = include_str!("data/mouse.svg");
const CURSOR_SVG_SIZE: &str = include_str!("data/mouse_size.svg");
const CURSOR_BOX_W: usize = 15;
const CURSOR_BOX_H: usize = 19;
const CURSOR_BOX_SIZE_W: usize = 19;
const CURSOR_BOX_SIZE_H: usize = 19;

struct CursorBitmap {
    pixels: Vec<u8>,
    shadow: Vec<u8>,
    w: usize,
    h: usize,
    shadow_w: usize,
    shadow_h: usize,
}

static mut CURSOR_NORMAL: Option<CursorBitmap> = None;
static mut CURSOR_RESIZE: Option<CursorBitmap> = None;

fn prerender_cursor(svg: &str, w: usize, h: usize, blur_r: i32) -> CursorBitmap {
    let svg_buf = svg::rasterize_svg_to_buffer(svg, w, h);

    let mut silhouette: Vec<f32> = alloc::vec![0.0; w * h];
    for i in 0..w * h {
        if svg_buf[i * 4 + 3] > 0 {
            silhouette[i] = 1.0;
        }
    }

    let pad = blur_r as usize;
    let pw = w + pad * 2;
    let ph = h + pad * 2;
    let mut padded: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..h {
        for x in 0..w {
            padded[(y + pad) * pw + (x + pad)] = silhouette[y * w + x];
        }
    }

    let sigma = blur_r as f32 / 3.0;
    let mut kernel: Vec<f32> = alloc::vec![0.0; (blur_r * 2 + 1) as usize];
    let mut k_sum = 0.0f32;
    for i in 0..=blur_r * 2 {
        let x = (i - blur_r) as f32;
        let w = libm::expf(-x * x / (2.0 * sigma * sigma));
        kernel[i as usize] = w;
        k_sum += w;
    }
    for k in kernel.iter_mut() {
        *k /= k_sum;
    }

    let mut tmp: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..ph {
        for x in 0..pw {
            let mut sum = 0.0f32;
            for dx in -blur_r..=blur_r {
                let sx = x as i32 + dx;
                if sx >= 0 && sx < pw as i32 {
                    sum += padded[y * pw + sx as usize] * kernel[(dx + blur_r) as usize];
                }
            }
            tmp[y * pw + x] = sum;
        }
    }
    let mut result: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..ph {
        for x in 0..pw {
            let mut sum = 0.0f32;
            for dy in -blur_r..=blur_r {
                let sy = y as i32 + dy;
                if sy >= 0 && sy < ph as i32 {
                    sum += tmp[sy as usize * pw + x] * kernel[(dy + blur_r) as usize];
                }
            }
            result[y * pw + x] = sum;
        }
    }

    let mut shadow: Vec<u8> = alloc::vec![0u8; pw * ph * 4];
    for i in 0..pw * ph {
        let a = (result[i] * 120.0).min(255.0) as u8;
        shadow[i * 4] = 0;
        shadow[i * 4 + 1] = 0;
        shadow[i * 4 + 2] = 0;
        shadow[i * 4 + 3] = a;
    }

    CursorBitmap {
        pixels: svg_buf,
        shadow,
        w, h,
        shadow_w: pw, shadow_h: ph,
    }
}

const APP_DEMO: &str = include_str!("app/demo.u1");

const WALLPAPER_PNG: &[u8] = include_bytes!("data/wallpaper/reflect.png");
const ICON_NONAME_PNG: &[u8] = include_bytes!("app/icon/noname.png");
const ICON_FILES_PNG: &[u8] = include_bytes!("app/icon/files.png");
const ICON_MANAGER_PNG: &[u8] = include_bytes!("app/icon/manager.png");

struct IconBitmap {
    pixels: Vec<[u8; 4]>,
    w: usize,
    h: usize,
}

static mut ICON_NONAME: Option<IconBitmap> = None;
static mut ICON_FILES: Option<IconBitmap> = None;
static mut ICON_MANAGER: Option<IconBitmap> = None;

fn decode_icon(bytes: &[u8], size: usize) -> Option<IconBitmap> {
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
    Some(IconBitmap { pixels: buf, w: size, h: size })
}

fn ensure_icon(icon: &mut Option<IconBitmap>, bytes: &[u8], size: usize) {
    if icon.is_none() {
        *icon = decode_icon(bytes, size);
    }
}

fn icon_for_title(title: &str) -> Option<&'static IconBitmap> {
    unsafe {
        if title.contains("Manager") || title.contains("タスク") {
            ensure_icon(&mut ICON_MANAGER, ICON_MANAGER_PNG, 40);
            ICON_MANAGER.as_ref()
        } else if title.contains("File") || title.contains("Explorer") || title.contains("ファイル") {
            ensure_icon(&mut ICON_FILES, ICON_FILES_PNG, 40);
            ICON_FILES.as_ref()
        } else {
            ensure_icon(&mut ICON_NONAME, ICON_NONAME_PNG, 40);
            ICON_NONAME.as_ref()
        }
    }
}

fn draw_cursor_into_layer(layer: &mut LayerSystem, cx: i32, cy: i32, resizing: bool) {
    unsafe {
        let bitmap = if resizing {
            CURSOR_RESIZE.as_ref()
        } else {
            CURSOR_NORMAL.as_ref()
        };
        if let Some(bmp) = bitmap {
            let blur_r = 12i32;
            let pad = blur_r as i32;
            svg::blit_shadow(layer, &bmp.shadow, bmp.shadow_w, bmp.shadow_h,
                cx + 3 - pad, cy + 4 - pad);
            svg::blit_cached(layer, &bmp.pixels, bmp.w, bmp.h, cx, cy);
            return;
        }
    }
    if resizing {
        svg::draw_svg_shadow(layer, CURSOR_SVG_SIZE, cx + 5, cy + 4,
            CURSOR_BOX_SIZE_W as f32, CURSOR_BOX_SIZE_H as f32, 12, 0);
        svg::draw_svg_into(layer, CURSOR_SVG_SIZE, cx, cy,
            CURSOR_BOX_SIZE_W as f32, CURSOR_BOX_SIZE_H as f32);
    } else {
        svg::draw_svg_shadow(layer, CURSOR_SVG, cx + 5, cy + 4,
            CURSOR_BOX_W as f32, CURSOR_BOX_H as f32, 12, 0);
        svg::draw_svg_into(layer, CURSOR_SVG, cx, cy,
            CURSOR_BOX_W as f32, CURSOR_BOX_H as f32);
    }
}

#[entry]
fn main() -> Status {
    let _ = uefi::helpers::init();
    ttf_font::init();
    ttf_font_hud::init();

    unsafe {
        CURSOR_NORMAL = Some(prerender_cursor(CURSOR_SVG, CURSOR_BOX_W, CURSOR_BOX_H, 8));
        CURSOR_RESIZE = Some(prerender_cursor(CURSOR_SVG_SIZE, CURSOR_BOX_SIZE_W, CURSOR_BOX_SIZE_H, 8));
    }

    let mut screen = match Screen::take() {
        Ok(s) => s,
        Err(_s) => return Status::UNSUPPORTED,
    };

    let mut mouse_opt: Option<Mouse> = match Mouse::open() {
        Ok(m) => Some(m),
        Err(_) => None,
    };
    let has_kbd = Keyboard::is_present();
    if has_kbd { Keyboard::reset(); }

    let mut cursor_x: i32 = (screen.width() / 2) as i32;
    let mut cursor_y: i32 = (screen.height() / 2) as i32;

    let mut wm = WindowManager::new(screen.width(), screen.height());
    let mut layer = LayerSystem::new(screen.width(), screen.height());

    
    wm.add("システム情報", 40, 60, 320, 220);
    wm.add("Task Manager", 400, 80, 340, 260);

    
    let ui_win_id = wm.add("UI Script Demo", 600, 100, 400, 350);
    let ui_commands = uiscript::parse(APP_DEMO);

    let mut last_keys: Vec<&'static str> = Vec::with_capacity(8);
    let mut mouse_ev_count: u32 = 0;
    let mut key_ev_count: u32 = 0;
    let mut frames: u32 = 0;
    let mut fps: u32 = 0;
    let mut frames_since_tick: u32 = 0;
    let mut start_time = runtime::get_time().unwrap_or_else(|_| runtime::Time::invalid());
    let mut mouse_down = false;
    let mut new_window_idx: u32 = 0;

    let mouse_mode_label = match &mouse_opt {
        Some(m) if m.is_absolute() => "Absolute",
        Some(_) => "Simple Ptr",
        None => "None",
    };

    
    let mut cached_wallpaper: Option<Vec<u32>> = None;
    if let Ok((header, pixels)) = png_decoder::decode(WALLPAPER_PNG) {
        let w = screen.width();
        let h = screen.height();
        let img_w = header.width as usize;
        let img_h = header.height as usize;
        let mut buf = alloc::vec![0u32; w * h];

        for y in 0..h {
            let sy = y * img_h / h;
            let src_row = sy * img_w;
            let dst_row = y * w;
            for x in 0..w {
                let sx = x * img_w / w;
                let px = pixels[src_row + sx];
                buf[dst_row + x] = Color::rgb(px[0], px[1], px[2]).0;
            }
        }
        cached_wallpaper = Some(buf);
    }

    let mut scene_dirty = true;
    let mut cached_scene: Vec<u32> = alloc::vec![0u32; screen.width() * screen.height()];
    let mut prev_cursor_x = cursor_x;
    let mut prev_cursor_y = cursor_y;
    let mut prev_is_resizing = false;
    let shadow_pad = 35i32;

    let mut cached_taskbar: Option<Vec<u32>> = None;

    let mut tb_add_progress: f32 = -1.0f32;
    let mut tb_remove_progress: f32 = -1.0f32;
    let mut tb_shift_x: f32 = 0.0f32;

    render_scene(&mut layer, &mut wm, mouse_ev_count, key_ev_count,
                 fps, mouse_mode_label,
                 &ui_commands, Some(ui_win_id), cached_wallpaper.as_deref(),
                 &mut cached_taskbar, -1.0, -1.0, 0.0);
    cached_scene.copy_from_slice(layer.buf_ref());
    draw_cursor_into_layer(&mut layer, cursor_x, cursor_y, false);
    layer.flush(&mut screen);

    loop {
        let mut dirty = false;

        let prev_dirty = wm.dirty_bbox(shadow_pad);

        
        if has_kbd {
            while let Some(ev) = Keyboard::poll() {
                key_ev_count = key_ev_count.wrapping_add(1);
                if last_keys.len() >= 6 { last_keys.remove(0); }
                last_keys.push(ev.label());

                match ev.scancode {
                    0x01 => wm.scroll_focused(-SCROLL_SPEED),
                    0x02 => wm.scroll_focused(SCROLL_SPEED),
                    _ => {}
                }
                if let Some(c) = ev.printable {
                    match c {
                        b'n' | b'N' => {
                            let titles = ["Notes", "Terminal", "Files", "Settings", "Explorer"];
                            let idx = (new_window_idx as usize) % titles.len();
                            let x = 60 + ((new_window_idx as i32 * 37) % 300);
                            let y = 80 + ((new_window_idx as i32 * 23) % 200);
                            wm.add(titles[idx], x, y, 300, 200);
                            tb_add_progress = 0.0;
                            tb_shift_x = 26.0;
                            new_window_idx = new_window_idx.wrapping_add(1);
                        }
                        _ => {}
                    }
                }
                dirty = true;
                scene_dirty = true;
            }
        }

        
        if let Some(mouse) = mouse_opt.as_mut() {
            while let Some(ev) = mouse.poll() {
                mouse_ev_count = mouse_ev_count.wrapping_add(1);

                let (cx, cy) = apply_mouse_event(
                    &mut cursor_x, &mut cursor_y, &ev,
                    screen.width(), screen.height(), mouse.abs_max(),
                );

                
                if ev.scroll != 0 {
                    let scroll_delta = -ev.scroll * SCROLL_SPEED;
                    if let Some(id) = wm.window_at(cx, cy) {
                        wm.scroll_window(id, scroll_delta);
                        scene_dirty = true;
                    }
                }

                if ev.left && !mouse_down {
                    mouse_down = true;
                    let sh = screen.height();
                    if cy >= sh as i32 - TASKBAR_H as i32 {
                        let ids = wm.insertion_ids();
                        let count = ids.len();
                        let btn_d = 40i32;
                        let btn_gap = 12i32;
                        let total_w = count as i32 * (btn_d + btn_gap) - btn_gap;
                        let mut bx = ((screen.width() as i32 - total_w) / 2).max(0);
                        let btn_y = (sh as usize).saturating_sub(TASKBAR_H) + (TASKBAR_H - 40) / 2;
                        for id in &ids {
                            let dx = cx - bx - btn_d / 2;
                            let dy = cy - btn_y as i32 - btn_d / 2;
                            if dx * dx + dy * dy <= (btn_d / 2) * (btn_d / 2) {
                                wm.focus(*id);
                                break;
                            }
                            bx += btn_d + btn_gap;
                        }
                    } else {
                        let before = wm.insertion_ids();
                        wm.on_mouse_down(cx, cy);
                        let after = wm.insertion_ids();
                        if after.len() < before.len() {
                            tb_remove_progress = 0.0;
                            tb_shift_x = -26.0;
                        }
                    }
                    scene_dirty = true;
                } else if !ev.left && mouse_down {
                    mouse_down = false;
                    wm.on_mouse_up();
                    scene_dirty = true;
                }

                if mouse_down {
                    wm.on_mouse_drag(cx, cy);
                    scene_dirty = true;
                }

                dirty = true;
            }
        }

        
        frames = frames.wrapping_add(1);
        frames_since_tick = frames_since_tick.wrapping_add(1);
        if let Ok(now) = runtime::get_time() {
            let elapsed_ns = time_diff_ns(&start_time, &now);
            if elapsed_ns >= 1_000_000_000 {
                fps = frames_since_tick;
                frames_since_tick = 0;
                start_time = now;
                dirty = true;
            }
        }

        if wm.take_order_changed() {
            scene_dirty = true;
            dirty = true;
        }

        let anim_speed = 10.0f32;
        let dt = 0.008f32;

        if tb_add_progress >= 0.0 {
            tb_add_progress = (tb_add_progress + anim_speed * dt).min(1.0);
            dirty = true;
            scene_dirty = true;
        }

        if tb_remove_progress >= 0.0 {
            tb_remove_progress = (tb_remove_progress + anim_speed * dt).min(1.0);
            dirty = true;
            scene_dirty = true;
        }

        if tb_shift_x.abs() > 0.5 {
            tb_shift_x *= 0.8;
            dirty = true;
            scene_dirty = true;
            if tb_shift_x.abs() < 0.5 { tb_shift_x = 0.0; }
        }

        if dirty {
            let is_resizing = wm.is_any_resizing() || wm.is_over_resize_handle(cursor_x, cursor_y);

            if scene_dirty {
                let (bx0, by0, bx1, by1) = prev_dirty;

                render_scene(&mut layer, &mut wm, mouse_ev_count, key_ev_count,
                             fps, mouse_mode_label,
                             &ui_commands, Some(ui_win_id), cached_wallpaper.as_deref(),
                             &mut cached_taskbar,
                             tb_add_progress, tb_remove_progress,
                             tb_shift_x);

                if tb_add_progress >= 1.0 { tb_add_progress = -1.0; }
                if tb_remove_progress >= 1.0 { tb_remove_progress = -1.0; }

                let (ax0, ay0, ax1, ay1) = wm.dirty_bbox(shadow_pad);
                let rx0 = bx0.min(ax0);
                let ry0 = by0.min(ay0);
                let rx1 = bx1.max(ax1);
                let ry1 = by1.max(ay1);

                let w = screen.width();
                let h = screen.height();
                let tb_y = h.saturating_sub(TASKBAR_H);
                let ry1 = ry1.max(h);
                let ry0 = ry0.min(tb_y);

                cached_scene.copy_from_slice(layer.buf_ref());
                scene_dirty = false;
                draw_cursor_into_layer(&mut layer, cursor_x, cursor_y, is_resizing);

                let pad = 32i32;
                let cur_w = if is_resizing { CURSOR_BOX_SIZE_W } else { CURSOR_BOX_W };
                let cur_h = if is_resizing { CURSOR_BOX_SIZE_H } else { CURSOR_BOX_H };
                let prev_w = if prev_is_resizing { CURSOR_BOX_SIZE_W } else { CURSOR_BOX_W };
                let prev_h = if prev_is_resizing { CURSOR_BOX_SIZE_H } else { CURSOR_BOX_H };
                let cx0 = (prev_cursor_x.min(cursor_x) - pad).max(0) as usize;
                let cy0 = (prev_cursor_y.min(cursor_y) - pad).max(0) as usize;
                let cx1 = (prev_cursor_x.max(cursor_x) + cur_w.max(prev_w) as i32 + pad).min(w as i32) as usize;
                let cy1 = (prev_cursor_y.max(cursor_y) + cur_h.max(prev_h) as i32 + pad).min(h as i32) as usize;
                let fx0 = rx0.min(cx0);
                let fy0 = ry0.min(cy0);
                let fx1 = rx1.max(cx1);
                let fy1 = ry1.max(cy1);
                layer.flush_rect(&mut screen, fx0, fy0, fx1, fy1);

                prev_cursor_x = cursor_x;
                prev_cursor_y = cursor_y;
                prev_is_resizing = is_resizing;
            } else {
                let w = screen.width();
                let h = screen.height();
                layer.buf_mut()[..w * h].copy_from_slice(&cached_scene);
                draw_cursor_into_layer(&mut layer, cursor_x, cursor_y, is_resizing);

                let pad = 32i32;
                let cur_w = if is_resizing { CURSOR_BOX_SIZE_W } else { CURSOR_BOX_W };
                let cur_h = if is_resizing { CURSOR_BOX_SIZE_H } else { CURSOR_BOX_H };
                let prev_w = if prev_is_resizing { CURSOR_BOX_SIZE_W } else { CURSOR_BOX_W };
                let prev_h = if prev_is_resizing { CURSOR_BOX_SIZE_H } else { CURSOR_BOX_H };
                let x0 = (prev_cursor_x.min(cursor_x) - pad).max(0) as usize;
                let y0 = (prev_cursor_y.min(cursor_y) - pad).max(0) as usize;
                let x1 = (prev_cursor_x.max(cursor_x) + cur_w.max(prev_w) as i32 + pad).min(w as i32) as usize;
                let y1 = (prev_cursor_y.max(cursor_y) + cur_h.max(prev_h) as i32 + pad).min(h as i32) as usize;
                layer.flush_rect(&mut screen, x0, y0, x1, y1);

                prev_cursor_x = cursor_x;
                prev_cursor_y = cursor_y;
                prev_is_resizing = is_resizing;
            }
        }

        uefi::boot::stall(core::time::Duration::from_micros(8_000));
    }
}


fn render_scene(layer: &mut LayerSystem, wm: &mut WindowManager,
                _mouse_ev: u32, key_ev: u32,
                fps: u32, _mouse_mode: &str,
                ui_commands: &[uiscript::Command], ui_win_id: Option<window::WinId>,
                wallpaper: Option<&[u32]>,
                cached_taskbar: &mut Option<Vec<u32>>,
                add_progress: f32,
                remove_progress: f32,
                shift_x: f32) {
    let w = layer.width();
    let h = layer.height();

    if let Some(pixels) = wallpaper {
        layer.buf_mut()[..w * h].copy_from_slice(pixels);
    } else {
        layer.clear(Color::BG);
    }

    let tb_y = h.saturating_sub(TASKBAR_H);

    if let Some(ref cached) = cached_taskbar {
        layer.buf_mut()[tb_y * w..h * w].copy_from_slice(cached);
    } else {
        let blur_r = TASKBAR_BLUR_R;

        let sigma = blur_r as f32 / 3.0;
        let sigma_sq2 = 2.0 * sigma * sigma;
        let kernel_size = (blur_r * 2 + 1) as usize;
        let mut kernel: alloc::vec::Vec<f32> = alloc::vec::Vec::with_capacity(kernel_size);
        let mut ksum = 0.0f32;
        for i in -blur_r..=blur_r {
            let kw = libm::expf(-(i as f32) * (i as f32) / sigma_sq2);
            kernel.push(kw);
            ksum += kw;
        }
        for kw in &mut kernel {
            *kw /= ksum;
        }

        let mut tmp: alloc::vec::Vec<u32> = alloc::vec![0u32; w * TASKBAR_H];

        for y in tb_y..h {
            for x in 0..w {
                let mut r = 0.0f32;
                let mut g = 0.0f32;
                let mut b = 0.0f32;
                for (i, &kw) in kernel.iter().enumerate() {
                    let sx = (x as i32 + i as i32 - blur_r).max(0).min(w as i32 - 1) as usize;
                    let pixel = Color(layer.buf_ref()[y * w + sx]);
                    r += pixel.r() as f32 * kw;
                    g += pixel.g() as f32 * kw;
                    b += pixel.b() as f32 * kw;
                }
                tmp[(y - tb_y) * w + x] = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }

        for y in tb_y..h {
            for x in 0..w {
                let mut r = 0.0f32;
                let mut g = 0.0f32;
                let mut b = 0.0f32;
                for (i, &kw) in kernel.iter().enumerate() {
                    let sy = (y as i32 + i as i32 - blur_r).max(tb_y as i32).min(h as i32 - 1) as usize;
                    let pixel = Color(tmp[(sy - tb_y) * w + x]);
                    r += pixel.r() as f32 * kw;
                    g += pixel.g() as f32 * kw;
                    b += pixel.b() as f32 * kw;
                }
                layer.buf_mut()[y * w + x] = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }

        let tb_alpha = 120u32;
        let tb_inv = 255 - tb_alpha;
        let tb_color = Color::TASKBAR;
        for y in tb_y..h {
            let row_start = y * w;
            for x in 0..w {
                let idx = row_start + x;
                let bg = Color(layer.buf_ref()[idx]);
                let r = (tb_color.r() as u32 * tb_alpha + bg.r() as u32 * tb_inv) / 255;
                let g = (tb_color.g() as u32 * tb_alpha + bg.g() as u32 * tb_inv) / 255;
                let b = (tb_color.b() as u32 * tb_alpha + bg.b() as u32 * tb_inv) / 255;
                layer.buf_mut()[idx] = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }

        let mut bar = alloc::vec![0u32; w * TASKBAR_H];
        bar.copy_from_slice(&layer.buf_ref()[tb_y * w..h * w]);
        *cached_taskbar = Some(bar);
    }

    wm.draw_all(layer, ui_win_id.map(|id| (id, ui_commands)));

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
        let is_focused = wm.focused_id == Some(*id);

        let scale = if add_progress >= 0.0 && i == count - 1 {
            add_scale
        } else {
            1.0
        };

        let ca = if is_focused { 255u32 } else { 100u32 };
        let bx = base_bx + shift_x as i32 + i as i32 * (btn_d as i32 + btn_gap);
        let scaled_d = (btn_d as f32 * scale) as usize;
        if scaled_d == 0 { continue; }
        let offset = if scaled_d > btn_d { 0 } else { (btn_d - scaled_d) / 2 };
        for py in 0..scaled_d {
            for px in 0..scaled_d {
                let dx = px as f32 + 0.5 - scaled_d as f32 / 2.0;
                let dy = py as f32 + 0.5 - scaled_d as f32 / 2.0;
                let dist_sq = dx * dx + dy * dy;
                let r_f = scaled_d as f32 / 2.0;
                let alpha = if dist_sq < (r_f - 1.0) * (r_f - 1.0) {
                    1.0f32
                } else if dist_sq > (r_f + 0.5) * (r_f + 0.5) {
                    0.0
                } else {
                    let dist = libm::sqrtf(dist_sq);
                    (r_f + 0.5 - dist).clamp(0.0, 1.0)
                };
                if alpha <= 0.0 { continue; }
                let sx = bx as usize + offset + px;
                let sy = btn_y + offset + py;
                if sx >= w || sy >= h { continue; }
                let idx = sy * w + sx;
                let bg = Color(layer.buf_ref()[idx]);
                let a = (alpha * ca as f32) as u32;
                let inv = 255 - a;
                let r = (255 * a + bg.r() as u32 * inv) / 255;
                let g = (255 * a + bg.g() as u32 * inv) / 255;
                let b = (255 * a + bg.b() as u32 * inv) / 255;
                layer.buf_mut()[idx] = Color::rgb(r as u8, g as u8, b as u8).0;
            }
        }

        if let Some(icon) = icon_for_title(title) {
            let icon_draw = (btn_d as f32 * scale) as usize;
            if icon_draw > 0 {
                let icon_offset = if icon_draw > btn_d { 0 } else { (btn_d - icon_draw) / 2 };
                let ix = bx as usize + icon_offset;
                let iy = btn_y + icon_offset;
                for py in 0..icon_draw {
                    for px in 0..icon_draw {
                        let src_x = px * icon.w / icon_draw;
                        let src_y = py * icon.h / icon_draw;
                        let src = icon.pixels[src_y * icon.w + src_x];
                        let a = src[3] as u32;
                        if a == 0 { continue; }
                        let sx = ix + px;
                        let sy = iy + py;
                        if sx >= w || sy >= h { continue; }
                        let idx = sy * w + sx;
                        let bg = Color(layer.buf_ref()[idx]);
                        let inv = 255 - a;
                        let r = (src[0] as u32 * a + bg.r() as u32 * inv) / 255;
                        let g = (src[1] as u32 * a + bg.g() as u32 * inv) / 255;
                        let b = (src[2] as u32 * a + bg.b() as u32 * inv) / 255;
                        layer.buf_mut()[idx] = Color::rgb(r as u8, g as u8, b as u8).0;
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

    layer.put_str_hud(16, tb_y + 6, "Baram OS (b2)", Color::MUTED);
    layer.put_str_hud(16, tb_y + 26, fb.as_str(), Color::MUTED);
}

fn render_frame(layer: &mut LayerSystem, wm: &mut WindowManager,
                _last_keys: &[&'static str], _mouse_ev: u32, key_ev: u32,
                fps: u32, mouse_mode: &str, cursor_x: i32, cursor_y: i32,
                ui_commands: &[uiscript::Command], ui_win_id: Option<window::WinId>,
                wallpaper: Option<&[u32]>,
                cached_taskbar: &mut Option<Vec<u32>>,
                add_progress: f32, remove_progress: f32,
                shift_x: f32) {
    render_scene(layer, wm, _mouse_ev, key_ev, fps, mouse_mode,
                 ui_commands, ui_win_id, wallpaper, cached_taskbar,
                 add_progress, remove_progress, shift_x);
    let is_resizing = wm.is_any_resizing();
    draw_cursor_into_layer(layer, cursor_x, cursor_y, is_resizing);
}

fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158f32;
    let c3 = c1 + 1.0;
    let t = t.min(1.0);
    1.0 + c3 * libm::powf(t - 1.0, 3.0) + c1 * libm::powf(t - 1.0, 2.0)
}

fn apply_mouse_event(cx: &mut i32, cy: &mut i32, ev: &MouseEvent,
                     screen_w: usize, screen_h: usize,
                     abs_max: (u64, u64)) -> (i32, i32) {
    let (abs_max_x, abs_max_y) = abs_max;
    if ev.is_absolute && abs_max_x > 0 && abs_max_y > 0 {
        let new_x = ((ev.abs_x as u128 * screen_w as u128) / abs_max_x as u128) as i32;
        let new_y = ((ev.abs_y as u128 * screen_h as u128) / abs_max_y as u128) as i32;
        *cx = new_x.max(0).min(screen_w as i32 - 1);
        *cy = new_y.max(0).min(screen_h as i32 - 1);
    } else {
        *cx = (*cx + ev.rel_dx).clamp(0, screen_w as i32 - 1);
        *cy = (*cy + ev.rel_dy).clamp(0, screen_h as i32 - 1);
    }
    (*cx, *cy)
}

fn time_diff_ns(a: &runtime::Time, b: &runtime::Time) -> u64 {
    let a_s = (a.hour() as u64) * 3600 + (a.minute() as u64) * 60 + a.second() as u64;
    let b_s = (b.hour() as u64) * 3600 + (b.minute() as u64) * 60 + b.second() as u64;
    let diff_s = b_s.saturating_sub(a_s);
    diff_s * 1_000_000_000 + (b.nanosecond() as u64).saturating_sub(a.nanosecond() as u64)
}
