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

const TASKBAR_H: usize = 32;
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

    let mut tmp: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..ph {
        for x in 0..pw {
            let mut sum = 0.0f32;
            let mut cnt = 0.0f32;
            for dx in -blur_r..=blur_r {
                let sx = x as i32 + dx;
                if sx >= 0 && sx < pw as i32 {
                    sum += padded[y * pw + sx as usize];
                    cnt += 1.0;
                }
            }
            tmp[y * pw + x] = sum / cnt;
        }
    }
    let mut result: Vec<f32> = alloc::vec![0.0; pw * ph];
    for y in 0..ph {
        for x in 0..pw {
            let mut sum = 0.0f32;
            let mut cnt = 0.0f32;
            for dy in -blur_r..=blur_r {
                let sy = y as i32 + dy;
                if sy >= 0 && sy < ph as i32 {
                    sum += tmp[sy as usize * pw + x];
                    cnt += 1.0;
                }
            }
            result[y * pw + x] = sum / cnt;
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

fn draw_cursor_into_layer(layer: &mut LayerSystem, cx: i32, cy: i32, resizing: bool) {
    unsafe {
        let bitmap = if resizing {
            CURSOR_RESIZE.as_ref()
        } else {
            CURSOR_NORMAL.as_ref()
        };
        if let Some(bmp) = bitmap {
            let blur_r = 8i32;
            let pad = blur_r as i32;
            svg::blit_shadow(layer, &bmp.shadow, bmp.shadow_w, bmp.shadow_h,
                cx + 3 - pad, cy + 4 - pad);
            svg::blit_cached(layer, &bmp.pixels, bmp.w, bmp.h, cx, cy);
            return;
        }
    }
    if resizing {
        svg::draw_svg_shadow(layer, CURSOR_SVG_SIZE, cx + 3, cy + 4,
            CURSOR_BOX_SIZE_W as f32, CURSOR_BOX_SIZE_H as f32, 8, 0);
        svg::draw_svg_into(layer, CURSOR_SVG_SIZE, cx, cy,
            CURSOR_BOX_SIZE_W as f32, CURSOR_BOX_SIZE_H as f32);
    } else {
        svg::draw_svg_shadow(layer, CURSOR_SVG, cx + 3, cy + 4,
            CURSOR_BOX_W as f32, CURSOR_BOX_H as f32, 8, 0);
        svg::draw_svg_into(layer, CURSOR_SVG, cx, cy,
            CURSOR_BOX_W as f32, CURSOR_BOX_H as f32);
    }
}

#[entry]
fn main() -> Status {
    let _ = uefi::helpers::init();
    ttf_font::init();

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

    render_scene(&mut layer, &mut wm, mouse_ev_count, key_ev_count,
                 fps, mouse_mode_label,
                 &ui_commands, Some(ui_win_id), cached_wallpaper.as_deref());
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
                        let ids = wm.sorted_ids();
                        let mut bx = 8i32;
                        for id in &ids {
                            if cx >= bx && cx < bx + 80 {
                                wm.focus(*id);
                                break;
                            }
                            bx += 88;
                        }
                    } else {
                        wm.on_mouse_down(cx, cy);
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
                scene_dirty = true;
            }
        }

        if wm.take_order_changed() {
            scene_dirty = true;
            dirty = true;
        }

        if dirty {
            let is_resizing = wm.is_any_resizing() || wm.is_over_resize_handle(cursor_x, cursor_y);

            if scene_dirty {
                let (bx0, by0, bx1, by1) = prev_dirty;

                render_scene(&mut layer, &mut wm, mouse_ev_count, key_ev_count,
                             fps, mouse_mode_label,
                             &ui_commands, Some(ui_win_id), cached_wallpaper.as_deref());

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
                wallpaper: Option<&[u32]>) {
    let w = layer.width();
    let h = layer.height();

    if let Some(pixels) = wallpaper {
        layer.buf_mut()[..w * h].copy_from_slice(pixels);
    } else {
        layer.clear(Color::BG);
    }

    let tb_y = h.saturating_sub(TASKBAR_H);
    let tb_alpha = 180u32;
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

    wm.draw_all(layer, ui_win_id.map(|id| (id, ui_commands)));

    let ids = wm.sorted_ids();
    let mut bx = 8i32;
    for id in &ids {
        let title = wm.get_title(*id).unwrap_or("???");
        let is_focused = wm.focused_id == Some(*id);
        let bg = if is_focused { Color::ACCENT } else { Color::WIN_INACTIVE };
        layer.fill_rect(bx as usize, tb_y + 6, 80, 20, bg);
        layer.put_str(bx as usize + 4, tb_y + 9, title, Color::TEXT);
        bx += 88;
    }

    let mut fb = FmtBuf::new();
    fb.push_str("Key:");
    fb.push_u32(key_ev);
    fb.push_str(" Window:");
    fb.push_u32(wm.count() as u32);
    fb.push_str(" ");
    fb.push_u32(fps);
    fb.push_str("FPS");

    layer.put_str(16, tb_y.saturating_sub(32), "Baram OS (b2)", Color::MUTED);
    layer.put_str(16, tb_y.saturating_sub(20), fb.as_str(),  Color::MUTED);
}

fn render_frame(layer: &mut LayerSystem, wm: &mut WindowManager,
                _last_keys: &[&'static str], _mouse_ev: u32, key_ev: u32,
                fps: u32, mouse_mode: &str, cursor_x: i32, cursor_y: i32,
                ui_commands: &[uiscript::Command], ui_win_id: Option<window::WinId>,
                wallpaper: Option<&[u32]>) {
    render_scene(layer, wm, _mouse_ev, key_ev, fps, mouse_mode,
                 ui_commands, ui_win_id, wallpaper);
    let is_resizing = wm.is_any_resizing();
    draw_cursor_into_layer(layer, cursor_x, cursor_y, is_resizing);
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
