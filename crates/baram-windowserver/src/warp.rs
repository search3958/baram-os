use baram_font::LayerFontExt;
extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use baram_bsd::config;
use baram_core::Color;
use baram_core::LayerSystem;
use baram_font::ttf_font;

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
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    visible: bool,
}

impl Node {
    fn get_id(&self) -> &str {
        for a in &self.attrs {
            if a.key == "id" {
                return &a.value;
            }
        }
        ""
    }
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
enum TkType {
    Word,
    Str,
    Punct,
    At,
    Eof,
}

impl Default for TkType {
    fn default() -> Self {
        TkType::Eof
    }
}

#[derive(Clone, Default)]
struct Token {
    r#type: TkType,
    val: String,
}

struct TextElem {
    x: i32,
    y: i32,
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
    pub dirty: bool,
    pub hover_idx: Option<usize>,
    pub last_command: Option<String>,
    pub focused_input: Option<usize>,
    pub content_height: i32,
}

fn measure_text_width(text: &str, _size: f32) -> i32 {
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
            dirty: false,
            hover_idx: None,
            last_command: None,
            focused_input: None,
            content_height: 0,
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

    pub fn update(&mut self, width: i32, height: i32) {
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
        while i < chars.len()
            && (chars[i] == ' ' || chars[i] == '\t' || chars[i].is_ascii_digit())
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
                && (chars[i] == ' '
                    || chars[i] == '\t'
                    || chars[i].is_ascii_digit())
            {
                i += 1;
            }
        }
        res
    }

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
            if self.focused_input == Some(idx) {
                val.push('|');
            }
            if self.texts.len() < MAX_TEXTS {
                self.texts.push(TextElem {
                    x: self.nodes[idx].x + 12,
                    y: self.nodes[idx].y + 16,
                    text: val,
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
                    let mut end = (start + max_chars).min(chars.len());
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
        if self.hover_idx.is_some() {
            self.hover_idx = None;
            self.dirty = true;
        }
    }

    pub fn draw_to_layer(&self, layer: &mut LayerSystem, ox: i32, oy: i32) {
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
                    let border_color = if self.focused_input == Some(idx) {
                        config::get_color("ui-theme/color/btn_primary", Color::BTN_PRIMARY)
                    } else {
                        config::get_color("ui-theme/color/border", Color::BORDER)
                    };
                    layer.rounded_rect_outline(x, y, w, h, 8, border_color);
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
    }

    pub fn click(&mut self, x: i32, y: i32) {
        self.parse_current_screen();
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
                    let ev = self.nodes[i].event_oneclick.clone();
                    if !ev.is_empty() {
                        self.execute_action(&ev);
                    }
                    self.focused_input = None;
                    break;
                }
                if tag == "input" {
                    self.focused_input = Some(i);
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

    pub fn handle_key(&mut self, c: u8) {
        if let Some(idx) = self.focused_input {
            if idx >= self.nodes.len() {
                self.focused_input = None;
                return;
            }
            let out_var = self.parse_out_var(idx);
            if out_var.is_empty() {
                return;
            }
            let mut val = self.get_state(&out_var);
            if c == 0x08 || c == 0x7F {
                val.pop();
            } else if c >= 0x20 && c < 0x7F {
                val.push(c as char);
            } else {
                return;
            }
            self.set_state(&out_var, &val);
            self.dirty = true;
        }
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
        self.set_state(key, val);
    }
}
