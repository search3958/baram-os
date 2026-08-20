impl WarpEngine {
    fn parse_out_var(&self, idx: usize) -> String {
        let raw = self.get_attr_raw(idx, "output");
        if raw.starts_with('(') {
            let end = raw.find(')').unwrap_or(raw.len());
            raw[1..end].to_string()
        } else {
            raw
        }
    }

    pub fn set_hover(&mut self, x: i32, y: i32) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_hover(x, y);
            self.hover_idx = engine.hovered_node();
            return;
        }
        self.parse_current_screen();
        let tb_h = crate::window::title_bar_h() as i32;
        if y < tb_h {
            if self.hover_idx.is_some() {
                self.hover_idx = None;
                self.dirty = true;
            }
            return;
        }
        let mut found = None;
        for i in (0..self.nodes.len()).rev() {
            if !self.nodes[i].visible {
                continue;
            }
            let tag = self.nodes[i].tag.as_str();
            if tag != "button" && tag != "tonalButton" {
                continue;
            }
            let n = &self.nodes[i];
            if x >= n.x && x <= n.x + n.w && y >= n.y && y <= n.y + n.h {
                found = Some(i);
                break;
            }
        }
        if self.hover_idx != found {
            self.hover_idx = found;
            self.dirty = true;
        }
    }

    pub fn clear_hover(&mut self) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.clear_hover();
            self.hover_idx = None;
            return;
        }
        if self.hover_idx.is_some() {
            self.hover_idx = None;
            self.dirty = true;
        }
    }

    pub fn release(&mut self) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.release();
        }
    }

    pub fn draw_to_layer(&mut self, layer: &mut LayerSystem, ox: i32, oy: i32) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.draw_to_layer(layer, ox, oy);
            return;
        }
        let layer_w = layer.width() as i32;
        let layer_h = layer.height() as i32;
        for idx in 0..self.nodes.len() {
            if !self.nodes[idx].visible {
                continue;
            }
            let tag = self.nodes[idx].tag.as_str();
            let n = &self.nodes[idx];
            let nx = n.x + ox;
            let ny = n.y + oy;
            let nw = n.w;
            let nh = n.h;

            if nx + nw <= 0 || ny + nh <= 0 || nx >= layer_w || ny >= layer_h {
                continue;
            }

            let (x, y, w, h) = if ny < 0 {
                (nx.max(0) as usize, 0usize, nw as usize, (nh + ny) as usize)
            } else {
                (nx.max(0) as usize, ny as usize, nw as usize, nh as usize)
            };

            match tag {
                "card" => {
                    let card_bg = config::get_color("ui-theme/color/card_bg", Color::CARD_BG);
                    let radius = config::get_usize("ui-theme/card/radius", 12);
                    layer.fill_rounded_rect(x, y, w, h, radius, card_bg);
                }
                "button" => {
                    let c = if self.hover_idx == Some(idx) {
                        config::get_color(
                            "ui-theme/color/btn_primary_hover",
                            Color::BTN_PRIMARY_HOVER,
                        )
                    } else {
                        config::get_color("ui-theme/color/btn_primary", Color::BTN_PRIMARY)
                    };
                    let radius = config::get_usize("ui-theme/button/corner", 20);
                    layer.fill_rounded_rect(x, y, w, h, radius, c);
                }
                "tonalButton" => {
                    let c = if self.hover_idx == Some(idx) {
                        config::get_color("ui-theme/color/btn_tonal_hover", Color::BTN_TONAL_HOVER)
                    } else {
                        config::get_color("ui-theme/color/btn_tonal", Color::BTN_TONAL)
                    };
                    let radius = config::get_usize("ui-theme/button/corner", 20);
                    layer.fill_rounded_rect(x, y, w, h, radius, c);
                }
                "switch" => {
                    let out_var = self.parse_out_var(idx);
                    let val = self.get_state(&out_var);
                    let on = val.contains("true");
                    let bg = if on {
                        config::get_color("ui-theme/color/switch_on", Color::SWITCH_ON)
                    } else {
                        config::get_color("ui-theme/color/switch_off", Color::SWITCH_OFF)
                    };
                    let sw = config::get_usize("ui-theme/switch/w", 44);
                    let sh = config::get_usize("ui-theme/switch/h", 44);
                    let sr = config::get_usize("ui-theme/switch/radius", 22);
                    let sx = (nx + (nw - sw as i32) / 2).max(0) as usize;
                    let sy = (ny + (nh - sh as i32) / 2).max(0) as usize;
                    layer.fill_rounded_rect(sx, sy, sw, sh, sr, bg);
                }
                "input" => {
                    layer.fill_rounded_rect(
                        x,
                        y,
                        w,
                        h,
                        8,
                        config::get_color("ui-theme/color/win_bg", Color::WIN_BG),
                    );
                    let out_var_name = self.get_attr(idx, "output");
                    let border_color = if self.focused_input_var == out_var_name {
                        config::get_color("ui-theme/color/btn_primary", Color::BTN_PRIMARY)
                    } else {
                        config::get_color("ui-theme/color/border", Color::BORDER)
                    };
                    let bg = config::get_color("ui-theme/color/win_bg", Color::WIN_BG);
                    layer.rounded_rect_outline(x, y, w, h, 8, border_color, bg);
                }
                _ => {}
            }
        }
    }

    pub fn draw_texts(&self, layer: &mut LayerSystem, ox: i32, oy: i32, _scale: f32) {
        let layer_w = layer.width() as i32;
        let layer_h = layer.height() as i32;
        for t in &self.texts {
            if t.text.is_empty() {
                continue;
            }
            let base_x = t.x + ox;
            let base_y = t.y + oy;
            if base_y >= layer_h {
                continue;
            }
            for (i, line) in t.text.split('\n').enumerate() {
                if line.is_empty() {
                    continue;
                }
                let y = base_y + (i as i32) * 22;
                if base_x >= layer_w || y >= layer_h || y < 0 {
                    continue;
                }
                let draw_x = base_x.max(0) as usize;
                let draw_y = y.max(0) as usize;
                layer.put_str(draw_x, draw_y, line, t.color);
            }
        }
        if let Some(idx) = self.focused_input {
            if self.caret_visible && self.nodes.get(idx).is_some_and(|node| node.visible) {
                let node = &self.nodes[idx];
                let value = self.get_state(&self.parse_out_var(idx));
                text_cursor::draw(
                    layer,
                    node.x + ox + 10 + measure_text_width(&value, 16.0),
                    node.y + oy + 7,
                    20,
                    config::get_color("ui-theme/color/text", Color::TEXT),
                );
            }
        }
    }

}

