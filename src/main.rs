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

// Mouse cursor SVG embedded at compile time from src/data/mouse.svg.
const CURSOR_SVG: &str = include_str!("data/mouse.svg");
// Cursor hit-box / draw-box size in pixels.  The SVG (15×19) is rendered
// inside this rectangle preserving aspect ratio.
const CURSOR_BOX_W: usize = 26;
const CURSOR_BOX_H: usize = 32;

fn draw_cursor_into_layer(layer: &mut LayerSystem, cx: i32, cy: i32) {
    svg::draw_svg_into(layer, CURSOR_SVG, cx, cy,
        CURSOR_BOX_W as f32, CURSOR_BOX_H as f32);
}

#[entry]
fn main() -> Status {
    let _ = uefi::helpers::init();
    ttf_font::init();

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

    // Create initial windows
    wm.add("システム情報", 40, 60, 320, 220);
    wm.add("Task Manager", 400, 80, 340, 260);

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

    // Initial full paint
    render_frame(&mut layer, &wm, &last_keys, mouse_ev_count, key_ev_count,
                 fps, mouse_mode_label, cursor_x, cursor_y);
    layer.flush(&mut screen);

    loop {
        let mut dirty = false;

        // ----- input -----
        if has_kbd {
            while let Some(ev) = Keyboard::poll() {
                key_ev_count = key_ev_count.wrapping_add(1);
                if last_keys.len() >= 6 { last_keys.remove(0); }
                last_keys.push(ev.label());

                let step = 12i32;
                let sw = layer.width() as i32;
                let sh = layer.height() as i32;
                match ev.scancode {
                    0x01 => cursor_y = (cursor_y - step).max(0),
                    0x02 => cursor_y = (cursor_y + step).min(sh - 1),
                    0x03 => cursor_x = (cursor_x + step).min(sw - 1),
                    0x04 => cursor_x = (cursor_x - step).max(0),
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
            }
        }

        if let Some(mouse) = mouse_opt.as_mut() {
            while let Some(ev) = mouse.poll() {
                mouse_ev_count = mouse_ev_count.wrapping_add(1);

                let (cx, cy) = apply_mouse_event(
                    &mut cursor_x, &mut cursor_y, &ev,
                    screen.width(), screen.height(), mouse.abs_max(),
                );

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
                } else if !ev.left && mouse_down {
                    mouse_down = false;
                    wm.on_mouse_up();
                }

                if mouse_down {
                    wm.on_mouse_drag(cx, cy);
                }

                dirty = true;
            }
        }

        // ----- frame timing -----
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

        if dirty || frames % 4 == 0 {
            render_frame(&mut layer, &wm, &last_keys, mouse_ev_count,
                         key_ev_count, fps, mouse_mode_label, cursor_x, cursor_y);
            layer.flush(&mut screen);
        }

        uefi::boot::stall(core::time::Duration::from_micros(8_000));
    }
}

/// Render the entire scene into the layer buffer.
fn render_frame(layer: &mut LayerSystem, wm: &WindowManager,
                _last_keys: &[&'static str], _mouse_ev: u32, key_ev: u32,
                fps: u32, mouse_mode: &str, cursor_x: i32, cursor_y: i32) {
    let w = layer.width();
    let h = layer.height();

    // 1. Background
    layer.clear(Color::BG);

    // 2. Title bar (Windows 10 dark style)
    layer.fill_rect(0, 0, w, 36, Color::WIN_INACTIVE);
    layer.put_str(16, 10, "BaramOS ウィンドウマネージャー", Color::TEXT);

    let mut fb = FmtBuf::new();
    fb.push_str("ウィンドウ数:");
    fb.push_u32(wm.count() as u32);
    fb.push_str(" FPS:");
    fb.push_u32(fps);
    layer.put_str(380, 10, fb.as_str(), Color::MUTED);

    let mut fb2 = FmtBuf::new();
    fb2.push_str("Mouse:");
    fb2.push_str(mouse_mode);
    fb2.push_str(" Keys:");
    fb2.push_u32(key_ev);
    layer.put_str(16, 22, fb2.as_str(), Color::MUTED);

    // 3. Windows (z-sorted)
    wm.draw_all(layer);

    // 4. Taskbar
    let tb_y = h.saturating_sub(TASKBAR_H);
    layer.fill_rect(0, tb_y, w, TASKBAR_H, Color::TASKBAR);

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

    // 5. Hint text
    let mid_y = tb_y.saturating_sub(20);
    layer.put_str(16, mid_y, "N: 新規ウィンドウ  |  Arrow keys: カーソルを移動", Color::MUTED);

    // 6. Cursor (on top of everything)
    draw_cursor_into_layer(layer, cursor_x, cursor_y);
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
