|| {
        let mut dirty = deferred_dirty;
        deferred_dirty = false;
        let mut cursor_moved = false;
        let mut scroll_input = false;
        let mut launcher_scroll_input_changed = false;
        let mut ui_timer_fired = timer_event.is_none();

        if let Some(ref mut timer) = timer_event {
            ui_timer_fired = uefi::boot::wait_for_event(core::slice::from_mut(timer)).is_ok();
        }
        if let Some(ref mut clock) = ui_clock {
            ui_time_ms = ui_time_ms.wrapping_add(clock.frame_delta_ms());
        } else if ui_timer_fired {
            ui_time_ms = ui_time_ms.wrapping_add(1);
        }
        let text_cursor_visible = baram_windowserver::text_cursor::visible(ui_time_ms * 1_000_000);
        if app_search_focused && text_cursor_visible != prev_text_cursor_visible {
            taskbar_surface.invalidate_search();
            scene_dirty = true;
            dirty = true;
        }
        prev_text_cursor_visible = text_cursor_visible;

        match baram_bsd::uri::check_system_commands(&mut display_state) {
            baram_bsd::uri::SystemCommand::ResetAll => {
                NanoSystem::cold_reset();
            }
            baram_bsd::uri::SystemCommand::None => {}
        }

        while let Some(nano_event) = nano.poll_keyboard() {
            let ev = kernel_key_event(nano_event);
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

            // UEFI may report Backspace only as a scan code. It still enters
            // the exact same OS text path as an on-screen Backspace key.
            if ev.scancode == 0x08
                && dispatch_text_input_key(
                    SoftKey::Backspace,
                    input_mode,
                    &mut japanese_ime,
                    &mut hangul_ime,
                    &mut pinyin_ime,
                    app_search_focused,
                    &mut app_search_query,
                    &app_entries,
                    &mut app_list,
                    &mut app_name_list,
                    &mut app_icon_list,
                    &mut app_launcher_scroll,
                    &mut show_app_launcher,
                    &mut cached_launcher_layer,
                    &mut launcher_content_dirty,
                    &mut taskbar_surface,
                    &mut wm,
                    &mut warp_engines,
                    &mut html_engines,
                )
            {
                dirty = true;
                scene_dirty = true;
                continue;
            }

            if ev.ctrl_or_cmd() || (mousekey_mode && nano.shift_held()) {
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
                dispatch_text_input_key(
                    SoftKey::Character(c),
                    input_mode,
                    &mut japanese_ime,
                    &mut hangul_ime,
                    &mut pinyin_ime,
                    app_search_focused,
                    &mut app_search_query,
                    &app_entries,
                    &mut app_list,
                    &mut app_name_list,
                    &mut app_icon_list,
                    &mut app_launcher_scroll,
                    &mut show_app_launcher,
                    &mut cached_launcher_layer,
                    &mut launcher_content_dirty,
                    &mut taskbar_surface,
                    &mut wm,
                    &mut warp_engines,
                    &mut html_engines,
                );
            }
            dirty = true;
            scene_dirty = true;
        }

        {
            let shift_held = nano.shift_held();
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
                            let nx = (screen.width() as i32 - 400) / 2;
                            let ny = (screen.height() as i32 - 300) / 2;
                            let win_id = wm.add("マウスキー", nx, ny, 400, 300);
                            let mut engine = baram_windowserver::html::HtmlEngine::new_warp4(
                                "mousekeydialog.w4a",
                            );
                            engine.update(380, 260);
                            html_engines.push((win_id, engine));
                            mousekey_win_id = Some(win_id);
                            tb_add_progress = 0.0;
                            tb_add_started_ms = None;
                            tb_shift_x = 26.0;
                            dirty = true;
                            scene_dirty = true;
                        } else {
                            if let Some(wid) = mousekey_win_id.take() {
                                wm.remove(wid);
                                warp_engines.retain(|(id, _)| *id != wid);
                                html_engines.retain(|(id, _)| *id != wid);
                                dirty = true;
                                scene_dirty = true;
                            }
                        }
                    }
                }
            }
        }

        if nano.ctrl_or_cmd_held() || mousekey_mode {
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
                if nano.key_is_held(usb_code) {
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

        {
            while let Some(nano_event) = nano.poll_pointer() {
                let ev = mouse_motion.process(kernel_pointer_event(nano_event, nano.input_state));
                mouse_ev_count = mouse_ev_count.wrapping_add(1);

                let old_cursor = (cursor_x, cursor_y);
                let (cx, cy) = baram_iokit::mouse::apply_mouse_event(
                    &mut cursor_x,
                    &mut cursor_y,
                    &ev,
                    screen.width(),
                    screen.height(),
                    nano.pointer_abs_max(),
                );
                cursor_moved |= (cx, cy) != old_cursor;

                // A keyboard drag is owned by the OS overlay and must never
                // leak through to the launcher or a window beneath it.
                if soft_keyboard.is_dragging() {
                    if ev.left {
                        if soft_keyboard.drag_to(cx, cy, screen.width(), screen.height()) {
                            scene_dirty = true;
                        }
                    } else {
                        soft_keyboard.end_drag();
                        mouse_down = false;
                        wm.on_mouse_up();
                        scene_dirty = true;
                    }
                    dirty = true;
                    continue;
                }

                if ev.scroll != 0 {
                    scroll_input = true;
                    let window_scroll_delta = ev
                        .scroll
                        .saturating_neg()
                        .saturating_mul(baram_windowserver::window::scroll_speed());
                    let panel_y = screen.height() as i32 - TASKBAR_H as i32 - (3 * 88 + 24) as i32;
                    let on_launcher = show_app_launcher
                        && cx >= 12
                        && cx < 300
                        && cy >= panel_y
                        && cy < screen.height() as i32 - TASKBAR_H as i32;
                    if on_launcher {
                        app_launcher_scroll.set_max(app_launcher_scroll_max(app_list.len()));
                        launcher_scroll_input_changed |=
                            app_launcher_scroll.scroll(window_scroll_delta);
                        launcher_content_dirty |= launcher_scroll_input_changed;
                        dirty = true;
                        scene_dirty = true;
                    } else if !show_app_launcher {
                        if let Some(id) = wm.window_at(cx, cy) {
                            if wm.is_file_dialog(id) {
                                if wm.file_dialog_scroll(id, window_scroll_delta) {
                                    dirty = true;
                                    scene_dirty = true;
                                }
                            } else {
                                wm.scroll_window(id, window_scroll_delta);
                                dirty = true;
                                scene_dirty = true;
                            }
                        }
                    }
                }

                if ev.left && !mouse_down {
                    mouse_down = true;
                    // This OS overlay is above every compositor layer. Consume
                    // its pointer event before launcher/window focus handling,
                    // preserving the input target that opened the keyboard.
                    if soft_keyboard.contains(cx, cy, screen.width(), screen.height()) {
                        if let Some(key) =
                            soft_keyboard.click(cx, cy, screen.width(), screen.height())
                        {
                            if key == SoftKey::Close {
                                soft_keyboard.close();
                            } else {
                                dispatch_text_input_key(
                                    key,
                                    input_mode,
                                    &mut japanese_ime,
                                    &mut hangul_ime,
                                    &mut pinyin_ime,
                                    app_search_focused,
                                    &mut app_search_query,
                                    &app_entries,
                                    &mut app_list,
                                    &mut app_name_list,
                                    &mut app_icon_list,
                                    &mut app_launcher_scroll,
                                    &mut show_app_launcher,
                                    &mut cached_launcher_layer,
                                    &mut launcher_content_dirty,
                                    &mut taskbar_surface,
                                    &mut wm,
                                    &mut warp_engines,
                                    &mut html_engines,
                                );
                            }
                        }
                        scene_dirty = true;
                        dirty = true;
                        continue;
                    }
                    let (ime_x, ime_y_local, ime_w, ime_h) =
                        ime_button_bounds(screen.width(), battery_info.valid_percentage());
                    let ime_y = screen.height() as i32 - TASKBAR_H as i32 + ime_y_local;
                    let keyboard_x = ime_x - ime_w - 12;
                    if cx >= keyboard_x
                        && cx < keyboard_x + ime_w
                        && cy >= ime_y
                        && cy < ime_y + ime_h
                    {
                        soft_keyboard.toggle();
                        scene_dirty = true;
                        dirty = true;
                        continue;
                    }
                    // Clicking a different input ends the previous composition.
                    japanese_ime.reset();
                    hangul_ime.reset();
                    pinyin_ime.reset();
                    let sh = screen.height();

                    // The mode picker is modal: a click selects a row or
                    // dismisses it before reaching the window underneath.
                    if show_ime_menu && !ime_menu_closing {
                        if let Some(selection) = ime_menu_mode_at(
                            cx,
                            cy,
                            screen.width(),
                            screen.height(),
                            battery_info.valid_percentage(),
                        ) {
                            input_mode = input_mode_for_menu_selection(selection);
                            hangul_ime.reset();
                            pinyin_ime.reset();
                            taskbar_surface.invalidate();
                        }
                        ime_menu_closing = true;
                        ime_menu_close_started_ms = Some(ui_time_ms);
                        scene_dirty = true;
                        dirty = true;
                        continue;
                    }

                    if show_app_launcher {
                        let search_x = 12i32;
                        let search_y = sh as i32 - TASKBAR_H as i32 + (TASKBAR_H as i32 - 40) / 2;
                        if cx >= search_x
                            && cx < search_x + 190
                            && cy >= search_y
                            && cy < search_y + 40
                        {
                            app_search_focused = true;
                            taskbar_surface.invalidate_search();
                            scene_dirty = true;
                            dirty = true;
                            continue;
                        }
                        let cols = 4usize;
                        let icon_size = 52usize;
                        let icon_gap = 16usize;
                        let label_h = 20usize;
                        let cell_w = icon_size + icon_gap;
                        let cell_h = icon_size + label_h + icon_gap;
                        let grid_w = cols * cell_w;
                        let rows = 3usize;
                        let grid_h = rows * cell_h;
                        let grid_x = 20usize;
                        let grid_y = screen.height().saturating_sub(TASKBAR_H + grid_h + 16);
                        let content_y = grid_y + 4;
                        let panel_x = 12i32;
                        let panel_y = grid_y.saturating_sub(8) as i32;
                        let panel_w = (grid_w + 16) as i32;
                        let panel_h = (grid_h.max(40) + 16) as i32;
                        let on_launcher_panel = cx >= panel_x
                            && cx < panel_x + panel_w
                            && cy >= panel_y
                            && cy < panel_y + panel_h;
                        let mut clicked_app = None;
                        for (i, _) in app_list.iter().enumerate() {
                            let col = i % cols;
                            let row = i / cols;
                            let ix = grid_x + col * cell_w + icon_gap / 2;
                            let iy = content_y as i32 + row as i32 * cell_h as i32
                                - app_launcher_scroll.position;
                            if cx >= ix as i32
                                && cx < (ix + icon_size) as i32
                                && cy >= content_y as i32
                                && cy < (content_y + grid_h) as i32
                                && cy >= iy
                                && cy < iy + icon_size as i32
                            {
                                clicked_app = Some(i);
                                break;
                            }
                        }
                        if let Some(idx) = clicked_app {
                            let app_name = app_name_list[idx].clone();
                            pending_launcher_app = Some(app_name);
                            app_search_query.clear();
                            app_search_focused = false;
                            rebuild_filtered_apps(
                                &app_entries,
                                "",
                                &mut app_list,
                                &mut app_name_list,
                                &mut app_icon_list,
                            );
                            taskbar_surface.invalidate();
                            launcher_cache_drop_after_close = true;
                            show_app_launcher = false;
                        } else if on_launcher_panel {
                            app_search_focused = false;
                            taskbar_surface.invalidate_search();
                            show_app_launcher = true;
                        } else {
                            app_search_query.clear();
                            app_search_focused = false;
                            rebuild_filtered_apps(
                                &app_entries,
                                "",
                                &mut app_list,
                                &mut app_name_list,
                                &mut app_icon_list,
                            );
                            launcher_cache_drop_after_close = true;
                            taskbar_surface.invalidate_search();
                            show_app_launcher = false;
                        }
                        scene_dirty = true;
                    } else if cy >= sh as i32 - TASKBAR_H as i32 {
                        let (ime_x, ime_y, ime_w, ime_h) =
                            ime_button_bounds(screen.width(), battery_info.valid_percentage());
                        let ime_y = sh as i32 - TASKBAR_H as i32 + ime_y;
                        if cx >= ime_x
                            && cx < ime_x + ime_w + 10
                            && cy >= ime_y
                            && cy < ime_y + ime_h
                        {
                            show_ime_menu = true;
                            ime_menu_closing = false;
                            ime_menu_close_started_ms = None;
                            ime_menu_opacity = 255;
                            scene_dirty = true;
                            dirty = true;
                            continue;
                        }
                        let apps_icon_x = 12i32;
                        let apps_icon_size = 190i32;
                        let apps_icon_y =
                            sh as i32 - TASKBAR_H as i32 + (TASKBAR_H as i32 - 40) / 2;
                        let on_apps_icon = cx >= apps_icon_x
                            && cx < apps_icon_x + apps_icon_size
                            && cy >= apps_icon_y
                            && cy < apps_icon_y + 40;
                        if on_apps_icon {
                            app_launcher_scroll.reset();
                            app_launcher_scroll.set_max(app_launcher_scroll_max(app_list.len()));
                            app_search_focused = true;
                            show_app_launcher = true;
                            taskbar_surface.invalidate_search();
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
                                    if wm.is_focusable(*id) {
                                        wm.focus(*id);
                                    }
                                    break;
                                }
                                bx += btn_d + btn_gap;
                            }
                        }
                    } else {
                        let win_under = wm.window_at(cx, cy);
                        if let Some(id) = win_under {
                            if wm.is_focusable(id) {
                                wm.focus(id);
                            }
                            let btn = wm.button_hit_at(id, cx, cy);
                            match btn {
                                'c' => {
                                    wm.remove(id);
                                    warp_engines.retain(|(wid, _)| *wid != id);
                                    html_engines.retain(|(wid, _)| *wid != id);
                                    cancel_permission_for_closed_window(
                                        id,
                                        &mut wm,
                                        &mut html_engines,
                                        &mut pending_os_permission,
                                    );
                                    cancel_file_dialog_for_closed_window(
                                        id,
                                        &mut wm,
                                        &mut pending_file_dialog,
                                    );
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
                                    } else if wm.title_bar_hit_at(id, cx, cy) {
                                        wm.start_drag_at(id, cx, cy);
                                    }
                                }
                            }
                            let after = wm.insertion_ids();
                            let _ = after;
                        }
                        if let Some(clicked_id) = wm.window_at(cx, cy) {
                            if handle_native_file_dialog_click(
                                clicked_id,
                                cx,
                                cy,
                                &mut wm,
                                &mut pending_file_dialog,
                                &mut warp_engines,
                            ) {
                                scene_dirty = true;
                            }
                            for (wid, engine) in warp_engines.iter_mut() {
                                if clicked_id == *wid && !wm.is_interaction_blocked(clicked_id) {
                                    if let Some((wx, wy, ww, wh, scroll)) =
                                        wm.get_window_rect(clicked_id)
                                    {
                                        let rel_x = cx - wx;
                                        let rel_y = cy - wy;
                                        let tb_h = if wm.is_focusable(clicked_id) {
                                            baram_windowserver::window::title_bar_h() as i32
                                        } else {
                                            0
                                        };
                                        if rel_y >= tb_h {
                                            let warp_y = rel_y + scroll;
                                            engine.set_scroll(scroll);
                                            engine.set_runtime_metrics(
                                                fps,
                                                wm.count(),
                                                key_ev_count,
                                                mouse_ev_count,
                                            );
                                            engine.click(rel_x, warp_y);
                                            let content_h = wh.saturating_sub(tb_h as usize);
                                            engine.update(ww as i32, content_h as i32);
                                            wm.set_content_dirty(clicked_id);
                                            scene_dirty = true;

                                            if !engine.last_command.as_ref().is_some_and(|cmd| {
                                                cmd.starts_with("files-upload://")
                                            }) {
                                                if let Some(cmd) = engine.last_command.take() {
                                                    let is_hud_command =
                                                        baram_bsd::uri::parse(&cmd)
                                                            .map_or(false, |p| {
                                                                p.path.starts_with("display/hud")
                                                            });
                                                    let previous_hud = display_state.hud_enabled;
                                                    if authorize_os_setting(
                                                        &cmd,
                                                        engine.origin(),
                                                        &mut wm,
                                                        &mut html_engines,
                                                        &mut pending_os_permission,
                                                        Some(clicked_id),
                                                        120,
                                                        80,
                                                    ) && baram_bsd::uri::execute(
                                                        &cmd,
                                                        &mut display_state,
                                                    ) {
                                                        engine.update(ww as i32, content_h as i32);
                                                        if is_hud_command {
                                                            hud_damage_pending |= previous_hud
                                                                != display_state.hud_enabled;
                                                        } else {
                                                            wm.set_all_dirty();
                                                            taskbar_surface.invalidate();
                                                            cached_launcher_layer = None;
                                                            app_launcher_scroll.reset();
                                                            bg_cache = None;
                                                        }
                                                        scene_dirty = true;
                                                    }
                                                    if let Some(parsed) =
                                                        baram_bsd::uri::parse(&cmd)
                                                    {
                                                        if parsed
                                                            .path
                                                            .starts_with("display/wallpaper")
                                                        {
                                                            if display_state.wallpaper_mode
                                                            == baram_bsd::uri::WallpaperMode::Color
                                                        {
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
                                                        } else if parsed
                                                            .path
                                                            .starts_with("display/pointer")
                                                            || parsed
                                                                .path
                                                                .starts_with("display/hud")
                                                        {
                                                            scene_dirty = true;
                                                        } else {
                                                            scene_dirty = true;
                                                        }
                                                    }
                                                }
                                            }

                                            if let Some(enabled_str) =
                                                engine.get_state_value("--hudEnabled")
                                            {
                                                let new_enabled = enabled_str == "true";
                                                if display_state.hud_enabled != new_enabled {
                                                    display_state.hud_enabled = new_enabled;
                                                    hud_damage_pending = true;
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
                                if clicked_id != *wid || wm.is_interaction_blocked(clicked_id) {
                                    continue;
                                }
                                if let Some((wx, wy, ww, wh, scroll)) =
                                    wm.get_window_rect(clicked_id)
                                {
                                    let rel_x = cx - wx;
                                    let rel_y = cy - wy;
                                    let tb_h = if wm.is_focusable(clicked_id) {
                                        baram_windowserver::window::title_bar_h() as i32
                                    } else {
                                        0
                                    };
                                    if rel_y >= tb_h {
                                        engine.set_scroll(scroll);
                                        engine.set_runtime_metrics(
                                            fps,
                                            wm.count(),
                                            key_ev_count,
                                            mouse_ev_count,
                                        );
                                        engine.click(rel_x, rel_y + scroll);
                                        engine.update(
                                            ww as i32,
                                            wh.saturating_sub(tb_h as usize) as i32,
                                        );
                                        if let Some(target) = engine.take_scroll_request() {
                                            wm.set_window_scroll(clicked_id, target);
                                        }
                                        html_command = engine.last_command.take().map(|command| {
                                            (command, engine.origin().to_string(), *wid)
                                        });
                                        wm.set_content_dirty(clicked_id);
                                        scene_dirty = true;
                                    }
                                }
                                break;
                            }
                            if let Some((cmd, origin, source_win_id)) = html_command {
                                let nx = 100 + ((new_window_idx as i32 * 37) % 300);
                                let ny = 60 + ((new_window_idx as i32 * 23) % 200);
                                match handle_navigation(
                                    &cmd,
                                    &app_entries,
                                    &mut wm,
                                    &mut warp_engines,
                                    &mut html_engines,
                                    &mut display_state,
                                    &origin,
                                    source_win_id,
                                    &mut pending_os_permission,
                                    &mut pending_file_dialog,
                                    nx,
                                    ny,
                                ) {
                                    NavigationEffect::AppOpened => {
                                        new_window_idx = new_window_idx.wrapping_add(1);
                                        tb_add_progress = 0.0;
                                        tb_add_started_ms = None;
                                        tb_shift_x = 26.0;
                                    }
                                    NavigationEffect::SystemChanged => {
                                        taskbar_surface.invalidate();
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
                    scene_dirty = true;
                } else if !ev.left && mouse_down {
                    mouse_down = false;
                    soft_keyboard.end_drag();
                    wm.on_mouse_up();
                    for (_, engine) in warp_engines.iter_mut() {
                        engine.release();
                    }
                    scene_dirty = true;
                }

                if mouse_down {
                    let mut warp_control_drag = false;
                    if let Some(drag_id) = wm.window_at(cx, cy) {
                        if !wm.is_interaction_blocked(drag_id) {
                            if let Some((wx, wy, _ww, _wh, scroll)) = wm.get_window_rect(drag_id) {
                                let rel_x = cx - wx;
                                let rel_y = cy - wy;
                                let tb_h = if wm.is_focusable(drag_id) {
                                    baram_windowserver::window::title_bar_h() as i32
                                } else {
                                    0
                                };
                                if rel_y >= tb_h {
                                    for (wid, engine) in warp_engines.iter_mut() {
                                        if *wid == drag_id && engine.has_pointer_capture() {
                                            engine.set_scroll(scroll);
                                            if engine.pointer_move(rel_x, rel_y + scroll) {
                                                wm.set_content_dirty(drag_id);
                                                warp_control_drag = true;
                                                scene_dirty = true;
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !warp_control_drag && wm.has_pointer_capture() {
                        wm.on_mouse_drag(cx, cy);
                    }
                    scene_dirty = true;
                }

                dirty = true;
            }
        }
        (dirty, cursor_moved, scroll_input, launcher_scroll_input_changed)
}

