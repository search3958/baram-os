extern crate alloc;

use alloc::format;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use crate::gop::Color;
use crate::window::LayerSystem;
use crate::ttf_font;

const MAX_VARS: usize = 256;
const MAX_SCREENS: usize = 64;
const MAX_SCRIPTS: usize = 64;
const MAX_TEXTS: usize = 1024;

#[derive(Clone, Default)]
struct Attr {
    key: String,
    value: String,
}

#[derive(Clone, Default)]
struct Node {
    tag: String,
    attrs: Vec<Attr>,
    event_oneclick: String,
    children: Vec<usize>,
    x: i32, y: i32, w: i32, h: i32,
}

#[derive(Clone, Default)]
struct ScriptBlock {
    r#type: String,
    condition: String,
    actions: String,
}

#[derive(Clone, Default)]
struct Script {
    name: String,
    blocks: Vec<ScriptBlock>,
}

#[derive(Clone, Copy, PartialEq)]
enum TkType { Word, Str, Punct, At, Eof }

impl Default for TkType { fn default() -> Self { TkType::Eof } }

#[derive(Clone, Default)]
struct Token {
    r#type: TkType,
    val: String,
}

struct TextElem {
    x: i32, y: i32,
    text: String,
    color: Color,
    size: f32,
}

struct ScreenInfo {
    id: String,
    token_index: usize,
}

pub struct WarpEngine {
    state: Vec<(String, String)>,
    current_screen: String,
    parsed_screen_id: String,
    screens: Vec<ScreenInfo>,
    nodes: Vec<Node>,
    root_nodes: Vec<usize>,
    scripts: Vec<Script>,
    src: Vec<char>,
    src_ptr: usize,
    tokens: Vec<Token>,
    token_pos: usize,
    pub texts: Vec<TextElem>,
    pub svg_output: String,
    pub dirty: bool,
}

fn measure_text_width(text: &str, size: f32) -> i32 {
    if !ttf_font::is_available() {
        return (text.len() as i32) * 8;
    }
    let mut total = 0i32;
    for ch in text.chars() {
        let g = ttf_font::glyph(ch);
        if g.w > 0 {
            total += g.advance.max(0) as i32;
        } else {
            total += 8;
        }
    }
    total
}

impl WarpEngine {
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
            dirty: true,
        };
        loop {
            let tk = ctx.next_token();
            if tk.r#type == TkType::Eof || ctx.tokens.len() >= 4096 { break; }
            ctx.tokens.push(tk);
        }
        ctx.token_pos = 0;
        while ctx.token_pos < ctx.tokens.len() {
            if ctx.tokens[ctx.token_pos].r#type == TkType::At {
                ctx.parse_script();
            } else if ctx.tokens[ctx.token_pos].r#type == TkType::Word && ctx.tokens[ctx.token_pos].val == "screen" {
                let mut screen_id = String::from("main");
                let start_pos = ctx.token_pos;
                if ctx.token_pos + 1 < ctx.tokens.len() && ctx.tokens[ctx.token_pos + 1].val.starts_with('{') {
                    let mut j = ctx.token_pos + 2;
                    let mut depth = 1;
                    while j < ctx.tokens.len() && depth > 0 {
                        let vf = ctx.tokens[j].val.chars().next().unwrap_or(' ');
                        if ctx.tokens[j].r#type == TkType::Punct {
                            if vf == '{' { depth += 1; }
                            else if vf == '}' { depth -= 1; }
                        }
                        if depth == 1 && ctx.tokens[j].r#type == TkType::Word && ctx.tokens[j].val == "id" && j + 1 < ctx.tokens.len() && ctx.tokens[j+1].val.starts_with(':') {
                            let mut k = j + 2;
                            if k < ctx.tokens.len() && ctx.tokens[k].val.starts_with('(') { k += 1; }
                            if k < ctx.tokens.len() && ctx.tokens[k].r#type != TkType::Punct {
                                screen_id = ctx.tokens[k].val.clone();
                            }
                        }
                        j += 1;
                    }
                    if ctx.screens.len() < MAX_SCREENS {
                        ctx.screens.push(ScreenInfo {
                            id: if screen_id.is_empty() { String::from("main") } else { screen_id },
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

    pub fn update(&mut self, width: i32, height: i32) {
        self.parse_current_screen();
        self.texts.clear();
        self.svg_output.clear();
        let root_nodes = self.root_nodes.clone();
        let mut total_h = height;
        for node_idx in &root_nodes {
            let h = self.layout_node(*node_idx, 0, 0, width);
            if h > total_h { total_h = h; }
        }
        self.svg_output.push_str(&format!(
            "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\">\n",
            width, total_h
        ));
        for node_idx in &root_nodes {
            self.emit_svg(*node_idx);
        }
        self.svg_output.push_str("</svg>");
        self.dirty = true;
    }

    fn set_state(&mut self, key: &str, val: &str) {
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
        for s in &self.state {
            if s.0.eq_ignore_ascii_case(key) { return s.1.clone(); }
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
                            out.push(match chars[i] { 'n' => '\n', '"' => '"', '\'' => '\'', '\\' => '\\', x => x });
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
                    let c2 = chars[i];
                    if c2 == '"' || c2 == '\'' || c2 == '+' || c2 == ' ' || c2 == ')' || c2 == ',' || c2 == '}' { break; }
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
            if a.key == key { return self.eval_expr(&a.value); }
        }
        String::new()
    }

    fn get_attr_raw(&self, idx: usize, key: &str) -> String {
        for a in &self.nodes[idx].attrs {
            if a.key == key { return a.value.clone(); }
        }
        String::new()
    }

    fn strtol(s: &str) -> i64 {
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

    fn eval_math(&self, s: &str) -> i64 {
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') { i += 1; }
        if i >= chars.len() { return 0; }
        let mut res = Self::strtol(&chars[i..].iter().collect::<String>());
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t' || chars[i] == '-' || chars[i].is_ascii_digit()) { i += 1; }
        while i < chars.len() {
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') { i += 1; }
            if i >= chars.len() { break; }
            let op = chars[i]; i += 1;
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') { i += 1; }
            let v = Self::strtol(&chars[i..].iter().collect::<String>());
            match op {
                '+' => res += v, '-' => res -= v, '*' => res *= v,
                '/' => if v != 0 { res /= v; }, _ => {}
            }
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t' || chars[i] == '-' || chars[i].is_ascii_digit()) { i += 1; }
        }
        res
    }

    fn next_token(&mut self) -> Token {
        while self.src_ptr < self.src.len() && (self.src[self.src_ptr] as u32) <= 32 {
            self.src_ptr += 1;
        }
        if self.src_ptr >= self.src.len() {
            return Token { r#type: TkType::Eof, val: String::new() };
        }
        let c = self.src[self.src_ptr];
        if c == '@' {
            self.src_ptr += 1;
            return Token { r#type: TkType::At, val: String::from("@") };
        }
        if c == '"' || c == '\'' {
            let quote = c;
            self.src_ptr += 1;
            let mut val = String::new();
            while self.src_ptr < self.src.len() && self.src[self.src_ptr] != quote {
                if self.src[self.src_ptr] == '\\' {
                    self.src_ptr += 1;
                    if self.src_ptr < self.src.len() { val.push(self.src[self.src_ptr]); self.src_ptr += 1; }
                } else {
                    val.push(self.src[self.src_ptr]); self.src_ptr += 1;
                }
            }
            if self.src_ptr < self.src.len() { self.src_ptr += 1; }
            return Token { r#type: TkType::Str, val };
        }
        let punct = "{}():;=+,";
        if punct.contains(c) {
            self.src_ptr += 1;
            return Token { r#type: TkType::Punct, val: c.to_string() };
        }
        let mut val = String::new();
        while self.src_ptr < self.src.len() {
            let c2 = self.src[self.src_ptr];
            if (c2 as u32) <= 32 || punct.contains(c2) { break; }
            val.push(c2);
            self.src_ptr += 1;
        }
        Token { r#type: TkType::Word, val }
    }

    fn alloc_node(&mut self) -> Option<usize> {
        self.nodes.push(Node::default());
        Some(self.nodes.len() - 1)
    }

    fn skip_block(&mut self) {
        if self.token_pos + 1 >= self.tokens.len() || !self.tokens[self.token_pos + 1].val.starts_with('{') {
            self.token_pos += 1;
            return;
        }
        self.token_pos += 2;
        let mut depth = 1;
        while self.token_pos < self.tokens.len() && depth > 0 {
            if self.tokens[self.token_pos].r#type == TkType::Punct {
                let c = self.tokens[self.token_pos].val.chars().next().unwrap_or(' ');
                if c == '{' { depth += 1; } else if c == '}' { depth -= 1; }
            }
            self.token_pos += 1;
        }
    }

    fn parse_current_screen(&mut self) {
        if self.current_screen == self.parsed_screen_id && !self.root_nodes.is_empty() { return; }
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
        for c in children { self.init_state_from_ast(c); }
    }

    fn parse_script(&mut self) {
        self.token_pos += 1;
        if self.token_pos >= self.tokens.len() { return; }
        if self.scripts.len() >= MAX_SCRIPTS { self.token_pos += 1; return; }
        let name = self.tokens[self.token_pos].val.clone();
        self.token_pos += 1;
        let mut script = Script { name, blocks: Vec::new() };
        if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with('{') {
            self.token_pos += 1;
            while self.token_pos < self.tokens.len() && !self.tokens[self.token_pos].val.starts_with('}') {
                let val = self.tokens[self.token_pos].val.clone();
                if val == "if" || val == "elseIf" {
                    if script.blocks.len() < 100 {
                        let mut block = ScriptBlock { r#type: val, condition: String::new(), actions: String::new() };
                        self.token_pos += 1;
                        if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with(':') { self.token_pos += 1; }
                        if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with('(') {
                            self.token_pos += 1;
                            let mut p = 1;
                            while p > 0 && self.token_pos < self.tokens.len() {
                                let c = self.tokens[self.token_pos].val.chars().next().unwrap_or(' ');
                                if c == '(' { p += 1; } else if c == ')' { p -= 1; }
                                if p > 0 {
                                    if self.tokens[self.token_pos].r#type == TkType::Str { block.condition.push('"'); }
                                    block.condition.push_str(&self.tokens[self.token_pos].val);
                                    if self.tokens[self.token_pos].r#type == TkType::Str { block.condition.push('"'); }
                                    self.token_pos += 1;
                                }
                            }
                            if self.token_pos < self.tokens.len() { self.token_pos += 1; }
                        }
                        if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with('{') {
                            self.token_pos += 1;
                            let mut bc = 1;
                            let mut prev_type = TkType::Eof;
                            while bc > 0 && self.token_pos < self.tokens.len() {
                                let c = self.tokens[self.token_pos].val.chars().next().unwrap_or(' ');
                                if c == '{' { bc += 1; } else if c == '}' { bc -= 1; }
                                if bc > 0 {
                                    let cur_type = self.tokens[self.token_pos].r#type;
                                    if !block.actions.is_empty() && (prev_type == TkType::Word || prev_type == TkType::Str) && (cur_type == TkType::Word || cur_type == TkType::Str) {
                                        block.actions.push(' ');
                                    }
                                    if cur_type == TkType::Str { block.actions.push('"'); }
                                    block.actions.push_str(&self.tokens[self.token_pos].val);
                                    if cur_type == TkType::Str { block.actions.push('"'); }
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
        if self.tokens[self.token_pos].r#type == TkType::At {
            self.parse_script();
            return None;
        }
        let tag_name = self.tokens[self.token_pos].val.clone();
        if self.token_pos + 1 < self.tokens.len() && self.tokens[self.token_pos + 1].val.starts_with('{') {
            let idx = self.alloc_node()?;
            self.nodes[idx].tag = tag_name;
            self.token_pos += 2;
            while self.token_pos < self.tokens.len() && !self.tokens[self.token_pos].val.starts_with('}') {
                if self.token_pos + 1 < self.tokens.len() && self.tokens[self.token_pos + 1].val.starts_with('{') {
                    if let Some(ci) = self.parse_node() {
                        self.nodes[idx].children.push(ci);
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
                        let mut prev_type = TkType::Eof;
                        while p > 0 && self.token_pos < self.tokens.len() {
                            let c = self.tokens[self.token_pos].val.chars().next().unwrap_or(' ');
                            if c == '(' { p += 1; } else if c == ')' { p -= 1; }
                            if p > 0 {
                                let cur_type = self.tokens[self.token_pos].r#type;
                                if !expr.is_empty() && (prev_type == TkType::Word || prev_type == TkType::Str) && (cur_type == TkType::Word || cur_type == TkType::Str) {
                                    expr.push(' ');
                                }
                                if cur_type == TkType::Str { expr.push('"'); }
                                expr.push_str(&self.tokens[self.token_pos].val);
                                if cur_type == TkType::Str { expr.push('"'); }
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
                        self.nodes[idx].event_oneclick = expr;
                    } else {
                        self.nodes[idx].attrs.push(Attr { key, value: expr });
                    }
                    if self.token_pos < self.tokens.len() && self.tokens[self.token_pos].val.starts_with(',') {
                        self.token_pos += 1;
                    }
                    continue;
                }
                self.token_pos += 1;
            }
            if self.token_pos < self.tokens.len() { self.token_pos += 1; }
            return Some(idx);
        }
        self.token_pos += 1;
        None
    }

    fn layout_node(&mut self, idx: usize, px: i32, py: i32, limit_w: i32) -> i32 {
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
            if self.nodes[idx].h < 600 { self.nodes[idx].h = 600; }
        } else if tag == "card" {
            cy += 12;
            let title = self.get_attr(idx, "text");
            if !title.is_empty() && self.texts.len() < MAX_TEXTS {
                self.texts.push(TextElem {
                    x: px + 24, y: cy + 4, text: title, size: 20.0,
                    color: Color::TEXT,
                });
                cy += 36;
            }
            let children = self.nodes[idx].children.clone();
            for ci in children {
                let h = self.layout_node(ci, px + 24, cy, limit_w - 48);
                cy += h + 8;
            }
            self.nodes[idx].h = cy - py + 12;
        } else if tag == "button" || tag == "tonalButton" {
            self.nodes[idx].h = 40;
            let text = self.get_attr(idx, "text");
            let text_w = measure_text_width(&text, 16.0);
            self.nodes[idx].w = text_w + 32;
            if self.nodes[idx].w < 70 { self.nodes[idx].w = 70; }
            if self.nodes[idx].w > limit_w { self.nodes[idx].w = limit_w; }
            if self.texts.len() < MAX_TEXTS {
                self.texts.push(TextElem {
                    x: self.nodes[idx].x + (self.nodes[idx].w - text_w) / 2,
                    y: self.nodes[idx].y + 10,
                    text, size: 16.0,
                    color: if tag == "tonalButton" { Color::TEXT } else { Color::rgb(255, 255, 255) },
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
            let mut val = if !out_var.is_empty() { self.get_state(&out_var) } else { String::new() };
            if val.is_empty() { val = placeholder; }
            if self.texts.len() < MAX_TEXTS {
                self.texts.push(TextElem {
                    x: self.nodes[idx].x + 12, y: self.nodes[idx].y + 16,
                    text: val, size: 16.0, color: Color::TEXT,
                });
            }
        } else if tag == "text" {
            let text = self.get_attr(idx, "text");
            if self.texts.len() < MAX_TEXTS {
                self.texts.push(TextElem {
                    x: px, y: py, text: text.clone(), size: 16.0,
                    color: Color::TEXT,
                });
            }
            let lines = text.matches('\n').count() as i32 + 1;
            self.nodes[idx].h = lines * 22;
        } else if tag == "hStack" {
            let mut cx = px;
            let mut max_h = 0;
            let div = if self.nodes[idx].children.is_empty() { 1 } else { self.nodes[idx].children.len() as i32 };
            let children = self.nodes[idx].children.clone();
            for ci in children {
                let h = self.layout_node(ci, cx, py, limit_w / div);
                if h > max_h { max_h = h; }
                cx += self.nodes[ci].w + 8;
            }
            self.nodes[idx].h = max_h;
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

    fn parse_out_var(&self, idx: usize) -> String {
        let raw = self.get_attr_raw(idx, "output");
        if raw.starts_with('(') {
            let end = raw.find(')').unwrap_or(raw.len());
            raw[1..end].to_string()
        } else { raw }
    }

    fn emit_svg(&mut self, idx: usize) {
        let tag = self.nodes[idx].tag.clone();
        if tag == "screen" {
            // transparent background
        } else if tag == "card" {
            let n = &self.nodes[idx];
            self.svg_output.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"12\" ry=\"12\" fill=\"#ffffff\" />\n",
                n.x, n.y, n.w, n.h
            ));
        } else if tag == "button" {
            let n = &self.nodes[idx];
            self.svg_output.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"20\" ry=\"20\" fill=\"#0A60FF\" />\n",
                n.x, n.y, n.w, n.h
            ));
        } else if tag == "tonalButton" {
            let n = &self.nodes[idx];
            self.svg_output.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"20\" ry=\"20\" fill=\"#000000\" opacity=\"0.1\" />\n",
                n.x, n.y, n.w, n.h
            ));
        } else if tag == "switch" {
            let out_var = self.parse_out_var(idx);
            let val = self.get_state(&out_var);
            let on = val.contains("true");
            let bg = if on { "#0A60FF" } else { "#dddddd" };
            let n = &self.nodes[idx];
            let x = n.x + (n.w - 44) / 2;
            let y = n.y + (n.h - 44) / 2;
            self.svg_output.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"44\" height=\"44\" rx=\"22\" ry=\"22\" fill=\"{}\" />\n",
                x, y, bg
            ));
            if on {
                self.svg_output.push_str(&format!(
                    "<path d=\"M{} {} L{} {} L{} {}\" stroke=\"#ffffff\" stroke-width=\"4\" fill=\"none\" />\n",
                    x + 12, y + 22, x + 20, y + 30, x + 34, y + 14
                ));
            }
        } else if tag == "input" {
            let n = &self.nodes[idx];
            self.svg_output.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\" ry=\"8\" fill=\"#ffffff\" stroke=\"#dddddd\" stroke-width=\"1\" />\n",
                n.x, n.y, n.w, n.h
            ));
        }
        let children = self.nodes[idx].children.clone();
        for ci in children {
            if self.nodes[ci].tag != "Header" {
                self.emit_svg(ci);
            }
        }
    }

    pub fn click(&mut self, x: i32, y: i32) {
        self.parse_current_screen();
        for i in (0..self.nodes.len()).rev() {
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
                            if !ev.is_empty() { self.execute_action(&ev); }
                        }
                    }
                    break;
                }
                if tag == "button" || tag == "tonalButton" {
                    let ev = self.nodes[i].event_oneclick.clone();
                    if !ev.is_empty() { self.execute_action(&ev); }
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

    fn execute_script(&mut self, name: &str) {
        let scripts = self.scripts.clone();
        for script in &scripts {
            if script.name == name {
                let mut handled = false;
                for block in &script.blocks {
                    let cond = &block.condition;
                    if cond.contains('=') {
                        let parts: alloc::vec::Vec<&str> = cond.splitn(2, '=').collect();
                        let lv = self.eval_expr(parts[0]);
                        let rv = self.eval_expr(parts[1]);
                        if lv == rv {
                            let actions = block.actions.clone();
                            self.execute_action(&actions);
                            handled = true;
                            break;
                        }
                    } else if block.r#type == "if" || !handled {
                        let actions = block.actions.clone();
                        self.execute_action(&actions);
                        handled = true;
                        break;
                    }
                }
                return;
            }
        }
    }

    fn execute_action(&mut self, action_str: &str) {
        if action_str.is_empty() { return; }
        let actions: alloc::vec::Vec<&str> = action_str.split(',').collect();
        for act in actions {
            let act = act.trim();
            if act.starts_with("setScreen{") {
                let scr = act[10..].trim_end_matches('}');
                self.set_state("_currentScreen", scr);
            } else if act.starts_with("script{") {
                let sn = act[7..].trim_end_matches('}');
                self.execute_script(sn);
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
                        let key = format!("--{}Status", id);
                        self.set_state(&key, args);
                    }
                }
            } else if act.contains('=') || act.contains(':') {
                let parts: alloc::vec::Vec<&str> = if act.contains('=') { act.splitn(2, '=').collect() } else { act.splitn(2, ':').collect() };
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
                    if !old_s.is_empty() { base.replace(&old_s, &new_s) } else { base }
                } else {
                    self.eval_expr(rhs)
                };
                self.set_state(var_name, &val);
            }
        }
    }

    pub fn draw_texts(&self, layer: &mut LayerSystem, ox: i32, oy: i32, scale: f32) {
        for t in &self.texts {
            let x = ((t.x as f32) * scale) as i32 + ox;
            let y = ((t.y as f32) * scale) as i32 + oy;
            let sz = t.size * scale;
            if t.text.is_empty() { continue; }
            if sz >= 14.0 {
                layer.put_str(x as usize, y as usize, &t.text, t.color);
            } else {
                layer.put_str(x as usize, y as usize, &t.text, t.color);
            }
        }
    }

    pub fn draw_svg_to_layer(&self, layer: &mut LayerSystem, ox: i32, oy: i32, win_w: usize, win_h: usize) {
        if self.svg_output.is_empty() { return; }
        let lw = layer.width();
        let lh = layer.height();
        let buf = layer.buf_mut();
        let bytes = self.svg_output.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        while i < len {
            if bytes[i] == b'<' {
                let mut j = i + 1;
                if j < len && bytes[j] == b'/' { i += 1; continue; }
                let name_start = j;
                while j < len && bytes[j] != b'>' && bytes[j] != b' ' && bytes[j] != b'/' { j += 1; }
                let name = &bytes[name_start..j];
                let attr_start = j;
                while j < len && bytes[j] != b'>' { j += 1; }
                if j < len { j += 1; }
                let attrs_str = core::str::from_utf8(&bytes[attr_start..j.min(len)]).unwrap_or("");
                i = j;
                if name == b"rect" {
                    let rx = parse_svg_attr(attrs_str, "x").unwrap_or(0.0) as i32 + ox;
                    let ry = parse_svg_attr(attrs_str, "y").unwrap_or(0.0) as i32 + oy;
                    let rw = parse_svg_attr(attrs_str, "width").unwrap_or(0.0) as usize;
                    let rh = parse_svg_attr(attrs_str, "height").unwrap_or(0.0) as usize;
                    let fill = parse_svg_fill(attrs_str);
                    let stroke = parse_svg_stroke(attrs_str);
                    let stroke_w = parse_svg_attr(attrs_str, "stroke-width").unwrap_or(0.0) as usize;
                    let opacity = parse_svg_opacity(attrs_str);
                    fill_rounded_rect_in_buf(buf, lw, lh, rx as usize, ry as usize, rw, rh, 8, fill, opacity);
                    if stroke_w > 0 {
                        outline_rect_in_buf(buf, lw, lh, rx as usize, ry as usize, rw, rh, stroke, opacity);
                    }
                } else if name == b"path" {
                    if let Some(d) = parse_svg_attr_str(attrs_str, "d") {
                        let stroke = parse_svg_stroke(attrs_str);
                        let sw = parse_svg_attr(attrs_str, "stroke-width").unwrap_or(1.0) as u32;
                        draw_svg_path_simple(buf, lw, lh, d, stroke, sw, ox, oy);
                    }
                }
            } else {
                i += 1;
            }
        }
    }
}

fn parse_svg_attr(attrs: &str, key: &str) -> Option<f32> {
    let needle_len = key.len() + 2;
    let mut i = 0;
    let bytes = attrs.as_bytes();
    while i < bytes.len() {
        if i + key.len() + 1 < bytes.len() && &bytes[i..i + key.len()] == key.as_bytes() && bytes[i + key.len()] == b'=' {
            let mut j = i + key.len() + 1;
            if j < bytes.len() && bytes[j] == b'"' {
                j += 1;
                let start = j;
                while j < bytes.len() && bytes[j] != b'"' { j += 1; }
                if let Ok(v) = core::str::from_utf8(&bytes[start..j]) {
                    return v.trim().parse().ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn parse_svg_attr_str<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    let bytes = attrs.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + key.len() + 1 < bytes.len() && &bytes[i..i + key.len()] == key.as_bytes() && bytes[i + key.len()] == b'=' {
            let mut j = i + key.len() + 1;
            if j < bytes.len() && bytes[j] == b'"' {
                j += 1;
                let start = j;
                while j < bytes.len() && bytes[j] != b'"' { j += 1; }
                return core::str::from_utf8(&bytes[start..j]).ok();
            }
        }
        i += 1;
    }
    None
}

fn parse_svg_fill(attrs: &str) -> Color {
    if let Some(v) = parse_svg_attr_str(attrs, "fill") { parse_hex_color(v) } else { Color::BLACK }
}

fn parse_svg_stroke(attrs: &str) -> Color {
    if let Some(v) = parse_svg_attr_str(attrs, "stroke") { parse_hex_color(v) } else { Color::TRANSPARENT }
}

fn parse_svg_opacity(attrs: &str) -> f32 {
    parse_svg_attr(attrs, "opacity").unwrap_or(1.0)
}

fn parse_hex_color(s: &str) -> Color {
    let s = s.trim().trim_start_matches('#');
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
            Color::rgb(r, g, b)
        }
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&s[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&s[2..3], 16).unwrap_or(0) * 17;
            Color::rgb(r, g, b)
        }
        _ => Color::BLACK,
    }
}

fn fill_rounded_rect_in_buf(buf: &mut [u32], stride: usize, buf_h: usize, x: usize, y: usize, w: usize, h: usize, r: usize, c: Color, opacity: f32) {
    let r = r.min(w / 2).min(h / 2);
    let x0 = x.min(stride);
    let y0 = y.min(buf_h);
    let x1 = (x + w).min(stride);
    let y1 = (y + h).min(buf_h);
    if x0 >= x1 || y0 >= y1 { return; }
    let v = c.0;
    let a = (opacity * 255.0) as u32;
    if a >= 255 {
        for py in y0..y1 {
            buf[py * stride + x0..py * stride + x1].fill(v);
        }
    } else {
        let inv = 255 - a;
        let cr = c.r() as u32;
        let cg = c.g() as u32;
        let cb = c.b() as u32;
        for py in y0..y1 {
            for px in x0..x1 {
                let bg = Color(buf[py * stride + px]);
                let r2 = (cr * a + bg.r() as u32 * inv) / 255;
                let g2 = (cg * a + bg.g() as u32 * inv) / 255;
                let b2 = (cb * a + bg.b() as u32 * inv) / 255;
                buf[py * stride + px] = Color::rgb(r2 as u8, g2 as u8, b2 as u8).0;
            }
        }
    }
}

fn outline_rect_in_buf(buf: &mut [u32], stride: usize, buf_h: usize, x: usize, y: usize, w: usize, h: usize, c: Color, opacity: f32) {
    if w == 0 || h == 0 { return; }
    let x0 = x.min(stride);
    let y0 = y.min(buf_h);
    let x1 = (x + w).min(stride);
    let y1 = (y + h).min(buf_h);
    let a = (opacity * 255.0) as u32;
    let inv = 255 - a;
    let cr = c.r() as u32;
    let cg = c.g() as u32;
    let cb = c.b() as u32;
    let draw = |buf: &mut [u32], px: usize, py: usize| {
        if px >= x0 && px < x1 && py >= y0 && py < y1 {
            let idx = py * stride + px;
            let bg = Color(buf[idx]);
            let r2 = (cr * a + bg.r() as u32 * inv) / 255;
            let g2 = (cg * a + bg.g() as u32 * inv) / 255;
            let b2 = (cb * a + bg.b() as u32 * inv) / 255;
            buf[idx] = Color::rgb(r2 as u8, g2 as u8, b2 as u8).0;
        }
    };
    for px in x0..x1 { draw(buf, px, y0); draw(buf, px, (y1 - 1).min(buf_h - 1)); }
    for py in y0..y1 { draw(buf, x0, py); draw(buf, (x1 - 1).min(stride - 1), py); }
}

fn draw_svg_path_simple(buf: &mut [u32], stride: usize, buf_h: usize, d: &str, color: Color, sw: u32, ox: i32, oy: i32) {
    let mut px = 0.0f32;
    let mut py = 0.0f32;
    let bytes = d.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut last_x = 0.0f32;
    let mut last_y = 0.0f32;
    while i < len {
        while i < len && (bytes[i] == b' ' || bytes[i] == b',') { i += 1; }
        if i >= len { break; }
        let cmd = bytes[i] as char;
        i += 1;
        match cmd {
            'M' | 'm' => {
                let nx = parse_svg_num(d, &mut i);
                let ny = parse_svg_num(d, &mut i);
                if cmd == 'm' { px += nx; py += ny; } else { px = nx; py = ny; }
                last_x = px; last_y = py;
            }
            'L' | 'l' => {
                let nx = parse_svg_num(d, &mut i);
                let ny = parse_svg_num(d, &mut i);
                if cmd == 'l' { px += nx; py += ny; } else { px = nx; py = ny; }
                draw_line_simple(buf, stride, buf_h, last_x + ox as f32, last_y + oy as f32, px + ox as f32, py + oy as f32, color, sw);
                last_x = px; last_y = py;
            }
            'Z' | 'z' => {
                draw_line_simple(buf, stride, buf_h, last_x + ox as f32, last_y + oy as f32, px + ox as f32, py + oy as f32, color, sw);
            }
            _ => {
                while i < len && bytes[i] != b'M' && bytes[i] != b'm' && bytes[i] != b'L' && bytes[i] != b'l' && bytes[i] != b'Z' && bytes[i] != b'z' { i += 1; }
            }
        }
    }
}

fn parse_svg_num(s: &str, i: &mut usize) -> f32 {
    let bytes = s.as_bytes();
    let len = bytes.len();
    while *i < len && (bytes[*i] == b' ' || bytes[*i] == b',') { *i += 1; }
    let start = *i;
    if *i < len && bytes[*i] == b'-' { *i += 1; }
    while *i < len && bytes[*i].is_ascii_digit() { *i += 1; }
    if *i < len && bytes[*i] == b'.' { *i += 1; while *i < len && bytes[*i].is_ascii_digit() { *i += 1; } }
    if *i == start { return 0.0; }
    core::str::from_utf8(&bytes[start..*i]).unwrap_or("0").parse().unwrap_or(0.0)
}

fn draw_line_simple(buf: &mut [u32], stride: usize, buf_h: usize, x0: f32, y0: f32, x1: f32, y1: f32, color: Color, sw: u32) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = (dx.abs().max(dy.abs()) * 2.0) as i32;
    if steps == 0 { return; }
    let inv_steps = 1.0 / steps as f32;
    let hw = sw as f32 / 2.0;
    for s in 0..=steps {
        let t = s as f32 * inv_steps;
        let cx = x0 + dx * t;
        let cy = y0 + dy * t;
        let min_x = (cx - hw).max(0.0) as usize;
        let max_x = (cx + hw).min(stride as f32 - 1.0) as usize + 1;
        let min_y = (cy - hw).max(0.0) as usize;
        let max_y = (cy + hw).min(buf_h as f32 - 1.0) as usize + 1;
        for py in min_y..max_y {
            for px in min_x..max_x {
                buf[py * stride + px] = color.0;
            }
        }
    }
}
