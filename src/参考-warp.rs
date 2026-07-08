use std::ffi::{CStr, CString};

// --- 外部関数の宣言 ---
extern "C" {
    fn set_w1_global(key: *const i8, val: *const i8);
    fn get_w1_global(key: *const i8) -> *const i8;
    fn measure_ttf_width(text: *const i8, size: f32) -> i32;
    fn sys_restart();
    fn set_pending_command(cmd: *const i8);
    fn layer_draw_ttf(layer: *mut std::ffi::c_void, x: i32, y: i32, s: *const i8, sz: f32, c: u32);
}

const MAX_VARS: usize = 256;
const MAX_SCREENS: usize = 64;
const MAX_SCRIPTS: usize = 64;
const MAX_TEXTS: usize = 1024;

// --- 型定義 ---
#[derive(Clone, Default)]
pub struct Warp1Attr {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Default)]
pub struct Warp1Node {
    pub tag: String,
    pub attrs: Vec<Warp1Attr>,
    pub event_oneclick: String,
    pub event_longpress: String,
    pub children: Vec<usize>,
    pub x: i32, pub y: i32, pub w: i32, pub h: i32,
    pub is_dynamic: bool,
    pub prev_x: i32, pub prev_y: i32, pub prev_w: i32, pub prev_h: i32,
    pub is_dirty: bool,
}

#[derive(Clone, Default)]
pub struct ScriptBlock1 {
    pub r#type: String,
    pub condition: String,
    pub actions: String,
}

#[derive(Clone, Default)]
pub struct Script1 {
    pub name: String,
    pub blocks: Vec<ScriptBlock1>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tk1Type {
    Word, Str, Punct, At, Eof,
}

impl Default for Tk1Type {
    fn default() -> Self { Tk1Type::Eof }
}

#[derive(Clone, Default)]
pub struct Token1 {
    pub r#type: Tk1Type,
    pub val: String,
}

pub struct Warp1Text {
    pub x: i32, pub y: i32,
    pub text: String,
    pub color: u32,
    pub size: f32,
}

pub struct W1ScreenInfo {
    pub id: String,
    pub token_index: usize,
}

pub struct Warp1Context {
    pub state: Vec<(String, String)>,
    pub current_screen: String,
    pub parsed_screen_id: String,
    pub screens: Vec<W1ScreenInfo>,
    
    pub nodes: Vec<Warp1Node>,
    pub root_nodes: Vec<usize>,
    pub scripts: Vec<Script1>,
    
    pub src: Vec<char>,
    pub src_ptr: usize,
    pub tokens: Vec<Token1>,
    pub token_pos: usize,
    
    pub texts: Vec<Warp1Text>,
    pub svg_output: String,
    pub engine_dirty: bool,
    pub engine_status: String,
    
    pub mouse_x: i32, pub mouse_y: i32,
    pub win_w: i32, pub win_h: i32,
    
    pub focused_node_idx: isize,
    
    pub screen_ids: Vec<String>,
    pub screen_content_heights: Vec<i32>,
    pub screen_scroll_ys: Vec<f32>,
    pub screen_count: usize,
}

impl Warp1Context {
    pub fn new(code: &str) -> Self {
        let mut ctx = Self {
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
            svg_output: String::new(),
            engine_dirty: true,
            engine_status: String::new(),
            mouse_x: 0, mouse_y: 0,
            win_w: 0, win_h: 0,
            focused_node_idx: -1,
            screen_ids: Vec::new(),
            screen_content_heights: Vec::new(),
            screen_scroll_ys: Vec::new(),
            screen_count: 0,
        };
        
        // トークナイズ
        loop {
            let tk = ctx.next_token();
            if tk.r#type == Tk1Type::Eof || ctx.tokens.len() >= 4096 { break; }
            ctx.tokens.push(tk);
        }
        
        // スクリーンのパース
        ctx.token_pos = 0;
        while ctx.token_pos < ctx.tokens.len() {
            if ctx.tokens[ctx.token_pos].r#type == Tk1Type::At {
                ctx.parse_script();
            } else if ctx.tokens[ctx.token_pos].r#type == Tk1Type::Word && ctx.tokens[ctx.token_pos].val == "screen" {
                let mut screen_id = "main".to_string();
                let start_pos = ctx.token_pos;
                if ctx.token_pos + 1 < ctx.tokens.len() && ctx.tokens[ctx.token_pos + 1].val.starts_with('{') {
                    let mut j = ctx.token_pos + 2;
                    let mut depth = 1;
                    while j < ctx.tokens.len() && depth > 0 {
                        let val_first = ctx.tokens[j].val.chars().next().unwrap_or(' ');
                        if ctx.tokens[j].r#type == Tk1Type::Punct {
                            if val_first == '{' { depth += 1; }
                            else if val_first == '}' { depth -= 1; }
                        }
                        if depth == 1 && ctx.tokens[j].r#type == Tk1Type::Word && ctx.tokens[j].val == "id" && j + 1 < ctx.tokens.len() && ctx.tokens[j+1].val.starts_with(':') {
                            let mut k = j + 2;
                            if k < ctx.tokens.len() && ctx.tokens[k].val.starts_with('(') { k += 1; }
                            if k < ctx.tokens.len() && ctx.tokens[k].r#type != Tk1Type::Punct {
                                screen_id = ctx.tokens[k].val.clone();
                            }
                        }
                        j += 1;
                    }
                    if ctx.screens.len() < MAX_SCREENS {
                        ctx.screens.push(W1ScreenInfo {
                            id: if screen_id.is_empty() { "main".to_string() } else { screen_id },
                            token_index: start_pos,
                        });
                    }
                    ctx.token_pos = j;
                } else {
                    ctx.token_pos += 1;
                }
            } else {
                ctx.skip_block1();
            }
        }

        if !ctx.screens.is_empty() {
            ctx.current_screen = ctx.screens[0].id.clone();
            ctx.parse_current_screen1();
        } else {
            ctx.current_screen = "main".to_string();
        }
        
        ctx
    }

    pub fn update(&mut self, width: i32, height: i32) {
        self.parse_current_screen1();
        self.texts.clear();
        self.svg_output.clear();
        self.win_w = width;
        self.win_h = height;
        
        let mut total_h = height;
        let root_nodes = self.root_nodes.clone();
        for node_idx in &root_nodes {
            let h = self.layout_node1(*node_idx, 0, 0, width);
            if h > total_h { total_h = h; }
        }
        
        self.svg_output.push_str(&format!(
            "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\">\n",
            width, total_h
        ));
        
        for node_idx in &root_nodes {
            self.emit_svg_recursive1(*node_idx);
        }
        self.svg_output.push_str("</svg>");
        
        let mut screen_idx = -1;
        for i in 0..self.screen_ids.len() {
            if self.screen_ids[i] == self.current_screen {
                screen_idx = i as i32;
                break;
            }
        }
        if screen_idx < 0 && self.screen_count < MAX_SCREENS {
            screen_idx = self.screen_count as i32;
            self.screen_count += 1;
            self.screen_ids.push(self.current_screen.clone());
            self.screen_scroll_ys.push(0.0);
            self.screen_content_heights.push(0);
        }
        if screen_idx >= 0 {
            self.screen_content_heights[screen_idx as usize] = total_h;
        }
    }

    // --- 内部ユーティリティ ---
    fn set_state(&mut self, key: &str, val: &str) {
        if key.starts_with("~~") || key.starts_with("--") {
            let c_key = CString::new(key).unwrap();
            let c_val = CString::new(val).unwrap();
            unsafe { set_w1_global(c_key.as_ptr(), c_val.as_ptr()); }
            return;
        }
        if key.eq_ignore_ascii_case("_currentScreen") {
            self.current_screen = val.chars().take(63).collect();
            return;
        }
        for state in &mut self.state {
            if state.0.eq_ignore_ascii_case(key) {
                state.1 = val.chars().take(511).collect();
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
        if key.starts_with("~~") || key.starts_with("--") {
            unsafe {
                let c_key = CString::new(key).unwrap();
                let ptr = get_w1_global(c_key.as_ptr());
                if ptr.is_null() { return String::new(); }
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        } else if key.eq_ignore_ascii_case("_currentScreen") {
            self.current_screen.clone()
        } else {
            for state in &self.state {
                if state.0.eq_ignore_ascii_case(key) {
                    return state.1.clone();
                }
            }
            unsafe {
                let c_key = CString::new(key).unwrap();
                let ptr = get_w1_global(c_key.as_ptr());
                if ptr.is_null() { return String::new(); }
                let val = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                if !val.is_empty() { return val; }
            }
            String::new()
        }
    }

    fn eval_math1(&self, s: &str) -> i64 {
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') { i += 1; }
        if i >= chars.len() { return 0; }
        
        let mut res = Self::w1_strtol(&chars[i..].iter().collect::<String>());
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t' || chars[i] == '-' || chars[i].is_ascii_digit()) { i += 1; }
        
        while i < chars.len() {
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') { i += 1; }
            if i >= chars.len() { break; }
            let op = chars[i]; i += 1;
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') { i += 1; }
            let v = Self::w1_strtol(&chars[i..].iter().collect::<String>());
            match op {
                '+' => res += v,
                '-' => res -= v,
                '*' => res *= v,
                '/' => if v != 0 { res /= v; },
                _ => {}
            }
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t' || chars[i] == '-' || chars[i].is_ascii_digit()) { i += 1; }
        }
        res
    }

    fn w1_strtol(s: &str) -> i64 {
        let mut res: i64 = 0;
        let mut sign = 1;
        let mut chars = s.chars().peekable();
        while let Some(&c) = chars.peek() {
            if c == ' ' || c == '\t' { chars.next(); } else { break; }
        }
        if let Some(&'-') = chars.peek() { sign = -1; chars.next(); }
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                res = res * 10 + (c as i64 - '0' as i64);
                chars.next();
            } else { break; }
        }
        res * sign
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
                            let escaped = chars[i];
                            let actual = match escaped {
                                'n' => '\n', '"' => '"', '\'' => '\'', '\\' => '\\',
                                _ => escaped,
                            };
                            out.push(actual);
                        }
                    } else {
                        out.push(chars[i]);
                    }
                    i += 1;
                }
                if i < chars.len() { i += 1; }
            } else if (c == '-' && chars.get(i + 1) == Some(&'-')) || (c == '~' && chars.get(i + 1) == Some(&'~')) {
                let mut var = String::new();
                while i < chars.len() {
                    let c = chars[i];
                    if c == '"' || c == '\'' || c == '+' || c == ' ' || c == ')' || c == ',' || c == '}' {
                        break;
                    }
                    var.push(c);
                    i += 1;
                }
                let val = self.get_state(&var);
                out.push_str(&val);
            } else if c == '+' {
                i += 1;
            } else {
                out.push(c);
                i += 1;
            }
        }
        out
    }

    fn get_attr1(&self, node_idx: usize, key: &str) -> String {
        for attr in &self.nodes[node_idx].attrs {
            if attr.key == key { return attr.value.clone(); }
        }
        String::new()
    }

    fn eval_attr(&self, node_idx: usize, key: &str) -> String {
        for attr in &self.nodes[node_idx].attrs {
            if attr.key == key { return self.eval_expr(&attr.value); }
        }
        String::new()
    }

    fn next_token(&mut self) -> Token1 {
        while self.src_ptr < self.src.len() && (self.src[self.src_ptr] as u32) <= 32 {
            self.src_ptr += 1;
        }
        if self.src_ptr >= self.src.len() {
            return Token1 { r#type: Tk1Type::Eof, val: String::new() };
        }
        let c = self.src[self.src_ptr];
        if c == '@' {
            self.src_ptr += 1;
            return Token1 { r#type: Tk1Type::At, val: "@".to_string() };
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
            if self.src_ptr < self.src.len() { self.src_ptr += 1; }
            return Token1 { r#type: Tk1Type::Str, val };
        }
        let punct = "{}():;=+,";
        if punct.contains(c) {
            self.src_ptr += 1;
            return Token1 { r#type: Tk1Type::Punct, val: c.to_string() };
        }
        let mut val = String::new();
        while self.src_ptr < self.src.len() {
            let c = self.src[self.src_ptr];
            if (c as u32) <= 32 || punct.contains(c) { break; }
            val.push(c);
            self.src_ptr += 1;
        }
        Token1 { r#type: Tk1Type::Word, val }
    }

    fn alloc_node(&mut self) -> Option<usize> {
        self.nodes.push(Warp1Node::default());
        Some(self.nodes.len() - 1)
    }

    fn skip_block1(&mut self) {
        if self.token_pos + 1 >= self.tokens.len() || !self.tokens[self.token_pos + 1].val.starts_with('{') {
            self.token_pos += 1;
            return;
        }
        self.token_pos += 2;
        let mut depth = 1;
        while self.token_pos < self.tokens.len() && depth > 0 {
            if self.tokens[self.token_pos].r#type == Tk1Type::Punct {
                let c = self.tokens[self.token_pos].val.chars().next().unwrap_or(' ');
                if c == '{' { depth += 1; }
                else if c == '}' { depth -= 1; }
            }
            self.token_pos += 1;
        }
    }

    fn parse_current_screen1(&mut self) {
        if self.current_screen == self.parsed_screen_id && !self.root_nodes.is_empty() { return; }
        
        self.nodes.clear();
        self.root_nodes.clear();
        self.texts.clear();
        
        for i in 0..self.screens.len() {
            if self.screens[i].id == self.current_screen {
                self.token_pos = self.screens[i].token_index;
                if let Some(node_idx) = self.parse_node() {
                    self.root_nodes.push(node_idx);
                    self.init_state_from_ast1(node_idx);
                }
                self.parsed_screen_id = self.current_screen.clone();
                return;
            }
        }
        self.parsed_screen_id.clear();
    }

    fn init_state_from_ast1(&mut self, node_idx: usize) {
        let attrs = self.nodes[node_idx].attrs.clone();
        for attr in attrs {
            if attr.key.starts_with("--") {
                let val = self.eval_expr(&attr.value);
                self.set_state(&attr.key, &val);
            }
        }
        let children = self.nodes[node_idx].children.clone();
        for child_idx in children {
            self.init_state_from_ast1(child_idx);
        }
    }

    fn parse_script(&mut self) {
        self.token_pos += 1;
        if self.token_pos >= self.tokens.len() { return; }
        if self.scripts.len() >= MAX_SCRIPTS { self.token_pos += 1; return; }
        
        let name = self.tokens[self.token_pos].val.clone();
        self.token_pos += 1;
        let mut script = Script1 { name, blocks: Vec::new() };
        
        if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with('{') {
            self.token_pos += 1;
            while self.token_pos < self.tokens.len() && !self.tokens[self.token_pos].val.starts_with('}') {
                let val = self.tokens[self.token_pos].val.clone();
                if val == "if" || val == "elseIf" {
                    if script.blocks.len() < 100 {
                        let mut block = ScriptBlock1 { r#type: val.clone(), condition: String::new(), actions: String::new() };
                        self.token_pos += 1;
                        if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with(':') { self.token_pos += 1; }
                        if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with('(') {
                            self.token_pos += 1;
                            let mut p = 1;
                            while p > 0 && self.token_pos < self.tokens.len() {
                                let c = self.tokens[self.token_pos].val.chars().next().unwrap_or(' ');
                                if c == '(' { p += 1; }
                                else if c == ')' { p -= 1; }
                                if p > 0 {
                                    if self.tokens[self.token_pos].r#type == Tk1Type::Str { block.condition.push('"'); }
                                    block.condition.push_str(&self.tokens[self.token_pos].val);
                                    if self.tokens[self.token_pos].r#type == Tk1Type::Str { block.condition.push('"'); }
                                    self.token_pos += 1;
                                }
                            }
                            if self.token_pos < self.tokens.len() { self.token_pos += 1; }
                        }
                        if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with('{') {
                            self.token_pos += 1;
                            let mut bc = 1;
                            let mut prev_type = Tk1Type::Eof;
                            while bc > 0 && self.token_pos < self.tokens.len() {
                                let c = self.tokens[self.token_pos].val.chars().next().unwrap_or(' ');
                                if c == '{' { bc += 1; }
                                else if c == '}' { bc -= 1; }
                                if bc > 0 {
                                    let cur_type = self.tokens[self.token_pos].r#type;
                                    if !block.actions.is_empty() && (prev_type == Tk1Type::Word || prev_type == Tk1Type::Str) && (cur_type == Tk1Type::Word || cur_type == Tk1Type::Str) {
                                        block.actions.push(' ');
                                    }
                                    if cur_type == Tk1Type::Str { block.actions.push('"'); }
                                    block.actions.push_str(&self.tokens[self.token_pos].val);
                                    if cur_type == Tk1Type::Str { block.actions.push('"'); }
                                    prev_type = cur_type;
                                    self.token_pos += 1;
                                }
                            }
                            if self.token_pos < self.tokens.len() { self.token_pos += 1; }
                        }
                        script.blocks.push(block);
                    } else { self.token_pos += 1; }
                } else { self.token_pos += 1; }
            }
            if self.token_pos < self.tokens.len() { self.token_pos += 1; }
        }
        self.scripts.push(script);
    }

    fn parse_node(&mut self) -> Option<usize> {
        if self.token_pos >= self.tokens.len() { return None; }
        if self.tokens[self.token_pos].r#type == Tk1Type::At {
            self.parse_script();
            return None;
        }
        let tag_name = self.tokens[self.token_pos].val.clone();
        if self.token_pos + 1 < self.tokens.len() && self.tokens[self.token_pos + 1].val.starts_with('{') {
            let node_idx = self.alloc_node()?;
            self.nodes[node_idx].tag = tag_name;
            self.token_pos += 2;
            
            while self.token_pos < self.tokens.len() && !self.tokens[self.token_pos].val.starts_with('}') {
                if self.token_pos + 1 < self.tokens.len() && self.tokens[self.token_pos + 1].val.starts_with('{') {
                    if let Some(child_idx) = self.parse_node() {
                        self.nodes[node_idx].children.push(child_idx);
                    }
                    continue;
                }
                if self.token_pos + 1 < self.tokens.len() && self.tokens[self.token_pos + 1].val.starts_with(':') {
                    let key = self.tokens[self.token_pos].val.clone();
                    self.token_pos += 2;
                    let mut expr = String::new();
                    
                    if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with('(') {
                        self.token_pos += 1;
                        let mut p = 1;
                        let mut prev_type = Tk1Type::Eof;
                        while p > 0 && self.token_pos < self.tokens.len() {
                            let c = self.tokens[self.token_pos].val.chars().next().unwrap_or(' ');
                            if c == '(' { p += 1; }
                            else if c == ')' { p -= 1; }
                            if p > 0 {
                                let cur_type = self.tokens[self.token_pos].r#type;
                                if !expr.is_empty() && (prev_type == Tk1Type::Word || prev_type == Tk1Type::Str) && (cur_type == Tk1Type::Word || cur_type == Tk1Type::Str) {
                                    expr.push(' ');
                                }
                                if cur_type == Tk1Type::Str { expr.push('"'); }
                                expr.push_str(&self.tokens[self.token_pos].val);
                                if cur_type == Tk1Type::Str { expr.push('"'); }
                                prev_type = cur_type;
                                self.token_pos += 1;
                            }
                        }
                        if self.token_pos < self.tokens.len() { self.token_pos += 1; }
                    } else {
                        expr.push_str(&self.tokens[self.token_pos].val);
                        self.token_pos += 1;
                    }
                    
                    if key == "oneClick" {
                        self.nodes[node_idx].event_oneclick = expr;
                    } else {
                        self.nodes[node_idx].attrs.push(Warp1Attr { key, value: expr });
                    }
                    if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with(',') {
                        self.token_pos += 1;
                    }
                    continue;
                }
                self.token_pos += 1;
            }
            if self.token_pos < self.tokens.len() { self.token_pos += 1; }
            return Some(node_idx);
        }
        self.token_pos += 1;
        None
    }

    fn layout_node1(&mut self, node_idx: usize, px: i32, py: i32, limit_w: i32) -> i32 {
        self.nodes[node_idx].x = px;
        self.nodes[node_idx].y = py;
        self.nodes[node_idx].w = limit_w;
        let mut cy = py;
        
        let is_dark = self.get_state("~~main/dark") == "true";
        let frame_v = self.eval_attr(node_idx, "frame");
        let pos_v = self.eval_attr(node_idx, "position");
        
        if !frame_v.is_empty() {
            if frame_v.contains("width") {
                self.nodes[node_idx].w = if frame_v.contains("100vw") { self.win_w - 40 } else { 200 };
            }
            if frame_v.contains("height") { self.nodes[node_idx].h = 40; }
        }
        if !pos_v.is_empty() {
            if pos_v.contains("bottom") { self.nodes[node_idx].y = self.win_h - 60; }
            if pos_v.contains("left") { self.nodes[node_idx].x = 20; }
        }

        let tag = self.nodes[node_idx].tag.clone();
        if tag == "screen" {
            cy = py + 16;
            let children = self.nodes[node_idx].children.clone();
            for child_idx in children {
                if self.nodes[child_idx].tag != "Header" {
                    let h = self.layout_node1(child_idx, self.nodes[node_idx].x + 24, cy, limit_w - 48);
                    cy += h + 12;
                }
            }
            self.nodes[node_idx].h = cy - py + 4;
            if self.nodes[node_idx].h < self.win_h { self.nodes[node_idx].h = self.win_h; }
        } else if tag == "card" {
            cy += 12;
            let title = self.eval_attr(node_idx, "text");
            if !title.is_empty() && self.texts.len() < MAX_TEXTS {
                self.texts.push(Warp1Text {
                    x: px + 24, y: cy + 4, text: title, size: 20.0,
                    color: if is_dark { 0xFFEEEEEE } else { 0xFF121212 },
                });
                cy += 36;
            }
            let children = self.nodes[node_idx].children.clone();
            for child_idx in children {
                let h = self.layout_node1(child_idx, px + 24, cy, limit_w - 48);
                cy += h + 8;
            }
            self.nodes[node_idx].h = cy - py + 12;
        } else if tag == "button" || tag == "tonalButton" {
            self.nodes[node_idx].h = 40;
            let text = self.eval_attr(node_idx, "text");
            let c_text = CString::new(text.clone()).unwrap();
            let text_w = unsafe { measure_ttf_width(c_text.as_ptr(), 16.0) };
            self.nodes[node_idx].w = text_w + 32;
            if self.nodes[node_idx].w < 70 { self.nodes[node_idx].w = 70; }
            if self.nodes[node_idx].w > limit_w { self.nodes[node_idx].w = limit_w; }
            
            if self.texts.len() < MAX_TEXTS {
                self.texts.push(Warp1Text {
                    x: self.nodes[node_idx].x + (self.nodes[node_idx].w - text_w) / 2,
                    y: self.nodes[node_idx].y + 10,
                    text, size: 16.0,
                    color: if tag == "tonalButton" {
                        if is_dark { 0xFFFFFFFF } else { 0xFF000000 }
                    } else { 0xFFFFFFFF },
                });
            }
        } else if tag == "switch" {
            self.nodes[node_idx].w = 44;
            self.nodes[node_idx].h = 44;
            return self.nodes[node_idx].h;
        } else if tag == "input" {
            self.nodes[node_idx].w = limit_w;
            self.nodes[node_idx].h = 48;
            let out_var_raw = self.get_attr1(node_idx, "output");
            let out_var = if out_var_raw.starts_with('(') {
                let end = out_var_raw.find(')').unwrap_or(out_var_raw.len());
                out_var_raw[1..end].to_string()
            } else { out_var_raw };
            
            let placeholder = self.eval_attr(node_idx, "placeholder");
            let mut val = String::new();
            if !out_var.is_empty() { val = self.get_state(&out_var); }
            
            let id = self.eval_attr(node_idx, "id");
            if !id.is_empty() {
                let content_key = format!("--{}Content", id);
                let cv = self.get_state(&content_key);
                if !cv.is_empty() { val = cv; }
            }
            
            if self.texts.len() < MAX_TEXTS {
                let mut text_val = String::new();
                let mut color = 0;
                if !val.is_empty() {
                    text_val = val.clone();
                    color = if is_dark { 0xFFCCCCCC } else { 0xFF333333 };
                    if self.focused_node_idx == node_idx as isize {
                        let ticks_s = self.get_state("--warpTicks");
                        let ticks = Self::w1_strtol(&ticks_s);
                        if (ticks / 30) % 2 == 0 { text_val.push('|'); }
                    }
                } else {
                    text_val = placeholder.clone();
                    color = if is_dark { 0xFF666666 } else { 0xFF888888 };
                    if self.focused_node_idx == node_idx as isize {
                        let ticks_s = self.get_state("--warpTicks");
                        let ticks = Self::w1_strtol(&ticks_s);
                        if (ticks / 30) % 2 == 0 { text_val = "|".to_string(); }
                    }
                }
                self.texts.push(Warp1Text {
                    x: self.nodes[node_idx].x + 12,
                    y: self.nodes[node_idx].y + 16,
                    text: text_val, size: 16.0, color,
                });
            }
        } else if tag == "text" {
            let text = self.eval_attr(node_idx, "text");
            if self.texts.len() < MAX_TEXTS {
                self.texts.push(Warp1Text {
                    x: px, y: py, text: text.clone(), size: 16.0,
                    color: if is_dark { 0xFFCCCCCC } else { 0xFF333333 },
                });
            }
            let lines = text.matches('\n').count() as i32 + 1;
            self.nodes[node_idx].h = lines * 22;
        } else if tag == "hStack" {
            let mut cx = px;
            let mut max_h = 0;
            let div = if self.nodes[node_idx].children.is_empty() { 1 } else { self.nodes[node_idx].children.len() as i32 };
            let children = self.nodes[node_idx].children.clone();
            for child_idx in children {
                let h = self.layout_node1(child_idx, cx, py, limit_w / div);
                if h > max_h { max_h = h; }
                cx += self.nodes[child_idx].w + 8;
            }
            self.nodes[node_idx].h = max_h;
        } else if tag == "vStack" {
            let children = self.nodes[node_idx].children.clone();
            for child_idx in children {
                let h = self.layout_node1(child_idx, px, cy, limit_w);
                cy += h + 8;
            }
            self.nodes[node_idx].h = cy - py;
        } else {
            let children = self.nodes[node_idx].children.clone();
            for child_idx in children {
                let h = self.layout_node1(child_idx, px, cy, limit_w);
                cy += h + 4;
            }
            self.nodes[node_idx].h = cy - py;
        }
        self.nodes[node_idx].h
    }

    // 通常の角丸（rx属性）を出力するヘルパー
    fn emit_rounded_rect(&mut self, x: i32, y: i32, w: i32, h: i32, rx: i32, fill: &str, extra: &str) {
        self.svg_output.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{}\" {} />\n",
            x, y, w, h, rx, rx, fill, extra
        ));
    }

    fn emit_svg_recursive1(&mut self, node_idx: usize) {
        let is_dark = self.get_state("~~main/dark") == "true";
        let tag = self.nodes[node_idx].tag.clone();
        
        if tag == "screen" {
            // 透過背景
        } else if tag == "card" {
            let n = &self.nodes[node_idx];
            self.emit_rounded_rect(n.x, n.y, n.w, n.h, 12, if is_dark { "#1e1e1e" } else { "#ffffff" }, "");
        } else if tag == "button" {
            let n = &self.nodes[node_idx];
            self.emit_rounded_rect(n.x, n.y, n.w, n.h, 20, "#0A60FF", ""); // rx=20 for normal rounded
        } else if tag == "tonalButton" {
            let n = &self.nodes[node_idx];
            let fill = if is_dark { "#ffffff" } else { "#000000" };
            self.emit_rounded_rect(n.x, n.y, n.w, n.h, 20, fill, "opacity=\"0.1\"");
        } else if tag == "switch" {
            let out_var_raw = self.get_attr1(node_idx, "output");
            let out_var = if out_var_raw.starts_with('(') {
                let end = out_var_raw.find(')').unwrap_or(out_var_raw.len());
                out_var_raw[1..end].to_string()
            } else { out_var_raw };
            
            let val = self.get_state(&out_var);
            let on = val.contains("true");
            let disabled = val.contains("Disabled");
            
            let bg_color = if disabled {
                if on { "#80A0FF" } else { "#eeeeee" }
            } else {
                if on { "#0A60FF" } else { "#dddddd" }
            };
            let size = 44;
            let n = &self.nodes[node_idx];
            let x = n.x + (n.w - size) / 2;
            let y = n.y + (n.h - size) / 2;
            self.emit_rounded_rect(x, y, size, size, size / 2, bg_color, ""); // 円形に近い角丸
            
            if on {
                self.svg_output.push_str(&format!(
                    "<path d=\"M{} {} L{} {} L{} {}\" stroke=\"#ffffff\" stroke-width=\"4\" fill=\"none\" />\n",
                    x + 12, y + 22, x + 20, y + 30, x + 34, y + 14
                ));
            }
        } else if tag == "input" {
            let n = &self.nodes[node_idx];
            let mut stroke = if is_dark { "#555555" } else { "#dddddd" };
            let mut stroke_w = "1";
            
            if self.focused_node_idx == node_idx as isize {
                stroke = "#0A60FF";
                stroke_w = "2";
            }
            
            let extra = format!("stroke=\"{}\" stroke-width=\"{}\"", stroke, stroke_w);
            let fill = if is_dark { "#333333" } else { "#ffffff" };
            self.emit_rounded_rect(n.x, n.y, n.w, n.h, 8, fill, &extra);
        }
        
        let children = self.nodes[node_idx].children.clone();
        for child_idx in children {
            if self.nodes[child_idx].tag != "Header" {
                self.emit_svg_recursive1(child_idx);
            }
        }
    }
    
    fn execute_script1(&mut self, name: &str) {
        let scripts = self.scripts.clone();
        for script in &scripts {
            if script.name == name {
                let mut handled = false;
                for block in &script.blocks {
                    let cond = &block.condition;
                    if cond.contains('=') {
                        let parts: Vec<&str> = cond.splitn(2, '=').collect();
                        let l_val = self.eval_expr(parts[0]);
                        let r_val = self.eval_expr(parts[1]);
                        if l_val == r_val {
                            let actions = block.actions.clone();
                            self.execute_action1(&actions);
                            handled = true;
                            break;
                        }
                    } else if block.r#type == "if" || !handled {
                        let actions = block.actions.clone();
                        self.execute_action1(&actions);
                        handled = true;
                        break;
                    }
                }
                return;
            }
        }
    }

    fn execute_action1(&mut self, action_str: &str) {
        if action_str.is_empty()) return;
        let actions: Vec<&str> = action_str.split(',').collect();
        for act in actions {
            let act = act.trim();
            if act.starts_with("reset{") {
                unsafe { sys_restart(); }
            } else if act.starts_with("setScreen{") {
                let scr = act[10..].trim_end_matches('}');
                self.set_state("_currentScreen", scr);
            } else if act.starts_with("script{") {
                let sname = act[7..].trim_end_matches('}');
                self.execute_script1(sname);
            } else if act.starts_with("run{") {
                let expr = act[4..].trim_end_matches('}');
                let cmd = self.eval_expr(expr);
                let c_cmd = CString::new(cmd).unwrap();
                unsafe { set_pending_command(c_cmd.as_ptr()); }
            } else if act.contains('.') {
                let parts: Vec<&str> = act.splitn(2, '.').collect();
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
                        let key = format!("--{}Status", id);
                        self.set_state(&key, args);
                    }
                }
            } else if act.contains('=') || act.contains(':') {
                let parts: Vec<&str> = if act.contains('=') { act.splitn(2, '=').collect() } else { act.splitn(2, ':').collect() };
                let var_name = parts[0].trim();
                let rhs = parts[1].trim();
                let val = if rhs.starts_with("calc{") {
                    let m_expr = rhs[5..].trim_end_matches('}');
                    let m_expanded = self.eval_expr(m_expr);
                    let res = self.eval_math1(&m_expanded);
                    res.to_string()
                } else if rhs.contains(".replace{") {
                    let parts: Vec<&str> = rhs.splitn(2, ".replace{").collect();
                    let base = self.eval_expr(parts[0]);
                    let args = parts[1].trim_end_matches('}');
                    let replace_parts: Vec<&str> = args.splitn(2, ',').collect();
                    let old_s = self.eval_expr(replace_parts[0]);
                    let new_s = self.eval_expr(replace_parts[1]);
                    if !old_s.is_empty() {
                        base.replace(&old_s, &new_s)
                    } else { base }
                } else {
                    self.eval_expr(rhs)
                };
                self.set_state(var_name, &val);
            }
        }
    }

    pub fn click(&mut self, x: i32, y: i32) {
        self.parse_current_screen1();
        let mut clicked = false;
        
        for i in (0..self.nodes.len()).rev() {
            let n = &self.nodes[i];
            if x >= n.x && x <= n.x + n.w && y >= n.y && y <= n.y + n.h {
                clicked = true;
                self.focused_node_idx = -1;
                let tag = n.tag.clone();
                
                if tag == "switch" {
                    let out_var_raw = self.get_attr1(i, "output");
                    let out_var = if out_var_raw.starts_with('(') {
                        let end = out_var_raw.find(')').unwrap_or(out_var_raw.len());
                        out_var_raw[1..end].to_string()
                    } else { out_var_raw };
                    
                    if !out_var.is_empty() {
                        let current = self.get_state(&out_var);
                        if !current.contains("Disabled") {
                            let on = current.contains("true");
                            self.set_state(&out_var, if on { "false" } else { "true" });
                            let event = self.nodes[i].event_oneclick.clone();
                            if !event.is_empty() { self.execute_action1(&event); }
                        }
                    }
                    break;
                }
                if tag == "input" {
                    self.focused_node_idx = i as isize;
                    break;
                }
                if tag == "button" || tag == "tonalButton" {
                    let event = self.nodes[i].event_oneclick.clone();
                    if !event.is_empty() { self.execute_action1(&event); }
                    break;
                }
                let event = self.nodes[i].event_oneclick.clone();
                if !event.is_empty() {
                    self.execute_action1(&event);
                    break;
                }
            }
        }
        if !clicked { self.focused_node_idx = -1; }
        self.engine_dirty = true;
    }

    pub fn key_input(&mut self, c: char) {
        let uc = c as u32;
        if uc == 0x11 || uc == 0x13 {
            let start = if self.focused_node_idx <= 0 { self.nodes.len() as isize - 1 } else { self.focused_node_idx - 1 };
            for i in 0..self.nodes.len() {
                let idx = ((start - i as isize + self.nodes.len() as isize) % self.nodes.len() as isize) as usize;
                if self.nodes[idx].tag == "input" {
                    self.focused_node_idx = idx as isize;
                    break;
                }
            }
            self.engine_dirty = true;
            return;
        }
        if uc == 0x12 || uc == 0x14 || uc == '\t' as u32 {
            let start = if self.focused_node_idx < 0 { 0 } else { self.focused_node_idx + 1 };
            for i in 0..self.nodes.len() {
                let idx = ((start + i as isize) % self.nodes.len() as isize) as usize;
                if self.nodes[idx].tag == "input" {
                    self.focused_node_idx = idx as isize;
                    self.engine_dirty = true;
                    return;
                }
            }
        }

        if self.focused_node_idx < 0 || self.focused_node_idx >= self.nodes.len() as isize { return; }
        let idx = self.focused_node_idx as usize;
        if self.nodes[idx].tag != "input" { return; }

        let out_var_raw = self.get_attr1(idx, "output");
        let out_var = if out_var_raw.starts_with('(') {
            let end = out_var_raw.find(')').unwrap_or(out_var_raw.len());
            out_var_raw[1..end].to_string()
        } else { out_var_raw };
        
        if out_var.is_empty() { return; }

        let mut val = self.get_state(&out_var);
        if uc == 8 || uc == 127 {
            val.pop();
            self.set_state(&out_var, &val);
        } else if (32..=126).contains(&uc) {
            val.push(c);
            self.set_state(&out_var, &val);
        }
        self.engine_dirty = true;
    }
    
    // --- 公開API ---
    pub fn get_svg(&self) -> &str { &self.svg_output }
    
    pub fn draw_texts(&self, layer: *mut std::ffi::c_void, ox: i32, oy: i32, scale: f32) {
        for text in &self.texts {
            let c_text = CString::new(text.text.clone()).unwrap();
            unsafe {
                layer_draw_ttf(
                    layer,
                    ((text.x as f32) * scale) as i32 + ox,
                    ((text.y as f32) * scale) as i32 + oy,
                    c_text.as_ptr(),
                    text.size * scale,
                    text.color,
                );
            }
        }
    }

    pub fn set_state_ext(&mut self, k: &str, v: &str) {
        self.set_state(k, v);
        if k.eq_ignore_ascii_case("_currentScreen") {
            self.parse_current_screen1();
        }
        self.engine_dirty = true;
    }

    pub fn set_mouse(&mut self, x: i32, y: i32) { self.mouse_x = x; self.mouse_y = y; }
    pub fn is_dirty(&self) -> bool { self.engine_dirty }
    pub fn clear_dirty(&mut self) { self.engine_dirty = false; }
    pub fn get_node_count(&self) -> usize { self.nodes.len() }
    
    pub fn get_node_info(&self, index: usize) -> Option<(i32, i32, i32, i32, bool)> {
        self.nodes.get(index).map(|n| (n.x, n.y, n.w, n.h, n.is_dirty))
    }
    
    pub fn get_node_svg(&self, _i: usize) -> &str { "" }
    pub fn get_node_prev_rect(&self, _i: usize) -> (i32, i32, i32, i32) { (0, 0, 0, 0) }
    pub fn get_status(&self) -> &str { &self.engine_status }

    fn find_header_node1(&self) -> Option<usize> {
        for i in 0..self.nodes.len() {
            if self.nodes[i].tag == "Header" { return Some(i); }
        }
        None
    }

    pub fn get_header_info(&mut self, max_len: usize) -> Option<(String, usize)> {
        if let Some(h_idx) = self.find_header_node1() {
            let t = self.eval_attr(h_idx, "text");
            let t = if t.len() > max_len { t[..max_len].to_string() } else { t };
            let c = self.nodes[h_idx].children.len();
            Some((t, c))
        } else { None }
    }

    pub fn get_header_action_info(&mut self, i: usize, max_len: usize) -> Option<String> {
        if let Some(h_idx) = self.find_header_node1() {
            if i < self.nodes[h_idx].children.len() {
                let child_idx = self.nodes[h_idx].children[i];
                let t = self.eval_attr(child_idx, "text");
                return Some(if t.len() > max_len { t[..max_len].to_string() } else { t });
            }
        }
        None
    }

    pub fn click_header_action(&mut self, i: usize) {
        if let Some(h_idx) = self.find_header_node1() {
            if i < self.nodes[h_idx].children.len() {
                let child_idx = self.nodes[h_idx].children[i];
                let event = self.nodes[child_idx].event_oneclick.clone();
                if !event.is_empty() {
                    self.execute_action1(&event);
                }
            }
        }
    }
}