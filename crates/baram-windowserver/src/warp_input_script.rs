impl WarpEngine {
    pub fn click(&mut self, x: i32, y: i32) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.click(x, y);
            self.hover_idx = engine.hovered_node();
            self.focused_input = engine.has_focused_input().then_some(0);
            self.focused_input_var = if self.focused_input.is_some() {
                "__warp4__".into()
            } else {
                String::new()
            };
            self.last_command = engine.take_command();
            return;
        }
        self.parse_current_screen();
        self.last_clicked_id = None;
        let tb_h = crate::window::title_bar_h() as i32;
        if y < tb_h {
            self.dirty = true;
            return;
        }
        for i in (0..self.nodes.len()).rev() {
            if !self.nodes[i].visible {
                continue;
            }
            let n = &self.nodes[i];
            if x >= n.x && x <= n.x + n.w && y >= n.y && y <= n.y + n.h {
                let tag = n.tag.clone();
                if tag == "switch" {
                    let out_var = self.parse_out_var(i);
                    if !out_var.is_empty() {
                        let current = self.get_state(&out_var);
                        if !current.contains("Disabled") {
                            let on = current.contains("true");
                            self.set_state(&out_var, if on { "false" } else { "true" });
                            let ev = self.nodes[i].event_oneclick.clone();
                            if !ev.is_empty() {
                                self.execute_action(&ev);
                            }
                        }
                    }
                    break;
                }
                if tag == "button" || tag == "tonalButton" {
                    let id = self.nodes[i].get_id();
                    if !id.is_empty() {
                        self.last_clicked_id = Some(id.to_string());
                    }
                    let ev = self.nodes[i].event_oneclick.clone();
                    if !ev.is_empty() {
                        self.execute_action(&ev);
                    }
                    self.focused_input = None;
                    self.focused_input_var.clear();
                    break;
                }
                if tag == "input" {
                    self.focused_input = Some(i);
                    self.focused_input_var = self.parse_out_var(i);
                    self.dirty = true;
                    break;
                }
                let ev = self.nodes[i].event_oneclick.clone();
                if !ev.is_empty() {
                    self.execute_action(&ev);
                    break;
                }
            }
        }
        self.dirty = true;
    }

    /// Returns the last clicked control id without exposing an application
    /// command or URI. OS-owned Warp surfaces use this as their input bridge.
    pub fn take_clicked_id(&mut self) -> Option<String> {
        if let Some(engine) = self.warp4.as_mut() {
            return engine.take_clicked_id();
        }
        self.last_clicked_id.take()
    }

    pub fn handle_key(&mut self, c: u8) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.handle_key(c);
            return;
        }
        if c == 0x08 || c == 0x7F {
            self.handle_text("", 1);
        } else if c >= 0x20 && c < 0x7F {
            let mut text = [0u8; 1];
            text[0] = c;
            let text = unsafe { core::str::from_utf8_unchecked(&text) };
            self.handle_text(text, 0);
        }
    }

    /// Replaces the active IME composition and accepts UTF-8 text.
    pub fn handle_text(&mut self, text: &str, replace_chars: usize) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.handle_text(text, replace_chars);
            return;
        }
        if self.focused_input_var.is_empty() {
            self.focused_input = None;
            return;
        }
        let out_var = self.focused_input_var.clone();
        let mut val = self.get_state(&out_var);
        for _ in 0..replace_chars {
            val.pop();
        }
        val.push_str(text);
        self.set_state(&out_var, &val);
        self.dirty = true;
    }

    fn execute_script(&mut self, name: &str) {
        let scripts = self.scripts.clone();
        for script in &scripts {
            if script.name == name {
                let mut any_matched = false;
                for block in &script.blocks {
                    let cond = &block.condition;
                    if block.r#type == "elseIf" && any_matched {
                        continue;
                    }
                    if cond.is_empty() {
                        let actions = block.actions.clone();
                        self.execute_action(&actions);
                        any_matched = true;
                    } else if cond.contains('=') {
                        let parts: alloc::vec::Vec<&str> = cond.splitn(2, '=').collect();
                        let lv = self.eval_expr(parts[0]);
                        let rv = self.eval_expr(parts[1]);
                        if lv == rv {
                            let actions = block.actions.clone();
                            self.execute_action(&actions);
                            any_matched = true;
                        }
                    }
                }
                return;
            }
        }
    }

    fn execute_action(&mut self, action_str: &str) {
        if action_str.is_empty() {
            return;
        }
        let actions: alloc::vec::Vec<&str> = action_str.split(',').collect();
        for act in actions {
            let act = act.trim();
            if act.starts_with("setScreen{") {
                let scr = act[10..].trim_end_matches('}');
                self.set_state("_currentScreen", scr);
            } else if act.starts_with("script{") {
                let sn = act[7..].trim_end_matches('}');
                self.execute_script(sn);
            } else if act.starts_with("hide{") {
                let id = act[5..].trim_end_matches('}');
                for n in &mut self.nodes {
                    if n.get_id() == id {
                        n.visible = false;
                    }
                }
            } else if act.starts_with("show{") {
                let id = act[5..].trim_end_matches('}');
                for n in &mut self.nodes {
                    if n.get_id() == id {
                        n.visible = true;
                    }
                }
            } else if act.starts_with("add{") {
                let inner = &act[4..].trim_end_matches('}');
                let parts: alloc::vec::Vec<&str> = inner.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let container_id = parts[0].trim();
                    let _child_src = parts[1].trim().trim_matches('"').trim_matches('\'');
                    let container_idx = self.find_node_by_id(container_id);
                    if let Some(ci) = container_idx {
                        let new_idx = self.alloc_node().unwrap_or(0);
                        self.nodes[new_idx].tag = String::from("button");
                        self.nodes[new_idx].visible = true;
                        self.nodes[new_idx].attrs.push(Attr {
                            key: String::from("text"),
                            value: String::from("\"ボタンを追加\""),
                        });
                        self.nodes[ci].children.push(new_idx);
                    }
                }
            } else if act.starts_with("del{") {
                let id = act[4..].trim_end_matches('}');
                let container_idx = self.find_node_by_id(id);
                if let Some(ci) = container_idx {
                    if let Some(last) = self.nodes[ci].children.pop() {
                        self.nodes[last].visible = false;
                    }
                }
            } else if act.starts_with("clr{") {
                let id = act[4..].trim_end_matches('}');
                let container_idx = self.find_node_by_id(id);
                if let Some(ci) = container_idx {
                    let children: alloc::vec::Vec<usize> = self.nodes[ci].children.clone();
                    for child_idx in children {
                        self.nodes[child_idx].visible = false;
                    }
                    self.nodes[ci].children.clear();
                }
            } else if act.starts_with("runCommand") {
                if let Some(eq_pos) = act.find('=') {
                    let rhs = act[eq_pos + 1..].trim();
                    let cmd = self.eval_expr(rhs);
                    if !cmd.is_empty() {
                        self.last_command = Some(cmd);
                    }
                }
            } else if act.contains('.') {
                let parts: alloc::vec::Vec<&str> = act.splitn(2, '.').collect();
                let id = parts[0];
                let method_with_args = parts[1];
                if let Some(open_b) = method_with_args.find('{') {
                    let method = &method_with_args[..open_b];
                    let args = &method_with_args[open_b + 1..].trim_end_matches('}');
                    if method == "changeContent" {
                        let val = self.eval_expr(args);
                        let key = format!("--{}Content", id);
                        self.set_state(&key, &val);
                    } else if method == "setStatus" {
                        if args.trim() == "unset" {
                            let key = format!("--{}Disabled", id);
                            self.set_state(&key, "false");
                        } else {
                            let key = format!("--{}Disabled", id);
                            self.set_state(&key, args);
                        }
                    }
                }
            } else if act.contains('=') || act.contains(':') {
                let parts: alloc::vec::Vec<&str> = if act.contains('=') {
                    act.splitn(2, '=').collect()
                } else {
                    act.splitn(2, ':').collect()
                };
                let var_name = parts[0].trim();
                let rhs = parts[1].trim();
                let val = if rhs.starts_with("calc{") {
                    let m_expr = rhs[5..].trim_end_matches('}');
                    let m_expanded = self.eval_expr(m_expr);
                    self.eval_math(&m_expanded).to_string()
                } else if rhs.contains(".replace{") {
                    let p: alloc::vec::Vec<&str> = rhs.splitn(2, ".replace{").collect();
                    let base = self.eval_expr(p[0]);
                    let args = p[1].trim_end_matches('}');
                    let rp: alloc::vec::Vec<&str> = args.splitn(2, ',').collect();
                    let old_s = self.eval_expr(rp[0]);
                    let new_s = self.eval_expr(rp[1]);
                    if !old_s.is_empty() {
                        base.replace(&old_s, &new_s)
                    } else {
                        base
                    }
                } else {
                    self.eval_expr(rhs)
                };
                self.set_state(var_name, &val);
            }
        }
    }

    fn find_node_by_id(&self, id: &str) -> Option<usize> {
        for (i, n) in self.nodes.iter().enumerate() {
            if n.get_id() == id {
                return Some(i);
            }
        }
        None
    }

    pub fn get_state_value(&self, key: &str) -> Option<&str> {
        for s in &self.state {
            if s.0 == key {
                return Some(&s.1);
            }
        }
        None
    }

    pub fn set_state_value(&mut self, key: &str, val: &str) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_state_value(key, val);
        } else {
            self.set_state(key, val);
        }
    }
}

