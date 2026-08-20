fn compute_shadow_alpha(w: &Window, _screen_w: i32, _screen_h: i32) -> Option<CachedShadow> {
    let pad = shadow_pad().max(0);
    let (alpha, sw, sh) = compute_rounded_shadow_alpha(w.w, w.h, win_radius(), pad)?;

    Some(CachedShadow {
        win_x: w.x,
        win_y: w.y,
        win_w: w.w,
        win_h: w.h,
        alpha,
        x0: pad as usize,
        y0: pad as usize,
        w: sw,
        h: sh,
    })
}

fn compute_rounded_shadow_alpha(
    width: usize,
    height: usize,
    radius: usize,
    pad: i32,
) -> Option<(Vec<u8>, usize, usize)> {
    let blur_r = pad;
    let r = radius.min(width / 2).min(height / 2) as i32;
    let ww = width as i32;
    let wh = height as i32;
    let sw = (ww + blur_r * 2).max(0) as usize;
    let sh = (wh + blur_r * 2).max(0) as usize;
    if sw == 0 || sh == 0 {
        return None;
    }

    let mut alpha = alloc::vec![0u8; sw * sh];
    let left = blur_r.max(0) as usize;
    let top = blur_r.max(0) as usize;
    let right = left + width;
    let bottom = top + height;
    let radius = r as usize;
    let mask = ShadowMaskPass {
        alpha: alpha.as_mut_ptr(),
        stride: sw,
        left,
        right,
        top,
        bottom,
        radius,
    };
    baram_core::parallel::for_each(height, &mask, fill_shadow_mask_row);
    let box_radius = (blur_r.max(1) as usize / 2).max(1);
    for _ in 0..2 {
        box_blur_shadow(&mut alpha, sw, sh, box_radius);
    }

    Some((alpha, sw, sh))
}

struct ShadowMaskPass {
    alpha: *mut u8,
    stride: usize,
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
    radius: usize,
}

unsafe impl Sync for ShadowMaskPass {}

fn fill_shadow_mask_row(pass: &ShadowMaskPass, local_y: usize) {
    let py = pass.top + local_y;
    if py >= pass.bottom {
        return;
    }
    let r = pass.radius as i32;
    for px in pass.left..pass.right {
        let dx = if px < pass.left + pass.radius {
            pass.left as i32 + r - px as i32
        } else if px >= pass.right - pass.radius {
            px as i32 - (pass.right as i32 - r - 1)
        } else {
            0
        };
        let dy = if py < pass.top + pass.radius {
            pass.top as i32 + r - py as i32
        } else if py >= pass.bottom - pass.radius {
            py as i32 - (pass.bottom as i32 - r - 1)
        } else {
            0
        };
        if dx == 0 || dy == 0 || dx * dx + dy * dy <= r * r {
            unsafe {
                *pass.alpha.add(py * pass.stride + px) = 45;
            }
        }
    }
}

struct ShadowHorizontalPass {
    src: *const u8,
    dst: *mut u8,
    width: usize,
    radius: usize,
}
unsafe impl Sync for ShadowHorizontalPass {}

fn blur_shadow_row(pass: &ShadowHorizontalPass, y: usize) {
    let diameter = pass.radius * 2 + 1;
    let mut sum = 0u32;
    for x in 0..pass.width + pass.radius {
        unsafe {
            if x < pass.width {
                sum += *pass.src.add(y * pass.width + x) as u32;
            }
            if x >= diameter && x - diameter < pass.width {
                sum -= *pass.src.add(y * pass.width + x - diameter) as u32;
            }
            if x >= pass.radius && x - pass.radius < pass.width {
                *pass.dst.add(y * pass.width + x - pass.radius) = (sum / diameter as u32) as u8;
            }
        }
    }
}

struct ShadowVerticalPass {
    src: *const u8,
    dst: *mut u8,
    width: usize,
    height: usize,
    radius: usize,
}
unsafe impl Sync for ShadowVerticalPass {}

fn blur_shadow_column(pass: &ShadowVerticalPass, x: usize) {
    let diameter = pass.radius * 2 + 1;
    let mut sum = 0u32;
    for y in 0..pass.height + pass.radius {
        unsafe {
            if y < pass.height {
                sum += *pass.src.add(y * pass.width + x) as u32;
            }
            if y >= diameter && y - diameter < pass.height {
                sum -= *pass.src.add((y - diameter) * pass.width + x) as u32;
            }
            if y >= pass.radius && y - pass.radius < pass.height {
                *pass.dst.add((y - pass.radius) * pass.width + x) = (sum / diameter as u32) as u8;
            }
        }
    }
}

fn box_blur_shadow(alpha: &mut [u8], width: usize, height: usize, radius: usize) {
    let mut tmp = alloc::vec![0u8; alpha.len()];
    let horizontal = ShadowHorizontalPass {
        src: alpha.as_ptr(),
        dst: tmp.as_mut_ptr(),
        width,
        radius,
    };
    baram_core::parallel::for_each(height, &horizontal, blur_shadow_row);
    let vertical = ShadowVerticalPass {
        src: tmp.as_ptr(),
        dst: alpha.as_mut_ptr(),
        width,
        height,
        radius,
    };
    baram_core::parallel::for_each(width, &vertical, blur_shadow_column);
}

fn draw_title_bar(layer: &mut LayerSystem, w: &Window, ox: i32, oy: i32, skip_blur: bool) {
    let x = ox.max(0) as usize;
    let y = oy.max(0) as usize;
    let sw = layer.width();
    let sh = layer.height();
    if x >= sw || y >= sh {
        return;
    }
    let x1 = (x + w.w).min(sw);
    let y1 = (y + w.h).min(sh);
    let w_draw = x1.saturating_sub(x);
    let h_draw = y1.saturating_sub(y);
    if w_draw == 0 || h_draw == 0 {
        return;
    }
    if !w.chrome_visible {
        layer.fill_rect(
            x,
            y,
            w_draw,
            h_draw,
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
        );
        return;
    }

    let tb_h = title_bar_h().min(h_draw);
    draw_title_bar_background(layer, x, y, w_draw, tb_h, skip_blur, w.warp4_theme);

    let base_x = x as i32 + 10;
    let btn_y = y as i32 + 10;
    let bs = btn_size() as i32;
    let btn_center_x = base_x + bs / 2;
    let btn_center_y = btn_y + bs / 2;

    if btn_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            btn_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    let mini_x = base_x + bs + 5;
    let mini_center_x = mini_x + bs / 2;

    if mini_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            mini_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    let max_x = base_x + bs * 2 + 10;
    let max_center_x = max_x + bs / 2;

    if max_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            max_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    if w.focused {
        if base_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            svg::draw_svg_into_alpha(
                layer,
                CLOSE_ICON_SVG,
                base_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }

        if mini_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            svg::draw_svg_into_alpha(
                layer,
                MIN_ICON_SVG,
                mini_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }

        if max_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            let icon = if w.maximized {
                MINI_ICON_SVG
            } else {
                MAX_ICON_SVG
            };
            svg::draw_svg_into_alpha(
                layer,
                icon,
                max_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }

        let title = w.title_str();
        if !title.is_empty() {
            let title_x = (base_x + bs * 3 + 20) as usize;
            let title_y = (y as i32 + 13) as usize;
            if title_x < sw && title_y < sh {
                layer.put_str(title_x, title_y, title, Color::TEXT);
            }
        }
    }
}

fn draw_settings_permission_overlay(
    layer: &mut LayerSystem,
    width: usize,
    height: usize,
    show_message: bool,
) {
    let content_top = title_bar_h().min(height);
    let buffer_width = layer.width();
    let buffer_height = layer.height();
    let buffer = layer.buf_mut();
    for y in content_top..height.min(buffer_height) {
        let row = y * buffer_width;
        for x in 0..width.min(buffer_width) {
            let index = row + x;
            let color = Color(buffer[index]);
            let blend = |channel: u8| ((channel as u32 * 70 + 255 * 185) / 255) as u8;
            buffer[index] = Color::rgb(blend(color.r()), blend(color.g()), blend(color.b())).0;
        }
    }

    if !show_message {
        return;
    }

    let lines = [
        "操作体系の設定変更を要求しています",
        "確認ウィンドウでアクションを選択してください",
    ];
    let line_height = 24usize;
    let block_height = line_height * lines.len();
    let content_height = height.saturating_sub(content_top);
    let start_y = content_top + content_height.saturating_sub(block_height) / 2;
    for (line_index, text) in lines.iter().enumerate() {
        let text_width = text
            .chars()
            .map(|ch| {
                if baram_font::ttf_font::is_available() {
                    let glyph = baram_font::ttf_font::glyph(ch);
                    if glyph.w > 0 {
                        glyph.advance.max(0) as usize
                    } else {
                        8
                    }
                } else {
                    8
                }
            })
            .sum::<usize>();
        let x = width.saturating_sub(text_width) / 2;
        layer.put_str(
            x,
            start_y + line_index * line_height,
            text,
            Color::rgb(40, 40, 40),
        );
    }
}

fn draw_window_body(layer: &mut LayerSystem, w: &Window, rounded: bool, ox: i32, oy: i32) {
    let x = ox.max(0) as usize;
    let y = oy.max(0) as usize;
    let sw = layer.width();
    let sh = layer.height();
    if x >= sw || y >= sh {
        return;
    }
    let x1 = (x + w.w).min(sw);
    let y1 = (y + w.h).min(sh);
    let w_draw = x1.saturating_sub(x);
    let h_draw = y1.saturating_sub(y);
    if w_draw == 0 || h_draw == 0 {
        return;
    }
    if !w.chrome_visible {
        layer.fill_rect(
            x,
            y,
            w_draw,
            h_draw,
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
        );
        return;
    }

    let (title_bg, body_bg) = if w.focused {
        (
            config::get_color("ui-theme/color/panel", Color::PANEL),
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
        )
    } else {
        (
            config::get_color("ui-theme/color/win_inactive", Color::WIN_INACTIVE),
            config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
        )
    };
    let title_color = if w.focused {
        config::get_color("ui-theme/color/text", Color::TEXT)
    } else {
        config::get_color("ui-theme/color/win_inactive", Color::WIN_INACTIVE)
    };

    if rounded {
        layer.fill_rounded_rect(x, y, w_draw, h_draw, win_radius(), body_bg);
    } else {
        layer.fill_rect(x, y, w_draw, h_draw, body_bg);
    }

    let tb_h = title_bar_h().min(h_draw);
    layer.fill_rect(x, y, w_draw, tb_h, title_bg);

    let base_x = x as i32 + 10;
    let btn_y = y as i32 + 10;
    let bs = btn_size() as i32;
    let btn_center_x = base_x + bs / 2;
    let btn_center_y = btn_y + bs / 2;

    if btn_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            btn_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    let mini_x = base_x + bs + 5;
    let mini_center_x = mini_x + bs / 2;

    if mini_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            mini_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    let max_x = base_x + bs * 2 + 10;
    let max_center_x = max_x + bs / 2;

    if max_center_x + btn_bg_radius() as i32 <= sw as i32
        && btn_center_y + btn_bg_radius() as i32 <= sh as i32
    {
        layer.fill_circle(
            max_center_x as usize,
            btn_center_y as usize,
            btn_bg_radius(),
            btn_bg_color(),
        );
    }

    if w.focused {
        if base_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            svg::draw_svg_into_alpha(
                layer,
                CLOSE_ICON_SVG,
                base_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }

        if mini_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            svg::draw_svg_into_alpha(
                layer,
                MIN_ICON_SVG,
                mini_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }

        if max_x + bs <= sw as i32 && btn_y + bs <= sh as i32 {
            let icon = if w.maximized {
                MINI_ICON_SVG
            } else {
                MAX_ICON_SVG
            };
            svg::draw_svg_into_alpha(
                layer,
                icon,
                max_x + 4,
                btn_y + 4,
                (btn_size() - 8) as f32,
                (btn_size() - 8) as f32,
                77u32,
            );
        }
    }

    layer.put_str(x + btn_area_w(), y + 13, w.title_str(), title_color);
}

fn draw_window_border(_layer: &mut LayerSystem, _w: &Window) {}

fn draw_window(layer: &mut LayerSystem, w: &Window, ox: i32, oy: i32) {
    draw_window_body(layer, w, false, ox, oy);
    draw_window_border(layer, w);
}

