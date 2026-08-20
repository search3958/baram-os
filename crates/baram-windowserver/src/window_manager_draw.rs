impl WindowManager {
    pub fn draw_all(
        &mut self,
        layer: &mut LayerSystem,
        warp_engines: &mut alloc::vec::Vec<(WinId, super::warp::WarpEngine)>,
        html_engines: &mut alloc::vec::Vec<(WinId, super::html::HtmlEngine)>,
    ) {
        if self.windows.is_empty() {
            return;
        }

        let n = self.windows.len();
        let screen_w = layer.width();
        let screen_h = layer.height();

        const MAX_WINDOWS: usize = 16;
        let top_z = self.windows.iter().map(|w| w.z).max().unwrap_or(0) + 1;
        for w in &mut self.windows {
            if w.always_on_top {
                w.z = top_z;
            }
        }
        let sort_n = n.min(MAX_WINDOWS);
        let mut indices = [0usize; MAX_WINDOWS];
        for i in 0..sort_n {
            indices[i] = i;
        }
        for i in 1..sort_n {
            let mut j = i;
            while j > 0 && self.windows[indices[j - 1]].z > self.windows[indices[j]].z {
                indices.swap(j - 1, j);
                j -= 1;
            }
        }

        for i in 0..sort_n {
            let idx = indices[i];
            let w = &self.windows[idx];
            if !w.visible || (w.minimized && !w.is_motion_animating()) || w.maximized {
                continue;
            }
            let entry = self.shadow_cache.iter_mut().find(|(wid2, _)| *wid2 == w.id);
            if let Some((_, ref mut cache_opt)) = entry {
                let need_recompute = match cache_opt {
                    Some(c) => c.win_w != w.w || c.win_h != w.h,
                    None => true,
                };
                if need_recompute {
                    *cache_opt = compute_shadow_alpha(w, self.screen_w, self.screen_h);
                }
                if let Some(ref mut c) = cache_opt {
                    c.win_x = w.x;
                    c.win_y = w.y;
                }
            }
        }

        // Allocate on the BSP; AP jobs below only touch disjoint window layers.
        for i in 0..sort_n {
            let idx = indices[i];
            if self.windows[idx].visible
                && (!self.windows[idx].minimized || self.windows[idx].is_motion_animating())
            {
                self.windows[idx].ensure_layer(screen_w, screen_h);
            }
        }
        let body_bg = config::get_color("ui-theme/color/win_bg", Color::WIN_BG);
        let radius = win_radius();
        let title_height = title_bar_h();
        let mut redraw_polygons: Vec<Vec<(f32, f32)>> = Vec::new();
        for i in 0..sort_n {
            let w = &self.windows[indices[i]];
            if w.visible
                && (!w.minimized || w.is_motion_animating())
                && w.content_dirty
                && w.content_damage.is_none()
                && !w.maximized
            {
                redraw_polygons.push(LayerSystem::squircle_polygon(
                    w.w as f32,
                    w.h as f32,
                    radius.min(w.w / 2).min(w.h / 2) as f32,
                ));
            }
        }
        let mut polygon_index = 0usize;
        let mut redraw_jobs: Vec<WindowBaseRedraw> = Vec::new();
        for i in 0..sort_n {
            let w = &mut self.windows[indices[i]];
            if !w.visible || (w.minimized && !w.is_motion_animating()) || !w.content_dirty {
                continue;
            }
            let (polygon, polygon_len) = if w.content_damage.is_none() && !w.maximized {
                let poly = &redraw_polygons[polygon_index];
                polygon_index += 1;
                (poly.as_ptr(), poly.len())
            } else {
                (core::ptr::null(), 0)
            };
            redraw_jobs.push(WindowBaseRedraw {
                layer: w.layer.as_mut().unwrap() as *mut LayerSystem,
                width: w.w,
                height: w.h,
                damage: w.content_damage,
                maximized: w.maximized,
                body_bg,
                title_height,
                radius,
                polygon,
                polygon_len,
            });
        }
        baram_core::parallel::for_each(redraw_jobs.len(), &redraw_jobs, redraw_window_base);

        for i in 0..sort_n {
            let idx = indices[i];
            if !self.windows[idx].visible
                || (self.windows[idx].minimized && !self.windows[idx].is_motion_animating())
            {
                continue;
            }
            self.windows[idx].ensure_layer(screen_w, screen_h);

            let wx = self.windows[idx].x;
            let wy = self.windows[idx].render_y();
            let ww = self.windows[idx].w;
            let wh = self.windows[idx].h;
            let scroll_y = self.windows[idx].scroll_y;
            let win_id = self.windows[idx].id;
            let is_max = self.windows[idx].maximized;
            let shadow_dirty = self.windows[idx].shadow_dirty;
            let content_dirty = self.windows[idx].content_dirty;
            if !is_max {
                if shadow_dirty {
                    if let Some(entry) = self.shadow_cache.iter().find(|(wid2, _)| *wid2 == win_id)
                    {
                        if let Some(ref cache) = entry.1 {
                            let old_sx = (self.windows[idx].prev_x - shadow_pad()).max(0) as usize;
                            let old_sy = (self.windows[idx].prev_y - shadow_pad()).max(0) as usize;
                            let new_sx = (self.windows[idx].x - shadow_pad()).max(0) as usize;
                            let new_sy = (self.windows[idx].y - shadow_pad()).max(0) as usize;
                            let shadow_layer = self.windows[idx].shadow_layer.as_mut().unwrap();
                            let slw = shadow_layer.width();
                            let slh = shadow_layer.height();
                            let scx0 = old_sx.min(new_sx);
                            let scy0 = old_sy.min(new_sy);
                            let scx1 = (old_sx + cache.w).max(new_sx + cache.w).min(slw);
                            let scy1 = (old_sy + cache.h).max(new_sy + cache.h).min(slh);
                            if scx1 > scx0 && scy1 > scy0 {
                                for row in scy0..scy1 {
                                    let start = row * slw + scx0;
                                    let end = row * slw + scx1;
                                    shadow_layer.buf_mut()[start..end].fill(Color::TRANSPARENT.0);
                                }
                            }
                            let shadow_buf = shadow_layer.buf_mut();
                            for py in 0..cache.h {
                                let alpha_row = py * cache.w;
                                for px in 0..cache.w {
                                    let a = cache.alpha[alpha_row + px];
                                    if a == 0 {
                                        continue;
                                    }
                                    if px >= slw || py >= slh {
                                        continue;
                                    }
                                    shadow_buf[py * slw + px] = 0x0000_0000 | (a as u32);
                                }
                            }
                            self.windows[idx].shadow_dirty = false;
                        }
                    }
                }

                if let Some(entry) = self.shadow_cache.iter().find(|(wid2, _)| *wid2 == win_id) {
                    if entry.1.is_some() {
                        let shadow_ref = self.windows[idx].shadow_layer.as_ref().unwrap();
                        let shadow_size = ww + shadow_pad() as usize * 2;
                        let shadow_h = wh + shadow_pad() as usize * 2;
                        let shadow_x = wx - shadow_pad() as i32;
                        let shadow_y = wy - shadow_pad() as i32;

                        let src_x = if shadow_x < 0 {
                            (-shadow_x) as usize
                        } else {
                            0
                        };
                        let src_y = if shadow_y < 0 {
                            (-shadow_y) as usize
                        } else {
                            0
                        };
                        let dst_x = shadow_x.max(0) as usize;
                        let dst_y = shadow_y.max(0) as usize;
                        let draw_w = (shadow_size as i32 - src_x as i32).max(0) as usize;
                        let draw_h = (shadow_h as i32 - src_y as i32).max(0) as usize;

                        if draw_w > 0 && draw_h > 0 {
                            layer.composit_shadow_alpha(
                                shadow_ref, dst_x, dst_y, src_x, src_y, draw_w, draw_h,
                            );
                        }
                    }
                }
            }

            if content_dirty {
                let chrome_h = if self.windows[idx].chrome_visible {
                    title_bar_h()
                } else {
                    0
                };
                let skip_title_blur = self.windows[idx].is_motion_animating()
                    || html_engines
                        .iter()
                        .any(|(id, engine)| *id == win_id && engine.is_animating());
                let layer_ptr = self.windows[idx].layer.as_mut().unwrap() as *mut LayerSystem;
                let w_ptr = &self.windows[idx] as *const Window;
                let damage = self.windows[idx].content_damage.take();
                unsafe {
                    let lw = (*layer_ptr).width();
                    let lh = (*layer_ptr).height();
                    let (cx0, cy0, cx1, cy1) = damage.unwrap_or((0, 0, lw, lh));
                    (*layer_ptr).push_clip(cx0, cy0, cx1, cy1);

                    // A Warp3 hover patch owns every pixel in its damage rect.
                    // Do not enter generic window chrome/body rendering here:
                    // some SVG/font paths are not damage-clip aware and would
                    // touch title-bar pixels outside the hovered control.
                    // Base clearing/fill ran in parallel. SVG, font and engine
                    // caches remain on the BSP because they can allocate.

                    for i in 0..warp_engines.len() {
                        if win_id == warp_engines[i].0 {
                            let engine = &mut warp_engines[i].1;
                            (*layer_ptr).push_clip(0, chrome_h, ww, wh);
                            engine.draw_to_layer(&mut *layer_ptr, 0, -scroll_y);
                            engine.draw_texts(&mut *layer_ptr, 0, -scroll_y, 1.0);
                            (*layer_ptr).pop_clip();
                            break;
                        }
                    }
                    for i in 0..html_engines.len() {
                        if win_id == html_engines[i].0 {
                            let engine = &mut html_engines[i].1;
                            let content_top = if engine.is_warp3() { 0 } else { chrome_h };
                            (*layer_ptr).push_clip(0, content_top, ww, wh);
                            engine.draw_to_layer(&mut *layer_ptr, 0, -scroll_y);
                            (*layer_ptr).pop_clip();
                            break;
                        }
                    }
                    if let Some(dialog) = self
                        .file_dialog
                        .as_ref()
                        .filter(|dialog| dialog.win_id() == win_id)
                    {
                        (*layer_ptr).push_clip(0, chrome_h, ww, wh);
                        dialog.draw_to_layer(&mut *layer_ptr, chrome_h as i32);
                        (*layer_ptr).pop_clip();
                    }
                    if self.interaction_blocked == Some(win_id) {
                        draw_settings_permission_overlay(
                            &mut *layer_ptr,
                            ww,
                            wh,
                            self.file_dialog.is_none(),
                        );
                    }
                    // The Warp3 document reaches the top of the window and
                    // therefore sits behind the title bar. Repaint the full
                    // chrome only when this damage touches it; body-only
                    // hover patches retain the cheap, clipped path.
                    let repaint_title = self.windows[idx].chrome_visible
                        && damage.map_or(true, |(_, y0, _, _)| y0 < chrome_h);
                    (*layer_ptr).pop_clip();
                    if repaint_title {
                        draw_title_bar(&mut *layer_ptr, &*w_ptr, 0, 0, skip_title_blur);
                    }
                }
                self.windows[idx].prev_x = self.windows[idx].x;
                self.windows[idx].prev_y = self.windows[idx].y;
                self.windows[idx].prev_render_y_offset = self.windows[idx].render_y_offset;
                self.windows[idx].content_dirty = false;
            }

            let win_layer = self.windows[idx].layer.as_ref().unwrap();
            let _screen_w = layer.width() as i32;
            let _screen_h = layer.height() as i32;

            let src_x = if wx < 0 { (-wx) as usize } else { 0 };
            let src_y = if wy < 0 { (-wy) as usize } else { 0 };
            let dst_x = wx.max(0) as usize;
            let dst_y = wy.max(0) as usize;
            let draw_w = (ww as i32 - src_x as i32).max(0) as usize;
            let draw_h = (wh as i32 - src_y as i32).max(0) as usize;

            if draw_w == 0 || draw_h == 0 {
                continue;
            }

            if is_max {
                layer.composit_rect(win_layer, dst_x, dst_y, src_x, src_y, draw_w, draw_h);
            } else {
                layer.composit_rounded(
                    win_layer,
                    dst_x,
                    dst_y,
                    src_x,
                    src_y,
                    draw_w,
                    draw_h,
                    win_radius(),
                );
                draw_window_border(layer, &self.windows[idx]);
            }
            self.windows[idx].prev_x = self.windows[idx].x;
            self.windows[idx].prev_y = self.windows[idx].y;
            self.windows[idx].prev_render_y_offset = self.windows[idx].render_y_offset;
            self.windows[idx].prev_w = self.windows[idx].w;
            self.windows[idx].prev_h = self.windows[idx].h;
        }
    }

}

