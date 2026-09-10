use super::*;
#[derive(Clone)]
pub(crate) enum Action {
    Command {
        name: String,
        target: String,
        value: String,
    },
    If {
        left: String,
        op: String,
        right: String,
        body: Vec<Action>,
    },
    Call(String),
    Break,
}

#[derive(Clone, Default)]
pub(crate) struct Script {
    pub(crate) init: Vec<Action>,
    pub(crate) clicks: Vec<(String, Vec<Action>)>,
    pub(crate) functions: Vec<(String, Vec<Action>)>,
}

impl Warp4Engine {
    pub(crate) fn run_click_actions(&mut self, idx: usize) {
        let id = self.nodes[idx].id().to_string();
        if !id.is_empty() {
            if let Some((_, actions)) = self
                .script
                .clicks
                .iter()
                .find(|(name, _)| name == &id)
                .cloned()
            {
                self.execute(&actions);
            }
        }
    }

    pub(crate) fn spinner_item_count(&self, idx: usize) -> usize {
        let count = self.nodes[idx]
            .attr("items")
            .split(',')
            .filter(|item| !item.trim().is_empty())
            .count();
        count.max(3)
    }

    pub(crate) fn spinner_popup_rect(&self, idx: usize) -> (i32, i32, i32, i32) {
        let node = &self.nodes[idx];
        let row_h = ui_px(36);
        let h = self.spinner_item_count(idx) as i32 * row_h + ui_px(8);
        let bottom = self.node_screen_y(idx) + node.h;
        let layer_h = self.height + self.chrome_height;
        let y = if bottom + h <= layer_h {
            bottom
        } else {
            (self.node_screen_y(idx) - h).max(self.chrome_height)
        };
        (node.x, y, node.w.max(1), h)
    }

    pub(crate) fn spinner_popup_hit(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        let idx = self.spinner_open?;
        let node = self.nodes.get(idx)?;
        if !node.visible() || !node.is("Spinner") {
            return None;
        }
        let (_, popup_y, popup_w, popup_h) = self.spinner_popup_rect(idx);
        let screen_y = y - self.scroll;
        if x < node.x
            || x >= node.x + popup_w
            || screen_y < popup_y
            || screen_y >= popup_y + popup_h
        {
            return None;
        }
        let local_y = screen_y - popup_y - ui_px(4);
        if local_y < 0 {
            return None;
        }
        let item = (local_y / ui_px(36).max(1)) as usize;
        (item < self.spinner_item_count(idx)).then_some((idx, item))
    }

    pub(crate) fn execute(&mut self, actions: &[Action]) {
        for (pos, action) in actions.iter().take(MAX_ACTIONS).enumerate() {
            if self.wait_until_ns.is_some() {
                self.pending.extend_from_slice(&actions[pos..]);
                return;
            }
            if self.break_requested {
                return;
            }
            match action {
                Action::If {
                    left,
                    op,
                    right,
                    body,
                } => {
                    let a = self.value(left);
                    let b = self.value(right);
                    let yes = match op.as_str() {
                        "=" | "==" => a == b,
                        "!=" => a != b,
                        "<" => a < b,
                        ">" => a > b,
                        _ => false,
                    };
                    if yes {
                        self.execute(body);
                    }
                }
                Action::Command {
                    name,
                    target,
                    value,
                } => self.command(name, target, value),
                Action::Call(name) => {
                    if let Some((_, body)) = self
                        .script
                        .functions
                        .iter()
                        .find(|(key, _)| key == name)
                        .cloned()
                    {
                        self.execute(&body);
                    }
                }
                Action::Break => {
                    self.break_requested = true;
                    return;
                }
            }
        }
    }

    pub(crate) fn command(&mut self, name: &str, target: &str, raw: &str) {
        if matches!(name, "var.edit" | "var.set") && raw.trim().starts_with("append ") {
            let suffix = self.value(raw.trim().strip_prefix("append ").unwrap_or(""));
            let current = self.state(target);
            self.set_state(target, &format!("{current}{suffix}"));
            self.dirty = true;
            return;
        }
        let value = self.value(raw);
        match name {
            "var.set" | "var.edit" | "const.set" => self.set_state(target, &value),
            "fun" => {
                if let Some((_, body)) = self
                    .script
                    .functions
                    .iter()
                    .find(|(key, _)| key == &value)
                    .cloned()
                {
                    self.execute(&body);
                }
            }
            "for" => {
                if let Some((_, body)) = self
                    .script
                    .functions
                    .iter()
                    .find(|(key, _)| key == &value)
                    .cloned()
                {
                    for _ in 0..MAX_ACTIONS {
                        self.break_requested = false;
                        self.execute(&body);
                        if self.wait_until_ns.is_some() {
                            return;
                        }
                        if self.break_requested {
                            self.break_requested = false;
                            break;
                        }
                    }
                }
            }
            "break" => self.break_requested = true,
            "print" => {}
            "wait" => {
                if let Some(duration) = duration_ns(&value) {
                    self.wait_until_ns = Some(self.now_ns.saturating_add(duration));
                }
            }
            "run" => {
                // Keep the Warp 3 `run = os://...` form and also accept the
                // compact `run (os://...)` form in native Warp 4 scripts.
                let command = value.trim().trim_start_matches('=').trim();
                if !command.is_empty() {
                    self.last_command = Some(command.into());
                }
            }
            "BaramOS.get" => {
                let path = value
                    .trim()
                    .strip_prefix("os://")
                    .unwrap_or(value.trim())
                    .trim_start_matches("--");
                let current = if let Some(now_path) = path.strip_prefix("now://") {
                    self.now_value(now_path).unwrap_or_default()
                } else {
                    config::get_config().get(path).unwrap_or("").to_string()
                };
                self.set_state(target.trim(), &current);
            }
            "BaramOS.getFile" => {
                if let Some(path) = vfs::parse_files_uri(&value) {
                    let content = String::from_utf8_lossy(&vfs::read_file(&path)).into_owned();
                    self.set_state(target.trim(), &content);
                }
            }
            "BaramOS.uploadFile" => {
                if is_safe_script_name(target) && vfs::parse_files_uri(&value).is_some() {
                    self.last_command = Some(format!(
                        "files-upload://open?var={}&path={}",
                        target.trim(),
                        value.trim()
                    ));
                }
            }
            "WarpUI.text" => {
                if let Some(i) = self.find(target) {
                    set_attr(&mut self.nodes[i], "text", &value);
                    bdf_font::preload_text(&value);
                    self.layout_dirty = true;
                }
            }
            "WarpUI.getText" => {
                if let Some(i) = self.find(target) {
                    let text = self.nodes[i].attr("text").to_string();
                    self.set_state(raw.trim(), &text);
                }
            }
            "WarpUI.editText" => {
                if let Some(i) = self.find(target) {
                    set_attr(&mut self.nodes[i], "text", &value);
                    bdf_font::preload_text(&value);
                    self.layout_dirty = true;
                }
            }
            "WarpUI.visibility" => {
                if let Some(i) = self.find(target) {
                    set_attr(&mut self.nodes[i], "visibility", &value);
                    self.layout_dirty = true;
                }
            }
            "WarpUI.textColor" | "WarpUI.background" | "WarpUI.textSize" => {
                if let Some(i) = self.find(target) {
                    let key = name.strip_prefix("WarpUI.").unwrap_or(name);
                    set_attr(&mut self.nodes[i], key, &value);
                }
            }
            "WarpUI.screen" => self.set_screen(&value),
            name if name.starts_with("WarpUI.") => {
                if let Some(i) = self.find(target) {
                    let key = name.strip_prefix("WarpUI.").unwrap_or(name);
                    set_attr(&mut self.nodes[i], key, &value);
                }
                if let Some(uri) = value.strip_prefix("app://") {
                    self.last_command = Some(format!("app://{uri}"));
                }
            }
            _ => {}
        }
        self.dirty = true;
    }

    pub(crate) fn find(&self, id: &str) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.id() == id.trim_start_matches("@+id/"))
    }
    pub(crate) fn set_state(&mut self, key: &str, value: &str) {
        if let Some((_, v)) = self.state.iter_mut().find(|(k, _)| k == key) {
            *v = value.into();
        } else if self.state.len() < 256 {
            self.state.push((key.into(), value.into()));
        }
    }
    pub(crate) fn state(&self, key: &str) -> String {
        self.state
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }
    pub(crate) fn value(&self, raw: &str) -> String {
        let mut s = raw
            .trim()
            .trim_matches('(')
            .trim_matches(')')
            .trim()
            .to_string();
        if s == "()" {
            return String::new();
        }
        for _ in 0..128 {
            let mut next = String::new();
            let mut changed = false;
            let chars: Vec<char> = s.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                let kind = if chars[i..].starts_with(&['v', 'a', 'r', '[']) {
                    Some(("var", 4))
                } else if chars[i..].starts_with(&['c', 'o', 'n', 's', 't', '[']) {
                    Some(("const", 6))
                } else if chars[i..].starts_with(&['c', 'a', 'l', 'c', '[']) {
                    Some(("calc", 5))
                } else {
                    None
                };
                let Some((kind, open)) = kind else {
                    next.push(chars[i]);
                    i += 1;
                    continue;
                };
                let mut depth = 1i32;
                let mut end = i + open;
                while end < chars.len() {
                    if chars[end] == '[' {
                        depth += 1;
                    }
                    if chars[end] == ']' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    end += 1;
                }
                if end >= chars.len() {
                    next.push(chars[i]);
                    i += 1;
                    continue;
                }
                let inner: String = chars[i + open..end].iter().collect();
                let expanded = self.value(&inner);
                let replacement = if kind == "calc" {
                    eval_calc(&expanded)
                } else {
                    self.state(expanded.trim())
                };
                if kind == "calc" || !replacement.is_empty() || self.state_contains(expanded.trim())
                {
                    next.push_str(&replacement);
                    changed = true;
                } else {
                    next.extend(chars[i..=end].iter());
                }
                i = end + 1;
            }
            if !changed || next == s {
                break;
            }
            s = next;
        }
        if s.len() >= 2
            && ((s.starts_with('"') && s.ends_with('"'))
                || (s.starts_with('\'') && s.ends_with('\'')))
        {
            s[1..s.len() - 1].to_string()
        } else {
            s
        }
    }

    pub(crate) fn now_value(&self, path: &str) -> Option<String> {
        let time = runtime::get_time().ok()?;
        let timezone_minutes = config::timezone_offset_minutes();
        let utc_seconds =
            time.hour() as i32 * 3600 + time.minute() as i32 * 60 + time.second() as i32;
        let local_seconds = (utc_seconds + timezone_minutes * 60).rem_euclid(24 * 3600);
        let hour = (local_seconds / 3600) as u8;
        let minute = ((local_seconds / 60) % 60) as u8;
        let second = (local_seconds % 60) as u8;
        Some(match path.trim_matches('/') {
            "fps" => self.runtime_fps.to_string(),
            "window" | "windows" => self.runtime_windows.to_string(),
            "key" | "keys" => self.runtime_keys.to_string(),
            "mouse" => self.runtime_mouse.to_string(),
            "h" => hour.to_string(),
            "m" => minute.to_string(),
            "s" => second.to_string(),
            "hh" => format!("{hour:02}"),
            "mm" => format!("{minute:02}"),
            "ss" => format!("{second:02}"),
            "hhmm" => format!("{hour:02}:{minute:02}"),
            "hhmmss" => format!("{hour:02}:{minute:02}:{second:02}"),
            _ => return None,
        })
    }

    pub(crate) fn state_contains(&self, key: &str) -> bool {
        self.state.iter().any(|(name, _)| name == key)
    }
}

fn eval_calc(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut parser = CalcParser { chars, pos: 0 };
    let value = parser.expr();
    if (value as i64) as f64 == value {
        (value as i64).to_string()
    } else {
        value.to_string()
    }
}

struct CalcParser {
    chars: Vec<char>,
    pos: usize,
}
impl CalcParser {
    pub(crate) fn skip(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }
    pub(crate) fn expr(&mut self) -> f64 {
        let mut value = self.term();
        loop {
            self.skip();
            let op = self.chars.get(self.pos).copied();
            if op != Some('+') && op != Some('-') {
                break;
            }
            self.pos += 1;
            let rhs = self.term();
            value = if op == Some('+') {
                value + rhs
            } else {
                value - rhs
            };
        }
        value
    }
    pub(crate) fn term(&mut self) -> f64 {
        let mut value = self.factor();
        loop {
            self.skip();
            let op = self.chars.get(self.pos).copied();
            if !matches!(op, Some('*') | Some('/') | Some('%')) {
                break;
            }
            self.pos += 1;
            let rhs = self.factor();
            value = match op {
                Some('*') => value * rhs,
                Some('/') => {
                    if rhs == 0.0 {
                        0.0
                    } else {
                        value / rhs
                    }
                }
                Some('%') => value % rhs,
                _ => value,
            };
        }
        value
    }
    pub(crate) fn factor(&mut self) -> f64 {
        self.skip();
        if self.chars.get(self.pos) == Some(&'-') {
            self.pos += 1;
            return -self.factor();
        }
        if self.chars.get(self.pos) == Some(&'(') {
            self.pos += 1;
            let value = self.expr();
            self.skip();
            if self.chars.get(self.pos) == Some(&')') {
                self.pos += 1;
            }
            return value;
        }
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_digit() || self.chars[self.pos] == '.')
        {
            self.pos += 1;
        }
        self.chars[start..self.pos]
            .iter()
            .collect::<String>()
            .parse()
            .unwrap_or(0.0)
    }
}
pub(crate) fn bg() -> Color {
    palette().warp4_bg
}
