//! Minimal x86_64 kiosk entry point.
//!
//! This intentionally uses only Nano System, the UEFI-backed Baram filesystem,
//! and Warp4.  There is no desktop, window manager, subsystem scheduler, or
//! return path: one application is selected and then owns the whole screen.

#![allow(dead_code)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use uefi::prelude::*;
use uefi::runtime;

use baram_bsd::{app, config};
use baram_core::{Color, LayerSystem, Screen};
use baram_warp4::Warp4Engine;
use nano_system::{NanoBasicPointerEvent, NanoKeyEvent, NanoSystem};

const LIST_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<LinearLayout xmlns:baram="http://schemas.baram.com/apk/res/baram" baram:layout_width="fill_parent" baram:layout_height="fill_parent" baram:orientation="vertical" baram:padding="32dip" baram:layout_gap="12dip">
  <TextView baram:id="title" baram:layout_width="fill_parent" baram:layout_height="wrap_content" baram:text="BaramOS" baram:textSize="30sp" />
  <TextView baram:id="subtitle" baram:layout_width="fill_parent" baram:layout_height="wrap_content" baram:text="アプリケーションを選択してください" baram:textSize="16sp" />
  <Button baram:id="app0" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app1" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app2" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app3" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app4" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app5" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app6" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app7" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
</LinearLayout>"#;
const LIST_CONFIG: &str = "version=4\nscreen=main\nname=BaramOS\n";

struct Entry {
    name: String,
    title: String,
}

fn entries() -> Vec<Entry> {
    let text = app::read_index_yaml();
    let mut result = Vec::new();
    let mut name = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if line.starts_with("  ") && trimmed.ends_with(':') && !trimmed.starts_with("autostart:") {
            let candidate = trimmed.trim_end_matches(':').trim();
            if candidate.ends_with(".w4a") {
                name = Some(candidate.to_string());
            }
        } else if let (Some(current), Some(value)) = (name.as_ref(), trimmed.strip_prefix("title:")) {
            result.push(Entry { name: current.clone(), title: value.trim().to_string() });
            name = None;
        }
    }
    result
}

fn now_ns() -> u64 {
    runtime::get_time().ok().map(|t| {
        (t.hour() as u64 * 3_600 + t.minute() as u64 * 60 + t.second() as u64) * 1_000_000_000
            + t.nanosecond() as u64
    }).unwrap_or(0)
}

fn key_value(event: NanoKeyEvent) -> Option<u8> { event.printable }

fn pointer_xy(event: NanoBasicPointerEvent, nano: &NanoSystem, x: &mut i32, y: &mut i32, w: usize, h: usize) -> bool {
    if let Some((px, py, max_x, max_y)) = event.absolute {
        *x = ((px.saturating_mul(w as u64)) / max_x.max(1)) as i32;
        *y = ((py.saturating_mul(h as u64)) / max_y.max(1)) as i32;
    } else {
        *x = (*x + event.dx).clamp(0, w.saturating_sub(1) as i32);
        *y = (*y + event.dy).clamp(0, h.saturating_sub(1) as i32);
    }
    let _ = nano;
    nano.input_state.left
}

const CURSOR_W: i32 = 12;
const CURSOR_H: i32 = 17;

fn cursor_pixel(screen: &mut Screen, x: i32, y: i32, color: Color) {
    if x >= 0 && y >= 0 {
        screen.put_pixel(x as usize, y as usize, color);
    }
}

fn draw_cursor(screen: &mut Screen, x: i32, y: i32) {
    // The cursor is an independent screen overlay. It must not dirty the
    // backing layer, otherwise a pointer move would force a full UI flush.
    let x = x.max(0);
    let y = y.max(0);
    const WHITE: Color = Color::rgb(255, 255, 255);
    for row in 0..15usize {
        let width = if row < 10 { row / 2 + 1 } else { 2 };
        for dy in 0..2 {
            for dx in 0..width + 2 {
                cursor_pixel(screen, x + dx as i32, y + row as i32 + dy, Color::BLACK);
            }
        }
        if width > 1 {
            for dx in 0..width - 1 {
                cursor_pixel(screen, x + 1 + dx as i32, y + row as i32, WHITE);
            }
        }
    }
    for dy in 0..5 {
        for dx in 0..4 {
            cursor_pixel(screen, x + 3 + dx, y + 10 + dy, Color::BLACK);
        }
    }
    for dy in 0..4 {
        for dx in 0..2 {
            cursor_pixel(screen, x + 4 + dx, y + 10 + dy, WHITE);
        }
    }
}

fn restore_cursor_background(screen: &mut Screen, layer: &LayerSystem, x: i32, y: i32) {
    let x0 = x.max(0) as usize;
    let y0 = y.max(0) as usize;
    let x1 = (x + CURSOR_W).clamp(0, layer.width() as i32) as usize;
    let y1 = (y + CURSOR_H).clamp(0, layer.height() as i32) as usize;
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    for row in y0..y1 {
        let start = row * layer.width() + x0;
        let end = row * layer.width() + x1;
        screen.flush_layer_row_range(row, x0, &layer.buf_ref()[start..end]);
    }
}

pub fn run(mut nano: NanoSystem) -> Status {
    config::init_config();
    // Keep the 757 KiB BDF on the FAT volume. The parser streams it and keeps
    // only glyphs used by the current display while painting.
    baram_font::bdf_font::init_file("\\EFI\\BOOT\\MISAKI_GOTHIC_2ND.BDF");
    baram_font::bdf_font::clear_cache();
    // Xiao uses a 320x180 working surface: exactly half the previous 640x360
    // target. Warp4 is compiled with its Xiao scale profile as well, so its
    // layout metrics and rounded-corner geometry match this surface.
    let mut screen = match Screen::take_with_target(320, 180) {
        Ok(s) => s,
        Err(_) => return Status::UNSUPPORTED,
    };
    NanoSystem::serial_log(&format!("xiao: screen {}x{}\r\n", screen.width(), screen.height()));
    unsafe { baram_font::log::init_screen(&screen); }
    let mut timer = nano.take_timer_event();
    let mut layer = LayerSystem::new(screen.width(), screen.height());
    let apps = entries();
    let sources = [("config.ini", LIST_CONFIG), ("main.w4u", LIST_XML)];
    let mut list = Warp4Engine::new_embedded("__os_kiosk__", &sources);
    list.set_chrome_visible(false);
    for i in 0..8 {
        list.set_text(&format!("app{}", i), apps.get(i).map(|e| e.title.as_str()).unwrap_or(""));
    }
    let mut selected: Option<Warp4Engine> = None;
    let mut x = (screen.width() / 2) as i32;
    let mut y = (screen.height() / 2) as i32;
    let mut cursor_x = x;
    let mut cursor_y = y;
    let mut cursor_drawn = false;
    let mut content_dirty = true;
    loop {
        if let Some(ref mut event) = timer { let _ = uefi::boot::wait_for_event(core::slice::from_mut(event)); }
        if selected.is_none() {
            let mut display_changed = content_dirty;
            content_dirty = false;
            while let Some(event) = nano.poll_keyboard() {
                if let Some(key) = key_value(event) {
                    list.handle_key(key);
                    display_changed = true;
                }
            }
            while let Some(event) = nano.poll_pointer() {
                let was_down = pointer_xy(event, &nano, &mut x, &mut y, screen.width(), screen.height());
                if was_down {
                    list.click(x, y);
                    display_changed = true;
                }
                display_changed |= list.pointer_move(x, y);
            }
            display_changed |= list.tick(now_ns());
            if let Some(id) = list.take_clicked_id() {
                if let Some(index) = id.strip_prefix("app").and_then(|v| v.parse::<usize>().ok()) {
                    if let Some(entry) = apps.get(index) {
                        // The launcher glyphs are no longer displayed. Drop
                        // them before building the full-screen app cache.
                        baram_font::bdf_font::clear_cache();
                        let mut engine = Warp4Engine::new(&entry.name);
                        engine.set_chrome_visible(false);
                        selected = Some(engine);
                        content_dirty = true;
                        display_changed = true;
                    }
                }
            }
            if display_changed {
                baram_font::bdf_font::clear_cache();
                list.draw_to_layer(&mut layer, 0, 0);
                layer.flush(&mut screen);
            }
        } else if let Some(engine) = selected.as_mut() {
            let mut display_changed = content_dirty;
            content_dirty = false;
            while let Some(event) = nano.poll_keyboard() {
                if let Some(key) = key_value(event) {
                    engine.handle_key(key);
                    display_changed = true;
                }
            }
            while let Some(event) = nano.poll_pointer() {
                let down = pointer_xy(event, &nano, &mut x, &mut y, screen.width(), screen.height());
                if down {
                    engine.click(x, y);
                    display_changed = true;
                } else if engine.has_pointer_capture() {
                    engine.release();
                    display_changed = true;
                }
                display_changed |= engine.pointer_move(x, y);
            }
            display_changed |= engine.tick(now_ns());
            if display_changed {
                baram_font::bdf_font::clear_cache();
                engine.draw_to_layer(&mut layer, 0, 0);
                layer.flush(&mut screen);
            }
        }
        if !cursor_drawn {
            draw_cursor(&mut screen, x, y);
            cursor_drawn = true;
        } else if cursor_x != x || cursor_y != y {
            restore_cursor_background(&mut screen, &layer, cursor_x, cursor_y);
            draw_cursor(&mut screen, x, y);
        }
        cursor_x = x;
        cursor_y = y;
    }
}
