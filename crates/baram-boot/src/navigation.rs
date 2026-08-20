fn draw_boot_logo(screen: &mut Screen) {
    const LOGO_PNG: &[u8] = include_bytes!("../../../files/data/logo.png");
    let Ok((header, pixels)) = png_decoder::decode(LOGO_PNG) else {
        screen.clear(Color::BLACK);
        return;
    };
    let img_w = header.width as usize;
    let img_h = header.height as usize;
    let screen_w = screen.width();
    let screen_h = screen.height();
    let mut logo_layer = LayerSystem::new(screen_w, screen_h);
    logo_layer.clear(Color::BLACK);
    let origin_x = screen_w.saturating_sub(img_w) / 2;
    let origin_y = screen_h.saturating_sub(img_h) / 2;
    let draw_w = img_w.min(screen_w);
    let draw_h = img_h.min(screen_h);
    let source_x = img_w.saturating_sub(screen_w) / 2;
    let source_y = img_h.saturating_sub(screen_h) / 2;
    let buffer = logo_layer.buf_mut();
    for y in 0..draw_h {
        let dst_row = (origin_y + y) * screen_w + origin_x;
        let src_row = (source_y + y) * img_w + source_x;
        for x in 0..draw_w {
            let pixel = pixels[src_row + x];
            buffer[dst_row + x] = Color::rgb(pixel[0], pixel[1], pixel[2]).0;
        }
    }
    logo_layer.flush(screen);
}

#[derive(Clone, Copy, PartialEq)]
enum NavigationEffect {
    None,
    SystemChanged,
    AppOpened,
}

struct PendingOsPermission {
    command: alloc::string::String,
    app_hash: alloc::string::String,
    dialog_win_id: WinId,
    source_win_id: Option<WinId>,
}

struct PendingFileDialog {
    dialog_win_id: WinId,
    source_win_id: WinId,
    var_name: alloc::string::String,
}

fn rebuild_filtered_apps(
    entries: &[AppEntry],
    query: &str,
    titles: &mut alloc::vec::Vec<alloc::string::String>,
    names: &mut alloc::vec::Vec<alloc::string::String>,
    icons: &mut alloc::vec::Vec<alloc::string::String>,
) {
    titles.clear();
    names.clear();
    icons.clear();
    let needle = query.trim().to_ascii_lowercase();
    for entry in entries {
        let matches = needle.is_empty()
            || entry.title.to_ascii_lowercase().contains(&needle)
            || entry.name.to_ascii_lowercase().contains(&needle)
            || entry
                .tags
                .iter()
                .any(|tag| tag.to_ascii_lowercase().contains(&needle));
        if matches {
            titles.push(entry.title.clone());
            names.push(entry.name.clone());
            icons.push(entry.icon.clone());
        }
    }
}

/// Single OS-level text injection path shared by hardware and software keys.
/// It owns target selection, IME composition, layout refresh, and damage.
fn dispatch_text_input_key(
    key: SoftKey,
    input_mode: InputMode,
    japanese_ime: &mut JapaneseIme,
    hangul_ime: &mut HangulIme,
    pinyin_ime: &mut PinyinIme,
    app_search_focused: bool,
    app_search_query: &mut alloc::string::String,
    app_entries: &[AppEntry],
    app_list: &mut alloc::vec::Vec<alloc::string::String>,
    app_name_list: &mut alloc::vec::Vec<alloc::string::String>,
    app_icon_list: &mut alloc::vec::Vec<alloc::string::String>,
    app_launcher_scroll: &mut SmoothScroll,
    show_app_launcher: &mut bool,
    cached_launcher_layer: &mut Option<alloc::vec::Vec<u32>>,
    launcher_content_dirty: &mut bool,
    taskbar_surface: &mut TaskbarSurface,
    wm: &mut WindowManager,
    warp_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::warp::WarpEngine)>,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
) -> bool {
    let candidate_edit = match key {
        SoftKey::Candidate(index) => match input_mode {
            InputMode::Hiragana => japanese_ime.commit_candidate(index),
            InputMode::Pinyin => pinyin_ime.commit_candidate(index),
            _ => None,
        },
        _ => None,
    };
    let byte = match key {
        SoftKey::Character(c) => c,
        SoftKey::Backspace => 0x08,
        SoftKey::Enter => b'\n',
        SoftKey::Candidate(_) => 0,
        SoftKey::Close => return false,
    };

    if app_search_focused {
        if byte == b'\n' || byte == b'\r' {
            return false;
        }
        if let Some((text, replace_chars)) = candidate_edit.as_ref() {
            for _ in 0..*replace_chars {
                app_search_query.pop();
            }
            app_search_query.push_str(text);
        } else if let Some((text, replace_chars)) =
            ime_edit_for_key(input_mode, japanese_ime, hangul_ime, pinyin_ime, byte)
        {
            for _ in 0..replace_chars {
                app_search_query.pop();
            }
            app_search_query.push_str(&text);
        } else if byte == 0x08 || byte == 0x7f {
            app_search_query.pop();
        } else if (0x20..=0x7e).contains(&byte) {
            app_search_query.push(byte as char);
        }
        rebuild_filtered_apps(
            app_entries,
            app_search_query,
            app_list,
            app_name_list,
            app_icon_list,
        );
        app_launcher_scroll.set_max(app_launcher_scroll_max(app_list.len()));
        *show_app_launcher = true;
        *cached_launcher_layer = None;
        *launcher_content_dirty = true;
        taskbar_surface.invalidate_search();
        return true;
    }

    let Some(focused) = wm.focused_id else {
        return false;
    };
    if wm.is_interaction_blocked(focused) {
        return false;
    }

    for (wid, engine) in warp_engines.iter_mut() {
        if *wid != focused || engine.focused_input_var.is_empty() {
            continue;
        }
        if let Some((text, replace_chars)) = candidate_edit.as_ref() {
            engine.handle_text(text, *replace_chars);
        } else if byte == b'\n' || byte == b'\r' {
            engine.handle_text("\n", 0);
        } else if let Some((text, replace_chars)) =
            ime_edit_for_key(input_mode, japanese_ime, hangul_ime, pinyin_ime, byte)
        {
            engine.handle_text(&text, replace_chars);
        } else {
            engine.handle_key(byte);
        }
        if let Some((_, _, ww, wh, _)) = wm.get_window_rect(*wid) {
            let content_h =
                (wh as i32).saturating_sub(baram_windowserver::window::title_bar_h() as i32);
            engine.update(ww as i32, content_h);
            wm.clamp_window_scroll(*wid, engine.content_height);
        }
        wm.set_content_dirty(*wid);
        return true;
    }

    for (wid, engine) in html_engines.iter_mut() {
        if *wid != focused || !engine.has_focused_input() {
            continue;
        }
        if let Some((text, replace_chars)) = candidate_edit.as_ref() {
            engine.handle_text(text, *replace_chars);
        } else if byte == b'\n' || byte == b'\r' {
            engine.handle_text("\n", 0);
        } else if let Some((text, replace_chars)) =
            ime_edit_for_key(input_mode, japanese_ime, hangul_ime, pinyin_ime, byte)
        {
            engine.handle_text(&text, replace_chars);
        } else {
            engine.handle_key(byte);
        }
        if let Some((_, _, ww, wh, scroll)) = wm.get_window_rect(*wid) {
            let content_h = wh.saturating_sub(baram_windowserver::window::title_bar_h());
            engine.set_scroll(scroll);
            engine.update(ww as i32, content_h as i32);
            wm.clamp_window_scroll(*wid, engine.content_height());
        }
        wm.set_content_dirty(*wid);
        return true;
    }
    false
}

fn app_launcher_scroll_max(app_count: usize) -> i32 {
    const COLS: usize = 4;
    const VISIBLE_ROWS: usize = 3;
    const CELL_H: usize = 88;
    let rows = (app_count + COLS - 1) / COLS;
    rows.saturating_sub(VISIBLE_ROWS).saturating_mul(CELL_H) as i32
}

fn open_app(
    name: &str,
    app_entries: &[AppEntry],
    wm: &mut WindowManager,
    warp_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::warp::WarpEngine)>,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    x: i32,
    y: i32,
    w: usize,
    h: usize,
) -> Option<WinId> {
    let entry = app_entries.iter().find(|entry| entry.name == name)?;
    let is_unsupported_ui_script = entry.app_type.starts_with("uiscript");
    let window_title = if is_unsupported_ui_script {
        "UI Script（非対応）"
    } else {
        entry.title.as_str()
    };
    let win_id = wm.add(window_title, x, y, w, h);
    if entry.app_type.starts_with("warp-4") {
        wm.set_warp4_theme(win_id, true);
    }
    if is_unsupported_ui_script {
        wm.set_icon(win_id, "redstar.png");
    } else {
        wm.set_icon(win_id, &entry.icon);
    }
    let content_h = h.saturating_sub(baram_windowserver::window::title_bar_h());

    if entry.app_type.starts_with("warp-4") {
        let mut engine = baram_windowserver::warp::WarpEngine::new_warp4(&entry.name);
        engine.update(w as i32, content_h as i32);
        warp_engines.push((win_id, engine));
    } else if entry.app_type.starts_with("warp-3") {
        let mut engine = baram_windowserver::html::HtmlEngine::new_warp3(&entry.name);
        engine.update(w as i32, content_h as i32);
        html_engines.push((win_id, engine));
    } else if entry.app_type.starts_with("html") {
        let (html, css) = baram_bsd::app::load_html_document(&entry.name);
        let mut engine = baram_windowserver::html::HtmlEngine::new(&html, &css);
        engine.set_origin(&entry.name);
        engine.update(w as i32, content_h as i32);
        html_engines.push((win_id, engine));
    } else if is_unsupported_ui_script {
        // Keep the file association so opening an existing .u1 file still
        // reaches BaramOS, but do not parse or execute the legacy format.
        // Showing this as a normal Warp 3 window also gives it the same
        // close/focus behavior as the other application dialogs.
        let config = "version = 3\nscreen = main\nname = UI Script（非対応）";
        let main = alloc::format!(
            "config {{ title(\"UI Script（非対応）\") }}\nhead {{ text(\"UI Scriptはサポートされていません\") }}\ntext {{ text(\"BaramOSではUI Scriptのサポートを終了しました。\") }}\ntext {{ text(\"このファイルは開けません: {}\") }}",
            entry.name
        );
        let mut engine = baram_windowserver::html::HtmlEngine::new_embedded_warp3(
            "unsupported-ui-script",
            &[("config.ini", config), ("main.w3u", &main)],
        );
        engine.update(w as i32, content_h as i32);
        html_engines.push((win_id, engine));
    } else {
        let source = baram_bsd::app::load_app_source(&entry.name);
        let mut engine = baram_windowserver::warp::WarpEngine::new(&source);
        engine.set_origin(&entry.name);
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
    display_state: &mut baram_bsd::uri::DisplayState,
    origin: &str,
    source_win_id: WinId,
    pending_permission: &mut Option<PendingOsPermission>,
    pending_file_dialog: &mut Option<PendingFileDialog>,
    x: i32,
    y: i32,
) -> NavigationEffect {
    if command.starts_with("files-upload://") {
        return handle_file_dialog_command(
            command,
            origin,
            source_win_id,
            pending_file_dialog,
            wm,
            warp_engines,
            html_engines,
            x,
            y,
        );
    }
    if let Some(decision) = command.strip_prefix("security://") {
        let Some(pending) = pending_permission.take() else {
            return NavigationEffect::None;
        };
        if source_win_id != pending.dialog_win_id || origin != "ospermission.w4a" {
            *pending_permission = Some(pending);
            return NavigationEffect::None;
        }
        wm.remove(pending.dialog_win_id);
        html_engines.retain(|(wid, _)| *wid != pending.dialog_win_id);
        wm.set_interaction_blocked(None);
        if decision == "always" {
            baram_bsd::security::allow_always(&pending.app_hash);
        }
        let effect = if decision == "once" || decision == "always" {
            execute_os_setting(&pending.command, wm, html_engines, display_state)
        } else {
            NavigationEffect::None
        };
        if let Some(source_win_id) = pending.source_win_id {
            if let Some((_, engine)) = html_engines
                .iter_mut()
                .find(|(wid, _)| *wid == source_win_id)
            {
                engine.complete_warp3_command();
            }
        }
        return effect;
    }

    if let Some(name) = baram_bsd::app::parse_app_uri(command) {
        if open_app(
            name,
            app_entries,
            wm,
            warp_engines,
            html_engines,
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

    if baram_bsd::security::is_settings_write(command) {
        let had_pending = pending_permission.is_some();
        if authorize_os_setting(
            command,
            origin,
            wm,
            html_engines,
            pending_permission,
            Some(source_win_id),
            x,
            y,
        ) {
            return execute_os_setting(command, wm, html_engines, display_state);
        }
        return if !had_pending && pending_permission.is_some() {
            NavigationEffect::AppOpened
        } else {
            NavigationEffect::None
        };
    }

    NavigationEffect::None
}

fn handle_file_dialog_command(
    command: &str,
    _origin: &str,
    source_win_id: WinId,
    pending: &mut Option<PendingFileDialog>,
    wm: &mut WindowManager,
    _warp_engines: &mut Vec<(WinId, baram_windowserver::warp::WarpEngine)>,
    _html_engines: &mut Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    x: i32,
    y: i32,
) -> NavigationEffect {
    let command = command.trim();
    if let Some(query) = command.strip_prefix("files-upload://open?") {
        if pending.is_some() {
            return NavigationEffect::None;
        }
        let Some(var_name) =
            file_dialog_query(query, "var").filter(|name| is_safe_file_dialog_name(name))
        else {
            return NavigationEffect::None;
        };
        let Some(request_path) = file_dialog_query(query, "path") else {
            return NavigationEffect::None;
        };
        let Some(path) = baram_bsd::vfs::parse_files_uri(request_path) else {
            return NavigationEffect::None;
        };
        let dialog_win_id = wm.add("ファイルをアップロード", x, y, 560, 372);
        wm.set_icon(dialog_win_id, "files.png");
        wm.configure_special(dialog_win_id, false, true, true);
        wm.open_file_dialog(dialog_win_id, &path);
        wm.set_interaction_blocked(Some(source_win_id));
        *pending = Some(PendingFileDialog {
            dialog_win_id,
            source_win_id,
            var_name: var_name.into(),
        });
        return NavigationEffect::AppOpened;
    }
    NavigationEffect::None
}

fn file_dialog_query<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        (name == key).then_some(value)
    })
}

fn is_safe_file_dialog_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn close_file_dialog(pending: &mut Option<PendingFileDialog>, wm: &mut WindowManager) {
    let Some(state) = pending.take() else {
        return;
    };
    wm.remove(state.dialog_win_id);
    wm.close_file_dialog();
    wm.set_interaction_blocked(None);
}

fn cancel_file_dialog_for_closed_window(
    closed_win_id: WinId,
    wm: &mut WindowManager,
    pending: &mut Option<PendingFileDialog>,
) {
    let should_cancel = pending.as_ref().is_some_and(|state| {
        state.dialog_win_id == closed_win_id || state.source_win_id == closed_win_id
    });
    if should_cancel {
        close_file_dialog(pending, wm);
    }
}

fn handle_native_file_dialog_click(
    clicked_id: WinId,
    cx: i32,
    cy: i32,
    wm: &mut WindowManager,
    pending: &mut Option<PendingFileDialog>,
    warp_engines: &mut Vec<(WinId, baram_windowserver::warp::WarpEngine)>,
) -> bool {
    if !wm.is_file_dialog(clicked_id) || wm.is_interaction_blocked(clicked_id) {
        return false;
    }
    let Some((wx, wy, _ww, _wh, _scroll)) = wm.get_window_rect(clicked_id) else {
        return false;
    };
    let rel_x = cx - wx;
    let rel_y = cy - wy;
    let action = wm.file_dialog_click(clicked_id, rel_x, rel_y);
    match action {
        NativeFileDialogAction::None | NativeFileDialogAction::Changed => true,
        NativeFileDialogAction::Cancel => {
            close_file_dialog(pending, wm);
            true
        }
        NativeFileDialogAction::Confirm => {
            let Some(state) = pending.as_ref() else {
                return true;
            };
            if state.dialog_win_id != clicked_id {
                return true;
            }
            let Some(file_path) = wm.file_dialog_selected_path(clicked_id) else {
                return true;
            };
            let content =
                alloc::string::String::from_utf8_lossy(&baram_bsd::vfs::read_file(&file_path))
                    .into_owned();
            let source_win_id = state.source_win_id;
            let var_name = state.var_name.clone();
            if let Some((_, engine)) = warp_engines
                .iter_mut()
                .find(|(wid, _)| *wid == source_win_id)
            {
                engine.set_state_value(&var_name, &content);
                engine.set_text("file-content", &content);
                let display_path = file_path
                    .strip_prefix("files/")
                    .map_or(file_path.as_str(), |path| path);
                engine.set_text("file-path", &format!("files://{}", display_path));
                wm.set_content_dirty(source_win_id);
            }
            close_file_dialog(pending, wm);
            true
        }
    }
}

fn execute_os_setting(
    command: &str,
    wm: &mut WindowManager,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    display_state: &mut baram_bsd::uri::DisplayState,
) -> NavigationEffect {
    if !baram_bsd::uri::execute(command, display_state) {
        return NavigationEffect::None;
    }
    for (_, engine) in html_engines.iter_mut() {
        engine.refresh_config();
    }
    wm.set_all_dirty();
    NavigationEffect::SystemChanged
}

fn authorize_os_setting(
    command: &str,
    origin: &str,
    wm: &mut WindowManager,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    pending_permission: &mut Option<PendingOsPermission>,
    source_win_id: Option<WinId>,
    x: i32,
    y: i32,
) -> bool {
    if !baram_bsd::security::is_settings_write(command) {
        return false;
    }
    let Some(hash) = baram_bsd::security::app_hash(origin) else {
        return false;
    };
    if baram_bsd::security::is_always_allowed(&hash) {
        return true;
    }
    if pending_permission.is_some() {
        return false;
    }

    let dialog_win_id = wm.add("操作体系設定の変更", x, y, 520, 360);
    wm.set_icon(dialog_win_id, "redstar.png");
    let mut dialog = baram_windowserver::html::HtmlEngine::new_warp4("ospermission.w4a");
    dialog.set_warp3_text("app-name", &alloc::format!("アプリ: {origin}"));
    dialog.set_warp3_text("request-path", command);
    dialog.update(
        520,
        330usize.saturating_sub(baram_windowserver::window::title_bar_h()) as i32,
    );
    html_engines.push((dialog_win_id, dialog));
    if let Some(source_win_id) = source_win_id {
        wm.set_interaction_blocked(Some(source_win_id));
        if let Some((_, engine)) = html_engines
            .iter_mut()
            .find(|(wid, _)| *wid == source_win_id)
        {
            engine.hold_warp3_command();
        }
    }
    *pending_permission = Some(PendingOsPermission {
        command: command.into(),
        app_hash: hash,
        dialog_win_id,
        source_win_id,
    });
    false
}

fn cancel_permission_for_closed_window(
    closed_win_id: WinId,
    wm: &mut WindowManager,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    pending_permission: &mut Option<PendingOsPermission>,
) {
    let should_cancel = pending_permission.as_ref().is_some_and(|pending| {
        pending.dialog_win_id == closed_win_id || pending.source_win_id == Some(closed_win_id)
    });
    if !should_cancel {
        return;
    }
    let Some(pending) = pending_permission.take() else {
        return;
    };
    if pending.dialog_win_id != closed_win_id {
        wm.remove(pending.dialog_win_id);
        html_engines.retain(|(wid, _)| *wid != pending.dialog_win_id);
    }
    wm.set_interaction_blocked(None);
    if let Some(source_win_id) = pending.source_win_id {
        if source_win_id != closed_win_id {
            if let Some((_, engine)) = html_engines
                .iter_mut()
                .find(|(wid, _)| *wid == source_win_id)
            {
                engine.complete_warp3_command();
            }
        }
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

