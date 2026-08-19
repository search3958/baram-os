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

fn draw_cursor(layer: &mut LayerSystem, x: i32, y: i32) {
    // Small OS-owned arrow cursor. Drawn after Warp4 so it remains visible
    // over both the launcher and the selected full-screen application.
    let x = x.max(0) as usize;
    let y = y.max(0) as usize;
    const WHITE: Color = Color::rgb(255, 255, 255);
    for row in 0..15usize {
        let width = if row < 10 { row / 2 + 1 } else { 2 };
        layer.fill_rect(x, y + row, width + 2, 2, Color::BLACK);
        if width > 1 {
            layer.fill_rect(x + 1, y + row, width - 1, 1, WHITE);
        }
    }
    layer.fill_rect(x + 3, y + 10, 4, 5, Color::BLACK);
    layer.fill_rect(x + 4, y + 10, 2, 4, WHITE);
}

pub fn run(mut nano: NanoSystem) -> Status {
    config::init_config();
    baram_font::ttf_font::init();
    baram_font::ttf_font_hud::init();
    let mut screen = match Screen::take_with_target(640, 360) {
        Ok(s) => s,
        Err(_) => return Status::UNSUPPORTED,
    };
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
    loop {
        if let Some(ref mut event) = timer { let _ = uefi::boot::wait_for_event(core::slice::from_mut(event)); }
        if selected.is_none() {
            while let Some(event) = nano.poll_keyboard() {
                if let Some(key) = key_value(event) { list.handle_key(key); }
            }
            while let Some(event) = nano.poll_pointer() {
                let was_down = pointer_xy(event, &nano, &mut x, &mut y, screen.width(), screen.height());
                if was_down { list.click(x, y); }
                list.pointer_move(x, y);
            }
            list.tick(now_ns());
            if let Some(id) = list.take_clicked_id() {
                if let Some(index) = id.strip_prefix("app").and_then(|v| v.parse::<usize>().ok()) {
                    if let Some(entry) = apps.get(index) {
                        let mut engine = Warp4Engine::new(&entry.name);
                        engine.set_chrome_visible(false);
                        selected = Some(engine);
                    }
                }
            }
            list.draw_to_layer(&mut layer, 0, 0);
        } else if let Some(engine) = selected.as_mut() {
            while let Some(event) = nano.poll_keyboard() {
                if let Some(key) = key_value(event) { engine.handle_key(key); }
            }
            while let Some(event) = nano.poll_pointer() {
                let down = pointer_xy(event, &nano, &mut x, &mut y, screen.width(), screen.height());
                if down { engine.click(x, y); } else { engine.release(); }
                engine.pointer_move(x, y);
            }
            engine.tick(now_ns());
            engine.draw_to_layer(&mut layer, 0, 0);
        }
        draw_cursor(&mut layer, x, y);
        layer.flush(&mut screen);
    }
}
