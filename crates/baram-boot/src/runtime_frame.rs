|| {

        // Scroll positions are sampled from absolute time. The backing
        // document is already rasterized; each sample only changes the source
        // offset used for the viewport copy.
        let transition_now_ns = ui_time_ms * 1_000_000;
        // Do not use the UI scheduler's bounded frame delta for motion. It is
        // intentionally capped under load and would stretch scroll/transition
        // durations; the hardware monotonic counter is not.
        let motion_now_ns = ui_clock
            .as_ref()
            .map(UiMonotonicClock::elapsed_ns)
            .unwrap_or(transition_now_ns);
        if wm.tick_scroll_animations(motion_now_ns) {
            scene_dirty = true;
            dirty = true;
        }
        // Window motion uses the un-clamped hardware monotonic counter. The
        // UI scheduler's 1–16ms frame delta intentionally smooths other UI
        // work, but must not stretch animation duration under load.
        if wm.tick_window_animations(motion_now_ns) {
            scene_dirty = true;
            dirty = true;
        }
        let launcher_scroll_changed =
            launcher_scroll_input_changed || app_launcher_scroll.tick(transition_now_ns);
        if launcher_scroll_changed {
            launcher_content_dirty = true;
            scene_dirty = true;
            dirty = true;
        }

        if show_app_launcher != launcher_target_prev {
            launcher_target_prev = show_app_launcher;
            launcher_anim_phase = if show_app_launcher { 1 } else { -1 };
            launcher_anim_started_ms = ui_time_ms;
            launcher_anim_elapsed_ms = 0;
            launcher_render_visible = true;
            scene_dirty = true;
            dirty = true;
        }
        if launcher_anim_phase != 0 {
            launcher_anim_elapsed_ms = ui_time_ms
                .saturating_sub(launcher_anim_started_ms)
                .min(u32::MAX as u64) as u32;
            let duration = if launcher_anim_phase < 0 { 100 } else { 200 };
            if launcher_anim_elapsed_ms >= duration {
                if launcher_anim_phase < 0 {
                    launcher_render_visible = false;
                    if launcher_cache_drop_after_close {
                        cached_launcher_layer = None;
                        launcher_cache_drop_after_close = false;
                    }
                    if pending_launcher_app.is_some() {
                        // Present one launcher-free frame before creating the
                        // new window, so close completion is visually clear.
                        launcher_app_open_at_ms = Some(ui_time_ms.saturating_add(16));
                    }
                }
                launcher_anim_phase = 0;
            }
            launcher_content_dirty = true;
            scene_dirty = true;
            dirty = true;
        }

        if launcher_app_open_at_ms.is_some_and(|ready| ui_time_ms >= ready) {
            launcher_app_open_at_ms = None;
            if let Some(app_name) = pending_launcher_app.take() {
                let nx = 100 + ((new_window_idx as i32 * 37) % 300);
                let ny = 60 + ((new_window_idx as i32 * 23) % 200);
                if open_app(
                    &app_name,
                    &app_entries,
                    &mut wm,
                    &mut warp_engines,
                    &mut html_engines,
                    nx,
                    ny,
                    400,
                    450,
                )
                .is_some()
                {
                    tb_add_progress = 0.0;
                    tb_add_started_ms = None;
                    tb_shift_x = 26.0;
                    new_window_idx = new_window_idx.wrapping_add(1);
                    scene_dirty = true;
                    dirty = true;
                }
            }
        }

        if ime_menu_closing {
            const IME_MENU_CLOSE_MS: u64 = 120;
            let started = ime_menu_close_started_ms.unwrap_or(ui_time_ms);
            let elapsed = ui_time_ms.saturating_sub(started);
            let t = (elapsed as f32 / IME_MENU_CLOSE_MS as f32).clamp(0.0, 1.0);
            let eased = t * t * (3.0 - 2.0 * t);
            ime_menu_opacity = ((1.0 - eased) * 255.0) as u8;
            if elapsed >= IME_MENU_CLOSE_MS {
                show_ime_menu = false;
                ime_menu_closing = false;
                ime_menu_close_started_ms = None;
                ime_menu_opacity = 255;
            }
            scene_dirty = true;
            dirty = true;
        }

        {
            let mut hovered_any =
                soft_keyboard.contains(cursor_x, cursor_y, screen.width(), screen.height());
            if soft_keyboard.set_hover(cursor_x, cursor_y, screen.width(), screen.height()) {
                scene_dirty = true;
                dirty = true;
            }
            if hovered_any {
                for (_, engine) in warp_engines.iter_mut() {
                    engine.clear_hover();
                }
                for (_, engine) in html_engines.iter_mut() {
                    engine.clear_hover();
                }
            }
            if !hovered_any {
                if let Some(hover_id) = wm.window_at(cursor_x, cursor_y) {
                    let scrolling = wm.is_scroll_animating(hover_id);
                    for (wid, engine) in warp_engines.iter_mut() {
                        if hover_id == *wid {
                            if scrolling {
                                engine.clear_hover();
                                hovered_any = true;
                                break;
                            }
                            if let Some((wx, wy, _ww, _wh, scroll)) = wm.get_window_rect(hover_id) {
                                let rel_x = cursor_x - wx;
                                let rel_y = cursor_y - wy;
                                let tb_h = baram_windowserver::window::title_bar_h() as i32;
                                let prev_hover = engine.hover_idx;
                                engine.set_scroll(scroll);
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
                            if scrolling {
                                engine.cancel_hover();
                                hovered_any = true;
                                break;
                            }
                            if let Some((wx, wy, _ww, _wh, scroll)) = wm.get_window_rect(hover_id) {
                                let rel_x = cursor_x - wx;
                                let rel_y = cursor_y - wy;
                                let tb_h = baram_windowserver::window::title_bar_h() as i32;
                                let previous = engine.hovered_node();
                                if rel_y >= tb_h {
                                    engine.set_scroll(scroll);
                                    engine.set_hover(rel_x, rel_y + scroll);
                                } else {
                                    engine.clear_hover();
                                }
                                if engine.hovered_node() != previous {
                                    if let Some((x0, y0, x1, y1)) = engine.window_damage() {
                                        wm.set_content_damage(hover_id, x0, y0, x1, y1);
                                    } else {
                                        wm.set_content_dirty(hover_id);
                                    }
                                    scene_dirty = true;
                                    dirty = true;
                                }
                                hovered_any = true;
                            }
                            break;
                        }
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

        // Absolute monotonic UI time: transitions derive their progress from
        // this clock, without a runtime-service call in the render hot path.
        let mut deferred_html_commands = alloc::vec::Vec::new();
        let runtime_window_count = wm.count();
        let keyboard_context_changed =
            if let Some((_, candidates, _)) = japanese_ime.conversion_view() {
                soft_keyboard.set_input_context(keyboard_language(input_mode), candidates)
            } else if let Some((_, candidates, _)) = pinyin_ime.conversion_view() {
                soft_keyboard.set_input_context(keyboard_language(input_mode), candidates)
            } else if soft_keyboard.is_open() {
                if let Some((_, candidates)) = japanese_ime.prediction_view() {
                    soft_keyboard.set_input_context(keyboard_language(input_mode), candidates)
                } else if let Some((_, candidates)) = pinyin_ime.prediction_view() {
                    soft_keyboard.set_input_context(keyboard_language(input_mode), candidates)
                } else {
                    soft_keyboard.set_input_context(keyboard_language(input_mode), &[])
                }
            } else {
                soft_keyboard.set_input_context(keyboard_language(input_mode), &[])
            };
        if keyboard_context_changed {
            scene_dirty = true;
            dirty = true;
        }
        if soft_keyboard.tick(motion_now_ns) {
            scene_dirty = true;
            dirty = true;
        }
        for (wid, engine) in html_engines.iter_mut() {
            engine.set_runtime_metrics(fps, runtime_window_count, key_ev_count, mouse_ev_count);
            if engine.tick(motion_now_ns) {
                if let Some((x0, y0, x1, y1)) = engine.window_damage() {
                    wm.set_content_damage(*wid, x0, y0, x1, y1);
                } else {
                    wm.set_content_dirty(*wid);
                }
                scene_dirty = true;
                dirty = true;
            }
            if let Some(command) = engine.last_command.take() {
                deferred_html_commands.push((command, engine.origin().to_string(), *wid));
            }
        }
        for (wid, engine) in warp_engines.iter_mut() {
            engine.set_runtime_metrics(fps, runtime_window_count, key_ev_count, mouse_ev_count);
            if engine.tick(motion_now_ns) {
                wm.set_content_dirty(*wid);
                scene_dirty = true;
                dirty = true;
            }
            if let Some(command) = engine.last_command.take() {
                deferred_html_commands.push((command, engine.origin().to_string(), *wid));
            }
        }
        for (command, origin, source_win_id) in deferred_html_commands {
            let previous_hud = display_state.hud_enabled;
            let nx = 100 + ((new_window_idx as i32 * 37) % 300);
            let ny = 60 + ((new_window_idx as i32 * 23) % 200);
            match handle_navigation(
                &command,
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
                    hud_damage_pending |= previous_hud != display_state.hud_enabled;
                    taskbar_surface.invalidate();
                    cached_launcher_layer = None;
                    bg_cache = None;
                    cached_wallpaper =
                        wallpaper_for_state(&display_state, screen.width(), screen.height());
                    prev_wallpaper_idx = display_state.wallpaper_index;
                }
                NavigationEffect::None => {}
            }
            scene_dirty = true;
            dirty = true;
        }

        frames = frames.wrapping_add(1);
        frames_since_tick = frames_since_tick.wrapping_add(1);
        if let Ok(now) = runtime::get_time() {
            let elapsed_ns = time_diff_ns(&start_time, &now);
            if elapsed_ns >= 1_000_000_000 {
                fps = frames_since_tick;
                frames_since_tick = 0;
                start_time = now;

                let total_min =
                    (now.hour() as i32) * 60 + (now.minute() as i32) + timezone_offset_minutes;
                let day_min = total_min.rem_euclid(24 * 60);
                let next_hh = (day_min / 60) as u8;
                let next_mm = (day_min % 60) as u8;
                let clock_changed = next_hh != clock_hh || next_mm != clock_mm;
                clock_hh = next_hh;
                clock_mm = next_mm;

                battery_poll_seconds = battery_poll_seconds.saturating_add(1);
                let mut battery_changed = false;
                if battery_poll_seconds >= 60 {
                    battery_poll_seconds = 0;
                    let next_battery = baram_iokit::battery::read_battery();
                    battery_changed =
                        next_battery.valid_percentage() != battery_info.valid_percentage();
                    battery_info = next_battery;
                }
                if clock_changed || battery_changed {
                    taskbar_surface.invalidate();
                }

                dirty = true;
                scene_dirty = true;
            }
        }

        if wm.take_order_changed() {
            scene_dirty = true;
            dirty = true;
        }

        for (wid, engine) in warp_engines.iter_mut() {
            if let Some((_, _, ww, wh, scroll)) = wm.get_window_rect(*wid) {
                let content_h = wh.saturating_sub(baram_windowserver::window::title_bar_h());
                engine.update(ww as i32, content_h as i32);
                engine.set_scroll(scroll);
                wm.clamp_window_scroll(*wid, engine.content_height);
            }
        }
        for (wid, engine) in html_engines.iter_mut() {
            if let Some((_, _, ww, wh, scroll)) = wm.get_window_rect(*wid) {
                let content_h = wh.saturating_sub(baram_windowserver::window::title_bar_h());
                engine.set_scroll(scroll);
                engine.update(ww as i32, content_h as i32);
                wm.clamp_window_scroll(*wid, engine.content_height);
            }
        }

        if tb_add_progress >= 0.0 {
            if tb_add_started_ms.is_none() {
                tb_add_started_ms = Some(ui_time_ms);
            }
            let started = tb_add_started_ms.unwrap_or(ui_time_ms);
            tb_add_progress = (ui_time_ms.saturating_sub(started) as f32
                / TASKBAR_ADD_ANIMATION_MS as f32)
                .min(1.0);
            let remaining = 1.0 - tb_add_progress;
            let eased = 1.0 - remaining * remaining * remaining;
            tb_shift_x = 26.0 * (1.0 - eased);
            dirty = true;
            scene_dirty = true;
        }

        if tb_remove_progress >= 0.0 {
            tb_remove_progress = (tb_remove_progress + 0.2).min(1.0);
            dirty = true;
            scene_dirty = true;
        }

        let scroll_animating = wm.has_scroll_animation();
        let taskbar_animating = tb_add_progress >= 0.0 || tb_remove_progress >= 0.0;
        let continuous_motion = scroll_animating
            || wm.has_window_animation()
            || app_launcher_scroll.is_animating()
            || launcher_anim_phase != 0
            || ime_menu_closing;
        // New scroll input is presented immediately. During easing, use a
        // short deadline instead of the normal 16 ms scene deadline.
        if dirty && ui_time_ms < next_present_ms && !cursor_moved && !scroll_input {
            deferred_dirty = true;
            return true;
        }

        if dirty {
            // The taskbar has to rasterize and flush the whole bottom strip.
            // Cap it at the normal 60 Hz cadence, while retaining the tighter
            // interval for lightweight scrolling and launcher motion.
            let present_interval_ms = if taskbar_animating {
                16
            } else if continuous_motion {
                4
            } else {
                16
            };
            next_present_ms = ui_time_ms.saturating_add(present_interval_ms);
            let is_resizing = wm.is_any_resizing() || wm.is_over_resize_handle(cursor_x, cursor_y);

            if scene_dirty {
                let (bx0, by0, bx1, by1) = wm.dirty_bbox(shadow_pad);

                let bg_valid =
                    bg_cache.is_some() && prev_wallpaper_idx == display_state.wallpaper_index;

                if !launcher_render_visible && (bx1 > bx0 || !bg_valid) {
                    cached_launcher_layer = None;
                }

                let taskbar_dirty = !taskbar_surface.is_valid()
                    || tb_add_progress >= 0.0
                    || tb_remove_progress >= 0.0
                    || tb_shift_x.abs() > 0.5
                    || wm.count() != prev_window_count
                    || wm.focused_id != prev_focused_id
                    || by1 > screen.height().saturating_sub(TASKBAR_H)
                    || !bg_valid;
                let taskbar_search_dirty = taskbar_surface.is_search_dirty();

                let launcher_changed = launcher_render_visible != prev_show_app_launcher;
                let launcher_needs_redraw =
                    launcher_changed || launcher_content_dirty || launcher_anim_phase != 0;
                let hud_dirty = display_state.hud_enabled && !taskbar_surface.is_valid();
                let ime_menu_changed = show_ime_menu != prev_show_ime_menu;
                let (ime_menu_x, ime_menu_y, ime_menu_w, ime_menu_h) = ime_menu_bounds(
                    screen.width(),
                    screen.height(),
                    battery_info.valid_percentage(),
                );
                let ime_menu_cache_dirty = show_ime_menu
                    && (ime_menu_changed
                        || !bg_valid
                        || (bx1 > bx0
                            && bx0 < (ime_menu_x + ime_menu_w + 54).max(0) as usize
                            && bx1 > (ime_menu_x - 54).max(0) as usize
                            && by0 < (ime_menu_y + ime_menu_h + 54).max(0) as usize
                            && by1 > (ime_menu_y - 54).max(0) as usize));
                let ime_menu_needs_redraw =
                    ime_menu_changed || ime_menu_cache_dirty || ime_menu_closing;
                if ime_menu_changed || ime_menu_cache_dirty {
                    cached_ime_menu_layer = None;
                }

                let taskbar_only = taskbar_dirty
                    && bx1 <= bx0
                    && !hud_dirty
                    && wm.count() == prev_window_count
                    && wm.focused_id == prev_focused_id
                    && prev_wallpaper_idx == display_state.wallpaper_index
                    && bg_cache.is_some()
                    && !launcher_render_visible
                    && !launcher_changed
                    && !ime_menu_changed;

                let launcher_only_redraw = (launcher_anim_phase != 0 || launcher_scroll_changed)
                    && launcher_needs_redraw
                    && cached_launcher_layer.is_some()
                    && bx1 <= bx0
                    && !taskbar_dirty
                    && !hud_dirty
                    && bg_valid;

                let launcher_cursor_separate = cursor_moved
                    && launcher_needs_redraw
                    && bx1 <= bx0
                    && !taskbar_dirty
                    && !hud_dirty
                    && bg_valid;

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
                let keyboard_damage = soft_keyboard.take_damage(w, h);
                let tb_y = h.saturating_sub(TASKBAR_H);
                let hud_y0 = tb_y.saturating_sub(44);
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
                } else if cursor_moved && !launcher_cursor_separate {
                    (bx0.min(cx0), by0.min(cy0), bx1.max(cx1), by1.max(cy1))
                } else {
                    (bx0, by0, bx1, by1)
                };
                if let Some((x0, y0, x1, y1)) = keyboard_damage {
                    fx0 = fx0.min(x0.max(0) as usize);
                    fy0 = fy0.min(y0.max(0) as usize);
                    fx1 = fx1.max(x1.max(0) as usize).min(w);
                    fy1 = fy1.max(y1.max(0) as usize).min(h);
                }
                if taskbar_dirty && !taskbar_only {
                    fx0 = 0;
                    fy0 = fy0.min(tb_y);
                    fx1 = w;
                    fy1 = h;
                }
                if taskbar_search_dirty && !taskbar_dirty {
                    fx0 = fx0.min(0);
                    fy0 = fy0.min(tb_y);
                    fx1 = fx1.max(226.min(w));
                    fy1 = fy1.max(h);
                }
                if hud_dirty {
                    fx0 = 0;
                    fy0 = fy0.min(tb_y.saturating_sub(44));
                    fx1 = w;
                }
                if launcher_needs_redraw {
                    let grid_h = 3 * 88usize;
                    let grid_y = h.saturating_sub(TASKBAR_H + grid_h + 16);
                    let panel_x = 12usize;
                    let panel_y = grid_y.saturating_sub(8);
                    let panel_w = 4 * (52 + 16) + 16;
                    let panel_h = grid_h + 16;
                    let pad = 54usize;
                    fx0 = fx0.min(panel_x.saturating_sub(pad));
                    fy0 = fy0.min(panel_y.saturating_sub(pad));
                    fx1 = fx1.max((panel_x + panel_w + pad).min(w));
                    fy1 = fy1.max((panel_y + panel_h + pad).min(h));
                }
                if !bg_valid {
                    fx0 = 0;
                    fy0 = 0;
                    fx1 = w;
                    fy1 = h;
                }
                let ime_conversion_visible =
                    japanese_ime.conversion.is_some() || pinyin_ime.conversion.is_some();
                if ime_menu_needs_redraw {
                    let (menu_x, menu_y, menu_w, menu_h) =
                        (ime_menu_x, ime_menu_y, ime_menu_w, ime_menu_h);
                    let pad = 28usize;
                    fx0 = fx0.min((menu_x.max(0) as usize).saturating_sub(pad));
                    fy0 = fy0.min((menu_y.max(0) as usize).saturating_sub(pad));
                    fx1 = fx1.max((menu_x + menu_w).max(0) as usize + pad).min(w);
                    fy1 = fy1.max((menu_y + menu_h).max(0) as usize + pad).min(h);
                }
                if ime_conversion_visible || prev_ime_conversion_visible {
                    fx0 = 0;
                    fy0 = fy0.min(tb_y.saturating_sub(76));
                    fx1 = w;
                    fy1 = fy1.max(tb_y);
                }
                let (ime_reading, ime_candidates, ime_selected) =
                    if let Some((reading, candidates, selected)) = japanese_ime.conversion_view() {
                        (Some(reading), candidates, selected)
                    } else if let Some((reading, candidates, selected)) =
                        pinyin_ime.conversion_view()
                    {
                        (Some(reading), candidates, selected)
                    } else {
                        (None, &[][..], 0)
                    };
                layer.push_clip(fx0, fy0, fx1, fy1);

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
                    taskbar_dirty,
                    tb_add_progress,
                    tb_remove_progress,
                    tb_shift_x,
                    display_state.hud_enabled,
                    &mut bg_cache,
                    bg_valid,
                    launcher_render_visible,
                    &app_list,
                    &app_icon_list,
                    hover_apps_icon,
                    hover_keyboard_icon,
                    hover_ime_icon,
                    ime_hover_dirty,
                    app_search_focused,
                    &app_search_query,
                    baram_windowserver::text_cursor::visible(ui_time_ms * 1_000_000),
                    app_launcher_scroll.position.max(0) as usize,
                    launcher_anim_phase,
                    launcher_anim_elapsed_ms,
                    launcher_scroll_changed,
                    launcher_only_redraw,
                    taskbar_only,
                    clock_hh,
                    clock_mm,
                    battery_info.valid_percentage(),
                    show_ime_menu,
                    &mut soft_keyboard,
                    ime_menu_opacity,
                    ime_menu_selection(input_mode),
                    ime_reading,
                    ime_candidates,
                    ime_selected,
                );
                layer.pop_clip();

                let hud_redraw_separate =
                    hud_damage_pending && !(fx0 == 0 && fy0 <= hud_y0 && fx1 == w && fy1 >= tb_y);
                if hud_redraw_separate {
                    layer.push_clip(0, hud_y0, w, tb_y);
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
                        tb_add_progress,
                        tb_remove_progress,
                        tb_shift_x,
                        display_state.hud_enabled,
                        &mut bg_cache,
                        true,
                        launcher_render_visible,
                        &app_list,
                        &app_icon_list,
                        hover_apps_icon,
                        hover_keyboard_icon,
                        hover_ime_icon,
                        ime_hover_dirty,
                        app_search_focused,
                        &app_search_query,
                        baram_windowserver::text_cursor::visible(ui_time_ms * 1_000_000),
                        app_launcher_scroll.position.max(0) as usize,
                        launcher_anim_phase,
                        launcher_anim_elapsed_ms,
                        launcher_scroll_changed,
                        false,
                        false,
                        clock_hh,
                        clock_mm,
                        battery_info.valid_percentage(),
                        show_ime_menu,
                        &mut soft_keyboard,
                        ime_menu_opacity,
                        ime_menu_selection(input_mode),
                        ime_reading,
                        ime_candidates,
                        ime_selected,
                    );
                    layer.pop_clip();
                }

                prev_window_count = wm.count();
                prev_focused_id = wm.focused_id;
                prev_show_ime_menu = show_ime_menu;
                prev_ime_conversion_visible = ime_conversion_visible;
                ime_hover_dirty = false;

                if tb_add_progress >= 1.0 {
                    tb_add_progress = -1.0;
                    tb_add_started_ms = None;
                    tb_shift_x = 0.0;
                }
                if tb_remove_progress >= 1.0 {
                    tb_remove_progress = -1.0;
                }

                for y in fy0..fy1 {
                    let s = y * w + fx0;
                    let e = y * w + fx1;
                    cached_scene[s..e].copy_from_slice(&layer.buf_ref()[s..e]);
                }
                if hud_redraw_separate {
                    for y in hud_y0..tb_y {
                        let s = y * w;
                        let e = s + w;
                        cached_scene[s..e].copy_from_slice(&layer.buf_ref()[s..e]);
                    }
                }
                hud_damage_pending = false;
                scene_dirty = false;
                launcher_content_dirty = false;
                wm.clear_pending_damage();

                if launcher_cursor_separate {
                    let buf = layer.buf_mut();
                    for y in cy0..cy1 {
                        let s = y * w + cx0;
                        let e = y * w + cx1;
                        buf[s..e].copy_from_slice(&cached_scene[s..e]);
                    }
                }

                cursor::draw_cursor_into_layer(
                    &mut layer,
                    cursor_x,
                    cursor_y,
                    is_resizing,
                    display_state.pointer_size,
                );
                prev_show_app_launcher = launcher_render_visible;
                let fw = fx1 - fx0;
                let fh = fy1 - fy0;
                let full_area = w * h;
                if taskbar_only {
                    layer.flush_rect(&mut screen, 0, tb_y, w, h);
                    layer.flush_rect(&mut screen, cx0, cy0, cx1, cy1);
                } else if !bg_valid || fw * fh >= full_area * 3 / 4 {
                    layer.flush(&mut screen);
                } else {
                    layer.flush_rect(&mut screen, fx0, fy0, fx1, fy1);
                    if launcher_cursor_separate {
                        layer.flush_rect(&mut screen, cx0, cy0, cx1, cy1);
                    }
                    if hud_redraw_separate {
                        layer.flush_rect(&mut screen, 0, hud_y0, w, tb_y);
                    }
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
        false
}

