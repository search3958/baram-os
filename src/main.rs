#![no_std]
#![no_main]

extern crate alloc;

mod absolute_pointer;
mod cursor;
mod font;
mod gop;
mod keyboard;
mod mouse;
mod ui;
mod usb_hid;
mod window;

use alloc::vec::Vec;

use uefi::prelude::*;
use uefi::runtime;

use crate::cursor::Cursor;
use crate::gop::{Color, Screen};
use crate::keyboard::Keyboard;
use crate::mouse::{Mouse, MouseEvent};
use crate::ui::{FmtBuf, put_str};
use crate::window::{WindowManager, LayerSystem};

const TASKBAR_H: usize = 32;

#[entry]
fn main() -> Status {
    let _ = uefi::helpers::init();

    let mut screen = match Screen::take() {
        Ok(s) => s,
        Err(_s) => return Status::UNSUPPORTED,
    };

    screen.clear(Color::BG);
    draw_boot_splash(&mut screen);

    let mut mouse_opt: Option<Mouse> = match Mouse::open() {
        Ok(m) => Some(m),
        Err(_) => None,
    };
    let has_kbd = Keyboard::is_present();
    if has_kbd { Keyboard::reset(); }

    let mut cursor = Cursor::new(
        (screen.width() / 2) as i32,
        (screen.height() / 2) as i32,
    );
    cursor.init_save_buffer();

    let mut wm = WindowManager::new();
    let mut _layer = LayerSystem::new(screen.width(), screen.height());

    // Create initial windows (the "2 cards" → 2 windows)
    wm.add("System Info", 40, 60, 320, 220);
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
    draw_all(&mut screen, &cursor, &wm, &last_keys, mouse_ev_count, key_ev_count,
             fps, mouse_mode_label, has_kbd);
    cursor.draw(&mut screen);

    loop {
        let mut dirty = false;

        // ----- input -----
        if has_kbd {
            while let Some(ev) = Keyboard::poll() {
                key_ev_count = key_ev_count.wrapping_add(1);
                if last_keys.len() >= 6 { last_keys.remove(0); }
                last_keys.push(ev.label());

                let step = 12i32;
                match ev.scancode {
                    0x01 => cursor.move_by(0, -step, screen.width() as i32, screen.height() as i32),
                    0x02 => cursor.move_by(0,  step, screen.width() as i32, screen.height() as i32),
                    0x03 => cursor.move_by( step, 0, screen.width() as i32, screen.height() as i32),
                    0x04 => cursor.move_by(-step, 0, screen.width() as i32, screen.height() as i32),
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
                    &mut cursor, &ev, screen.width(), screen.height(),
                    mouse.abs_max(),
                );

                if ev.left && !mouse_down {
                    mouse_down = true;
                    // Check taskbar clicks
                    let sh = screen.height();
                    if cy >= sh as i32 - TASKBAR_H as i32 {
                        // Taskbar: click on window button to focus
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
            _layer.tick();
            cursor.restore_bg(&mut screen);
            draw_all(&mut screen, &cursor, &wm, &last_keys, mouse_ev_count,
                     key_ev_count, fps, mouse_mode_label, has_kbd);
            cursor.draw(&mut screen);
        }

        uefi::boot::stall(core::time::Duration::from_micros(8_000));
    }
}

fn apply_mouse_event(cursor: &mut Cursor, ev: &MouseEvent,
                     screen_w: usize, screen_h: usize,
                     abs_max: (u64, u64)) -> (i32, i32) {
    let (abs_max_x, abs_max_y) = abs_max;
    if ev.is_absolute && abs_max_x > 0 && abs_max_y > 0 {
        let new_x = ((ev.abs_x as u128 * screen_w as u128) / abs_max_x as u128) as i32;
        let new_y = ((ev.abs_y as u128 * screen_h as u128) / abs_max_y as u128) as i32;
        cursor.x = new_x.max(0).min(screen_w as i32 - 1);
        cursor.y = new_y.max(0).min(screen_h as i32 - 1);
    } else {
        cursor.move_by(ev.rel_dx, ev.rel_dy, screen_w as i32, screen_h as i32);
    }
    (cursor.x, cursor.y)
}

fn time_diff_ns(a: &runtime::Time, b: &runtime::Time) -> u64 {
    let a_s = (a.hour() as u64) * 3600 + (a.minute() as u64) * 60 + a.second() as u64;
    let b_s = (b.hour() as u64) * 3600 + (b.minute() as u64) * 60 + b.second() as u64;
    let diff_s = b_s.saturating_sub(a_s);
    diff_s * 1_000_000_000 + (b.nanosecond() as u64).saturating_sub(a.nanosecond() as u64)
}

fn draw_boot_splash(screen: &mut Screen) {
    let w = screen.width();
    let h = screen.height();
    screen.fill_rect(0, 0, w, 36, Color::PANEL);
    screen.fill_rect(0, 36, w, 2, Color::ACCENT);
    put_str(screen, 16, 10, "MyOS  v0.3  -  Window Manager",
            Color::TEXT, Color::PANEL);
    put_str(screen, 16, 22, "Drag title bars  |  Resize from corner  |  N=new  Q=quit",
            Color::MUTED, Color::PANEL);

    let fy = h.saturating_sub(TASKBAR_H + 2);
    screen.fill_rect(0, fy, w, 2, Color::ACCENT);
}

fn draw_all(screen: &mut Screen, _cursor: &Cursor, wm: &WindowManager,
            _last_keys: &[&'static str], _mouse_ev: u32, key_ev: u32,
            fps: u32, mouse_mode: &str, _has_kbd: bool) {
    let w = screen.width();
    let h = screen.height();

    // Background
    screen.fill_rect(0, 0, w, h, Color::BG);

    // Title bar
    screen.fill_rect(0, 0, w, 36, Color::PANEL);
    screen.fill_rect(0, 36, w, 2, Color::ACCENT);
    put_str(screen, 16, 10, "MyOS  v0.3  -  Window Manager",
            Color::TEXT, Color::PANEL);

    let mut fb = FmtBuf::new();
    fb.push_str("Win:");
    fb.push_u32(wm.count() as u32);
    fb.push_str(" FPS:");
    fb.push_u32(fps);
    put_str(screen, 380, 10, fb.as_str(), Color::MUTED, Color::PANEL);

    let mut fb2 = FmtBuf::new();
    fb2.push_str("Mouse:");
    fb2.push_str(mouse_mode);
    fb2.push_str(" Keys:");
    fb2.push_u32(key_ev);
    put_str(screen, 16, 22, fb2.as_str(), Color::MUTED, Color::PANEL);

    // Windows
    wm.draw_all(screen);

    // Taskbar
    let tb_y = h.saturating_sub(TASKBAR_H);
    screen.fill_rect(0, tb_y, w, TASKBAR_H, Color::PANEL);
    screen.fill_rect(0, tb_y, w, 2, Color::ACCENT);

    let ids = wm.sorted_ids();
    let mut bx = 8i32;
    for id in &ids {
        let title = wm.get_title(*id).unwrap_or("???");
        let is_focused = wm.focused_id == Some(*id);
        let bg = if is_focused { Color::ACCENT } else { Color::BLACK };
        screen.fill_rect(bx as usize, tb_y + 6, 80, 20, bg);
        put_str(screen, bx as usize + 4, tb_y + 9, title, Color::TEXT, bg);
        bx += 88;
    }

    // Bottom hint
    let mid_y = tb_y.saturating_sub(20);
    put_str(screen, 16, mid_y,
            "N: new window  |  Arrow keys: move cursor",
            Color::MUTED, Color::BG);
}

fn log_line(s: &str) {
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.output_string(cstr16!("MyOS: "));
        let mut buf = alloc::vec::Vec::<u16>::with_capacity(s.len() + 1);
        for &b in s.as_bytes() {
            if b >= 0x80 { break; }
            buf.push(b as u16);
        }
        buf.push(0);
        if let Ok(cs) = uefi::CStr16::from_u16_with_nul(&buf) {
            let _ = stdout.output_string(cs);
        }
        let _ = stdout.output_string(cstr16!("\r\n"));
    });
}
