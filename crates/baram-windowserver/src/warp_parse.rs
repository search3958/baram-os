impl WarpEngine {
    pub fn new(code: &str) -> Self {
        let mut ctx = Self {
            warp4: None,
            origin: String::new(),
            state: Vec::new(),
            current_screen: String::new(),
            parsed_screen_id: String::new(),
            screens: Vec::new(),
            nodes: Vec::new(),
            root_nodes: Vec::new(),
            scripts: Vec::new(),
            src: code.chars().collect(),
            src_ptr: 0,
            tokens: Vec::new(),
            token_pos: 0,
            texts: Vec::new(),
            dirty: false,
            hover_idx: None,
            last_command: None,
            last_clicked_id: None,
            focused_input: None,
            focused_input_var: alloc::string::String::new(),
            content_height: 0,
            caret_visible: true,
        };
        loop {
            let tk = ctx.next_token();
            if tk.r#type == TkType::Eof || ctx.tokens.len() >= 4096 {
                break;
            }
            ctx.tokens.push(tk);
        }
        ctx.token_pos = 0;
        while ctx.token_pos < ctx.tokens.len() {
            if ctx.tokens[ctx.token_pos].r#type == TkType::At {
                ctx.parse_script();
            } else if ctx.tokens[ctx.token_pos].r#type == TkType::Word
                && ctx.tokens[ctx.token_pos].val == "screen"
            {
                let mut screen_id = String::from("main");
                let start_pos = ctx.token_pos;
                if ctx.token_pos + 1 < ctx.tokens.len()
                    && ctx.tokens[ctx.token_pos + 1].val.starts_with('{')
                {
                    let mut j = ctx.token_pos + 2;
                    let mut depth = 1;
                    while j < ctx.tokens.len() && depth > 0 {
                        let vf = ctx.tokens[j].val.chars().next().unwrap_or(' ');
                        if ctx.tokens[j].r#type == TkType::Punct {
                            if vf == '{' {
                                depth += 1;
                            } else if vf == '}' {
                                depth -= 1;
                            }
                        }
                        if depth == 1
                            && ctx.tokens[j].r#type == TkType::Word
                            && ctx.tokens[j].val == "id"
                            && j + 1 < ctx.tokens.len()
                            && ctx.tokens[j + 1].val.starts_with(':')
                        {
                            let mut k = j + 2;
                            if k < ctx.tokens.len() && ctx.tokens[k].val.starts_with('(') {
                                k += 1;
                            }
                            if k < ctx.tokens.len() && ctx.tokens[k].r#type != TkType::Punct {
                                screen_id = ctx.tokens[k].val.clone();
                            }
                        }
                        j += 1;
                    }
                    if ctx.screens.len() < MAX_SCREENS {
                        ctx.screens.push(ScreenInfo {
                            id: if screen_id.is_empty() {
                                String::from("main")
                            } else {
                                screen_id
                            },
                            token_index: start_pos,
                        });
                    }
                    ctx.token_pos = j;
                } else {
                    ctx.token_pos += 1;
                }
            } else {
                ctx.skip_block();
            }
        }
        if !ctx.screens.is_empty() {
            ctx.current_screen = ctx.screens[0].id.clone();
            ctx.parse_current_screen();
        } else {
            ctx.current_screen = String::from("main");
        }
        ctx
    }

    pub fn new_warp4(app_name: &str) -> Self {
        baram_warp4::set_ui_mode(baram_warp4::UiMode::Normal);
        baram_warp4::set_ui_scale_percent(100);
        let mut engine = Self::new("");
        engine.warp4 = Some(Warp4Engine::new(app_name));
        engine.origin = app_name.to_string();
        engine
    }

    pub fn new_embedded_warp4(name: &str, sources: &[(&str, &str)]) -> Self {
        baram_warp4::set_ui_mode(baram_warp4::UiMode::Normal);
        baram_warp4::set_ui_scale_percent(100);
        let mut engine = Self::new("");
        engine.warp4 = Some(Warp4Engine::new_embedded(name, sources));
        engine.origin = name.to_string();
        engine
    }

    pub fn set_origin(&mut self, app_name: &str) {
        self.origin = String::from(app_name);
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_origin(app_name);
        }
    }

    pub fn set_chrome_visible(&mut self, visible: bool) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_chrome_visible(visible);
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn update(&mut self, width: i32, height: i32) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.update(width, height);
            self.content_height = engine.content_height;
            self.focused_input = engine.has_focused_input().then_some(0);
            self.focused_input_var = if self.focused_input.is_some() {
                "__warp4__".into()
            } else {
                String::new()
            };
            self.hover_idx = engine.hovered_node();
            return;
        }
        self.parse_current_screen();
        self.texts.clear();
        let root_nodes = self.root_nodes.clone();
        let mut total_h = height;
        for node_idx in &root_nodes {
            let h = self.layout_node(*node_idx, 0, 30, width);
            if h > total_h {
                total_h = h;
            }
        }
        self.content_height = total_h;
        self.dirty = true;
    }

    pub fn set_scroll(&mut self, scroll: i32) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_scroll(scroll);
        }
    }

    pub fn take_scroll_request(&mut self) -> Option<i32> {
        self.warp4
            .as_mut()
            .and_then(|engine| engine.take_scroll_request())
    }

    pub fn window_damage(&self) -> Option<(i32, i32, i32, i32)> {
        self.warp4
            .as_ref()
            .and_then(|engine| engine.window_damage())
    }

    pub fn has_focused_input(&self) -> bool {
        self.warp4
            .as_ref()
            .is_some_and(|engine| engine.has_focused_input())
            || self.focused_input.is_some()
    }

    pub fn set_runtime_metrics(&mut self, fps: u32, windows: usize, keys: u32, mouse: u32) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_runtime_metrics(fps, windows, keys, mouse);
        }
    }

    pub fn pointer_move(&mut self, x: i32, y: i32) -> bool {
        let Some(engine) = self.warp4.as_mut() else {
            return false;
        };
        engine.pointer_move(x, y)
    }

    pub fn has_pointer_capture(&self) -> bool {
        self.warp4
            .as_ref()
            .is_some_and(Warp4Engine::has_pointer_capture)
    }

    pub fn tick(&mut self, now_ns: u64) -> bool {
        if let Some(engine) = self.warp4.as_mut() {
            let changed = engine.tick(now_ns);
            if changed {
                self.dirty = true;
                self.content_height = engine.content_height;
            }
            return changed;
        }
        let next_visible = self
            .focused_input
            .map_or(true, |_| text_cursor::visible(now_ns));
        let changed = self.caret_visible != next_visible;
        self.caret_visible = next_visible;
        if changed {
            self.dirty = true;
        }
        changed
    }

    pub fn set_screen(&mut self, screen: &str) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_screen(screen);
            return;
        }
        if self.current_screen != screen {
            self.current_screen = screen.chars().take(63).collect();
            self.parse_current_screen();
            self.dirty = true;
        }
    }

    pub fn node_bounds(&self, id: &str) -> Option<(i32, i32, i32, i32)> {
        self.find_node_by_id(id).map(|idx| {
            let node = &self.nodes[idx];
            (node.x, node.y, node.w, node.h)
        })
    }

    pub fn content_height(&self) -> i32 {
        self.warp4
            .as_ref()
            .map(|engine| engine.content_height)
            .unwrap_or(0)
    }

    pub fn hovered_node(&self) -> Option<usize> {
        self.warp4.as_ref().and_then(Warp4Engine::hovered_node)
    }

    pub fn set_text(&mut self, id: &str, text: &str) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_text(id, text);
        }
    }

    pub fn set_selected(&mut self, id: &str, selected: bool) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_selected(id, selected);
        }
    }

    pub fn set_candidate_items(&mut self, mode: &str, candidates: &[String]) {
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_text("candidate-mode", mode);
            for (index, candidate) in candidates.iter().take(5).enumerate() {
                engine.set_text(&alloc::format!("candidate-{index}"), candidate);
            }
        }
    }

    fn set_state(&mut self, key: &str, val: &str) {
        if key.eq_ignore_ascii_case("_currentScreen") {
            self.current_screen = val.chars().take(63).collect();
            return;
        }
        for s in &mut self.state {
            if s.0.eq_ignore_ascii_case(key) {
                s.1 = val.chars().take(511).collect();
                return;
            }
        }
        if self.state.len() < MAX_VARS {
            self.state.push((
                key.chars().take(63).collect(),
                val.chars().take(511).collect(),
            ));
        }
    }

    fn get_state(&self, key: &str) -> String {
        if key.eq_ignore_ascii_case("_currentScreen") {
            return self.current_screen.clone();
        }
        if let Some(cfg_path) = key.strip_prefix("--os://") {
            if let Some(val) = config::get_config().get(cfg_path) {
                return val.to_string();
            }
            return String::new();
        }
        for s in &self.state {
            if s.0.eq_ignore_ascii_case(key) {
                return s.1.clone();
            }
        }
        String::new()
    }

    fn eval_expr(&self, expr: &str) -> String {
        let mut out = String::new();
        let chars: Vec<char> = expr.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '"' || c == '\'' {
                let quote = c;
                i += 1;
                while i < chars.len() && chars[i] != quote {
                    if chars[i] == '\\' {
                        i += 1;
                        if i < chars.len() {
                            out.push(match chars[i] {
                                'n' => '\n',
                                '"' => '"',
                                '\'' => '\'',
                                '\\' => '\\',
                                x => x,
                            });
                        }
                    } else {
                        out.push(chars[i]);
                    }
                    i += 1;
                }
                if i < chars.len() {
                    i += 1;
                }
            } else if (c == '-' && chars.get(i + 1) == Some(&'-'))
                || (c == '~' && chars.get(i + 1) == Some(&'~'))
            {
                let mut var = String::new();
                while i < chars.len() {
                    let c2 = chars[i];
                    if c2 == '"'
                        || c2 == '\''
                        || c2 == '+'
                        || c2 == ' '
                        || c2 == ')'
                        || c2 == ','
                        || c2 == '}'
                    {
                        break;
                    }
                    var.push(c2);
                    i += 1;
                }
                out.push_str(&self.get_state(&var));
            } else if c == '+' {
                i += 1;
            } else {
                out.push(c);
                i += 1;
            }
        }
        out
    }

    fn get_attr(&self, idx: usize, key: &str) -> String {
        for a in &self.nodes[idx].attrs {
            if a.key == key {
                return self.eval_expr(&a.value);
            }
        }
        String::new()
    }

    fn get_attr_raw(&self, idx: usize, key: &str) -> String {
        for a in &self.nodes[idx].attrs {
            if a.key == key {
                return a.value.clone();
            }
        }
        String::new()
    }

    fn strtol(s: &str) -> i64 {
        let mut res: i64 = 0;
        let mut sign = 1;
        let mut chars = s.chars().peekable();
        while let Some(&c) = chars.peek() {
            if c == ' ' || c == '\t' {
                chars.next();
            } else {
                break;
            }
        }
        if let Some(&'-') = chars.peek() {
            sign = -1;
            chars.next();
        }
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                res = res * 10 + (c as i64 - '0' as i64);
                chars.next();
            } else {
                break;
            }
        }
        res * sign
    }

    fn eval_math(&self, s: &str) -> i64 {
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
            i += 1;
        }
        if i >= chars.len() {
            return 0;
        }
        let mut res = Self::strtol(&chars[i..].iter().collect::<String>());
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t' || chars[i].is_ascii_digit())
        {
            i += 1;
        }
        while i < chars.len() {
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                i += 1;
            }
            if i >= chars.len() {
                break;
            }
            let op = chars[i];
            i += 1;
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                i += 1;
            }
            let v = Self::strtol(&chars[i..].iter().collect::<String>());
            match op {
                '+' => res += v,
                '-' => res -= v,
                '*' => res *= v,
                '/' => {
                    if v != 0 {
                        res /= v;
                    }
                }
                _ => {}
            }
            while i < chars.len()
                && (chars[i] == ' ' || chars[i] == '\t' || chars[i].is_ascii_digit())
            {
                i += 1;
            }
        }
        res
    }
}

