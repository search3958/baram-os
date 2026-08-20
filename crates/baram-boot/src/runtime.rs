fn baram_kernel_main(mut nano: NanoSystem) -> Status {
    NanoSystem::serial_log("baram: kernel entry\r\n");
    let mut timer_event = nano.take_timer_event();
    NanoSystem::serial_log("baram: acquiring screen\r\n");
    let mut screen = match Screen::take() {
        Ok(screen) => screen,
        Err(_) => {
            NanoSystem::serial_log("baram: screen acquisition failed\r\n");
            NanoSystem::paint_failure_screen();
            return Status::UNSUPPORTED;
        }
    };

    NanoSystem::serial_log("baram: screen ready\r\n");
    unsafe { baram_font::log::init_screen(&screen) };
    log_line_str("BaramOS kernel: starting...");

    NanoSystem::serial_log("baram: panic reporter ready\r\n");
    unsafe { baram_kern::panic::init_from_screen(&screen) };

    // Nano System has already cleared the framebuffer before input probing;
    // replace that minimal handoff screen with the kernel boot logo now.
    draw_boot_logo(&mut screen);
    NanoSystem::serial_log("baram: boot logo drawn\r\n");

    let compute_workers = baram_core::parallel::init();
    NanoSystem::serial_log("baram: compute dispatcher ready\r\n");
    log_line_str(&alloc::format!(
        "BaramOS: {} compute APs enabled",
        compute_workers
    ));

    config::init_config();
    NanoSystem::serial_log("baram: config ready\r\n");
    let mut mouse_motion = baram_iokit::mouse::MouseMotionProcessor::new();
    log_line_str("BaramOS: config loaded");

    NanoSystem::serial_log("baram: initializing fonts\r\n");
    baram_font::ttf_font::init();
    NanoSystem::serial_log("baram: primary font ready\r\n");
    baram_font::ttf_font_hud::init();
    NanoSystem::serial_log("baram: HUD font ready\r\n");
    log_line_str("BaramOS: fonts initialized");

    NanoSystem::serial_log("baram: prerendering cursors\r\n");
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
    NanoSystem::serial_log("baram: cursors ready\r\n");

    log_line_str("BaramOS: input is owned by Nano System");
    nano.set_shift_key(shift_key::load_shift_key());

    let mut cursor_x: i32 = (screen.width() / 2) as i32;
    let mut cursor_y: i32 = (screen.height() / 2) as i32;
    let mut display_state = baram_bsd::uri::DisplayState::new();
    baram_bsd::uri::load_settings_from_config(&mut display_state);

    if !baram_bsd::setup::is_setup_done() {
        log_line_str("BaramOS: first boot detected, starting setup wizard");
        let mut wizard = baram_bsd::setup::SetupWizard::new();
        let setup_w = screen.width();
        let setup_h = screen.height();
        let mut setup_engine = baram_windowserver::warp::WarpEngine::new_warp4("setup.w4a");
        setup_engine.set_chrome_visible(false);
        let mut setup_scene = LayerSystem::new(setup_w, setup_h);
        let mut setup_present = LayerSystem::new(setup_w, setup_h);
        let mut setup_surface = LayerSystem::new(528, 320);
        let setup_origin = ((setup_w as i32 - 528) / 2, (setup_h as i32 - 320) / 2);
        let setup_card = (setup_origin.0, setup_origin.1, 528usize, 320usize);
        let setup_wallpaper = wallpaper_for_state(&display_state, setup_w, setup_h);
        let setup_background = setup_wallpaper.as_ref().map(|wallpaper| {
            NanoSystem::serial_log("baram: blurring setup wallpaper\r\n");
            let mut blurred = alloc::vec![0u32; setup_w * setup_h];
            let mut scratch = alloc::vec![0u32; setup_w * setup_h];
            baram_graphics::blur::blur_region_to_with_scratch(
                wallpaper,
                &mut blurred,
                &mut scratch,
                setup_w,
                0,
                setup_h,
                30,
            );
            blurred
        });
        NanoSystem::serial_log("baram: setup background ready\r\n");
        let card_radius = config::get_usize("ui-theme/card/radius", 12);
        let setup_shadow =
            baram_windowserver::window::RoundedShadow::new(setup_card.2, setup_card.3, card_radius);
        NanoSystem::serial_log("baram: setup shadow ready\r\n");
        let mut setup_scene_dirty = true;
        let mut setup_card_dirty = true;
        let mut setup_first_frame_logged = false;
        let mut setup_prev_cursor = (cursor_x, cursor_y);
        let mut setup_scroll = 0i32;
        let mut setup_now_ns = 0u64;
        // Setup used to advance this clock by one millisecond per rendered
        // loop. A costly card frame therefore stretched a 250ms Warp3
        // transition into seconds. Sample the hardware monotonic clock here
        // just as the desktop renderer does.
        let setup_clock = UiMonotonicClock::new();
        let mut setup_next_present_ms = 0u64;
        setup_engine.set_screen(wizard.warp3_screen());
        setup_engine.update(528, 320);
        NanoSystem::serial_log("baram: setup layout ready\r\n");

        loop {
            if let Some(ref mut timer) = timer_event {
                let _ = uefi::boot::wait_for_event(core::slice::from_mut(timer));
            }
            setup_now_ns = setup_clock
                .as_ref()
                .map(UiMonotonicClock::elapsed_ns)
                .unwrap_or_else(|| setup_now_ns.saturating_add(1_000_000));
            let mut setup_scroll_input = false;

            while let Some(nano_event) = nano.poll_keyboard() {
                let ev = kernel_key_event(nano_event);
                wizard.on_key(&ev);
            }

            {
                while let Some(nano_event) = nano.poll_pointer() {
                    let ev =
                        mouse_motion.process(kernel_pointer_event(nano_event, nano.input_state));
                    baram_iokit::mouse::apply_mouse_event(
                        &mut cursor_x,
                        &mut cursor_y,
                        &ev,
                        screen.width(),
                        screen.height(),
                        nano.pointer_abs_max(),
                    );
                    if ev.scroll != 0 {
                        setup_scroll_input = true;
                        let max_scroll = setup_engine.content_height().saturating_sub(320).max(0);
                        let delta = ev
                            .scroll
                            .saturating_neg()
                            .saturating_mul(baram_windowserver::window::scroll_speed());
                        setup_scroll = setup_scroll.saturating_add(delta).clamp(0, max_scroll);
                        setup_engine.set_scroll(setup_scroll);
                    }
                    setup_engine.set_hover(
                        cursor_x - setup_origin.0,
                        cursor_y - setup_origin.1 + setup_scroll,
                    );
                    setup_card_dirty = true;
                    if ev.left {
                        setup_engine.click(
                            cursor_x - setup_origin.0,
                            cursor_y - setup_origin.1 + setup_scroll,
                        );
                        if let Some(command) = setup_engine.last_command.take() {
                            wizard.on_command(&command);
                        }
                    }
                }
            }

            if wizard.screen == baram_bsd::setup::SetupScreen::Done {
                break;
            }

            cursor_x = cursor_x.max(0).min(screen.width() as i32 - 1);
            cursor_y = cursor_y.max(0).min(screen.height() as i32 - 1);
            if wizard.take_dirty() {
                setup_scroll = 0;
                setup_engine.set_screen(wizard.warp3_screen());
                setup_engine.update(528, 320);
                setup_engine.set_scroll(setup_scroll);
                setup_scene_dirty = true;
                setup_card_dirty = true;
            }
            // The setup card can take much longer to rasterize than a normal
            // desktop frame. Slow only its Warp3 transition clock so a single
            // expensive frame cannot skip nearly the whole transition.
            if setup_engine.tick(setup_now_ns / 3) {
                setup_card_dirty = true;
            }

            let setup_now_ms = setup_now_ns / 1_000_000;
            let cursor_changed = setup_prev_cursor != (cursor_x, cursor_y);
            if !setup_scene_dirty && !setup_card_dirty && !cursor_changed {
                continue;
            }
            // Mouse movement can present immediately; do the same for wheel
            // and touchpad input so setup scrolling is not held for the next
            // 16 ms frame slot.
            if setup_now_ms < setup_next_present_ms && !cursor_changed && !setup_scroll_input {
                continue;
            }
            setup_next_present_ms = setup_now_ms.saturating_add(16);

            if setup_scene_dirty {
                if let Some(ref background) = setup_background {
                    setup_scene.copy_from_screen_buffer(background);
                } else {
                    setup_scene.clear(config::get_color("ui-theme/color/bg", Color::BG));
                }
                if let Some(ref shadow) = setup_shadow {
                    shadow.composite_onto(&mut setup_scene, setup_card.0, setup_card.1);
                }
                setup_engine.update(528, 320);
                setup_surface.clear(Color::rgb(250, 250, 252));
                setup_engine.draw_to_layer(&mut setup_surface, 0, 0);
                setup_scene.composit_rounded(
                    &setup_surface,
                    setup_origin.0.max(0) as usize,
                    setup_origin.1.max(0) as usize,
                    0,
                    0,
                    528,
                    320,
                    card_radius,
                );
                setup_present.copy_from_screen_buffer(setup_scene.buf_ref());
            } else if setup_card_dirty {
                // Scroll and hover changes only affect the setup card. Avoid
                // rebuilding and copying the full blurred desktop per tick.
                setup_engine.update(528, 320);
                setup_surface.clear(Color::rgb(250, 250, 252));
                setup_engine.draw_to_layer(&mut setup_surface, 0, 0);
                setup_scene.composit_rounded(
                    &setup_surface,
                    setup_origin.0.max(0) as usize,
                    setup_origin.1.max(0) as usize,
                    0,
                    0,
                    528,
                    320,
                    card_radius,
                );
                let card_x0 = setup_origin.0.max(0) as usize;
                let card_y0 = setup_origin.1.max(0) as usize;
                let card_x1 = (setup_origin.0 + 528).min(setup_w as i32).max(0) as usize;
                let card_y1 = (setup_origin.1 + 320).min(setup_h as i32).max(0) as usize;
                setup_present.push_clip(card_x0, card_y0, card_x1, card_y1);
                setup_present.copy_from_screen_buffer(setup_scene.buf_ref());
                setup_present.pop_clip();
                // The card-only path does not overwrite a cursor that was
                // previously outside the card. Restore both cursor positions
                // before drawing the new one to prevent pointer trails.
                if cursor_changed {
                    let pad = 32i32;
                    let x0 = (setup_prev_cursor.0.min(cursor_x) - pad).max(0) as usize;
                    let y0 = (setup_prev_cursor.1.min(cursor_y) - pad).max(0) as usize;
                    let x1 = (setup_prev_cursor.0.max(cursor_x) + cursor::CURSOR_BOX_W as i32 + pad)
                        .min(setup_w as i32) as usize;
                    let y1 = (setup_prev_cursor.1.max(cursor_y) + cursor::CURSOR_BOX_H as i32 + pad)
                        .min(setup_h as i32) as usize;
                    setup_present.push_clip(x0, y0, x1, y1);
                    setup_present.copy_from_screen_buffer(setup_scene.buf_ref());
                    setup_present.pop_clip();
                }
            } else {
                let pad = 32i32;
                let x0 = (setup_prev_cursor.0.min(cursor_x) - pad).max(0) as usize;
                let y0 = (setup_prev_cursor.1.min(cursor_y) - pad).max(0) as usize;
                let x1 = (setup_prev_cursor.0.max(cursor_x) + cursor::CURSOR_BOX_W as i32 + pad)
                    .min(setup_w as i32) as usize;
                let y1 = (setup_prev_cursor.1.max(cursor_y) + cursor::CURSOR_BOX_H as i32 + pad)
                    .min(setup_h as i32) as usize;
                setup_present.push_clip(x0, y0, x1, y1);
                setup_present.copy_from_screen_buffer(setup_scene.buf_ref());
                setup_present.pop_clip();
            }

            cursor::draw_cursor_into_layer(
                &mut setup_present,
                cursor_x,
                cursor_y,
                false,
                display_state.pointer_size,
            );
            if setup_scene_dirty {
                setup_present.flush(&mut screen);
                if !setup_first_frame_logged {
                    NanoSystem::serial_log("baram: setup first frame ready\r\n");
                    setup_first_frame_logged = true;
                }
            } else if setup_card_dirty {
                let card_x0 = setup_origin.0.max(0) as usize;
                let card_y0 = setup_origin.1.max(0) as usize;
                let card_x1 = (setup_origin.0 + 528).min(setup_w as i32).max(0) as usize;
                let card_y1 = (setup_origin.1 + 320).min(setup_h as i32).max(0) as usize;
                setup_present.flush_rect(&mut screen, card_x0, card_y0, card_x1, card_y1);
                if cursor_changed {
                    let pad = 32i32;
                    let x0 = (setup_prev_cursor.0.min(cursor_x) - pad).max(0) as usize;
                    let y0 = (setup_prev_cursor.1.min(cursor_y) - pad).max(0) as usize;
                    let x1 = (setup_prev_cursor.0.max(cursor_x) + cursor::CURSOR_BOX_W as i32 + pad)
                        .min(setup_w as i32) as usize;
                    let y1 = (setup_prev_cursor.1.max(cursor_y) + cursor::CURSOR_BOX_H as i32 + pad)
                        .min(setup_h as i32) as usize;
                    setup_present.flush_rect(&mut screen, x0, y0, x1, y1);
                }
            } else {
                let pad = 32i32;
                let x0 = (setup_prev_cursor.0.min(cursor_x) - pad).max(0) as usize;
                let y0 = (setup_prev_cursor.1.min(cursor_y) - pad).max(0) as usize;
                let x1 = (setup_prev_cursor.0.max(cursor_x) + cursor::CURSOR_BOX_W as i32 + pad)
                    .min(setup_w as i32) as usize;
                let y1 = (setup_prev_cursor.1.max(cursor_y) + cursor::CURSOR_BOX_H as i32 + pad)
                    .min(setup_h as i32) as usize;
                setup_present.flush_rect(&mut screen, x0, y0, x1, y1);
            }
            setup_scene_dirty = false;
            setup_card_dirty = false;
            setup_prev_cursor = (cursor_x, cursor_y);
        }
        log_line_str("BaramOS: setup wizard completed");
        nano.set_shift_key(shift_key::load_shift_key());
    }

    let mut wm = WindowManager::new(screen.width(), screen.height());
    let mut layer = LayerSystem::new(screen.width(), screen.height());

    log_line_str("BaramOS: loading index.yaml...");
    let index_yaml = baram_bsd::app::read_index_yaml();
    log_line_str(&alloc::format!(
        "BaramOS: index.yaml {} bytes",
        index_yaml.len()
    ));
    let (autostart_list, app_entries) = parse_index_yaml(&index_yaml);
    let mut warp_engines: alloc::vec::Vec<(WinId, baram_windowserver::warp::WarpEngine)> =
        alloc::vec::Vec::new();
    let mut html_engines: alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)> =
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
    let mut pending_os_permission: Option<PendingOsPermission> = None;
    let mut pending_file_dialog: Option<PendingFileDialog> = None;

    let mouse_mode_label = if nano.input.absolute_pointer_available {
        "Absolute"
    } else if nano.input.pointer_available {
        "Simple Ptr"
    } else {
        "None"
    };

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

    let mut taskbar_surface = TaskbarSurface::new(screen.width());
    let mut cached_launcher_layer: Option<Vec<u32>> = None;
    let mut cached_ime_menu_layer: Option<Vec<u32>> = None;
    let mut prev_window_count: usize = 0;
    let mut prev_focused_id: Option<WinId> = None;
    let mut bg_cache: Option<Vec<u32>> = None;
    let mut prev_wallpaper_idx: usize = display_state.wallpaper_index;
    let mut hud_damage_pending = false;
    // Monotonic UI clock driven by the already-configured 1 ms timer event.
    // Do not query the slow, wall-clock UEFI runtime service per frame.
    let mut ui_time_ms: u64 = 0;
    let mut ui_clock = UiMonotonicClock::new();
    let mut next_present_ms: u64 = 0;
    let mut deferred_dirty = false;
    let mut prev_text_cursor_visible = true;

    let mut tb_add_progress: f32 = -1.0f32;
    let mut tb_add_started_ms: Option<u64> = None;
    let mut tb_remove_progress: f32 = -1.0f32;
    let mut tb_shift_x: f32 = 0.0f32;
    let mut show_app_launcher: bool = false;
    let mut app_search_focused: bool = false;
    let mut app_search_query = alloc::string::String::new();
    let mut app_launcher_scroll = SmoothScroll::new();
    let mut app_list: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    let mut app_name_list: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    let mut app_icon_list: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    for entry in &app_entries {
        app_list.push(entry.title.clone());
        app_name_list.push(entry.name.clone());
        app_icon_list.push(entry.icon.clone());
    }
    // The search box has no hover visual; keep this compatibility argument
    // stable without making pointer movement schedule a redraw.
    let hover_apps_icon = false;
    let mut prev_show_app_launcher: bool = false;
    let mut launcher_content_dirty: bool = false;
    let mut launcher_render_visible = false;
    let mut pending_launcher_app: Option<alloc::string::String> = None;
    let mut launcher_app_open_at_ms: Option<u64> = None;
    let mut launcher_target_prev = false;
    let mut launcher_anim_phase: i8 = 0; // 1 opening, -1 closing
    let mut launcher_anim_started_ms = 0u64;
    let mut launcher_anim_elapsed_ms = 0u32;
    let mut launcher_cache_drop_after_close = false;
    let mut input_mode = InputMode::Latin;
    let mut japanese_ime = JapaneseIme::new();
    let mut hangul_ime = HangulIme::new();
    let mut pinyin_ime = PinyinIme::new();
    let mut show_ime_menu = false;
    let mut soft_keyboard = SoftKeyboard::new();
    let mut prev_show_ime_menu = false;
    let mut ime_menu_closing = false;
    let mut ime_menu_close_started_ms: Option<u64> = None;
    let mut ime_menu_opacity = 255u8;
    let mut hover_ime_icon = false;
    let mut hover_keyboard_icon = false;
    let mut ime_hover_dirty = false;
    let mut prev_ime_conversion_visible = false;

    let timezone_offset_minutes = config::timezone_offset_minutes();

    let mut battery_info = baram_iokit::battery::read_battery();
    let mut battery_poll_seconds: u8 = 0;

    let (mut clock_hh, mut clock_mm) = {
        let tz = timezone_offset_minutes;
        match runtime::get_time() {
            Ok(t) => {
                let total_min = (t.hour() as i32) * 60 + (t.minute() as i32) + tz;
                let day_min = total_min.rem_euclid(24 * 60);
                ((day_min / 60) as u8, (day_min % 60) as u8)
            }
            Err(_) => (0u8, 0u8),
        }
    };

    render_scene(
        &mut layer,
        &mut taskbar_surface,
        &mut wm,
        mouse_ev_count,
        key_ev_count,
        fps,
        mouse_mode_label,
        &mut warp_engines,
        &mut html_engines,
        cached_wallpaper.as_deref(),
        &mut cached_launcher_layer,
        &mut cached_ime_menu_layer,
        true,
        -1.0,
        -1.0,
        0.0,
        display_state.hud_enabled,
        &mut bg_cache,
        false,
        launcher_render_visible,
        &app_list,
        &app_icon_list,
        hover_apps_icon,
        false,
        false,
        false,
        app_search_focused,
        &app_search_query,
        baram_windowserver::text_cursor::visible(ui_time_ms * 1_000_000),
        app_launcher_scroll.position.max(0) as usize,
        0,
        0,
        false,
        false,
        false,
        clock_hh,
        clock_mm,
        battery_info.valid_percentage(),
        false,
        &mut soft_keyboard,
        255,
        ime_menu_selection(input_mode),
        None,
        &[],
        0,
    );
    // Build the hidden launcher once while the boot scene is already hot.
    // With zero layer opacity this leaves the framebuffer unchanged, but the
    // first click never has to decode icons, rasterize labels, or blur glass.
    render_scene(
        &mut layer,
        &mut taskbar_surface,
        &mut wm,
        mouse_ev_count,
        key_ev_count,
        fps,
        mouse_mode_label,
        &mut warp_engines,
        &mut html_engines,
        cached_wallpaper.as_deref(),
        &mut cached_launcher_layer,
        &mut cached_ime_menu_layer,
        false,
        -1.0,
        -1.0,
        0.0,
        display_state.hud_enabled,
        &mut bg_cache,
        true,
        true,
        &app_list,
        &app_icon_list,
        hover_apps_icon,
        false,
        false,
        false,
        app_search_focused,
        &app_search_query,
        baram_windowserver::text_cursor::visible(ui_time_ms * 1_000_000),
        app_launcher_scroll.position.max(0) as usize,
        1,
        0,
        false,
        true,
        false,
        clock_hh,
        clock_mm,
        battery_info.valid_percentage(),
        false,
        &mut soft_keyboard,
        255,
        ime_menu_selection(input_mode),
        None,
        &[],
        0,
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
        let (mut dirty, mut cursor_moved, mut scroll_input, mut launcher_scroll_input_changed) =
            (include!("runtime_input.rs"))();
        if (include!("runtime_pointer.rs"))() {
            continue;
        }
        if (include!("runtime_frame.rs"))() {
            continue;
        }
    }
}
