#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

use uefi::prelude::*;
use uefi::runtime;

use baram_bsd::config;
use baram_bsd::shift_key;
use baram_core::{Color, LayerSystem, Screen};
use baram_font::{log_line_str, LayerFontExt};
use baram_iokit::keyboard::Keyboard;
use baram_iokit::mouse::Mouse;
use baram_windowserver::compositor::*;
use baram_windowserver::cursor;
use baram_windowserver::window::{WinId, WindowManager};

#[entry]
fn main() -> Status {
    log_line_str("BaramOS: starting...");
    let _ = uefi::helpers::init();
    log_line_str("BaramOS: UEFI helpers initialized");

    let _ = uefi::boot::set_watchdog_timer(0, 0, None);
    log_line_str("BaramOS: watchdog timer disabled");

    config::init_config();
    log_line_str("BaramOS: config loaded");

    baram_font::ttf_font::init();
    baram_font::ttf_font_hud::init();
    log_line_str("BaramOS: fonts initialized");

    unsafe {
        baram_windowserver::cursor::CURSOR_NORMAL = Some(cursor::prerender_cursor(
            cursor::CURSOR_SVG,
            cursor::CURSOR_BOX_W,
            cursor::CURSOR_BOX_H,
            8,
        ));
        baram_windowserver::cursor::CURSOR_RESIZE = Some(cursor::prerender_cursor(
            cursor::CURSOR_SVG_SIZE,
            cursor::CURSOR_BOX_SIZE_W,
            cursor::CURSOR_BOX_SIZE_H,
            8,
        ));
    }

    let mut screen = match Screen::take() {
        Ok(s) => {
            log_line_str(&alloc::format!(
                "BaramOS: screen {}x{}",
                s.width(),
                s.height()
            ));
            unsafe { baram_font::log::init_screen(&s) };
            s
        }
        Err(_s) => {
            log_line_str("BaramOS: screen init failed");
            return Status::UNSUPPORTED;
        }
    };

    unsafe { baram_kern::panic::init_from_screen(&screen) };

    log_line_str("BaramOS: opening mouse...");
    let mut mouse_opt: Option<Mouse> = match Mouse::open() {
        Ok(m) => Some(m),
        Err(_) => None,
    };
    let mouse_wait = Mouse::get_wait_event();
    log_line_str("BaramOS: opening keyboard...");
    let mut keyboard = Keyboard::open();

    let timer_event = unsafe {
        match uefi::boot::create_event(
            uefi_raw::table::boot::EventType::TIMER,
            uefi_raw::table::boot::Tpl::APPLICATION,
            None,
            None,
        ) {
            Ok(evt) => {
                let _ = uefi::boot::set_timer(
                    &evt,
                    uefi::boot::TimerTrigger::Periodic(core::time::Duration::from_millis(1)),
                );
                log_line_str("BaramOS: timer event created (1ms periodic)");
                Some(evt)
            }
            Err(_) => {
                log_line_str("BaramOS: failed to create timer event");
                None
            }
        }
    };

    let mut cursor_x: i32 = (screen.width() / 2) as i32;
    let mut cursor_y: i32 = (screen.height() / 2) as i32;

    if !baram_bsd::setup::is_setup_done() {
        log_line_str("BaramOS: first boot detected, starting setup wizard");
        {
            const LOGO_PNG: &[u8] = include_bytes!("../../../data/logo.png");
            if let Ok((header, pixels)) = png_decoder::decode(LOGO_PNG) {
                let img_w = header.width as usize;
                let img_h = header.height as usize;
                let sw = screen.width();
                let sh = screen.height();
                let mut logo_layer = LayerSystem::new(sw, sh);
                logo_layer.clear(Color::BLACK);
                let ox = (sw.saturating_sub(img_w)) / 2;
                let oy = (sh.saturating_sub(img_h)) / 2;
                let buf = logo_layer.buf_mut();
                for y in 0..img_h {
                    let dst_row = (oy + y) * sw + ox;
                    let src_row = y * img_w;
                    for x in 0..img_w {
                        let px = pixels[src_row + x];
                        buf[dst_row + x] = Color::rgb(px[0], px[1], px[2]).0;
                    }
                }
                logo_layer.flush(&mut screen);
            }
        }
        let mut wizard = baram_bsd::setup::SetupWizard::new();
        let mut setup_layer = LayerSystem::new(screen.width(), screen.height());
        let mut setup_buf: alloc::vec::Vec<u32> =
            alloc::vec![0u32; screen.width() * screen.height()];

        loop {
            if let Some(ref timer) = timer_event {
                let mut events = [unsafe { core::ptr::read(timer) }];
                let _ = uefi::boot::wait_for_event(&mut events);
            }

            while let Some(ev) = keyboard.poll() {
                wizard.on_key(&ev);
            }

            if let Some(mouse) = mouse_opt.as_mut() {
                while let Some(ev) = mouse.poll() {
                    baram_iokit::mouse::apply_mouse_event(
                        &mut cursor_x,
                        &mut cursor_y,
                        &ev,
                        screen.width(),
                        screen.height(),
                        mouse.abs_max(),
                    );
                    wizard.on_hover(cursor_x, cursor_y);
                    if ev.left {
                        wizard.on_click(cursor_x, cursor_y);
                    }
                }
            }

            if wizard.screen == baram_bsd::setup::SetupScreen::Done {
                break;
            }

            wizard.render(&mut setup_buf, screen.width(), screen.height());
            setup_layer.buf_mut()[..screen.width() * screen.height()].copy_from_slice(&setup_buf);

            cursor_x = cursor_x.max(0).min(screen.width() as i32 - 1);
            cursor_y = cursor_y.max(0).min(screen.height() as i32 - 1);
            cursor::draw_cursor_into_layer(&mut setup_layer, cursor_x, cursor_y, false, 1.0);
            setup_layer.flush(&mut screen);
        }
        log_line_str("BaramOS: setup wizard completed");
        keyboard.shift_key = shift_key::load_shift_key();
    }

    let mut wm = WindowManager::new(screen.width(), screen.height());
    let mut layer = LayerSystem::new(screen.width(), screen.height());

    log_line_str("BaramOS: loading index.yaml...");
    let index_yaml = baram_bsd::app::read_index_yaml();
    log_line_str(&alloc::format!(
        "BaramOS: index.yaml {} bytes",
        index_yaml.len()
    ));
    {
        const LOGO_PNG: &[u8] = include_bytes!("../../../data/logo.png");
        if let Ok((header, pixels)) = png_decoder::decode(LOGO_PNG) {
            let img_w = header.width as usize;
            let img_h = header.height as usize;
            let sw = screen.width();
            let sh = screen.height();
            let mut logo_layer = LayerSystem::new(sw, sh);
            logo_layer.clear(Color::BLACK);
            let ox = (sw.saturating_sub(img_w)) / 2;
            let oy = (sh.saturating_sub(img_h)) / 2;
            let buf = logo_layer.buf_mut();
            for y in 0..img_h {
                let dst_row = (oy + y) * sw + ox;
                let src_row = y * img_w;
                for x in 0..img_w {
                    let px = pixels[src_row + x];
                    buf[dst_row + x] = Color::rgb(px[0], px[1], px[2]).0;
                }
            }
            logo_layer.flush(&mut screen);
        }
    }
    let (autostart_list, app_entries) = parse_index_yaml(&index_yaml);
    let mut warp_engines: alloc::vec::Vec<(WinId, baram_windowserver::warp::WarpEngine)> =
        alloc::vec::Vec::new();
    let mut html_engines: alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)> =
        alloc::vec::Vec::new();
    let mut ui_win_id: Option<WinId> = None;
    let mut ui_commands: alloc::vec::Vec<baram_graphics::uiscript::Command> =
        alloc::vec::Vec::new();

    let mut auto_idx = 0i32;
    for autostart_name in &autostart_list {
        if let Some(entry) = app_entries.iter().find(|e| &e.name == autostart_name) {
            let x = 60 + (auto_idx * 120) % 500;
            let y = 60 + (auto_idx * 80) % 400;
            let w = 400;
            let h = 450;
            if open_app(
                &entry.name,
                &app_entries,
                &mut wm,
                &mut warp_engines,
                &mut html_engines,
                &mut ui_commands,
                &mut ui_win_id,
                x,
                y,
                w,
                h,
            )
            .is_some()
            {
                auto_idx += 1;
            }
        }
    }

    let mut last_keys: Vec<&'static str> = Vec::with_capacity(8);
    let mut mouse_ev_count: u32 = 0;
    let mut key_ev_count: u32 = 0;
    let mut frames: u32 = 0;
    let mut fps: u32 = 0;
    let mut frames_since_tick: u32 = 0;
    let mut start_time = runtime::get_time().unwrap_or_else(|_| runtime::Time::invalid());
    let mut mouse_down = false;
    let mut new_window_idx: u32 = 0;
    let mut keyboard_click: bool = false;
    let mut wasd_first_press: [u64; 4] = [0; 4];
    let mut wasd_moved: [bool; 4] = [false; 4];

    let mut mousekey_mode: bool = false;
    let mut shift_press_times: [u64; 3] = [0; 3];
    let mut shift_press_idx: usize = 0;
    let mut prev_shift_held: bool = false;
    let mut mousekey_win_id: Option<WinId> = None;

    let mouse_mode_label = match &mouse_opt {
        Some(m) if m.is_absolute() => "Absolute",
        Some(_) => "Simple Ptr",
        None => "None",
    };

    let mut display_state = baram_bsd::uri::DisplayState::new();
    baram_bsd::uri::load_settings_from_config(&mut display_state);

    let mut cached_wallpaper: Option<Vec<u32>> = None;
    if let Some(bytes) = WALLPAPERS.get(display_state.wallpaper_index) {
        cached_wallpaper = decode_wallpaper(bytes, screen.width(), screen.height());
    }

    let mut scene_dirty = true;
    let mut cached_scene: Vec<u32> = alloc::vec![0u32; screen.width() * screen.height()];
    let mut prev_cursor_x = cursor_x;
    let mut prev_cursor_y = cursor_y;
    let mut prev_is_resizing = false;
    let shadow_pad = 35i32;

    let mut cached_taskbar: Option<Vec<u32>> = None;
    let mut cached_taskbar_strip: Option<Vec<u32>> = None;
    let mut cached_launcher_layer: Option<Vec<u32>> = None;
    let mut scene_before_strip: Option<Vec<u32>> = None;
    let mut prev_window_count: usize = 0;
    let mut prev_focused_id: Option<WinId> = None;
    let mut bg_cache: Option<Vec<u32>> = None;
    let mut prev_wallpaper_idx: usize = display_state.wallpaper_index;

    let mut tb_add_progress: f32 = -1.0f32;
    let mut tb_remove_progress: f32 = -1.0f32;
    let mut tb_shift_x: f32 = 0.0f32;
    let mut show_app_launcher: bool = false;
    let mut app_list: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    let mut app_name_list: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    let mut app_icon_list: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    for entry in &app_entries {
        app_list.push(entry.title.clone());
        app_name_list.push(entry.name.clone());
        app_icon_list.push(entry.icon.clone());
    }
    let mut hover_apps_icon: bool = false;
    let mut prev_hover_apps_icon: bool = false;
    let mut prev_show_app_launcher: bool = false;

    let timezone_offset: i32 = config::get_config()
        .get_i32("system/timezone")
        .unwrap_or(9);

    let mut battery_info = baram_iokit::battery::read_battery_or_default();

    let (mut clock_hh, mut clock_mm) = {
        let tz = timezone_offset;
        match runtime::get_time() {
            Ok(t) => {
                let total_min = (t.hour() as i32) * 60 + (t.minute() as i32) + tz * 60;
                let day_min = total_min.rem_euclid(24 * 60);
                ((day_min / 60) as u8, (day_min % 60) as u8)
            }
            Err(_) => (0u8, 0u8),
        }
    };

    render_scene(
        &mut layer,
        &mut wm,
        mouse_ev_count,
        key_ev_count,
        fps,
        mouse_mode_label,
        &ui_commands,
        ui_win_id,
        &mut warp_engines,
        &mut html_engines,
        cached_wallpaper.as_deref(),
        &mut cached_taskbar,
        &mut cached_taskbar_strip,
        &mut cached_launcher_layer,
        true,
        -1.0,
        -1.0,
        0.0,
        display_state.hud_enabled,
        &mut bg_cache,
        false,
        show_app_launcher,
        &app_list,
        &app_icon_list,
        hover_apps_icon,
        false,
        &mut scene_before_strip,
        clock_hh,
        clock_mm,
        battery_info.percentage,
    );
    prev_window_count = wm.count();
    prev_focused_id = wm.focused_id;
    cached_scene.copy_from_slice(layer.buf_ref());
    cursor::draw_cursor_into_layer(
        &mut layer,
        cursor_x,
        cursor_y,
        false,
        display_state.pointer_size,
    );
    layer.flush(&mut screen);

    loop {
        let mut dirty = false;

        if let Some(ref timer) = timer_event {
            let mut events: [uefi::Event; 2] = [unsafe { core::ptr::read(timer) }, unsafe { core::ptr::read(timer) }];
            let mut n = 1;
            if let Some(mevt) = &mouse_wait {
                events[1] = unsafe { core::ptr::read(mevt) };
                n = 2;
            }
            let _ = uefi::boot::wait_for_event(&mut events[..n]);
        }

        match baram_bsd::uri::check_system_commands(&mut display_state) {
            baram_bsd::uri::SystemCommand::ResetAll => {
                uefi::runtime::reset(
                    uefi_raw::table::runtime::ResetType::COLD,
                    uefi::Status::SUCCESS,
                    None,
                );
            }
            baram_bsd::uri::SystemCommand::None => {}
        }

        while let Some(ev) = keyboard.poll() {
            key_ev_count = key_ev_count.wrapping_add(1);
            if last_keys.len() >= 6 {
                last_keys.remove(0);
            }
            last_keys.push(ev.label());

            match ev.scancode {
                0x01 => wm.scroll_focused(-baram_windowserver::window::scroll_speed()),
                0x02 => wm.scroll_focused(baram_windowserver::window::scroll_speed()),
                _ => {}
            }

            if ev.ctrl_or_cmd() || (mousekey_mode && keyboard.shift_held()) {
                if let Some(c) = ev.printable {
                    match c {
                        b' ' => {
                            keyboard_click = true;
                            dirty = true;
                            scene_dirty = true;
                        }
                        _ => {}
                    }
                }
            } else if let Some(c) = ev.printable {
                let mut handled = false;
                if let Some(focused_win) = wm.focused_id {
                    for (wid, engine) in warp_engines.iter_mut() {
                        if *wid == focused_win && !engine.focused_input_var.is_empty() {
                            engine.handle_key(c);
                            if let Some((_, _, ww, wh, _)) = wm.get_window_rect(*wid) {
                                let tb_h = baram_windowserver::window::title_bar_h() as i32;
                                let content_h = (wh as i32).saturating_sub(tb_h);
                                engine.update(ww as i32, content_h);
                                wm.clamp_window_scroll(*wid, engine.content_height);
                                wm.set_content_dirty(*wid);
                            }
                            handled = true;
                            dirty = true;
                            scene_dirty = true;
                            break;
                        }
                    }
                }
                if !handled {
                    match c {
                        _ => {}
                    }
                }
            }
            dirty = true;
            scene_dirty = true;
        }

        {
            let shift_held = keyboard.shift_held();
            let shift_just_pressed = shift_held && !prev_shift_held;
            prev_shift_held = shift_held;

            if shift_just_pressed {
                let now_ns = runtime::get_time()
                    .map(|t| {
                        t.nanosecond() as u64
                            + t.second() as u64 * 1_000_000_000
                            + t.minute() as u64 * 60_000_000_000
                            + t.hour() as u64 * 3_600_000_000_000
                    })
                    .unwrap_or(0);
                let threshold_ns = 1_000_000_000;

                shift_press_times[shift_press_idx % 3] = now_ns;
                shift_press_idx += 1;

                if shift_press_idx >= 3 {
                    let oldest = shift_press_times[(shift_press_idx - 3) % 3];
                    if now_ns.saturating_sub(oldest) <= threshold_ns {
                        mousekey_mode = !mousekey_mode;
                        shift_press_idx = 0;

                        if mousekey_mode {
                            let source = baram_bsd::app::load_app_source("mousekeydialog.warp");
                            let nx = (screen.width() as i32 - 400) / 2;
                            let ny = (screen.height() as i32 - 300) / 2;
                            let win_id = wm.add("マウスキー", nx, ny, 400, 300);
                            let mut engine = baram_windowserver::warp::WarpEngine::new(&source);
                            engine.update(380, 260);
                            warp_engines.push((win_id, engine));
                            mousekey_win_id = Some(win_id);
                            tb_add_progress = 0.0;
                            tb_shift_x = 26.0;
                            dirty = true;
                            scene_dirty = true;
                        } else {
                            if let Some(wid) = mousekey_win_id.take() {
                                wm.remove(wid);
                                warp_engines.retain(|(id, _)| *id != wid);
                                dirty = true;
                                scene_dirty = true;
                            }
                        }
                    }
                }
            }
        }

        if keyboard.ctrl_or_cmd_held() || mousekey_mode {
            let step = 8i32;
            let now_ns = runtime::get_time()
                .map(|t| {
                    t.nanosecond() as u64
                        + t.second() as u64 * 1_000_000_000
                        + t.minute() as u64 * 60_000_000_000
                        + t.hour() as u64 * 3_600_000_000_000
                })
                .unwrap_or(0);
            let delay_ns = 300_000_000;

            let keys = [
                (0x1A, 0usize),
                (0x04, 1usize),
                (0x16, 2usize),
                (0x07, 3usize),
            ];

            for (usb_code, idx) in keys {
                if keyboard.is_held(usb_code) {
                    if wasd_first_press[idx] == 0 {
                        wasd_first_press[idx] = now_ns;
                        wasd_moved[idx] = true;
                        match idx {
                            0 => {
                                cursor_y = (cursor_y - step).max(0);
                            }
                            1 => {
                                cursor_x = (cursor_x - step).max(0);
                            }
                            2 => {
                                cursor_y = (cursor_y + step).min(screen.height() as i32 - 1);
                            }
                            3 => {
                                cursor_x = (cursor_x + step).min(screen.width() as i32 - 1);
                            }
                            _ => {}
                        }
                        dirty = true;
                        scene_dirty = true;
                    } else {
                        let elapsed = now_ns.saturating_sub(wasd_first_press[idx]);
                        if elapsed >= delay_ns {
                            match idx {
                                0 => {
                                    cursor_y = (cursor_y - step).max(0);
                                }
                                1 => {
                                    cursor_x = (cursor_x - step).max(0);
                                }
                                2 => {
                                    cursor_y = (cursor_y + step).min(screen.height() as i32 - 1);
                                }
                                3 => {
                                    cursor_x = (cursor_x + step).min(screen.width() as i32 - 1);
                                }
                                _ => {}
                            }
                            dirty = true;
                            scene_dirty = true;
                        }
                    }
                } else {
                    wasd_first_press[idx] = 0;
                    wasd_moved[idx] = false;
                }
            }
        }

        if let Some(mouse) = mouse_opt.as_mut() {
            while let Some(ev) = mouse.poll() {
                mouse_ev_count = mouse_ev_count.wrapping_add(1);

                let (cx, cy) = baram_iokit::mouse::apply_mouse_event(
                    &mut cursor_x,
                    &mut cursor_y,
                    &ev,
                    screen.width(),
                    screen.height(),
                    mouse.abs_max(),
                );

                if ev.scroll != 0 {
                    let scroll_delta = ev
                        .scroll
                        .saturating_neg()
                        .saturating_mul(baram_windowserver::window::scroll_speed());
                    if let Some(id) = wm.window_at(cx, cy) {
                        wm.scroll_window(id, scroll_delta);
                        dirty = true;
                        scene_dirty = true;
                    }
                }

                if ev.left && !mouse_down {
                    mouse_down = true;
                    let sh = screen.height();

                    if show_app_launcher {
                        let cols = 5usize;
                        let icon_size = 64usize;
                        let icon_gap = 24usize;
                        let label_h = 20usize;
                        let cell_w = icon_size + icon_gap;
                        let cell_h = icon_size + label_h + icon_gap;
                        let grid_w = cols * cell_w;
                        let rows = (app_list.len() + cols - 1) / cols;
                        let grid_h = rows * cell_h;
                        let grid_x = (screen.width().saturating_sub(grid_w)) / 2;
                        let grid_y = ((screen.height() - TASKBAR_H).saturating_sub(grid_h)) / 2;
                        let mut clicked_app = None;
                        for (i, _) in app_list.iter().enumerate() {
                            let col = i % cols;
                            let row = i / cols;
                            let ix = grid_x + col * cell_w + icon_gap / 2;
                            let iy = grid_y + row * cell_h;
                            if cx >= ix as i32
                                && cx < (ix + icon_size) as i32
                                && cy >= iy as i32
                                && cy < (iy + icon_size) as i32
                            {
                                clicked_app = Some(i);
                                break;
                            }
                        }
                        if let Some(idx) = clicked_app {
                            let app_name = app_name_list[idx].clone();
                            let nx = 100 + ((new_window_idx as i32 * 37) % 300);
                            let ny = 60 + ((new_window_idx as i32 * 23) % 200);
                            open_app(
                                &app_name,
                                &app_entries,
                                &mut wm,
                                &mut warp_engines,
                                &mut html_engines,
                                &mut ui_commands,
                                &mut ui_win_id,
                                nx,
                                ny,
                                400,
                                450,
                            );
                            tb_add_progress = 0.0;
                            tb_shift_x = 26.0;
                            new_window_idx = new_window_idx.wrapping_add(1);
                        }
                        show_app_launcher = false;
                        scene_dirty = true;
                    } else if cy >= sh as i32 - TASKBAR_H as i32 {
                        let apps_icon_x = 16i32;
                        let apps_icon_size = 24i32;
                        let apps_icon_y = (sh as i32 - TASKBAR_H as i32
                            + (TASKBAR_H as i32 - apps_icon_size) / 2)
                            as i32;
                        let on_apps_icon = cx >= apps_icon_x
                            && cx < apps_icon_x + apps_icon_size
                            && cy >= apps_icon_y
                            && cy < apps_icon_y + apps_icon_size;
                        if on_apps_icon {
                            show_app_launcher = !show_app_launcher;
                            scene_dirty = true;
                        } else {
                            if show_app_launcher {
                                show_app_launcher = false;
                                scene_dirty = true;
                            }
                            let ids = wm.insertion_ids();
                            let count = ids.len();
                            let btn_d = 40i32;
                            let btn_gap = 12i32;
                            let total_w = count as i32 * (btn_d + btn_gap) - btn_gap;
                            let mut bx = ((screen.width() as i32 - total_w) / 2).max(0);
                            let btn_y =
                                (sh as usize).saturating_sub(TASKBAR_H) + (TASKBAR_H - 40) / 2;
                            for id in &ids {
                                let dx = cx - bx - btn_d / 2;
                                let dy = cy - btn_y as i32 - btn_d / 2;
                                if dx * dx + dy * dy <= (btn_d / 2) * (btn_d / 2) {
                                    if wm.is_minimized(*id) {
                                        wm.restore_minimized(*id);
                                    }
                                    wm.focus(*id);
                                    break;
                                }
                                bx += btn_d + btn_gap;
                            }
                        }
                    } else {
                        let win_under = wm.window_at(cx, cy);
                        if let Some(id) = win_under {
                            wm.focus(id);
                            let btn = wm.button_hit_at(id, cx, cy);
                            match btn {
                                'c' => {
                                    wm.remove(id);
                                    warp_engines.retain(|(wid, _)| *wid != id);
                                    html_engines.retain(|(wid, _)| *wid != id);
                                }
                                'm' => {
                                    wm.toggle_maximize_at(id);
                                }
                                'i' => {
                                    wm.toggle_minimize_at(id);
                                }
                                _ => {
                                    if wm.resize_hit_at(id, cx, cy) {
                                        wm.start_resize_at(id, cx, cy);
                                    } else {
                                        wm.start_drag_at(id, cx, cy);
                                    }
                                }
                            }
                            let after = wm.insertion_ids();
                            let _ = after;
                        }
                        if let Some(clicked_id) = wm.window_at(cx, cy) {
                            for (wid, engine) in warp_engines.iter_mut() {
                                if clicked_id == *wid {
                                    if let Some((wx, wy, ww, wh, scroll)) =
                                        wm.get_window_rect(clicked_id)
                                    {
                                        let rel_x = cx - wx;
                                        let rel_y = cy - wy;
                                        let tb_h = baram_windowserver::window::title_bar_h() as i32;
                                        if rel_y >= tb_h {
                                            let warp_y = rel_y + scroll;
                                            engine.click(rel_x, warp_y);
                                            let content_h = wh.saturating_sub(tb_h as usize);
                                            engine.update(ww as i32, content_h as i32);
                                            wm.set_content_dirty(clicked_id);
                                            scene_dirty = true;

                                            if let Some(cmd) = engine.last_command.take() {
                                                if baram_bsd::uri::execute(&cmd, &mut display_state) {
                                                    engine.update(ww as i32, content_h as i32);
                                                    wm.set_all_dirty();
                                                    cached_taskbar = None;
                                                    cached_taskbar_strip = None;
                                                    cached_launcher_layer = None;
                                                    bg_cache = None;
                                                    scene_dirty = true;
                                                }
                                                if let Some(parsed) = baram_bsd::uri::parse(&cmd) {
                                                    if parsed.path.starts_with("display/wallpaper") {
                                                        if display_state.wallpaper_mode == baram_bsd::uri::WallpaperMode::Color {
                                                            if let Some(color) =
                                                                display_state.wallpaper_color
                                                            {
                                                                cached_wallpaper =
                                                                    Some(make_solid_wallpaper(
                                                                        color,
                                                                        screen.width(),
                                                                        screen.height(),
                                                                    ));
                                                            }
                                                        } else {
                                                            if let Some(bytes) = WALLPAPERS
                                                                .get(display_state.wallpaper_index)
                                                            {
                                                                cached_wallpaper = decode_wallpaper(
                                                                    bytes,
                                                                    screen.width(),
                                                                    screen.height(),
                                                                );
                                                            } else {
                                                                log_line_str("NO WALLPAPER BYTES");
                                                            }
                                                        }
                                                        prev_wallpaper_idx =
                                                            display_state.wallpaper_index;
                                                        scene_dirty = true;
                                                    } else if parsed.path.starts_with("display/pointer")
                                                        || parsed.path.starts_with("display/hud")
                                                    {
                                                        scene_dirty = true;
                                                    } else {
                                                        scene_dirty = true;
                                                    }
                                                }
                                            }

                                            if let Some(enabled_str) =
                                                engine.get_state_value("--hudEnabled")
                                            {
                                                let new_enabled = enabled_str == "true";
                                                if display_state.hud_enabled != new_enabled {
                                                    display_state.hud_enabled = new_enabled;
                                                    scene_dirty = true;
                                                }
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                            let mut html_command = None;
                            for (wid, engine) in html_engines.iter_mut() {
                                if clicked_id != *wid {
                                    continue;
                                }
                                if let Some((wx, wy, ww, wh, scroll)) =
                                    wm.get_window_rect(clicked_id)
                                {
                                    let rel_x = cx - wx;
                                    let rel_y = cy - wy;
                                    let tb_h =
                                        baram_windowserver::window::title_bar_h() as i32;
                                    if rel_y >= tb_h {
                                        engine.click(rel_x, rel_y + scroll);
                                        engine.update(
                                            ww as i32,
                                            wh.saturating_sub(tb_h as usize) as i32,
                                        );
                                        html_command = engine.last_command.take();
                                        wm.set_content_dirty(clicked_id);
                                        scene_dirty = true;
                                    }
                                }
                                break;
                            }
                            if let Some(cmd) = html_command {
                                let nx = 100 + ((new_window_idx as i32 * 37) % 300);
                                let ny = 60 + ((new_window_idx as i32 * 23) % 200);
                                match handle_navigation(
                                    &cmd,
                                    &app_entries,
                                    &mut wm,
                                    &mut warp_engines,
                                    &mut html_engines,
                                    &mut ui_commands,
                                    &mut ui_win_id,
                                    &mut display_state,
                                    nx,
                                    ny,
                                ) {
                                    NavigationEffect::AppOpened => {
                                        new_window_idx = new_window_idx.wrapping_add(1);
                                        tb_add_progress = 0.0;
                                        tb_shift_x = 26.0;
                                    }
                                    NavigationEffect::SystemChanged => {
                                        cached_taskbar = None;
                                        cached_taskbar_strip = None;
                                        cached_launcher_layer = None;
                                        bg_cache = None;
                                        cached_wallpaper =
                                            wallpaper_for_state(
                                                &display_state,
                                                screen.width(),
                                                screen.height(),
                                            );
                                        prev_wallpaper_idx =
                                            display_state.wallpaper_index;
                                    }
                                    NavigationEffect::None => {}
                                }
                                scene_dirty = true;
                            }
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

        if keyboard_click {
            keyboard_click = false;
            let cx = cursor_x;
            let cy = cursor_y;
            let sh = screen.height();

            if show_app_launcher {
                let cols = 5usize;
                let icon_size = 64usize;
                let icon_gap = 24usize;
                let label_h = 20usize;
                let cell_w = icon_size + icon_gap;
                let cell_h = icon_size + label_h + icon_gap;
                let grid_w = cols * cell_w;
                let rows = (app_list.len() + cols - 1) / cols;
                let grid_h = rows * cell_h;
                let grid_x = (screen.width().saturating_sub(grid_w)) / 2;
                let grid_y = ((screen.height() - TASKBAR_H).saturating_sub(grid_h)) / 2;
                let mut clicked_app = None;
                for (i, _) in app_list.iter().enumerate() {
                    let col = i % cols;
                    let row = i / cols;
                    let ix = grid_x + col * cell_w + icon_gap / 2;
                    let iy = grid_y + row * cell_h;
                    if cx >= ix as i32
                        && cx < (ix + icon_size) as i32
                        && cy >= iy as i32
                        && cy < (iy + icon_size) as i32
                    {
                        clicked_app = Some(i);
                        break;
                    }
                }
                if let Some(idx) = clicked_app {
                    let app_name = app_name_list[idx].clone();
                    let nx = 100 + ((new_window_idx as i32 * 37) % 300);
                    let ny = 60 + ((new_window_idx as i32 * 23) % 200);
                    open_app(
                        &app_name,
                        &app_entries,
                        &mut wm,
                        &mut warp_engines,
                        &mut html_engines,
                        &mut ui_commands,
                        &mut ui_win_id,
                        nx,
                        ny,
                        400,
                        450,
                    );
                    tb_add_progress = 0.0;
                    tb_shift_x = 26.0;
                    new_window_idx = new_window_idx.wrapping_add(1);
                }
                show_app_launcher = false;
                scene_dirty = true;
            } else if cy >= sh as i32 - TASKBAR_H as i32 {
                let apps_icon_x = 16i32;
                let apps_icon_size = 24i32;
                let apps_icon_y =
                    (sh as i32 - TASKBAR_H as i32 + (TASKBAR_H as i32 - apps_icon_size) / 2) as i32;
                let on_apps_icon = cx >= apps_icon_x
                    && cx < apps_icon_x + apps_icon_size
                    && cy >= apps_icon_y
                    && cy < apps_icon_y + apps_icon_size;
                if on_apps_icon {
                    show_app_launcher = !show_app_launcher;
                    scene_dirty = true;
                } else {
                    if show_app_launcher {
                        show_app_launcher = false;
                        scene_dirty = true;
                    }
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
                            if wm.is_minimized(*id) {
                                wm.restore_minimized(*id);
                            }
                            wm.focus(*id);
                            break;
                        }
                        bx += btn_d + btn_gap;
                    }
                }
            } else {
                let win_under = wm.window_at(cx, cy);
                if let Some(id) = win_under {
                    wm.focus(id);
                    let btn = wm.button_hit_at(id, cx, cy);
                    match btn {
                        'c' => {
                            wm.remove(id);
                            warp_engines.retain(|(wid, _)| *wid != id);
                            html_engines.retain(|(wid, _)| *wid != id);
                        }
                        'm' => {
                            wm.toggle_maximize_at(id);
                        }
                        'i' => {
                            wm.toggle_minimize_at(id);
                        }
                        _ => {}
                    }
                }
                if let Some(clicked_id) = wm.window_at(cx, cy) {
                    for (wid, engine) in warp_engines.iter_mut() {
                        if clicked_id == *wid {
                            if let Some((wx, wy, ww, wh, scroll)) = wm.get_window_rect(clicked_id) {
                                let rel_x = cx - wx;
                                let rel_y = cy - wy;
                                let tb_h = baram_windowserver::window::title_bar_h() as i32;
                                if rel_y >= tb_h {
                                    let warp_y = rel_y + scroll;
                                    engine.click(rel_x, warp_y);
                                    let content_h = wh.saturating_sub(tb_h as usize);
                                    engine.update(ww as i32, content_h as i32);
                                    wm.set_content_dirty(clicked_id);
                                    scene_dirty = true;

                                    if let Some(cmd) = engine.last_command.take() {
                                        if baram_bsd::uri::execute(&cmd, &mut display_state) {
                                            engine.update(ww as i32, content_h as i32);
                                            wm.set_all_dirty();
                                            cached_taskbar = None;
                                            cached_taskbar_strip = None;
                                            cached_launcher_layer = None;
                                            bg_cache = None;
                                            scene_dirty = true;
                                        }
                                        if let Some(parsed) = baram_bsd::uri::parse(&cmd) {
                                            if parsed.path.starts_with("display/wallpaper") {
                                                if display_state.wallpaper_mode == baram_bsd::uri::WallpaperMode::Color {
                                                    if let Some(color) = display_state.wallpaper_color {
                                                        cached_wallpaper = Some(make_solid_wallpaper(
                                                            color,
                                                            screen.width(),
                                                            screen.height(),
                                                        ));
                                                    }
                                                } else {
                                                    if let Some(bytes) =
                                                        WALLPAPERS.get(display_state.wallpaper_index)
                                                    {
                                                        cached_wallpaper = decode_wallpaper(
                                                            bytes,
                                                            screen.width(),
                                                            screen.height(),
                                                        );
                                                    }
                                                }
                                                prev_wallpaper_idx = display_state.wallpaper_index;
                                                scene_dirty = true;
                                            } else if parsed.path.starts_with("display/pointer")
                                                || parsed.path.starts_with("display/hud")
                                            {
                                                scene_dirty = true;
                                            } else {
                                                scene_dirty = true;
                                            }
                                        }
                                    }

                                    if let Some(enabled_str) = engine.get_state_value("--hudEnabled") {
                                        let new_enabled = enabled_str == "true";
                                        if display_state.hud_enabled != new_enabled {
                                            display_state.hud_enabled = new_enabled;
                                            scene_dirty = true;
                                        }
                                    }
                                }
                            }
                            break;
                        }
                    }
                    let mut html_command = None;
                    for (wid, engine) in html_engines.iter_mut() {
                        if clicked_id != *wid {
                            continue;
                        }
                        if let Some((wx, wy, ww, wh, scroll)) = wm.get_window_rect(clicked_id) {
                            let rel_x = cx - wx;
                            let rel_y = cy - wy;
                            let tb_h = baram_windowserver::window::title_bar_h() as i32;
                            if rel_y >= tb_h {
                                engine.click(rel_x, rel_y + scroll);
                                engine.update(
                                    ww as i32,
                                    wh.saturating_sub(tb_h as usize) as i32,
                                );
                                html_command = engine.last_command.take();
                                wm.set_content_dirty(clicked_id);
                                scene_dirty = true;
                            }
                        }
                        break;
                    }
                    if let Some(cmd) = html_command {
                        let nx = 100 + ((new_window_idx as i32 * 37) % 300);
                        let ny = 60 + ((new_window_idx as i32 * 23) % 200);
                        match handle_navigation(
                            &cmd,
                            &app_entries,
                            &mut wm,
                            &mut warp_engines,
                            &mut html_engines,
                            &mut ui_commands,
                            &mut ui_win_id,
                            &mut display_state,
                            nx,
                            ny,
                        ) {
                            NavigationEffect::AppOpened => {
                                new_window_idx = new_window_idx.wrapping_add(1);
                                tb_add_progress = 0.0;
                                tb_shift_x = 26.0;
                            }
                            NavigationEffect::SystemChanged => {
                                cached_taskbar = None;
                                cached_taskbar_strip = None;
                                cached_launcher_layer = None;
                                bg_cache = None;
                                cached_wallpaper = wallpaper_for_state(
                                    &display_state,
                                    screen.width(),
                                    screen.height(),
                                );
                                prev_wallpaper_idx = display_state.wallpaper_index;
                            }
                            NavigationEffect::None => {}
                        }
                        scene_dirty = true;
                    }
                }
            }
            dirty = true;
        }

        {
            let sh = screen.height() as i32;
            let apps_icon_x = 16i32;
            let apps_icon_size = 24i32;
            let apps_icon_y = sh - TASKBAR_H as i32 + (TASKBAR_H as i32 - apps_icon_size) / 2;
            hover_apps_icon = cursor_x >= apps_icon_x
                && cursor_x < apps_icon_x + apps_icon_size
                && cursor_y >= apps_icon_y
                && cursor_y < apps_icon_y + apps_icon_size;
            if hover_apps_icon != prev_hover_apps_icon {
                dirty = true;
                scene_dirty = true;
                prev_hover_apps_icon = hover_apps_icon;
            }
        }

        {
            let mut hovered_any = false;
            if let Some(hover_id) = wm.window_at(cursor_x, cursor_y) {
                for (wid, engine) in warp_engines.iter_mut() {
                    if hover_id == *wid {
                        if let Some((wx, wy, _ww, _wh, scroll)) = wm.get_window_rect(hover_id) {
                            let rel_x = cursor_x - wx;
                            let rel_y = cursor_y - wy;
                            let tb_h = baram_windowserver::window::title_bar_h() as i32;
                            let prev_hover = engine.hover_idx;
                            if rel_y >= tb_h {
                                let warp_y = rel_y + scroll;
                                engine.set_hover(rel_x, warp_y);
                            } else {
                                engine.set_hover(rel_x, -1);
                            }
                            if engine.hover_idx != prev_hover {
                                wm.set_content_dirty(hover_id);
                                scene_dirty = true;
                                dirty = true;
                            }
                            hovered_any = true;
                        }
                        break;
                    }
                }
                for (wid, engine) in html_engines.iter_mut() {
                    if hover_id == *wid {
                        if let Some((wx, wy, _ww, _wh, scroll)) = wm.get_window_rect(hover_id) {
                            let rel_x = cursor_x - wx;
                            let rel_y = cursor_y - wy;
                            let tb_h = baram_windowserver::window::title_bar_h() as i32;
                            let previous = engine.hovered_node();
                            if rel_y >= tb_h {
                                engine.set_hover(rel_x, rel_y + scroll);
                            } else {
                                engine.clear_hover();
                            }
                            if engine.hovered_node() != previous {
                                wm.set_content_dirty(hover_id);
                                scene_dirty = true;
                                dirty = true;
                            }
                            hovered_any = true;
                        }
                        break;
                    }
                }
            }
            if !hovered_any {
                for (_, engine) in warp_engines.iter_mut() {
                    engine.clear_hover();
                }
                for (_, engine) in html_engines.iter_mut() {
                    engine.clear_hover();
                }
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

                let total_min = (now.hour() as i32) * 60
                    + (now.minute() as i32)
                    + timezone_offset * 60;
                let day_min = total_min.rem_euclid(24 * 60);
                clock_hh = (day_min / 60) as u8;
                clock_mm = (day_min % 60) as u8;

                battery_info = baram_iokit::battery::read_battery_or_default();
                cached_taskbar_strip = None;

                dirty = true;
                scene_dirty = true;
            }
        }

        if wm.take_order_changed() {
            scene_dirty = true;
            dirty = true;
        }

        for (wid, engine) in warp_engines.iter_mut() {
            if let Some((_, _, ww, wh, _)) = wm.get_window_rect(*wid) {
                let content_h = wh.saturating_sub(30);
                engine.update(ww as i32, content_h as i32);
                wm.clamp_window_scroll(*wid, engine.content_height);
            }
        }
        for (wid, engine) in html_engines.iter_mut() {
            if let Some((_, _, ww, wh, _)) = wm.get_window_rect(*wid) {
                let content_h =
                    wh.saturating_sub(baram_windowserver::window::title_bar_h());
                engine.update(ww as i32, content_h as i32);
                wm.clamp_window_scroll(*wid, engine.content_height);
            }
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
            if tb_shift_x.abs() < 0.5 {
                tb_shift_x = 0.0;
            }
        }

        if dirty {
            let is_resizing = wm.is_any_resizing() || wm.is_over_resize_handle(cursor_x, cursor_y);

            if scene_dirty {
                let (bx0, by0, bx1, by1) = wm.dirty_bbox(shadow_pad);

                let taskbar_dirty = cached_taskbar_strip.is_none()
                    || tb_add_progress >= 0.0
                    || tb_remove_progress >= 0.0
                    || tb_shift_x.abs() > 0.5
                    || wm.count() != prev_window_count
                    || wm.focused_id != prev_focused_id;

                let bg_valid = bg_cache.is_some()
                    && prev_wallpaper_idx == display_state.wallpaper_index
                    && tb_add_progress < 0.0
                    && tb_remove_progress < 0.0
                    && tb_shift_x.abs() <= 0.5;

                let launcher_changed = show_app_launcher != prev_show_app_launcher;

                let taskbar_only = taskbar_dirty
                    && scene_before_strip.is_some()
                    && wm.count() == prev_window_count
                    && wm.focused_id == prev_focused_id
                    && prev_wallpaper_idx == display_state.wallpaper_index
                    && bg_cache.is_some()
                    && !show_app_launcher
                    && !launcher_changed;

                if taskbar_only {
                    let w = screen.width();
                    let h = screen.height();
                    let pad = 32i32;
                    let prev_w = if prev_is_resizing {
                        cursor::CURSOR_BOX_SIZE_W
                    } else {
                        cursor::CURSOR_BOX_W
                    };
                    let prev_h = if prev_is_resizing {
                        cursor::CURSOR_BOX_SIZE_H
                    } else {
                        cursor::CURSOR_BOX_H
                    };
                    let x0 = (prev_cursor_x - pad).max(0) as usize;
                    let y0 = (prev_cursor_y - pad).max(0) as usize;
                    let x1 = (prev_cursor_x + prev_w as i32 + pad).min(w as i32) as usize;
                    let y1 = (prev_cursor_y + prev_h as i32 + pad).min(h as i32) as usize;
                    let buf = layer.buf_mut();
                    for y in y0..y1 {
                        let s = y * w + x0;
                        let e = y * w + x1;
                        buf[s..e].copy_from_slice(&cached_scene[s..e]);
                    }
                }

                let w = screen.width();
                let h = screen.height();
                let tb_y = h.saturating_sub(TASKBAR_H);
                let pad = 32i32;
                let cur_w = if is_resizing {
                    cursor::CURSOR_BOX_SIZE_W
                } else {
                    cursor::CURSOR_BOX_W
                };
                let cur_h = if is_resizing {
                    cursor::CURSOR_BOX_SIZE_H
                } else {
                    cursor::CURSOR_BOX_H
                };
                let prev_w = if prev_is_resizing {
                    cursor::CURSOR_BOX_SIZE_W
                } else {
                    cursor::CURSOR_BOX_W
                };
                let prev_h = if prev_is_resizing {
                    cursor::CURSOR_BOX_SIZE_H
                } else {
                    cursor::CURSOR_BOX_H
                };
                let cx0 = (prev_cursor_x.min(cursor_x) - pad).max(0) as usize;
                let cy0 = (prev_cursor_y.min(cursor_y) - pad).max(0) as usize;
                let cx1 = (prev_cursor_x.max(cursor_x) + cur_w.max(prev_w) as i32 + pad)
                    .min(w as i32) as usize;
                let cy1 = (prev_cursor_y.max(cursor_y) + cur_h.max(prev_h) as i32 + pad)
                    .min(h as i32) as usize;

                let (mut fx0, mut fy0, mut fx1, mut fy1) = if taskbar_only {
                    (0, tb_y, w, h)
                } else {
                    (bx0.min(cx0), by0.min(cy0), bx1.max(cx1), by1.max(cy1))
                };
                if taskbar_dirty && !taskbar_only {
                    fx0 = 0;
                    fy0 = fy0.min(tb_y);
                    fx1 = w;
                    fy1 = h;
                }
                if launcher_changed || !bg_valid {
                    fx0 = 0;
                    fy0 = 0;
                    fx1 = w;
                    fy1 = h;
                }
                layer.push_clip(fx0, fy0, fx1, fy1);

                render_scene(
                    &mut layer,
                    &mut wm,
                    mouse_ev_count,
                    key_ev_count,
                    fps,
                    mouse_mode_label,
                    &ui_commands,
                    ui_win_id,
                    &mut warp_engines,
                    &mut html_engines,
                    cached_wallpaper.as_deref(),
                    &mut cached_taskbar,
                    &mut cached_taskbar_strip,
                    &mut cached_launcher_layer,
                    taskbar_dirty,
                    tb_add_progress,
                    tb_remove_progress,
                    tb_shift_x,
                    display_state.hud_enabled,
                    &mut bg_cache,
                    bg_valid,
                    show_app_launcher,
                    &app_list,
                    &app_icon_list,
                    hover_apps_icon,
                    taskbar_only,
                    &mut scene_before_strip,
                    clock_hh,
                    clock_mm,
                    battery_info.percentage,
                );
                layer.pop_clip();

                if show_app_launcher {
                    if let Some(ref ll) = cached_launcher_layer {
                        let buf = layer.buf_mut();
                        let ww = screen.width();
                        let hh = screen.height();
                        let tby = hh.saturating_sub(TASKBAR_H);
                        for y in fy0..fy1.min(tby) {
                            let s = y * ww + fx0;
                            let e = y * ww + fx1;
                            buf[s..e].copy_from_slice(&ll[s..e]);
                        }
                    }
                }
                if launcher_changed {
                    layer.mark_all_dirty();
                }

                prev_window_count = wm.count();
                prev_focused_id = wm.focused_id;

                if tb_add_progress >= 1.0 {
                    tb_add_progress = -1.0;
                }
                if tb_remove_progress >= 1.0 {
                    tb_remove_progress = -1.0;
                }

                for y in fy0..fy1 {
                    let s = y * w + fx0;
                    let e = y * w + fx1;
                    cached_scene[s..e].copy_from_slice(&layer.buf_ref()[s..e]);
                }
                scene_dirty = false;
                wm.clear_pending_damage();

                cursor::draw_cursor_into_layer(
                    &mut layer,
                    cursor_x,
                    cursor_y,
                    is_resizing,
                    display_state.pointer_size,
                );
                prev_show_app_launcher = show_app_launcher;
                let fw = fx1 - fx0;
                let fh = fy1 - fy0;
                let full_area = w * h;
                if taskbar_only {
                    layer.flush_rect(&mut screen, 0, tb_y, w, h);
                    layer.flush_rect(&mut screen, cx0, cy0, cx1, cy1);
                } else if launcher_changed || !bg_valid || fw * fh >= full_area * 3 / 4 {
                    layer.flush(&mut screen);
                } else {
                    layer.flush_rect(&mut screen, fx0, fy0, fx1, fy1);
                }

                prev_cursor_x = cursor_x;
                prev_cursor_y = cursor_y;
                prev_is_resizing = is_resizing;
            } else {
                let w = screen.width();
                let h = screen.height();
                let pad = 32i32;
                let cur_w = if is_resizing {
                    cursor::CURSOR_BOX_SIZE_W
                } else {
                    cursor::CURSOR_BOX_W
                };
                let cur_h = if is_resizing {
                    cursor::CURSOR_BOX_SIZE_H
                } else {
                    cursor::CURSOR_BOX_H
                };
                let prev_w = if prev_is_resizing {
                    cursor::CURSOR_BOX_SIZE_W
                } else {
                    cursor::CURSOR_BOX_W
                };
                let prev_h = if prev_is_resizing {
                    cursor::CURSOR_BOX_SIZE_H
                } else {
                    cursor::CURSOR_BOX_H
                };
                let x0 = (prev_cursor_x.min(cursor_x) - pad).max(0) as usize;
                let y0 = (prev_cursor_y.min(cursor_y) - pad).max(0) as usize;
                let x1 = (prev_cursor_x.max(cursor_x) + cur_w.max(prev_w) as i32 + pad)
                    .min(w as i32) as usize;
                let y1 = (prev_cursor_y.max(cursor_y) + cur_h.max(prev_h) as i32 + pad)
                    .min(h as i32) as usize;

                {
                    let buf = layer.buf_mut();
                    for y in y0..y1 {
                        let s = y * w + x0;
                        let e = y * w + x1;
                        buf[s..e].copy_from_slice(&cached_scene[s..e]);
                    }
                }

                if show_app_launcher {
                    if let Some(ref ll) = cached_launcher_layer {
                        let buf = layer.buf_mut();
                        let ww = screen.width();
                        let hh = screen.height();
                        let tby = hh.saturating_sub(TASKBAR_H);
                        for y in y0..y1.min(tby) {
                            let s = y * ww + x0;
                            let e = y * ww + x1;
                            buf[s..e].copy_from_slice(&ll[s..e]);
                        }
                    }
                }

                cursor::draw_cursor_into_layer(
                    &mut layer,
                    cursor_x,
                    cursor_y,
                    is_resizing,
                    display_state.pointer_size,
                );
                layer.flush_rect(&mut screen, x0, y0, x1, y1);

                prev_cursor_x = cursor_x;
                prev_cursor_y = cursor_y;
                prev_is_resizing = is_resizing;
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum NavigationEffect {
    None,
    SystemChanged,
    AppOpened,
}

fn open_app(
    name: &str,
    app_entries: &[AppEntry],
    wm: &mut WindowManager,
    warp_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::warp::WarpEngine)>,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    ui_commands: &mut alloc::vec::Vec<baram_graphics::uiscript::Command>,
    ui_win_id: &mut Option<WinId>,
    x: i32,
    y: i32,
    w: usize,
    h: usize,
) -> Option<WinId> {
    let entry = app_entries.iter().find(|entry| entry.name == name)?;
    let win_id = wm.add(&entry.title, x, y, w, h);
    wm.set_icon(win_id, &entry.icon);
    let content_h = h.saturating_sub(baram_windowserver::window::title_bar_h());

    if entry.app_type.starts_with("html") {
        let (html, css) = baram_bsd::app::load_html_document(&entry.name);
        let mut engine = baram_windowserver::html::HtmlEngine::new(&html, &css);
        engine.update(w as i32, content_h as i32);
        html_engines.push((win_id, engine));
    } else if entry.app_type.starts_with("uiscript") {
        let source = baram_bsd::app::load_app_source(&entry.name);
        *ui_commands = baram_graphics::uiscript::parse(&source);
        *ui_win_id = Some(win_id);
    } else {
        let source = baram_bsd::app::load_app_source(&entry.name);
        let mut engine = baram_windowserver::warp::WarpEngine::new(&source);
        engine.update(w as i32, content_h as i32);
        warp_engines.push((win_id, engine));
    }
    Some(win_id)
}

fn handle_navigation(
    command: &str,
    app_entries: &[AppEntry],
    wm: &mut WindowManager,
    warp_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::warp::WarpEngine)>,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    ui_commands: &mut alloc::vec::Vec<baram_graphics::uiscript::Command>,
    ui_win_id: &mut Option<WinId>,
    display_state: &mut baram_bsd::uri::DisplayState,
    x: i32,
    y: i32,
) -> NavigationEffect {
    if let Some(name) = baram_bsd::app::parse_app_uri(command) {
        if open_app(
            name,
            app_entries,
            wm,
            warp_engines,
            html_engines,
            ui_commands,
            ui_win_id,
            x,
            y,
            400,
            450,
        )
        .is_some()
        {
            return NavigationEffect::AppOpened;
        }
        return NavigationEffect::None;
    }

    if baram_bsd::uri::execute(command, display_state) {
        for (_, engine) in html_engines.iter_mut() {
            engine.refresh_config();
        }
        wm.set_all_dirty();
        NavigationEffect::SystemChanged
    } else {
        NavigationEffect::None
    }
}

fn wallpaper_for_state(
    state: &baram_bsd::uri::DisplayState,
    screen_w: usize,
    screen_h: usize,
) -> Option<Vec<u32>> {
    if state.wallpaper_mode == baram_bsd::uri::WallpaperMode::Color {
        state
            .wallpaper_color
            .map(|color| make_solid_wallpaper(color, screen_w, screen_h))
    } else {
        WALLPAPERS
            .get(state.wallpaper_index)
            .and_then(|bytes| decode_wallpaper(bytes, screen_w, screen_h))
    }
}
