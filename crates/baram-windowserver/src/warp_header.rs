use baram_font::LayerFontExt;
extern crate alloc;

use crate::text_cursor;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use baram_bsd::config;
use baram_core::Color;
use baram_core::LayerSystem;
use baram_font::ttf_font;
use baram_warp4::Warp4Engine;

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
    warp4: Option<Warp4Engine>,
    origin: String,
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
    last_clicked_id: Option<String>,
    pub focused_input: Option<usize>,
    pub focused_input_var: alloc::string::String,
    pub content_height: i32,
    caret_visible: bool,
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


