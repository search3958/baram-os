|| {

        // Moving inside reuses the rendered SVG; crossing the edge repaints
        // just this 20px icon, not the taskbar surface.
        let (ime_x, ime_y, ime_w, ime_h) =
            ime_button_bounds(screen.width(), battery_info.valid_percentage());
        let ime_y = screen.height() as i32 - TASKBAR_H as i32 + ime_y;
        let keyboard_x = ime_x - ime_w - 12;
        let next_hover_ime_icon = cursor_x >= ime_x
            && cursor_x < ime_x + ime_w + 10
            && cursor_y >= ime_y
            && cursor_y < ime_y + ime_h;
        let next_hover_keyboard_icon = cursor_x >= keyboard_x
            && cursor_x < ime_x
            && cursor_y >= ime_y
            && cursor_y < ime_y + ime_h;
        if next_hover_ime_icon != hover_ime_icon {
            hover_ime_icon = next_hover_ime_icon;
            ime_hover_dirty = true;
            dirty = true;
            scene_dirty = true;
        }
        if next_hover_keyboard_icon != hover_keyboard_icon {
            hover_keyboard_icon = next_hover_keyboard_icon;
            ime_hover_dirty = true;
            dirty = true;
            scene_dirty = true;
        }

        if keyboard_click {
            keyboard_click = false;
            let cx = cursor_x;
            let cy = cursor_y;
            let sh = screen.height();
            let search_y = sh as i32 - TASKBAR_H as i32 + (TASKBAR_H as i32 - 40) / 2;
            let on_search = cx >= 12 && cx < 202 && cy >= search_y && cy < search_y + 40;

            if soft_keyboard.contains(cx, cy, screen.width(), screen.height()) {
                if let Some(key) = soft_keyboard.click(cx, cy, screen.width(), screen.height()) {
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
            } else if show_ime_menu && !ime_menu_closing {
                if let Some(selection) = ime_menu_mode_at(
                    cx,
                    cy,
                    screen.width(),
                    screen.height(),
                    battery_info.valid_percentage(),
                ) {
                    input_mode = input_mode_for_menu_selection(selection);
                    japanese_ime.reset();
                    hangul_ime.reset();
                    pinyin_ime.reset();
                    taskbar_surface.invalidate();
                }
                ime_menu_closing = true;
                ime_menu_close_started_ms = Some(ui_time_ms);
                scene_dirty = true;
                dirty = true;
            } else if show_app_launcher && on_search {
                app_search_focused = true;
                taskbar_surface.invalidate_search();
                scene_dirty = true;
            } else if show_app_launcher {
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
                let on_launcher_panel = cx >= 12
                    && cx < (12 + grid_w + 16) as i32
                    && cy >= grid_y.saturating_sub(8) as i32
                    && cy < (grid_y.saturating_sub(8) + grid_h.max(40) + 16) as i32;
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
                    rebuild_filtered_apps(
                        &app_entries,
                        "",
                        &mut app_list,
                        &mut app_name_list,
                        &mut app_icon_list,
                    );
                    show_app_launcher = false;
                    launcher_cache_drop_after_close = true;
                } else if on_launcher_panel {
                    app_search_focused = false;
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
                    show_app_launcher = false;
                }
                taskbar_surface.invalidate();
                scene_dirty = true;
            } else if cy >= sh as i32 - TASKBAR_H as i32 {
                let (ime_x, ime_y, ime_w, ime_h) =
                    ime_button_bounds(screen.width(), battery_info.valid_percentage());
                let ime_y = sh as i32 - TASKBAR_H as i32 + ime_y;
                let keyboard_x = ime_x - ime_w - 12;
                if cx >= keyboard_x && cx < keyboard_x + ime_w && cy >= ime_y && cy < ime_y + ime_h
                {
                    soft_keyboard.toggle();
                    scene_dirty = true;
                    dirty = true;
                    return true;
                }
                if cx >= ime_x && cx < ime_x + ime_w + 10 && cy >= ime_y && cy < ime_y + ime_h {
                    show_ime_menu = true;
                    ime_menu_closing = false;
                    ime_menu_close_started_ms = None;
                    ime_menu_opacity = 255;
                    scene_dirty = true;
                    dirty = true;
                    return true;
                }
                let apps_icon_x = 12i32;
                let apps_icon_size = 190i32;
                let apps_icon_y = search_y;
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
                    let btn_y = (sh as usize).saturating_sub(TASKBAR_H) + (TASKBAR_H - 40) / 2;
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
                        _ => {}
                    }
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

                                    if !engine
                                        .last_command
                                        .as_ref()
                                        .is_some_and(|cmd| cmd.starts_with("files-upload://"))
                                    {
                                        if let Some(cmd) = engine.last_command.take() {
                                            let is_hud_command = baram_bsd::uri::parse(&cmd)
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
                                                    hud_damage_pending |=
                                                        previous_hud != display_state.hud_enabled;
                                                } else {
                                                    wm.set_all_dirty();
                                                    taskbar_surface.invalidate();
                                                    cached_launcher_layer = None;
                                                    bg_cache = None;
                                                }
                                                scene_dirty = true;
                                            }
                                            if let Some(parsed) = baram_bsd::uri::parse(&cmd) {
                                                if parsed.path.starts_with("display/wallpaper") {
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
                        if let Some((wx, wy, ww, wh, scroll)) = wm.get_window_rect(clicked_id) {
                            let rel_x = cx - wx;
                            let rel_y = cy - wy;
                            let tb_h = baram_windowserver::window::title_bar_h() as i32;
                            if rel_y >= tb_h {
                                engine.set_scroll(scroll);
                                engine.set_runtime_metrics(
                                    fps,
                                    wm.count(),
                                    key_ev_count,
                                    mouse_ev_count,
                                );
                                engine.click(rel_x, rel_y + scroll);
                                engine.update(ww as i32, wh.saturating_sub(tb_h as usize) as i32);
                                if let Some(target) = engine.take_scroll_request() {
                                    wm.set_window_scroll(clicked_id, target);
                                }
                                html_command = engine
                                    .last_command
                                    .take()
                                    .map(|command| (command, engine.origin().to_string(), *wid));
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
            dirty = true;
        }
    false
}
