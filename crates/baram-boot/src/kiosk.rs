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
use crate::clock::UiMonotonicClock;
use nano_system::{NanoBasicPointerEvent, NanoKeyEvent, NanoSystem};

const LIST_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ScrollView xmlns:baram="http://schemas.baram.com/apk/res/baram" baram:layout_width="fill_parent" baram:layout_height="fill_parent" baram:fillViewport="false">
<LinearLayout baram:layout_width="fill_parent" baram:layout_height="wrap_content" baram:orientation="vertical" baram:padding="32dip" baram:layout_gap="12dip">
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
  <Button baram:id="app8" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app9" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app10" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app11" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app12" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app13" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app14" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app15" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app16" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app17" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app18" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app19" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app20" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app21" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app22" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app23" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app24" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app25" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app26" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app27" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app28" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app29" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app30" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
  <Button baram:id="app31" baram:layout_width="fill_parent" baram:layout_height="48dip" baram:text="" />
</LinearLayout>
</ScrollView>"#;
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
        } else if let (Some(current), Some(value)) = (name.as_ref(), trimmed.strip_prefix("title:"))
        {
            result.push(Entry {
                name: current.clone(),
                title: value.trim().to_string(),
            });
            name = None;
        }
    }
    result
}

fn now_ns() -> u64 {
    runtime::get_time()
        .ok()
        .map(|t| {
            (t.hour() as u64 * 3_600 + t.minute() as u64 * 60 + t.second() as u64) * 1_000_000_000
                + t.nanosecond() as u64
        })
        .unwrap_or(0)
}

// UEFI Simple Text Input scan codes: Up=1, Down=2, Right=3, Left=4.
// Horizontal arrows navigate controls; vertical arrows scroll the current
// document. Space activates the focused control, without XML changes.
fn handle_kiosk_key(engine: &mut Warp4Engine, event: NanoKeyEvent) -> bool {
    if let Some(key) = event.printable {
        if key == b' ' && engine.activate_focused() {
            return true;
        }
        engine.handle_key(key);
        return true;
    }
    match event.scancode {
        1 => engine.scroll_by(-engine.scroll_step()),
        2 => engine.scroll_by(engine.scroll_step()),
        3 => engine.focus_direction(1),
        4 => engine.focus_direction(-1),
        _ => false,
    }
}

fn pointer_xy(
    event: NanoBasicPointerEvent,
    nano: &NanoSystem,
    x: &mut i32,
    y: &mut i32,
    w: usize,
    h: usize,
) -> bool {
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

fn cursor_overlay_pixel(row: usize, col: usize) -> Option<Color> {
    let mut color = None;
    for shape_row in 0..15usize {
        let width = if shape_row < 10 { shape_row / 2 + 1 } else { 2 };
        if row >= shape_row && row < shape_row + 2 && col < width + 2 {
            color = Some(Color::BLACK);
        }
        if row == shape_row && width > 1 && col >= 1 && col < width {
            color = Some(Color::rgb(255, 255, 255));
        }
    }
    if (10..15).contains(&row) && (3..7).contains(&col) {
        color = Some(Color::BLACK);
    }
    if (10..14).contains(&row) && (4..6).contains(&col) {
        color = Some(Color::rgb(255, 255, 255));
    }
    color
}

fn draw_cursor(screen: &mut Screen, layer: &LayerSystem, x: i32, y: i32) {
    // The cursor is an independent screen overlay. It must not dirty the
    // backing layer, otherwise a pointer move would force a full UI flush.
    let x = x.max(0);
    let y = y.max(0);
    for row in 0..CURSOR_H as usize {
        let sy = y as usize + row;
        if sy >= layer.height() || x as usize >= layer.width() {
            continue;
        }
        let width = (CURSOR_W as usize).min(layer.width() - x as usize);
        let mut pixels = [Color::BLACK.0; CURSOR_W as usize];
        let start = sy * layer.width() + x as usize;
        pixels[..width].copy_from_slice(&layer.buf_ref()[start..start + width]);
        for col in 0..width {
            if let Some(color) = cursor_overlay_pixel(row, col) {
                pixels[col] = color.0;
            }
        }
        screen.flush_layer_row_range(sy, x as usize, &pixels[..width]);
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
    // Xiao is the only image that uses compact Warp4 metrics.  The normal
    // BaramOS binaries never call this and therefore remain at 100%.
    baram_warp4::set_ui_mode(baram_warp4::UiMode::Xiao);
    // Xiao calculates all XML layout metrics at 25%; button dimensions remain
    // owned by each screen's XML instead of a separate hardcoded profile.
    baram_warp4::set_ui_scale_percent(25);
    // Keep the 757 KiB BDF on the FAT volume. The parser streams it and keeps
    // only glyphs used by the current display while painting.
    baram_font::bdf_font::init_file("\\EFI\\BOOT\\MISAKI_GOTHIC_2ND.BDF");
    // Xiao uses the firmware's native 128x64 GOP mode. Warp4 uses the same
    // compact metrics for this kiosk only.
    let mut screen = match Screen::take_with_target(nano.display.width, nano.display.height) {
        Ok(s) => s,
        Err(_) => return Status::UNSUPPORTED,
    };
    NanoSystem::serial_log(&format!(
        "xiao: screen {}x{}\r\n",
        screen.width(),
        screen.height()
    ));
    unsafe {
        baram_font::log::init_screen(&screen);
    }
    let mut timer = nano.take_timer_event();
    // Runtime services expose wall-clock time with firmware-dependent
    // granularity. Xiao's animation clock must use the hardware monotonic
    // counter or a scroll can remain on its first frame until the next wall
    // clock tick.
    let ui_clock = UiMonotonicClock::new();
    let mut layer = LayerSystem::new_screen_backed(&mut screen);
    let mut display_state = baram_bsd::uri::DisplayState::new();
    baram_bsd::uri::load_settings_from_config(&mut display_state);
    let apps = entries();
    let app_titles: Vec<&str> = apps.iter().map(|entry| entry.title.as_str()).collect();
    baram_font::bdf_font::preload_texts(&app_titles);
    let sources = [("config.ini", LIST_CONFIG), ("main.w4u", LIST_XML)];
    let mut list = Some(Warp4Engine::new_embedded("__os_kiosk__", &sources));
    list.as_mut().unwrap().set_chrome_visible(false);
    for i in 0..32 {
        let id = format!("app{}", i);
        if let Some(entry) = apps.get(i) {
            list.as_mut().unwrap().set_text(&id, &entry.title);
            list.as_mut().unwrap().set_visible(&id, true);
        } else {
            list.as_mut().unwrap().set_visible(&id, false);
        }
    }
    // Resolve the first layout before accepting navigation input. This makes
    // a right-arrow pressed immediately after boot deterministic instead of
    // focusing a zero-sized, not-yet-laid-out node.
    if let Some(list) = list.as_mut() {
        list.draw_to_layer(&mut layer, 0, 0);
        layer.flush(&mut screen);
    }
    let mut selected: Option<Warp4Engine> = None;
    let mut x = (screen.width() / 2) as i32;
    let mut y = (screen.height() / 2) as i32;
    let mut cursor_x = x;
    let mut cursor_y = y;
    let mut cursor_drawn = false;
    let mut content_dirty = true;
    loop {
        if let Some(ref mut event) = timer {
            let _ = uefi::boot::wait_for_event(core::slice::from_mut(event));
        }
        if selected.is_none() {
            let mut display_changed = content_dirty;
            content_dirty = false;
            let mut clicked_id = None;
            if let Some(list) = list.as_mut() {
                while let Some(event) = nano.poll_keyboard() {
                    if handle_kiosk_key(list, event) {
                        display_changed = true;
                    }
                }
                while let Some(event) = nano.poll_pointer() {
                    let was_down = pointer_xy(
                        event,
                        &nano,
                        &mut x,
                        &mut y,
                        screen.width(),
                        screen.height(),
                    );
                    let document_y = y.saturating_add(list.scroll_position());
                    if was_down {
                        list.click(x, document_y);
                        display_changed = true;
                    } else if list.has_pressed() {
                        list.release();
                        display_changed = true;
                    }
                    display_changed |= list.set_hover_changed(x, document_y);
                    display_changed |= list.pointer_move(x, y);
                }
                let animation_now_ns = ui_clock
                    .as_ref()
                    .map(UiMonotonicClock::elapsed_ns)
                    .unwrap_or_else(now_ns);
                display_changed |= list.tick(animation_now_ns);
                clicked_id = list.take_clicked_id();
                if clicked_id.is_none() && display_changed {
                    list.draw_to_layer(&mut layer, 0, 0);
                    layer.flush(&mut screen);
                }
            }
            if let Some(id) = clicked_id {
                if let Some(index) = id.strip_prefix("app").and_then(|v| v.parse::<usize>().ok()) {
                    if let Some(entry) = apps.get(index) {
                        // Keep launcher glyphs while priming the selected
                        // application's XML text. The one-way kiosk shares a
                        // single cache for all repaint and scroll frames.
                        let mut engine = Warp4Engine::new(&entry.name);
                        engine.set_chrome_visible(false);
                        // Resolve the selected app before accepting its first
                        // key event. This also primes the visible glyph cache
                        // while the launcher is still on screen.
                        engine.draw_to_layer(&mut layer, 0, 0);
                        engine.start_transition();
                        selected = Some(engine);
                        // The kiosk is single-task. Release the launcher
                        // engine before the selected app becomes active.
                        list = None;
                        content_dirty = true;
                    }
                }
            }
        } else if let Some(engine) = selected.as_mut() {
            let mut display_changed = content_dirty;
            content_dirty = false;
            while let Some(event) = nano.poll_keyboard() {
                if handle_kiosk_key(engine, event) {
                    display_changed = true;
                }
            }
            while let Some(event) = nano.poll_pointer() {
                let down = pointer_xy(
                    event,
                    &nano,
                    &mut x,
                    &mut y,
                    screen.width(),
                    screen.height(),
                );
                let document_y = y.saturating_add(engine.scroll_position());
                if down {
                    engine.click(x, document_y);
                    display_changed = true;
                } else if engine.has_pressed() {
                    engine.release();
                    display_changed = true;
                }
                display_changed |= engine.set_hover_changed(x, document_y);
                display_changed |= engine.pointer_move(x, y);
            }
            let animation_now_ns = ui_clock
                .as_ref()
                .map(UiMonotonicClock::elapsed_ns)
                .unwrap_or_else(now_ns);
            display_changed |= engine.tick(animation_now_ns);
            // Xiao is a single-task kiosk: an app's OS-setting command is
            // executed immediately, without a permission window or hash
            // lookup.  There is no second app/window to authorize against.
            if let Some(command) = engine.take_command() {
                if command.starts_with("os://") {
                    let _ = baram_bsd::uri::execute(&command, &mut display_state);
                    engine.refresh_config();
                    display_changed = true;
                }
            }
            if display_changed {
                engine.draw_to_layer(&mut layer, 0, 0);
                layer.flush(&mut screen);
            }
        }
        if !cursor_drawn {
            draw_cursor(&mut screen, &layer, x, y);
            cursor_drawn = true;
        } else if cursor_x != x || cursor_y != y {
            restore_cursor_background(&mut screen, &layer, cursor_x, cursor_y);
            draw_cursor(&mut screen, &layer, x, y);
        }
        cursor_x = x;
        cursor_y = y;
    }
}
