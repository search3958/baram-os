impl Warp3Engine {
    fn run_click(&mut self, idx: usize) {
        let classes = self.nodes[idx].classes.clone();
        for class in classes {
            let sections = self.scripts.clone();
            for section in sections {
                if section.kind == SectionKind::Click && section.name == class {
                    self.execute(section.actions);
                }
            }
        }
    }

    fn execute(&mut self, actions: Vec<(String, String)>) {
        self.script_frames.push((actions, 0, None));
        if self.script_wait_until_ns.is_none() {
            self.resume_script();
        }
    }

    fn resume_script(&mut self) {
        while !self.script_frames.is_empty() {
            let finished = {
                let frame = self.script_frames.last().unwrap();
                frame.1 >= frame.0.len()
            };
            if finished {
                self.script_frames.pop();
                continue;
            }
            let (left, right) = {
                let frame = self.script_frames.last_mut().unwrap();
                let action = frame.0[frame.1].clone();
                frame.1 += 1;
                action
            };
            match left.as_str() {
                "screen" => {
                    self.screen = unquote(&right);
                    self.begin_screen_transition();
                    self.load_screen();
                }
                "scroll" => self.request_scroll(&right),
                "print" => {}
                "wait" => {
                    let duration_ns = parse_wait_ns(&self.value(&right));
                    if duration_ns > 0 {
                        self.script_wait_until_ns =
                            Some(self.animation_now_ns.saturating_add(duration_ns));
                        return;
                    }
                }
                "run" => {
                    let command = unquote(&right);
                    if command.starts_with("setup://")
                        || command.starts_with("os://")
                        || command.starts_with("app://")
                        || command.starts_with("security://")
                    {
                        self.command_queue.push(command);
                        // Yield once so the window server can execute the URI
                        // before a following action reads the changed value.
                        self.script_wait_until_ns = Some(self.animation_now_ns.saturating_add(1));
                        return;
                    }
                }
                command if command.starts_with("runSwitch ") => {
                    let class = command.trim_start_matches("runSwitch ").trim();
                    let enabled = self
                        .nodes
                        .iter()
                        .find(|node| {
                            node.is("switch") && node.classes.iter().any(|item| item == class)
                        })
                        .map_or(false, |node| node.prop("default") == "true");
                    let uri = alloc::format!("{}{}", unquote(&right), enabled);
                    self.command_queue.push(uri);
                    self.script_wait_until_ns = Some(self.animation_now_ns.saturating_add(1));
                    return;
                }
                "fun" => {
                    let name = unquote(&right);
                    if self
                        .script_frames
                        .iter()
                        .any(|(_, _, active)| active.as_ref() == Some(&name))
                    {
                        continue;
                    }
                    if let Some(section) = self.scripts.iter().find(|section| {
                        section.kind == SectionKind::Function && section.name == name
                    }) {
                        self.script_frames
                            .push((section.actions.clone(), 0, Some(name)));
                    }
                }
                command if command.starts_with("setText ") => {
                    let name = command.trim_start_matches("setText ").trim();
                    let value = self.value(&right);
                    self.set_element_text(name, &value);
                }
                command if command.starts_with("getText ") => {
                    let variable = command.trim_start_matches("getText ").trim();
                    let value = self.element_text(unquote(&right).as_str());
                    self.set_state(variable, &value);
                }
                variable => {
                    let value = if let Some(raw) = right.trim().strip_prefix("append ") {
                        let mut value = self.state(variable);
                        value.push_str(&self.value(raw));
                        value
                    } else if let Some(raw) = right.trim().strip_prefix("calculate ") {
                        eval_math(&self.value(raw)).to_string()
                    } else if right.trim() == "+1" || right.trim() == "-1" {
                        let delta = if right.trim().starts_with('-') { -1 } else { 1 };
                        (self.state(variable).parse::<i32>().unwrap_or(0) + delta).to_string()
                    } else {
                        self.value(&right)
                    };
                    self.set_state(variable, &value);
                }
            }
        }
    }

    fn request_scroll(&mut self, raw: &str) {
        let value = unquote(raw);
        if value == "+1" || value == "-1" {
            let mut points: Vec<i32> = self
                .nodes
                .iter()
                .filter(|node| node.is("scroll-point"))
                .map(|node| node.y)
                .collect();
            points.sort();
            let delta = if value.starts_with('-') { -1 } else { 1 };
            let current = points
                .iter()
                .rposition(|y| *y <= self.scroll + 40)
                .unwrap_or(0) as i32;
            let next = (current + delta).clamp(0, points.len().saturating_sub(1) as i32) as usize;
            self.scroll_request = points.get(next).copied();
        } else {
            self.scroll_request = self
                .nodes
                .iter()
                .find(|node| {
                    node.is("scroll-point")
                        && (node.classes.iter().any(|class| class == &value)
                            || node.prop("text") == value)
                })
                .map(|node| node.y);
        }
    }

    fn value(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.starts_with('"') {
            unquote(trimmed)
        } else if let Some(path) = trimmed.strip_prefix("now://") {
            self.now_value(path).unwrap_or_default()
        } else if let Some(path) = trimmed.strip_prefix("os://") {
            baram_bsd::config::get_config()
                .get(path.trim_end_matches('/'))
                .unwrap_or("")
                .to_string()
        } else {
            let state = self.state(trimmed);
            if state.is_empty() {
                trimmed.to_string()
            } else {
                state
            }
        }
    }

    fn now_value(&self, path: &str) -> Option<String> {
        let time = runtime::get_time().ok()?;
        let timezone_minutes = baram_bsd::config::timezone_offset_minutes();
        let utc_seconds =
            time.hour() as i32 * 3600 + time.minute() as i32 * 60 + time.second() as i32;
        let local_seconds = (utc_seconds + timezone_minutes * 60).rem_euclid(24 * 3600);
        let hour = (local_seconds / 3600) as u8;
        let minute = ((local_seconds / 60) % 60) as u8;
        let second = (local_seconds % 60) as u8;
        format_now_value(path, self.now, hour, minute, second)
    }

    fn state(&self, name: &str) -> String {
        self.state
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    }

    fn set_state(&mut self, name: &str, value: &str) {
        if let Some((_, current)) = self.state.iter_mut().find(|(key, _)| key == name) {
            *current = value.to_string();
        } else {
            self.state.push((name.to_string(), value.to_string()));
        }
    }

    fn element_text(&self, class: &str) -> String {
        self.nodes
            .iter()
            .find(|node| node.classes.iter().any(|item| item == class))
            .map(|node| node.prop("text").to_string())
            .unwrap_or_default()
    }

    fn set_element_text(&mut self, class: &str, value: &str) {
        if let Some(idx) = self
            .nodes
            .iter_mut()
            .position(|node| node.classes.iter().any(|item| item == class))
        {
            if self.nodes[idx].prop("text") == value {
                return;
            }
            let y = self.nodes[idx].y;
            set_prop(&mut self.nodes[idx], "text", value);
            self.invalidate_from(y);
        }
    }
}

