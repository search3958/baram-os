pub fn render_scene(
    layer: &mut LayerSystem,
    taskbar: &mut TaskbarSurface,
    wm: &mut WindowManager,
    _mouse_ev: u32,
    key_ev: u32,
    fps: u32,
    _mouse_mode: &str,
    warp_engines: &mut alloc::vec::Vec<(WinId, WarpEngine)>,
    html_engines: &mut alloc::vec::Vec<(WinId, HtmlEngine)>,
    wallpaper: Option<&[u32]>,
    cached_launcher_layer: &mut Option<Vec<u32>>,
    cached_ime_menu_layer: &mut Option<Vec<u32>>,
    taskbar_dirty: bool,
    add_progress: f32,
    _remove_progress: f32,
    shift_x: f32,
    hud_enabled: bool,
    bg_cache: &mut Option<Vec<u32>>,
    bg_cache_valid: bool,
    show_app_launcher: bool,
    app_list: &[alloc::string::String],
    app_icon_list: &[alloc::string::String],
    hover_apps_icon: bool,
    hover_keyboard_icon: bool,
    hover_ime_icon: bool,
    ime_hover_dirty: bool,
    search_focused: bool,
    search_query: &str,
    caret_visible: bool,
    launcher_scroll_y: usize,
    launcher_anim_phase: i8,
    launcher_anim_elapsed_ms: u32,
    launcher_scroll_changed: bool,
    launcher_only_redraw: bool,
    taskbar_only: bool,
    clock_hh: u8,
    clock_mm: u8,
    battery_pct: Option<u8>,
    ime_menu_open: bool,
    soft_keyboard: &mut SoftKeyboard,
    ime_menu_opacity: u8,
    ime_menu_selection: usize,
    ime_reading: Option<&str>,
    ime_candidates: &[alloc::string::String],
    ime_selected: usize,
) {
    let w = layer.width();
    let h = layer.height();
    let tb_y = h.saturating_sub(TASKBAR_H);

    // During launcher-only animation frames, restore the small captured
    // underlay instead of rebuilding the wallpaper, HUD, and every window.
    if launcher_only_redraw {
        let grid_h = 3 * (52 + 20 + 16);
        let grid_y = h.saturating_sub(TASKBAR_H + grid_h + 16);
        let panel_x = 12usize;
        let panel_y = grid_y.saturating_sub(8);
        let panel_w = 4 * (52 + 16) + 16;
        let panel_h = grid_h + 16;
        const CACHE_PAD: usize = 54;
        let cache_x = panel_x.saturating_sub(CACHE_PAD);
        let cache_y = panel_y.saturating_sub(CACHE_PAD);
        let cache_x1 = (panel_x + panel_w + CACHE_PAD).min(w);
        let cache_y1 = (panel_y + panel_h + CACHE_PAD).min(h);
        let cache_w = cache_x1.saturating_sub(cache_x);
        let cache_h = cache_y1.saturating_sub(cache_y);
        let cache_len = cache_w * cache_h;
        if let Some(cache) = cached_launcher_layer.as_deref() {
            if cache.len() == cache_len * 4 {
                layer.copy_rect_buffer(
                    &cache[cache_len * 2..cache_len * 3],
                    cache_w,
                    cache_h,
                    cache_x,
                    cache_y,
                );
            }
        }
    }

    if !taskbar_only && !launcher_only_redraw {
        if bg_cache_valid {
            if let Some(ref cached) = bg_cache {
                layer.copy_from_screen_buffer(cached);
            }
        } else if let Some(pixels) = wallpaper {
            layer.copy_from_screen_buffer(pixels);
        } else {
            layer.clear(config::get_color("ui-theme/color/bg", Color::BG));
        }

        if !bg_cache_valid {
            let mut bg = alloc::vec![0u32; w * h];
            bg.copy_from_slice(layer.buf_ref());
            *bg_cache = Some(bg);
        }

        if !bg_cache_valid || !taskbar.base_valid {
            taskbar.refresh_wallpaper_blur(layer, tb_y);
        }
    }

    let mut fb = FmtBuf::new();
    fb.push_str("Key:");
    fb.push_u32(key_ev);
    fb.push_str(" Window:");
    fb.push_u32(wm.count() as u32);
    fb.push_str(" ");
    fb.push_u32(fps);
    fb.push_str("FPS");

    if hud_enabled && !taskbar_only && !launcher_only_redraw {
        if let Some(ref bg) = bg_cache {
            let hud_y0 = (tb_y as i32 - 44).max(0) as usize;
            let hud_y1 = tb_y;
            for y in hud_y0..hud_y1 {
                let s = y * w;
                let e = s + w;
                if e <= bg.len() {
                    layer.buf_mut()[s..e].copy_from_slice(&bg[s..e]);
                }
            }
        }

        let hud_text1 = "Baram OS (1.2)";
        let mut hw1 = 0usize;
        for ch in hud_text1.chars() {
            if baram_font::ttf_font_hud::is_available() {
                let g = baram_font::ttf_font_hud::glyph(ch);
                hw1 += if g.w > 0 {
                    g.advance.max(0) as usize
                } else {
                    8
                };
            } else {
                hw1 += 8;
            }
        }
        layer.put_str_hud(
            w - hw1 - 16,
            tb_y - 28,
            hud_text1,
            config::get_color("ui-theme/color/muted", Color::MUTED),
        );

        let s2 = fb.as_str();
        let mut hw2 = 0usize;
        for ch in s2.chars() {
            if baram_font::ttf_font_hud::is_available() {
                let g = baram_font::ttf_font_hud::glyph(ch);
                hw2 += if g.w > 0 {
                    g.advance.max(0) as usize
                } else {
                    8
                };
            } else {
                hw2 += 8;
            }
        }
        layer.put_str_hud(
            w - hw2 - 16,
            tb_y - 12,
            s2,
            config::get_color("ui-theme/color/muted", Color::MUTED),
        );
    }

    // Stable z-order: background -> HUD -> windows/launcher -> taskbar.
    if !taskbar_only && !launcher_only_redraw {
        wm.draw_all(layer, warp_engines, html_engines);
    }

    if show_app_launcher {
        let cols = 4usize;
        let icon_size = 52usize;
        let icon_gap = 16usize;
        let label_h = 20usize;
        let cell_w = icon_size + icon_gap;
        let cell_h = icon_size + label_h + icon_gap;
        let grid_w = cols * cell_w;
        let visible_rows = 3usize;
        let grid_h = visible_rows * cell_h;
        let grid_x = 20usize;
        let grid_y = h.saturating_sub(TASKBAR_H + grid_h + 16);
        let content_y = grid_y + 4;
        let panel_h = grid_h.max(40) + 16;
        let panel_x = 12usize;
        let panel_y = grid_y.saturating_sub(8);
        let panel_w = grid_w + 16;
        let panel_radius = 18usize;
        let launcher_alpha = if launcher_anim_phase > 0 {
            let t = (launcher_anim_elapsed_ms as f32 / 200.0).clamp(0.0, 1.0);
            (ease_launcher_open(t) * 255.0) as u32
        } else if launcher_anim_phase < 0 {
            let t = (launcher_anim_elapsed_ms as f32 / 200.0).clamp(0.0, 1.0);
            ((1.0 - ease_in_out(t)) * 255.0) as u32
        } else {
            255
        };
        // The complete launcher is cached as one layer. Animation only
        // changes this layer's position and global opacity.
        let launcher_offset_y = if launcher_anim_phase > 0 {
            let t = (launcher_anim_elapsed_ms as f32 / 200.0).clamp(0.0, 1.0);
            ((1.0 - ease_launcher_open(t)) * 16.0) as usize
        } else {
            0
        };

        let building_launcher_cache = cached_launcher_layer.is_none();
        let rebuild_launcher_content = building_launcher_cache || launcher_scroll_changed;
        // Cache the complete glass-and-shadow base once per opening.
        if building_launcher_cache {
            const CACHE_PAD: usize = 54;
            let cache_x = panel_x.saturating_sub(CACHE_PAD);
            let cache_y = panel_y.saturating_sub(CACHE_PAD);
            let cache_x1 = (panel_x + panel_w + CACHE_PAD).min(w);
            let cache_y1 = (panel_y + panel_h + CACHE_PAD).min(h);
            let cache_w = cache_x1.saturating_sub(cache_x);
            let cache_h = cache_y1.saturating_sub(cache_y);
            let mut panel_base = LayerSystem::new(cache_w, cache_h);
            for py in 0..cache_h {
                let src_start = (cache_y + py) * w + cache_x;
                let dst_start = py * cache_w;
                panel_base.buf_mut()[dst_start..dst_start + cache_w]
                    .copy_from_slice(&layer.buf_ref()[src_start..src_start + cache_w]);
            }
            // Two box-blur passes with r=13 can read up to 26px beyond
            // the result. Keep that margin around the panel, rather than
            // filtering the entire screen.
            const BLUR_RADIUS: usize = 26;
            let blur_x0 = panel_x.saturating_sub(BLUR_RADIUS);
            let blur_y0 = panel_y.saturating_sub(BLUR_RADIUS);
            let blur_x1 = (panel_x + panel_w + BLUR_RADIUS).min(w);
            let blur_y1 = (panel_y + panel_h + BLUR_RADIUS).min(h);
            let blur_w = blur_x1.saturating_sub(blur_x0);
            let blur_h = blur_y1.saturating_sub(blur_y0);
            let mut blur_source = alloc::vec![0u32; blur_w * blur_h];
            for py in 0..blur_h {
                let src_start = (blur_y0 + py) * w + blur_x0;
                let dst_start = py * blur_w;
                blur_source[dst_start..dst_start + blur_w]
                    .copy_from_slice(&layer.buf_ref()[src_start..src_start + blur_w]);
            }
            let mut blurred = alloc::vec![0u32; blur_source.len()];
            blur::blur_region_to(&blur_source, &mut blurred, blur_w, 0, blur_h, 26);
            let underlay = panel_base.buf_ref().to_vec();
            draw_soft_box_shadow(
                &mut panel_base,
                panel_x - cache_x,
                panel_y - cache_y,
                panel_w,
                panel_h,
                panel_radius,
            );
            copy_rounded_region_from_crop(
                &mut panel_base,
                &blurred,
                blur_w,
                blur_x0 - cache_x,
                blur_y0 - cache_y,
                panel_x - cache_x,
                panel_y - cache_y,
                panel_w,
                panel_h,
                panel_radius,
            );
            blend_rounded_rect(
                &mut panel_base,
                panel_x - cache_x,
                panel_y - cache_y,
                panel_w,
                panel_h,
                panel_radius,
                Color::rgb(0xf5, 0xf5, 0xf5),
                200,
            );
            let panel = panel_base.buf_ref();
            let mut cache = Vec::with_capacity(panel.len() * 4);
            cache.extend_from_slice(panel); // fully composed launcher
            cache.extend_from_slice(panel); // fixed glass background
            cache.extend_from_slice(&underlay); // captured screen below it
            cache.extend_from_slice(panel); // per-frame layer scratch
            *cached_launcher_layer = Some(cache);
        }
        let (clip_x0, clip_y0, clip_x1, clip_y1) = layer.clip_bounds();
        if rebuild_launcher_content {
            if let Some(panel_base) = cached_launcher_layer.as_deref() {
                const CACHE_PAD: usize = 54;
                let cache_x = panel_x.saturating_sub(CACHE_PAD);
                let cache_y = panel_y.saturating_sub(CACHE_PAD);
                let cache_x1 = (panel_x + panel_w + CACHE_PAD).min(w);
                let cache_y1 = (panel_y + panel_h + CACHE_PAD).min(h);
                let cache_w = cache_x1.saturating_sub(cache_x);
                let cache_h = cache_y1.saturating_sub(cache_y);
                if panel_base.len() == cache_w * cache_h * 4 {
                    let panel_start = cache_w * cache_h;
                    for py in 0..cache_h {
                        let dst_y = cache_y + py;
                        if dst_y < clip_y0 || dst_y >= clip_y1 {
                            continue;
                        }
                        let src_start = panel_start + py * cache_w;
                        let draw_x0 = cache_x.max(clip_x0);
                        let draw_x1 = cache_x1.min(clip_x1);
                        if draw_x0 >= draw_x1 {
                            continue;
                        }
                        let src_start = src_start + draw_x0 - cache_x;
                        let dst_start = dst_y * w + draw_x0;
                        let draw_w = draw_x1 - draw_x0;
                        layer.buf_mut()[dst_start..dst_start + draw_w]
                            .copy_from_slice(&panel_base[src_start..src_start + draw_w]);
                    }
                }
            }

            if app_list.is_empty() {
                layer.put_str(
                    28,
                    content_y + 8,
                    "該当するアプリはありません",
                    Color::BLACK,
                );
            }

            let content_rows = ((app_list.len() + cols - 1) / cols).max(visible_rows);
            let content_h = content_rows * cell_h;
            let scroll_y = launcher_scroll_y.min(content_h.saturating_sub(grid_h));
            // Keep the scratch surface bounded to the viewport. The old code
            // allocated and cleared a surface as tall as the complete app
            // list for every animation frame.
            let first_scratch_row = (scroll_y / cell_h).saturating_sub(1);
            let scratch_y = first_scratch_row * cell_h;
            let viewport_src_y = scroll_y - scratch_y;
            let scratch_h = (grid_h + cell_h * 2).min(content_h.saturating_sub(scratch_y));
            let mut content = LayerSystem::new_transparent(grid_w, scratch_h);
            // Antialiased pixels must be blended against the actual panel
            // background, not transparent black. Seed the visible viewport
            // before rendering its icons and labels.
            for py in 0..scratch_h {
                let screen_y = content_y.saturating_add(py).saturating_sub(viewport_src_y);
                if screen_y >= h {
                    continue;
                }
                let src_start = screen_y * w + grid_x;
                let dst_start = py * grid_w;
                content.buf_mut()[dst_start..dst_start + grid_w]
                    .copy_from_slice(&layer.buf_ref()[src_start..src_start + grid_w]);
            }
            for (i, name) in app_list.iter().enumerate() {
                let col = i % cols;
                let row = i / cols;
                let cx = col * cell_w + icon_gap / 2;
                let item_y = row * cell_h;
                if item_y + cell_h <= scratch_y || item_y >= scratch_y + scratch_h {
                    continue;
                }
                let cy = item_y - scratch_y;

                content.fill_circle(
                    cx + icon_size / 2,
                    cy + icon_size / 2,
                    icon_size / 2,
                    Color::rgb(0xff, 0xff, 0xff),
                );

                let icon_name = app_icon_list.get(i).map(|s| s.as_str()).unwrap_or("");
                let resolved_icon = if icon_name.is_empty() || icon_name == "null" {
                    "noname.png"
                } else {
                    icon_name
                };
                {
                    if let Some(icon) = get_or_decode_icon(resolved_icon, icon_size) {
                        let pad = (icon_size - icon.w) / 2;
                        for py in 0..icon.h {
                            for px in 0..icon.w {
                                let src_px = icon.pixels[py * icon.w + px];
                                let a = src_px[3] as u32;
                                if a == 0 {
                                    continue;
                                }
                                let sx = cx + pad + px;
                                let sy = cy + pad + py;
                                if sx >= grid_w || sy >= scratch_h {
                                    continue;
                                }
                                let idx = sy * grid_w + sx;
                                let bg = Color(content.buf_ref()[idx]);
                                let inv = 255 - a;
                                let r = (src_px[0] as u32 * a + bg.r() as u32 * inv) / 255;
                                let g = (src_px[1] as u32 * a + bg.g() as u32 * inv) / 255;
                                let b = (src_px[2] as u32 * a + bg.b() as u32 * inv) / 255;
                                content.buf_mut()[idx] = Color::rgb(r as u8, g as u8, b as u8).0;
                            }
                        }
                    }
                }

                let char_w = 8usize;
                let max_chars = icon_size / char_w;
                let char_count = name.chars().count();
                let display_name = if char_count > max_chars {
                    let truncated_len = max_chars.saturating_sub(3);
                    let mut s = alloc::string::String::with_capacity(max_chars * 4);
                    for ch in name.chars().take(truncated_len) {
                        s.push(ch);
                    }
                    s.push_str("...");
                    s
                } else {
                    name.clone()
                };
                let mut tw = 0usize;
                for ch in display_name.chars() {
                    if baram_font::ttf_font::is_available() {
                        let g = baram_font::ttf_font::glyph(ch);
                        if g.w > 0 {
                            tw += g.advance.max(0) as usize;
                        } else {
                            tw += char_w;
                        }
                    } else {
                        tw += char_w;
                    }
                }
                let tx = cx + (icon_size.saturating_sub(tw)) / 2;
                let ty = cy + icon_size + 4;
                let label_color = Color::BLACK;
                content.put_str(tx, ty, &display_name, label_color);
            }
            layer.composit_rect_opaque(
                &content,
                grid_x,
                content_y,
                0,
                viewport_src_y,
                grid_w,
                grid_h,
            );

            // Replace the first half with the fully composed launcher
            // (glass, icons, and labels), then put the captured underlay back.
            const CACHE_PAD: usize = 54;
            let cache_x = panel_x.saturating_sub(CACHE_PAD);
            let cache_y = panel_y.saturating_sub(CACHE_PAD);
            let cache_x1 = (panel_x + panel_w + CACHE_PAD).min(w);
            let cache_y1 = (panel_y + panel_h + CACHE_PAD).min(h);
            let cache_w = cache_x1.saturating_sub(cache_x);
            let cache_h = cache_y1.saturating_sub(cache_y);
            let cache_len = cache_w * cache_h;
            if let Some(cache) = cached_launcher_layer.as_mut() {
                if cache.len() == cache_len * 4 {
                    for py in 0..cache_h {
                        let src_start = (cache_y + py) * w + cache_x;
                        let dst_start = py * cache_w;
                        cache[dst_start..dst_start + cache_w]
                            .copy_from_slice(&layer.buf_ref()[src_start..src_start + cache_w]);
                    }
                }
            }
            if let Some(cache) = cached_launcher_layer.as_deref() {
                if cache.len() == cache_len * 4 {
                    layer.copy_rect_buffer(
                        &cache[cache_len * 2..cache_len * 3],
                        cache_w,
                        cache_h,
                        cache_x,
                        cache_y,
                    );
                }
            }
        }

        // Build one launcher layer from a fixed glass background plus the
        // cached app pixels shifted inside it. Then apply opacity once to
        // that entire layer through the SIMD compositor.
        if let Some(cache) = cached_launcher_layer.as_mut() {
            const CACHE_PAD: usize = 54;
            let cache_x = panel_x.saturating_sub(CACHE_PAD);
            let cache_y = panel_y.saturating_sub(CACHE_PAD);
            let cache_x1 = (panel_x + panel_w + CACHE_PAD).min(w);
            let cache_y1 = (panel_y + panel_h + CACHE_PAD).min(h);
            let cache_w = cache_x1.saturating_sub(cache_x);
            let cache_h = cache_y1.saturating_sub(cache_y);
            let cache_len = cache_w * cache_h;
            if cache.len() == cache_len * 4 && launcher_alpha != 0 {
                if launcher_anim_phase == 0 {
                    layer.copy_rect_buffer(&cache[..cache_len], cache_w, cache_h, cache_x, cache_y);
                    // No opacity or internal motion remains in steady and
                    // scroll frames, so the finished cache is final.
                } else if launcher_anim_phase < 0 {
                    // Closing has no internal translation. Feed the final
                    // cached launcher straight into the SIMD alpha pass.
                    layer.composit_rect_global_alpha(
                        &cache[..cache_len],
                        cache_w,
                        cache_h,
                        cache_x,
                        cache_y,
                        launcher_alpha as u8,
                    );
                } else {
                    cache.copy_within(cache_len..cache_len * 2, cache_len * 3);

                    let content_x = grid_x - cache_x;
                    let content_base_y = content_y - cache_y;
                    for py in 0..grid_h {
                        let dst_py = content_base_y + py + launcher_offset_y;
                        if dst_py >= cache_h {
                            continue;
                        }
                        let src_row = (content_base_y + py) * cache_w + content_x;
                        let dst_row = dst_py * cache_w + content_x;
                        for px in 0..grid_w {
                            let src = src_row + px;
                            if cache[src] != cache[cache_len + src] {
                                cache[cache_len * 3 + dst_row + px] = cache[src];
                            }
                        }
                    }

                    layer.composit_rect_global_alpha(
                        &cache[cache_len * 3..cache_len * 4],
                        cache_w,
                        cache_h,
                        cache_x,
                        cache_y,
                        launcher_alpha as u8,
                    );
                }
            }
        }
    }

    let taskbar_full_redraw = taskbar_dirty || !taskbar.is_valid();
    let taskbar_search_redraw = !taskbar_full_redraw && taskbar.is_search_dirty();
    if taskbar_full_redraw {
        redraw_taskbar(
            taskbar,
            wm,
            add_progress,
            shift_x,
            hover_apps_icon,
            hover_keyboard_icon,
            hover_ime_icon,
            search_focused,
            search_query,
            caret_visible,
            clock_hh,
            clock_mm,
            battery_pct,
            ime_menu_selection,
        );
    } else if taskbar_search_redraw {
        redraw_taskbar_search(taskbar, search_focused, search_query, caret_visible);
    }
    let mut ime_status_partial = false;
    if ime_hover_dirty && !taskbar_full_redraw && !taskbar_search_redraw {
        ime_status_partial = taskbar.redraw_ime_status_strip(
            battery_pct,
            ime_menu_selection,
            hover_keyboard_icon,
            hover_ime_icon,
        );
        if !ime_status_partial {
            redraw_taskbar(
                taskbar,
                wm,
                add_progress,
                shift_x,
                hover_apps_icon,
                hover_keyboard_icon,
                hover_ime_icon,
                search_focused,
                search_query,
                caret_visible,
                clock_hh,
                clock_mm,
                battery_pct,
                ime_menu_selection,
            );
        }
    }
    if ime_menu_open {
        draw_ime_menu(
            layer,
            cached_ime_menu_layer,
            battery_pct,
            ime_menu_selection,
            ime_menu_opacity,
        );
    }
    // Candidate presentation belongs to the OS software keyboard. Keeping it
    // out of the taskbar prevents a desktop-style prediction popup while the
    // keyboard is hidden.
    let _ = (ime_reading, ime_candidates, ime_selected);
    // The scene damage pass restores its clip from the wallpaper before this
    // point.  Even when only the IME pixels changed, the damaged clip can
    // cover other taskbar pixels (for example while the cursor moves into the
    // icon).  Re-composite the cached full taskbar so those pixels cannot be
    // left as wallpaper. `LayerSystem` clips this copy to the actual damage;
    // the IME surface itself was still updated only in its small status strip.
    taskbar.composite_onto(layer, tb_y);
    // OS surfaces are composited after every user window, launcher, IME menu,
    // and the taskbar. The software keyboard therefore cannot be occluded by
    // a normal or always-on-top application window.
    soft_keyboard.draw(layer);
}


