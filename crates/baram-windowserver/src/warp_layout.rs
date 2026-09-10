impl WarpEngine {
    fn next_token(&mut self) -> Token {
        while self.src_ptr < self.src.len() && (self.src[self.src_ptr] as u32) <= 32 {
            self.src_ptr += 1;
        }
        if self.src_ptr >= self.src.len() {
            return Token {
                r#type: TkType::Eof,
                val: String::new(),
            };
        }
        let c = self.src[self.src_ptr];
        if c == '@' {
            self.src_ptr += 1;
            return Token {
                r#type: TkType::At,
                val: String::from("@"),
            };
        }
        if c == '"' || c == '\'' {
            let quote = c;
            self.src_ptr += 1;
            let mut val = String::new();
            while self.src_ptr < self.src.len() && self.src[self.src_ptr] != quote {
                if self.src[self.src_ptr] == '\\' {
                    self.src_ptr += 1;
                    if self.src_ptr < self.src.len() {
                        val.push(self.src[self.src_ptr]);
                        self.src_ptr += 1;
                    }
                } else {
                    val.push(self.src[self.src_ptr]);
                    self.src_ptr += 1;
                }
            }
            if self.src_ptr < self.src.len() {
                self.src_ptr += 1;
            }
            return Token {
                r#type: TkType::Str,
                val,
            };
        }
        let punct = "{}():;=+,";
        if punct.contains(c) {
            self.src_ptr += 1;
            return Token {
                r#type: TkType::Punct,
                val: c.to_string(),
            };
        }
        let mut val = String::new();
        while self.src_ptr < self.src.len() {
            let c2 = self.src[self.src_ptr];
            if (c2 as u32) <= 32 || punct.contains(c2) {
                break;
            }
            val.push(c2);
            self.src_ptr += 1;
        }
        Token {
            r#type: TkType::Word,
            val,
        }
    }

    fn alloc_node(&mut self) -> Option<usize> {
        let mut n = Node::default();
        n.visible = true;
        self.nodes.push(n);
        Some(self.nodes.len() - 1)
    }

    fn skip_block(&mut self) {
        if self.token_pos + 1 >= self.tokens.len()
            || !self.tokens[self.token_pos + 1].val.starts_with('{')
        {
            self.token_pos += 1;
            return;
        }
        self.token_pos += 2;
        let mut depth = 1;
        while self.token_pos < self.tokens.len() && depth > 0 {
            if self.tokens[self.token_pos].r#type == TkType::Punct {
                let c = self.tokens[self.token_pos]
                    .val
                    .chars()
                    .next()
                    .unwrap_or(' ');
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                }
            }
            self.token_pos += 1;
        }
    }

    fn parse_current_screen(&mut self) {
        if self.current_screen == self.parsed_screen_id && !self.root_nodes.is_empty() {
            return;
        }
        self.nodes.clear();
        self.root_nodes.clear();
        self.texts.clear();
        for i in 0..self.screens.len() {
            if self.screens[i].id == self.current_screen {
                self.token_pos = self.screens[i].token_index;
                if let Some(idx) = self.parse_node() {
                    self.root_nodes.push(idx);
                    self.init_state_from_ast(idx);
                }
                self.parsed_screen_id = self.current_screen.clone();
                return;
            }
        }
        self.parsed_screen_id.clear();
    }

    fn init_state_from_ast(&mut self, idx: usize) {
        let attrs = self.nodes[idx].attrs.clone();
        for a in attrs {
            if a.key.starts_with("--") {
                let val = self.eval_expr(&a.value);
                self.set_state(&a.key, &val);
            }
        }
        let children = self.nodes[idx].children.clone();
        for c in children {
            self.init_state_from_ast(c);
        }
    }

    fn parse_script(&mut self) {
        self.token_pos += 1;
        if self.token_pos >= self.tokens.len() {
            return;
        }
        if self.scripts.len() >= MAX_SCRIPTS {
            self.token_pos += 1;
            return;
        }
        let name = self.tokens[self.token_pos].val.clone();
        self.token_pos += 1;
        let mut script = Script {
            name,
            blocks: Vec::new(),
        };
        if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with('{') {
            self.token_pos += 1;
            while self.token_pos < self.tokens.len()
                && !self.tokens[self.token_pos].val.starts_with('}')
            {
                let val = self.tokens[self.token_pos].val.clone();
                if val == "if" || val == "elseIf" {
                    if script.blocks.len() < 100 {
                        let mut block = ScriptBlock {
                            r#type: val,
                            condition: String::new(),
                            actions: String::new(),
                        };
                        self.token_pos += 1;
                        if self.token_pos < self.tokens.len()
                            && self.tokens[self.token_pos].val.starts_with(':')
                        {
                            self.token_pos += 1;
                        }
                        if self.token_pos < self.tokens.len()
                            && self.tokens[self.token_pos].val.starts_with('(')
                        {
                            self.token_pos += 1;
                            let mut p = 1;
                            while p > 0 && self.token_pos < self.tokens.len() {
                                let c = self.tokens[self.token_pos]
                                    .val
                                    .chars()
                                    .next()
                                    .unwrap_or(' ');
                                if c == '(' {
                                    p += 1;
                                } else if c == ')' {
                                    p -= 1;
                                }
                                if p > 0 {
                                    if self.tokens[self.token_pos].r#type == TkType::Str {
                                        block.condition.push('"');
                                    }
                                    block.condition.push_str(&self.tokens[self.token_pos].val);
                                    if self.tokens[self.token_pos].r#type == TkType::Str {
                                        block.condition.push('"');
                                    }
                                    self.token_pos += 1;
                                }
                            }
                            if self.token_pos < self.tokens.len() {
                                self.token_pos += 1;
                            }
                        }
                        if self.token_pos < self.tokens.len()
                            && self.tokens[self.token_pos].val.starts_with('{')
                        {
                            self.token_pos += 1;
                            let mut bc = 1;
                            let mut prev_type = TkType::Eof;
                            while bc > 0 && self.token_pos < self.tokens.len() {
                                let c = self.tokens[self.token_pos]
                                    .val
                                    .chars()
                                    .next()
                                    .unwrap_or(' ');
                                if c == '{' {
                                    bc += 1;
                                } else if c == '}' {
                                    bc -= 1;
                                }
                                if bc > 0 {
                                    let cur_type = self.tokens[self.token_pos].r#type;
                                    if !block.actions.is_empty()
                                        && (prev_type == TkType::Word || prev_type == TkType::Str)
                                        && (cur_type == TkType::Word || cur_type == TkType::Str)
                                    {
                                        block.actions.push(' ');
                                    }
                                    if cur_type == TkType::Str {
                                        block.actions.push('"');
                                    }
                                    block.actions.push_str(&self.tokens[self.token_pos].val);
                                    if cur_type == TkType::Str {
                                        block.actions.push('"');
                                    }
                                    prev_type = cur_type;
                                    self.token_pos += 1;
                                }
                            }
                            if self.token_pos < self.tokens.len() {
                                self.token_pos += 1;
                            }
                        }
                        script.blocks.push(block);
                    } else {
                        self.token_pos += 1;
                    }
                } else {
                    self.token_pos += 1;
                }
            }
            if self.token_pos < self.tokens.len() {
                self.token_pos += 1;
            }
        }
        self.scripts.push(script);
    }

    fn parse_node(&mut self) -> Option<usize> {
        if self.token_pos >= self.tokens.len() {
            return None;
        }
        if self.tokens[self.token_pos].r#type == TkType::At {
            self.parse_script();
            return None;
        }
        let tag_name = self.tokens[self.token_pos].val.clone();
        if self.token_pos + 1 < self.tokens.len()
            && self.tokens[self.token_pos + 1].val.starts_with('{')
        {
            let idx = self.alloc_node()?;
            self.nodes[idx].tag = tag_name;
            self.token_pos += 2;
            while self.token_pos < self.tokens.len()
                && !self.tokens[self.token_pos].val.starts_with('}')
            {
                if self.token_pos + 1 < self.tokens.len()
                    && self.tokens[self.token_pos + 1].val.starts_with('{')
                {
                    if let Some(ci) = self.parse_node() {
                        self.nodes[idx].children.push(ci);
                    }
                    continue;
                }
                if self.token_pos + 1 < self.tokens.len()
                    && self.tokens[self.token_pos + 1].val.starts_with(':')
                {
                    let key = self.tokens[self.token_pos].val.clone();
                    self.token_pos += 2;
                    let mut expr = String::new();
                    if self.token_pos < self.tokens.len()
                        && self.tokens[self.token_pos].val.starts_with('(')
                    {
                        self.token_pos += 1;
                        let mut p = 1;
                        let mut prev_type = TkType::Eof;
                        while p > 0 && self.token_pos < self.tokens.len() {
                            let c = self.tokens[self.token_pos]
                                .val
                                .chars()
                                .next()
                                .unwrap_or(' ');
                            if c == '(' {
                                p += 1;
                            } else if c == ')' {
                                p -= 1;
                            }
                            if p > 0 {
                                let cur_type = self.tokens[self.token_pos].r#type;
                                if !expr.is_empty()
                                    && (prev_type == TkType::Word || prev_type == TkType::Str)
                                    && (cur_type == TkType::Word || cur_type == TkType::Str)
                                {
                                    expr.push(' ');
                                }
                                if cur_type == TkType::Str {
                                    expr.push('"');
                                }
                                expr.push_str(&self.tokens[self.token_pos].val);
                                if cur_type == TkType::Str {
                                    expr.push('"');
                                }
                                prev_type = cur_type;
                                self.token_pos += 1;
                            }
                        }
                        if self.token_pos < self.tokens.len() {
                            self.token_pos += 1;
                        }
                    } else {
                        expr.push_str(&self.tokens[self.token_pos].val);
                        self.token_pos += 1;
                    }
                    if key == "oneClick" {
                        self.nodes[idx].event_oneclick = expr;
                    } else {
                        self.nodes[idx].attrs.push(Attr { key, value: expr });
                    }
                    if self.token_pos < self.tokens.len()
                        && self.tokens[self.token_pos].val.starts_with(',')
                    {
                        self.token_pos += 1;
                    }
                    continue;
                }
                self.token_pos += 1;
            }
            if self.token_pos < self.tokens.len() {
                self.token_pos += 1;
            }
            return Some(idx);
        }
        self.token_pos += 1;
        None
    }

    fn layout_node(&mut self, idx: usize, px: i32, py: i32, limit_w: i32) -> i32 {
        if !self.nodes[idx].visible {
            return 0;
        }
        self.nodes[idx].x = px;
        self.nodes[idx].y = py;
        self.nodes[idx].w = limit_w;
        let mut cy = py;
        let tag = self.nodes[idx].tag.clone();
        if tag == "screen" {
            cy = py + 16;
            let children = self.nodes[idx].children.clone();
            for ci in children {
                if self.nodes[ci].tag != "Header" {
                    let h = self.layout_node(ci, self.nodes[idx].x + 24, cy, limit_w - 48);
                    cy += h + 12;
                }
            }
            self.nodes[idx].h = cy - py + 4;
            if self.nodes[idx].h < 600 {
                self.nodes[idx].h = 600;
            }
        } else if tag == "card" {
            cy += 12;
            let title = self.get_attr(idx, "text");
            if !title.is_empty() && self.texts.len() < MAX_TEXTS {
                self.texts.push(TextElem {
                    x: px + 24,
                    y: cy + 4,
                    text: title,
                    size: 20.0,
                    color: config::get_color("ui-theme/color/text", Color::TEXT),
                });
                cy += 36;
            }
            let children = self.nodes[idx].children.clone();
            for ci in children {
                let h = self.layout_node(ci, px + 24, cy, limit_w - 48);
                cy += h + 8;
            }
            self.nodes[idx].h = cy - py + 12;
            if let Ok(min_h) = self.get_attr(idx, "height").parse::<i32>() {
                self.nodes[idx].h = self.nodes[idx].h.max(min_h.max(0));
            }
        } else if tag == "button" || tag == "tonalButton" {
            self.nodes[idx].h = 40;
            let text = self.get_attr(idx, "text");
            let text_w = measure_text_width(&text, 16.0);
            self.nodes[idx].w = text_w + 32;
            if self.nodes[idx].w < 70 {
                self.nodes[idx].w = 70;
            }
            if self.nodes[idx].w > limit_w {
                self.nodes[idx].w = limit_w;
            }
            if let Ok(width) = self.get_attr(idx, "width").parse::<i32>() {
                self.nodes[idx].w = width.clamp(1, limit_w.max(1));
            }
            if self.texts.len() < MAX_TEXTS {
                self.texts.push(TextElem {
                    x: self.nodes[idx].x + (self.nodes[idx].w - text_w) / 2,
                    y: self.nodes[idx].y + 10,
                    text,
                    size: 16.0,
                    color: if tag == "tonalButton" {
                        config::get_color("ui-theme/color/text", Color::TEXT)
                    } else {
                        config::get_color("ui-theme/color/btn_text", Color::BTN_TEXT)
                    },
                });
            }
        } else if tag == "switch" {
            self.nodes[idx].w = 44;
            self.nodes[idx].h = 44;
            return self.nodes[idx].h;
        } else if tag == "input" {
            self.nodes[idx].w = limit_w;
            self.nodes[idx].h = 48;
            let out_var = self.parse_out_var(idx);
            let placeholder = self.get_attr(idx, "placeholder");
            let mut val = if !out_var.is_empty() {
                self.get_state(&out_var)
            } else {
                String::new()
            };
            if val.is_empty() {
                val = placeholder;
            }
            let out_var_name = self.parse_out_var(idx);
            if self.focused_input_var == out_var_name {
                val.push('|');
            }
            let char_w = 8i32;
            let max_chars = ((limit_w - 24) / char_w).max(1) as usize;
            let chars: alloc::vec::Vec<char> = val.chars().collect();
            let mut display_lines: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
            let mut start = 0;
            while start < chars.len() {
                let end = (start + max_chars).min(chars.len());
                if end < chars.len() {
                    let mut break_at = end;
                    while break_at > start {
                        let c = chars[break_at - 1];
                        if c == ' ' || c == ',' || c == '.' {
                            break_at -= 1;
                            break;
                        }
                        break_at -= 1;
                    }
                    if break_at <= start {
                        break_at = end;
                    }
                    let line: alloc::string::String = chars[start..break_at].iter().collect();
                    display_lines.push(line);
                    start = break_at;
                    if start < chars.len() && chars[start] == ' ' {
                        start += 1;
                    }
                } else {
                    let line: alloc::string::String = chars[start..].iter().collect();
                    display_lines.push(line);
                    start = chars.len();
                }
            }
            let display_text = display_lines.join("\n");
            if self.texts.len() < MAX_TEXTS {
                self.texts.push(TextElem {
                    x: self.nodes[idx].x + 12,
                    y: self.nodes[idx].y + 16,
                    text: display_text,
                    size: 16.0,
                    color: config::get_color("ui-theme/color/text", Color::TEXT),
                });
            }
        } else if tag == "text" {
            let text = self.get_attr(idx, "text");
            let char_w = 8i32;
            let max_chars = (limit_w / char_w).max(1) as usize;
            let mut lines: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
            for raw_line in text.split('\n') {
                if raw_line.is_empty() {
                    lines.push(alloc::string::String::new());
                    continue;
                }
                let chars: alloc::vec::Vec<char> = raw_line.chars().collect();
                let mut start = 0;
                while start < chars.len() {
                    let end = (start + max_chars).min(chars.len());
                    if end < chars.len() {
                        let mut break_at = end;
                        while break_at > start {
                            let c = chars[break_at - 1];
                            if c == ' ' || c == ',' || c == '.' {
                                break_at -= 1;
                                break;
                            }
                            break_at -= 1;
                        }
                        if break_at <= start {
                            break_at = end;
                        }
                        let line: alloc::string::String = chars[start..break_at].iter().collect();
                        lines.push(line);
                        start = break_at;
                        if start < chars.len() && chars[start] == ' ' {
                            start += 1;
                        }
                    } else {
                        let line: alloc::string::String = chars[start..].iter().collect();
                        lines.push(line);
                        start = chars.len();
                    }
                }
            }
            let wrapped = lines.join("\n");
            if self.texts.len() < MAX_TEXTS {
                self.texts.push(TextElem {
                    x: px,
                    y: py,
                    text: wrapped.clone(),
                    size: 16.0,
                    color: config::get_color("ui-theme/color/text", Color::TEXT),
                });
            }
            let line_count = lines.len() as i32;
            self.nodes[idx].h = line_count * 22;
        } else if tag == "spacer" {
            self.nodes[idx].h = self
                .get_attr(idx, "height")
                .parse::<i32>()
                .unwrap_or(0)
                .max(0);
        } else if tag == "hStack" {
            let mut cx = px;
            let mut row_h = 0i32;
            let mut max_h = 0i32;
            let mut row_start_y = py;
            let children = self.nodes[idx].children.clone();
            for ci in children {
                let h = self.layout_node(ci, cx, row_start_y, limit_w);
                let w = self.nodes[ci].w;
                if cx + w > px + limit_w && cx > px {
                    cx = px;
                    row_start_y += row_h + 8;
                    row_h = 0;
                    self.layout_node(ci, cx, row_start_y, limit_w);
                }
                cx += w + 8;
                if row_h < h {
                    row_h = h;
                }
                let bottom = row_start_y + h;
                if bottom > max_h {
                    max_h = bottom;
                }
            }
            self.nodes[idx].h = max_h - py;
        } else if tag == "vStack" {
            let children = self.nodes[idx].children.clone();
            for ci in children {
                let h = self.layout_node(ci, px, cy, limit_w);
                cy += h + 8;
            }
            self.nodes[idx].h = cy - py;
        } else {
            let children = self.nodes[idx].children.clone();
            for ci in children {
                let h = self.layout_node(ci, px, cy, limit_w);
                cy += h + 4;
            }
            self.nodes[idx].h = cy - py;
        }
        self.nodes[idx].h
    }

}

