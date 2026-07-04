//! MyOS — a tiny UEFI ARM64 OS demo.
//!
//! This crate is a UEFI application (PE32+ image) targeting `aarch64-unknown-uefi`.
//! It uses:
//!   * Graphics Output Protocol   — direct framebuffer drawing
//!   * Simple Pointer Protocol    — mouse driver (delta input → absolute cursor)
//!   * Simple Text Input          — keyboard driver
//!
//! The application initialises all three protocols, renders a small status
//! UI (title bar, mouse coords, last key pressed, FPS counter), and runs a
//! vsync-style main loop until the user powers off the machine.
//!
//! Build:
//! ```
//! cargo build --release
//! ```
//! The resulting `bootaa64.efi` is placed in `EFI/BOOT/BOOTAA64.EFI` on a
//! FAT image and launched by the firmware.

#![no_std]
#![no_main]

extern crate alloc;

mod cursor;
mod font;
mod gop;
mod keyboard;
mod mouse;
mod ui;

use alloc::vec::Vec;

use uefi::prelude::*;
use uefi::runtime;

use crate::cursor::Cursor;
use crate::gop::{Color, Screen};
use crate::keyboard::Keyboard;
use crate::ui::{FmtBuf, put_str, put_str_transparent};
/// Entry point.  In uefi-rs 0.38 the entry function takes no arguments;
/// the system table is reached via the `uefi::system` / `uefi::boot`
/// module functions.
#[entry]
fn main() -> Status {
    // Initialise uefi helpers (logger, panic handler, allocator).
    let _ = uefi::helpers::init();

    // Bring up the screen first so we have somewhere to draw errors.
    let mut screen = match Screen::take() {
        Ok(s) => s,
        Err(_s) => {
            // Without GOP we cannot draw anything; bail to firmware.
            return Status::UNSUPPORTED;
        }
    };

    // Fill the background.
    screen.clear(Color::BG);

    // Boot splash.
    draw_boot_splash(&mut screen);

    // Peripherals.
    let has_mouse = mouse::mouse_present();
    let has_kbd = Keyboard::is_present();
    if has_kbd { Keyboard::reset(); }
    if has_mouse { mouse::reset_all(); }

    // Cursor (centre of screen).
    let mut cursor = Cursor::new(
        (screen.width() / 2) as i32,
        (screen.height() / 2) as i32,
    );
    cursor.init_save_buffer();

    // UI state.
    let mut last_keys: Vec<&'static str> = Vec::with_capacity(8);
    let mut mouse_ev_count: u32 = 0;
    let mut key_ev_count: u32 = 0;
    let mut frames: u32 = 0;
    let mut frames_since_tick: u32 = 0;
    let mut fps: u32 = 0;
    let mut start_time = runtime::get_time().unwrap_or_else(|_| runtime::Time::invalid());

    // Initial full UI paint.
    draw_ui(&mut screen, &cursor, &last_keys, mouse_ev_count, key_ev_count, fps,
            has_mouse, has_kbd);
    cursor.draw(&mut screen);

    // Main loop.  Runs forever; user powers off the VM.
    loop {
        let mut dirty = false;

        // ----- input -----
        if has_kbd {
            while let Some(ev) = Keyboard::poll() {
                key_ev_count = key_ev_count.wrapping_add(1);
                if last_keys.len() >= 6 { last_keys.remove(0); }
                last_keys.push(ev.label());
                dirty = true;
            }
        }
        if has_mouse {
            while let Some(ev) = mouse::poll_mouse() {
                mouse_ev_count = mouse_ev_count.wrapping_add(1);
                cursor.move_by(ev.dx, ev.dy,
                               screen.width() as i32, screen.height() as i32);
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

        // Always repaint a few times per second even if no input, so the
        // FPS counter ticks.  We repaint every frame for simplicity — the
        // firmware is fast enough on QEMU.
        if dirty || frames % 4 == 0 {
            // Restore old cursor background.
            cursor.restore_bg(&mut screen);
            // Repaint UI.
            draw_ui(&mut screen, &cursor, &last_keys, mouse_ev_count,
                    key_ev_count, fps, has_mouse, has_kbd);
            // Draw cursor at new position.
            cursor.draw(&mut screen);
        }

        // Yield to firmware briefly so it can flush the framebuffer.
        // ~8ms (~120fps cap)
        uefi::boot::stall(core::time::Duration::from_micros(8_000));
    }
}

/// Compute elapsed nanoseconds between two UEFI `Time` values.
fn time_diff_ns(a: &runtime::Time, b: &runtime::Time) -> u64 {
    // Convert each to a rough nanosecond count.  UEFI Time has year/month/day
    // plus hour/minute/second/nanosecond.  We just diff the sub-day portion
    // since the loop runs in real time.
    let a_s = (a.hour() as u64) * 3600 + (a.minute() as u64) * 60 + a.second() as u64;
    let b_s = (b.hour() as u64) * 3600 + (b.minute() as u64) * 60 + b.second() as u64;
    let diff_s = b_s.saturating_sub(a_s);
    diff_s * 1_000_000_000 + (b.nanosecond() as u64).saturating_sub(a.nanosecond() as u64)
}

/// Static boot splash drawn once before the main loop starts.
fn draw_boot_splash(screen: &mut Screen) {
    let w = screen.width();
    let h = screen.height();
    // Title bar across the top.
    screen.fill_rect(0, 0, w, 36, Color::PANEL);
    screen.fill_rect(0, 36, w, 2, Color::ACCENT);
    put_str(screen, 16, 10, "MyOS  v0.1  -  UEFI ARM64  (Raspberry Pi / QEMU virt)",
            Color::TEXT, Color::PANEL);
    put_str(screen, 16, 22, "Mouse + Keyboard + Graphics demo",
            Color::MUTED, Color::PANEL);

    // Footer hint bar.
    let fy = h.saturating_sub(28);
    screen.fill_rect(0, fy, w, 28, Color::PANEL);
    screen.fill_rect(0, fy - 2, w, 2, Color::ACCENT);
    put_str(screen, 16, fy + 8,
            "Move the mouse  |  Press any key  |  Close QEMU window to exit",
            Color::MUTED, Color::PANEL);
}

/// Repaint the dynamic UI panels.
fn draw_ui(screen: &mut Screen,
           cursor: &Cursor,
           last_keys: &[&'static str],
           mouse_ev: u32,
           key_ev: u32,
           fps: u32,
           has_mouse: bool,
           has_kbd: bool) {
    let w = screen.width();
    let h = screen.height();

    // Side panel on the left.
    let px = 16;
    let py = 56;
    let pw = 320.min(w.saturating_sub(40));
    let ph = 220.min(h.saturating_sub(py + 60));
    screen.fill_rect(px, py, pw, ph, Color::PANEL);
    screen.rect_outline(px, py, pw, ph, Color::ACCENT);

    let mut line = py + 12;
    put_str(screen, px + 12, line, "INPUT STATUS", Color::ACCENT, Color::PANEL);
    line += 20;

    // --- Mouse block ---
    put_str(screen, px + 12, line, "Mouse", Color::TEXT, Color::PANEL);
    line += GLYPH_H_ + 4;
    let mut buf = FmtBuf::new();
    buf.push_str("Driver: ");
    buf.push_str(if has_mouse { "OK (Simple Pointer)" } else { "Not present" });
    put_str(screen, px + 12, line, buf.as_str(), Color::MUTED, Color::PANEL);
    line += GLYPH_H_ + 4;

    buf.clear();
    buf.push_str("Pos:    ");
    buf.push_i32(cursor.x);
    buf.push_str(", ");
    buf.push_i32(cursor.y);
    put_str(screen, px + 12, line, buf.as_str(), Color::TEXT, Color::PANEL);
    line += GLYPH_H_ + 4;

    buf.clear();
    buf.push_str("Events: ");
    buf.push_u32(mouse_ev);
    put_str(screen, px + 12, line, buf.as_str(), Color::TEXT, Color::PANEL);
    line += GLYPH_H_ + 8;

    // --- Keyboard block ---
    put_str(screen, px + 12, line, "Keyboard", Color::TEXT, Color::PANEL);
    line += GLYPH_H_ + 4;
    buf.clear();
    buf.push_str("Driver: ");
    buf.push_str(if has_kbd { "OK (Simple Text Input)" } else { "Not present" });
    put_str(screen, px + 12, line, buf.as_str(), Color::MUTED, Color::PANEL);
    line += GLYPH_H_ + 4;
    buf.clear();
    buf.push_str("Events: ");
    buf.push_u32(key_ev);
    put_str(screen, px + 12, line, buf.as_str(), Color::TEXT, Color::PANEL);
    line += GLYPH_H_ + 8;

    // --- FPS ---
    put_str(screen, px + 12, line, "System", Color::TEXT, Color::PANEL);
    line += GLYPH_H_ + 4;
    buf.clear();
    buf.push_str("FPS:    ");
    buf.push_u32(fps);
    put_str(screen, px + 12, line, buf.as_str(), Color::TEXT, Color::PANEL);

    // Right side: recent keys panel.
    let rx = px + pw + 16;
    let rw = (w - rx - 16).max(200);
    let rh = ph;
    if rw >= 200 {
        screen.fill_rect(rx, py, rw, rh, Color::PANEL);
        screen.rect_outline(rx, py, rw, rh, Color::ACCENT);
        let mut l2 = py + 12;
        put_str(screen, rx + 12, l2, "RECENT KEYS", Color::ACCENT, Color::PANEL);
        l2 += 20;
        if last_keys.is_empty() {
            put_str(screen, rx + 12, l2, "(press a key)", Color::MUTED, Color::PANEL);
        } else {
            for (i, k) in last_keys.iter().enumerate() {
                let mut b2 = FmtBuf::new();
                b2.push_u32((i + 1) as u32);
                b2.push_str(". ");
                b2.push_str(k);
                put_str(screen, rx + 12, l2, b2.as_str(),
                        if i == last_keys.len() - 1 { Color::GOOD } else { Color::TEXT },
                        Color::PANEL);
                l2 += GLYPH_H_ + 4;
            }
        }
    }

    // Bottom hint area between panels and footer.
    let mid_y = py + ph + 16;
    if mid_y + 80 < h.saturating_sub(36) {
        put_str_transparent(screen, 16, mid_y,
            "This is a UEFI application running in graphics mode.", Color::MUTED);
        put_str_transparent(screen, 16, mid_y + 20,
            "Framebuffer is mapped directly; no GPU driver required.", Color::MUTED);
        put_str_transparent(screen, 16, mid_y + 40,
            "The same .efi boots on real Raspberry Pi 4/5 (with UEFI firmware).",
            Color::MUTED);
    }
}

/// Glyph height convenience constant.
const GLYPH_H_: usize = 16;
