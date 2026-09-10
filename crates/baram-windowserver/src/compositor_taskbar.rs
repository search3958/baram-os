fn draw_taskbar_glyph(
    layer: &mut LayerSystem,
    data: &[u8],
    glyph_w: i32,
    glyph_h: i32,
    x: usize,
    top: i32,
    color: Color,
) {
    let w = layer.width();
    let h = layer.height();
    let buf = layer.buf_mut();
    for row in 0..glyph_h {
        let py = top + row;
        if py < 0 || py >= h as i32 {
            continue;
        }
        for col in 0..glyph_w {
            let px = x + col as usize;
            if px >= w {
                continue;
            }
            let a = data[(row * glyph_w + col) as usize] as u32;
            if a == 0 {
                continue;
            }
            let idx = py as usize * w + px;
            let bg = buf[idx];
            let inv = 255 - a;
            let r = (((color.0 >> 16) & 0xff) * a + ((bg >> 16) & 0xff) * inv) / 255;
            let g = (((color.0 >> 8) & 0xff) * a + ((bg >> 8) & 0xff) * inv) / 255;
            let b = ((color.0 & 0xff) * a + (bg & 0xff) * inv) / 255;
            buf[idx] = (r << 16) | (g << 8) | b;
        }
    }
}

fn draw_taskbar_text(
    layer: &mut LayerSystem,
    text: &str,
    mut x: usize,
    baseline_y: i32,
    color: Color,
    size: f32,
) {
    for ch in text.chars() {
        let glyph = baram_font::ttf_font_hud::glyph_at_size(ch, size);
        if glyph.w > 0 && glyph.h > 0 {
            draw_taskbar_glyph(
                layer,
                &glyph.data,
                glyph.w,
                glyph.h,
                x,
                baseline_y + glyph.y_off,
                color,
            );
            x += glyph.advance.max(0) as usize;
            continue;
        }
        // Google Sans used by the taskbar has no Japanese glyphs. Fall back
        // to the regular UI font so placeholder and typed Japanese stay visible.
        let fallback = baram_font::ttf_font::glyph_at_size(ch, size);
        if fallback.w > 0 && fallback.h > 0 {
            draw_taskbar_glyph(
                layer,
                &fallback.data,
                fallback.w,
                fallback.h,
                x,
                baseline_y + fallback.y_off,
                color,
            );
            x += fallback.advance.max(0) as usize;
        } else {
            x += 8;
        }
    }
}

fn draw_taskbar_search(
    layer: &mut LayerSystem,
    search_focused: bool,
    search_query: &str,
    caret_visible: bool,
) {
    let search_x = 12usize;
    let search_h = 32usize;
    let search_y = (TASKBAR_H - search_h) / 2;
    let search_w = 190usize;
    let search_bg = config::get_color("ui-theme/color/panel", Color::PANEL);
    let search_alpha = if search_focused { 255 } else { 128 };
    draw_control_shadow(
        layer,
        search_x,
        search_y,
        search_w,
        search_h,
        search_h / 2,
        2,
        0x33,
    );
    blend_rounded_rect(
        layer,
        search_x,
        search_y,
        search_w,
        search_h,
        search_h / 2,
        search_bg,
        search_alpha,
    );
    let text = if search_query.is_empty() {
        "アプリを検索..."
    } else {
        search_query
    };
    let text_color = if search_query.is_empty() {
        config::get_color("ui-theme/color/muted", Color::MUTED)
    } else {
        config::get_color("ui-theme/color/text", Color::TEXT)
    };
    draw_taskbar_text(
        layer,
        text,
        search_x + 12,
        search_y as i32 + 22,
        text_color,
        18.0,
    );
    if search_focused && caret_visible {
        let caret_x = search_x as i32 + 12 + taskbar_text_width(search_query, 18.0) as i32;
        text_cursor::draw(
            layer,
            caret_x,
            search_y as i32 + 7,
            20,
            config::get_color("ui-theme/color/text", Color::TEXT),
        );
    }
}

fn redraw_taskbar_search(
    surface: &mut TaskbarSurface,
    search_focused: bool,
    search_query: &str,
    caret_visible: bool,
) {
    const SEARCH_DAMAGE_W: usize = 226;
    let w = surface.layer.width();
    let copy_w = SEARCH_DAMAGE_W.min(w);
    if surface.base_valid {
        for y in 0..TASKBAR_H {
            let start = y * w;
            surface.layer.buf_mut()[start..start + copy_w]
                .copy_from_slice(&surface.base[start..start + copy_w]);
        }
    }
    draw_taskbar_search(
        &mut surface.layer,
        search_focused,
        search_query,
        caret_visible,
    );
    surface.search_dirty = false;
}

fn redraw_taskbar(
    surface: &mut TaskbarSurface,
    wm: &WindowManager,
    add_progress: f32,
    shift_x: f32,
    _hover_apps_icon: bool,
    hover_keyboard_icon: bool,
    hover_ime_icon: bool,
    search_focused: bool,
    search_query: &str,
    caret_visible: bool,
    clock_hh: u8,
    clock_mm: u8,
    battery_pct: Option<u8>,
    ime_menu_selection: usize,
) {
    let layer = &mut surface.layer;
    let w = layer.width();
    if surface.base_valid {
        layer.buf_mut().copy_from_slice(&surface.base);
    } else {
        layer.clear(config::get_color("ui-theme/color/taskbar", Color::TASKBAR));
    }

    let count = wm.count();
    let btn_d = 40usize;
    let btn_gap = 12i32;
    let total_w = count as i32 * (btn_d as i32 + btn_gap) - btn_gap;
    let base_bx = ((w as i32 - total_w) / 2).max(0);
    let btn_y = (TASKBAR_H - btn_d) / 2;
    let add_offset_y = if add_progress >= 0.0 {
        ((1.0 - ease_out_cubic(add_progress)) * (TASKBAR_H + 8) as f32) as usize
    } else {
        0
    };

    for i in 0..count {
        let Some(id) = wm.insertion_id_at(i) else {
            continue;
        };
        let icon_name = wm.get_icon_name(id);
        let is_focused = wm.focused_id == Some(id);
        let is_minimized = wm.is_minimized(id);
        let scaled_d = btn_d;
        let offset = if add_progress >= 0.0 && i == count - 1 {
            add_offset_y
        } else {
            0
        };
        let bx = base_bx + shift_x as i32 + i as i32 * (btn_d as i32 + btn_gap);
        let cached_btn = get_or_render_tb_btn(scaled_d, if is_focused { 255 } else { 100 });
        for py in 0..scaled_d {
            let dst_y = btn_y + offset + py;
            if dst_y >= TASKBAR_H {
                continue;
            }
            for px in 0..scaled_d {
                let a = (cached_btn[py * scaled_d + px] >> 24) & 0xff;
                if a == 0 {
                    continue;
                }
                let dst_x = bx + px as i32;
                if dst_x < 0 || dst_x >= w as i32 {
                    continue;
                }
                let idx = dst_y * w + dst_x as usize;
                let bg = layer.buf_ref()[idx];
                let inv = 255 - a;
                let r = (255 * a + ((bg >> 16) & 0xff) * inv) / 255;
                let g = (255 * a + ((bg >> 8) & 0xff) * inv) / 255;
                let b = (255 * a + (bg & 0xff) * inv) / 255;
                layer.buf_mut()[idx] = (r << 16) | (g << 8) | b;
            }
        }

        let resolved_icon = if icon_name.is_empty() {
            "noname.png"
        } else {
            icon_name
        };
        if let Some(icon) = get_or_decode_icon(resolved_icon, 40) {
            let icon_draw = scaled_d;
            let icon_offset = offset;
            for py in 0..icon_draw {
                let sy = py * icon.h / icon_draw;
                let dst_y = btn_y + icon_offset + py;
                if dst_y >= TASKBAR_H {
                    continue;
                }
                for px in 0..icon_draw {
                    let sx = px * icon.w / icon_draw;
                    let src = icon.pixels[sy * icon.w + sx];
                    let a = src[3] as u32 * if is_minimized { 128 } else { 255 } / 255;
                    if a == 0 {
                        continue;
                    }
                    let dst_x = bx + px as i32;
                    if dst_x < 0 || dst_x >= w as i32 {
                        continue;
                    }
                    let idx = dst_y * w + dst_x as usize;
                    let bg = layer.buf_ref()[idx];
                    let inv = 255 - a;
                    let r = (src[0] as u32 * a + ((bg >> 16) & 0xff) * inv) / 255;
                    let g = (src[1] as u32 * a + ((bg >> 8) & 0xff) * inv) / 255;
                    let b = (src[2] as u32 * a + (bg & 0xff) * inv) / 255;
                    layer.buf_mut()[idx] = (r << 16) | (g << 8) | b;
                }
            }
        }
    }

    let time_bytes = [
        b'0' + clock_hh / 10,
        b'0' + clock_hh % 10,
        b':',
        b'0' + clock_mm / 10,
        b'0' + clock_mm % 10,
    ];
    let time = unsafe { core::str::from_utf8_unchecked(&time_bytes) };
    let mut battery_bytes = [0u8; 4];
    let battery = battery_pct.map(|pct| {
        let len;
        if pct >= 100 {
            battery_bytes.copy_from_slice(b"100%");
            len = 4;
        } else if pct >= 10 {
            battery_bytes[0] = b'0' + pct / 10;
            battery_bytes[1] = b'0' + pct % 10;
            battery_bytes[2] = b'%';
            len = 3;
        } else {
            battery_bytes[0] = b'0' + pct % 10;
            battery_bytes[1] = b'%';
            len = 2;
        }
        unsafe { core::str::from_utf8_unchecked(&battery_bytes[..len]) }
    });

    let size = TASKBAR_STATUS_SIZE;
    let measure = taskbar_status_text_width;
    let gap = 12usize;
    let status_x = taskbar_status_x(w, battery_pct);
    let baseline = TASKBAR_H as i32 - baram_font::ttf_font_hud::ascent_at_size(size) + 9;
    let status_color = config::get_color("ui-theme/color/text", Color::TEXT);
    let ime_x = status_x.saturating_sub(IME_BUTTON_W + gap);
    let keyboard_x = ime_x.saturating_sub(IME_BUTTON_W + gap);
    let ime_y = (TASKBAR_H - IME_BUTTON_W) / 2;
    draw_taskbar_text(layer, time, status_x, baseline, status_color, size);
    if let Some(battery) = battery {
        draw_taskbar_text(
            layer,
            battery,
            status_x + measure(time) + gap,
            baseline,
            status_color,
            size,
        );
    }

    draw_taskbar_search(layer, search_focused, search_query, caret_visible);
    // Cache a small, exact status strip before painting the mutable IME SVG.
    // Hover therefore restores the real clock/battery pixels, never a broad
    // taskbar background approximation.
    let strip_x = keyboard_x.saturating_sub(8);
    let strip_w = (w - strip_x).min(IME_STATUS_STRIP_W);
    surface.ime_status_strip.resize(strip_w * TASKBAR_H, 0);
    for row in 0..TASKBAR_H {
        let source = row * w + strip_x;
        let target = row * strip_w;
        surface.ime_status_strip[target..target + strip_w]
            .copy_from_slice(&layer.buf_ref()[source..source + strip_w]);
    }
    surface.ime_status_strip_x = strip_x;
    surface.ime_status_strip_w = strip_w;
    draw_keyboard_icon(
        layer,
        keyboard_x,
        ime_y,
        IME_BUTTON_W,
        if hover_keyboard_icon { 128 } else { 255 },
    );
    // Unlike a control button, the active input source is a bare status icon
    // beside the clock: no pill background or shadow.
    draw_ime_icon(
        layer,
        ime_x,
        ime_y,
        IME_BUTTON_W,
        ime_menu_selection,
        if hover_ime_icon { 128 } else { 255 },
    );
    layer.mark_all_dirty();
    surface.valid = true;
    surface.search_dirty = false;
}

fn draw_ime_candidates(
    layer: &mut LayerSystem,
    taskbar_y: usize,
    reading: &str,
    candidates: &[alloc::string::String],
    selected: usize,
) {
    if candidates.is_empty() || taskbar_y < 76 {
        return;
    }
    let x = 24usize;
    let y = taskbar_y - 76;
    let width = 440usize.min(layer.width().saturating_sub(x * 2));
    let height = 64usize;
    draw_control_shadow(layer, x, y, width, height, 12, 2, 0x36);
    blend_rounded_rect(
        layer,
        x,
        y,
        width,
        height,
        12,
        config::get_color("ui-theme/color/panel", Color::PANEL),
        242,
    );
    let text_color = config::get_color("ui-theme/color/text", Color::TEXT);
    let accent = config::get_color("ui-theme/color/btn_primary", Color::BTN_PRIMARY);
    let mut heading = alloc::string::String::from("変換中: ");
    heading.push_str(reading);
    layer.put_str(x + 14, y + 8, &heading, text_color);

    let mut cx = x + 14;
    for (index, candidate) in candidates.iter().enumerate() {
        let candidate_width = candidate.chars().count() * 16 + 18;
        if cx + candidate_width > x + width - 10 {
            break;
        }
        if index == selected {
            blend_rounded_rect(layer, cx, y + 30, candidate_width, 25, 6, accent, 48);
        }
        layer.put_str(cx + 8, y + 34, candidate, text_color);
        if index == selected {
            // The underline marks the candidate currently composing in the
            // target input. Enter commits this underlined candidate.
            layer.fill_rect(
                cx + 7,
                y + 52,
                candidate_width.saturating_sub(14),
                2,
                accent,
            );
        }
        cx += candidate_width + 6;
    }
}

/// Bounds of the IME mode menu in screen coordinates.
pub fn ime_menu_bounds(
    width: usize,
    height: usize,
    battery_pct: Option<u8>,
) -> (i32, i32, i32, i32) {
    let (button_x, _, button_w, _) = ime_button_bounds(width, battery_pct);
    let x = (button_x + button_w - IME_MENU_W as i32).max(12);
    let y = height.saturating_sub(TASKBAR_H + IME_MENU_H + 12).max(8) as i32;
    (x, y, IME_MENU_W as i32, IME_MENU_H as i32)
}

/// Returns the mode row selected by a click in the IME menu.
pub fn ime_menu_mode_at(
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    battery_pct: Option<u8>,
) -> Option<usize> {
    let (menu_x, menu_y, menu_w, menu_h) = ime_menu_bounds(width, height, battery_pct);
    if x < menu_x || x >= menu_x + menu_w || y < menu_y || y >= menu_y + menu_h {
        return None;
    }
    // Header occupies the first 28px; six 38px rows follow it.
    (y >= menu_y + 30)
        .then(|| ((y - menu_y - 30) / 38) as usize)
        .filter(|row| *row < 6)
}

fn draw_ime_menu(
    layer: &mut LayerSystem,
    cached_layer: &mut Option<Vec<u32>>,
    battery_pct: Option<u8>,
    ime_menu_selection: usize,
    opacity: u8,
) {
    let (x, y, width, height) = ime_menu_bounds(layer.width(), layer.height(), battery_pct);
    let (x, y, width, height) = (x as usize, y as usize, width as usize, height as usize);
    // The glass background is expensive only when it opens or the window
    // below it changes. Cursor movement reuses this cached layer verbatim.
    const CACHE_PAD: usize = 54;
    let cache_x = x.saturating_sub(CACHE_PAD);
    let cache_y = y.saturating_sub(CACHE_PAD);
    let cache_x1 = (x + width + CACHE_PAD).min(layer.width());
    let cache_y1 = (y + height + CACHE_PAD).min(layer.height());
    let cache_w = cache_x1.saturating_sub(cache_x);
    let cache_h = cache_y1.saturating_sub(cache_y);
    if cache_w == 0 || cache_h == 0 {
        return;
    }
    let (clip_x0, clip_y0, clip_x1, clip_y1) = layer.clip_bounds();
    if clip_x1 <= cache_x || clip_x0 >= cache_x1 || clip_y1 <= cache_y || clip_y0 >= cache_y1 {
        return;
    }
    if cached_layer
        .as_ref()
        .is_none_or(|cache| cache.len() != cache_w * cache_h)
    {
        const BLUR_RADIUS: usize = 18;
        let mut glass = LayerSystem::new(cache_w, cache_h);
        for py in 0..cache_h {
            let src_start = (cache_y + py) * layer.width() + cache_x;
            let dst_start = py * cache_w;
            glass.buf_mut()[dst_start..dst_start + cache_w]
                .copy_from_slice(&layer.buf_ref()[src_start..src_start + cache_w]);
        }
        let blur_x0 = x.saturating_sub(BLUR_RADIUS);
        let blur_y0 = y.saturating_sub(BLUR_RADIUS);
        let blur_x1 = (x + width + BLUR_RADIUS).min(layer.width());
        let blur_y1 = (y + height + BLUR_RADIUS).min(layer.height());
        let blur_w = blur_x1.saturating_sub(blur_x0);
        let blur_h = blur_y1.saturating_sub(blur_y0);
        if blur_w == 0 || blur_h == 0 {
            return;
        }
        let mut source = alloc::vec![0u32; blur_w * blur_h];
        for py in 0..blur_h {
            let src_start = (blur_y0 + py) * layer.width() + blur_x0;
            let dst_start = py * blur_w;
            source[dst_start..dst_start + blur_w]
                .copy_from_slice(&layer.buf_ref()[src_start..src_start + blur_w]);
        }
        let mut blurred = alloc::vec![0u32; source.len()];
        blur::blur_region_to(&source, &mut blurred, blur_w, 0, blur_h, BLUR_RADIUS as i32);
        draw_soft_box_shadow(&mut glass, x - cache_x, y - cache_y, width, height, 18);
        copy_rounded_region_from_crop(
            &mut glass,
            &blurred,
            blur_w,
            blur_x0 - cache_x,
            blur_y0 - cache_y,
            x - cache_x,
            y - cache_y,
            width,
            height,
            18,
        );
        blend_rounded_rect(
            &mut glass,
            x - cache_x,
            y - cache_y,
            width,
            height,
            18,
            Color::rgb(0xf5, 0xf5, 0xf5),
            200,
        );
        // The entire static list lives in the open-menu cache too. This keeps
        // SVG blending and text layout out of pointer-move redraws.
        let text = config::get_color("ui-theme/color/text", Color::TEXT);
        let muted = config::get_color("ui-theme/color/muted", Color::MUTED);
        let menu_x = x - cache_x;
        let menu_y = y - cache_y;
        glass.put_str(menu_x + 14, menu_y + 9, "入力モード", muted);
        for (row, label) in [
            "英数",
            "ひらがな",
            "한국 두벌식",
            "한컴 로마자",
            "조선 두벌식",
            "简体拼音",
        ]
        .iter()
        .enumerate()
        {
            let row_y = menu_y + 30 + row * 38;
            if row == ime_menu_selection {
                blend_rounded_rect(
                    &mut glass,
                    menu_x + 8,
                    row_y,
                    width - 16,
                    34,
                    9,
                    Color::rgb(0xff, 0xff, 0xff),
                    0x99,
                );
            }
            draw_ime_icon(&mut glass, menu_x + 18, row_y + 9, 16, row, 255);
            glass.put_str(menu_x + 46, row_y + 9, label, text);
        }
        *cached_layer = Some(glass.buf_ref().to_vec());
    }
    if let Some(cache) = cached_layer.as_deref() {
        if opacity == 255 {
            layer.copy_rect_buffer(cache, cache_w, cache_h, cache_x, cache_y);
        } else {
            layer.composit_rect_global_alpha(cache, cache_w, cache_h, cache_x, cache_y, opacity);
        }
    }
}

/// Bounds of the taskbar IME toggle, kept in sync with the status layout.
pub fn ime_button_bounds(width: usize, battery_pct: Option<u8>) -> (i32, i32, i32, i32) {
    let status_x = taskbar_status_x(width, battery_pct);
    let x = status_x.saturating_sub(IME_BUTTON_W + 12) as i32;
    (
        x,
        (TASKBAR_H as i32 - IME_BUTTON_W as i32) / 2,
        IME_BUTTON_W as i32,
        IME_BUTTON_W as i32,
    )
}


