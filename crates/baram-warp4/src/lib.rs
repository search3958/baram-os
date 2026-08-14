//! Native Warp 4 application runtime.
//!
//! Warp 4 layouts look like Android XML, but they are not rendered by making
//! an HTML document.  This crate keeps the XML parser deliberately small and
//! `no_std`, builds a native view tree, lays that tree out, paints controls to
//! `LayerSystem`, and executes the `.w4s` program directly.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use baram_bsd::{app::Warp4Archive, config};
use baram_core::{Color, LayerSystem};
use baram_font::{ttf_font, LayerFontExt};

const TITLE_BAR: i32 = 30;
const MAX_NODES: usize = 2048;
const MAX_ACTIONS: usize = 2048;

#[derive(Clone, Default)]
struct Attr {
    key: String,
    value: String,
}

#[derive(Clone, Default)]
struct Node {
    tag: String,
    attrs: Vec<Attr>,
    children: Vec<usize>,
    parent: Option<usize>,
    text: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    hidden: bool,
}

impl Node {
    fn attr(&self, key: &str) -> &str {
        self.attrs
            .iter()
            .find(|a| a.key == key)
            .map(|a| a.value.as_str())
            .unwrap_or("")
    }
    fn id(&self) -> &str {
        let value = self.attr("id");
        value
            .strip_prefix("@+id/")
            .or_else(|| value.strip_prefix("@id/"))
            .unwrap_or(value)
    }
    fn is(&self, tag: &str) -> bool {
        self.tag.eq_ignore_ascii_case(tag)
    }
    fn visible(&self) -> bool {
        !self.hidden && self.attr("visibility") != "gone" && self.attr("visibility") != "invisible"
    }
}

#[derive(Clone)]
enum Action {
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
}

#[derive(Clone, Default)]
struct Script {
    init: Vec<Action>,
    clicks: Vec<(String, Vec<Action>)>,
    functions: Vec<(String, Vec<Action>)>,
}

#[derive(Clone, Copy, Default)]
struct Edges {
    top: i32,
    right: i32,
    bottom: i32,
    left: i32,
}

pub struct Warp4Engine {
    archive: Warp4Archive,
    origin: String,
    title: String,
    screen: String,
    nodes: Vec<Node>,
    roots: Vec<usize>,
    script: Script,
    state: Vec<(String, String)>,
    focused: Option<usize>,
    hovered: Option<usize>,
    width: i32,
    height: i32,
    scroll: i32,
    pub content_height: i32,
    pub last_command: Option<String>,
    dirty: bool,
}

impl Warp4Engine {
    pub fn new(app_name: &str) -> Self {
        Self::from_archive(Warp4Archive::open(app_name))
    }

    pub fn new_embedded(name: &str, sources: &[(&str, &str)]) -> Self {
        Self::from_archive(Warp4Archive::from_embedded(name, sources))
    }

    fn from_archive(archive: Warp4Archive) -> Self {
        let cfg = archive.read_text("config.ini");
        let screen = ini(&cfg, "screen").unwrap_or_else(|| "main".into());
        let title = ini(&cfg, "name").unwrap_or_else(|| "Warp 4".into());
        let mut this = Self {
            origin: archive.app_name().to_string(),
            archive,
            title,
            screen,
            nodes: Vec::new(),
            roots: Vec::new(),
            script: Script::default(),
            state: Vec::new(),
            focused: None,
            hovered: None,
            width: 0,
            height: 0,
            scroll: 0,
            content_height: 0,
            last_command: None,
            dirty: true,
        };
        this.load_screen();
        this
    }

    pub fn set_origin(&mut self, name: &str) {
        self.origin = name.into();
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn set_scroll(&mut self, scroll: i32) {
        self.scroll = scroll.max(0);
    }
    pub fn is_animating(&self) -> bool {
        false
    }
    pub fn window_damage(&self) -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn has_focused_input(&self) -> bool {
        self.focused.is_some()
    }
    pub fn hovered_node(&self) -> Option<usize> {
        self.hovered
    }
    pub fn clear_hover(&mut self) {
        self.hovered = None;
    }
    pub fn refresh_config(&mut self) {
        self.dirty = true;
    }
    pub fn take_command(&mut self) -> Option<String> {
        self.last_command.take()
    }
    pub fn take_scroll_request(&mut self) -> Option<i32> {
        None
    }
    pub fn tick(&mut self, _now_ns: u64) -> bool {
        false
    }

    pub fn set_screen(&mut self, screen: &str) {
        let screen = screen
            .trim()
            .trim_end_matches(".w4u")
            .trim_end_matches(".w3u");
        if self.screen != screen {
            self.screen = screen.into();
            self.load_screen();
        }
    }

    fn load_screen(&mut self) {
        self.nodes.clear();
        self.roots.clear();
        self.focused = None;
        self.hovered = None;
        let mut layout = self.archive.read_text(&format!("{}.w4u", self.screen));
        if layout.is_empty() {
            layout = self.archive.read_text(&format!("{}.w3u", self.screen));
        }
        let mut parser = XmlParser::new(&layout);
        if let Some(root) = parser.parse_element(None, &mut self.nodes) {
            self.roots.push(root);
        }
        let mut source = self.archive.read_text(&format!("{}.w4s", self.screen));
        if source.is_empty() {
            source = self.archive.read_text(&format!("{}.w3s", self.screen));
        }
        self.script = parse_script(&source);
        let init = self.script.init.clone();
        self.execute(&init);
        self.dirty = true;
    }

    pub fn update(&mut self, width: i32, height: i32) {
        self.width = width.max(1);
        self.height = height.max(1);
        if !self.dirty {
            return;
        }
        self.refresh_visibility();
        let roots = self.roots.clone();
        let mut y = TITLE_BAR + 16;
        // `height` is the window content height, while node coordinates are
        // full-window coordinates so they can share the existing compositor.
        let usable = (self.height - 16).max(1);
        for root in roots {
            let forced = if self.nodes[root].attr("layout_height") == "match_parent" {
                Some(usable)
            } else {
                None
            };
            let h = self.layout(root, 16, y, (self.width - 32).max(1), forced, "");
            y += h;
        }
        self.content_height = (y + 16).max(self.height);
        self.dirty = false;
    }

    pub fn draw_to_layer(&mut self, layer: &mut LayerSystem, ox: i32, _oy: i32) {
        if self.dirty {
            self.update(layer.width() as i32, layer.height() as i32 - TITLE_BAR);
        }
        layer.fill_rect(
            0,
            TITLE_BAR as usize,
            layer.width(),
            layer.height().saturating_sub(TITLE_BAR as usize),
            bg(),
        );
        let roots = self.roots.clone();
        for root in roots {
            // The compositor supplies the window-manager scroll as `oy`.
            // `set_scroll` remains useful for standalone callers, so both
            // forms are combined without applying the offset twice.
            self.paint(layer, root, ox, _oy - self.scroll);
        }
    }

    pub fn set_hover(&mut self, x: i32, y: i32) {
        self.hovered = self.hit(x, y + self.scroll);
    }

    pub fn click(&mut self, x: i32, y: i32) {
        let Some(idx) = self.hit(x, y + self.scroll) else {
            self.focused = None;
            return;
        };
        if self.nodes[idx].is("EditText")
            || self.nodes[idx].is("AutoCompleteTextView")
            || self.nodes[idx].is("MultiAutoCompleteTextView")
        {
            self.focused = Some(idx);
            return;
        }
        if self.nodes[idx].is("Switch")
            || self.nodes[idx].is("CheckBox")
            || self.nodes[idx].is("RadioButton")
            || self.nodes[idx].is("ToggleButton")
        {
            let value = self.nodes[idx].attr("checked") != "true";
            set_attr(
                &mut self.nodes[idx],
                "checked",
                if value { "true" } else { "false" },
            );
        }
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
        self.dirty = true;
    }

    pub fn handle_key(&mut self, key: u8) {
        if key == 8 || key == 127 {
            self.handle_text("", 1);
        } else if key >= 0x20 {
            let byte = [key];
            let text = unsafe { core::str::from_utf8_unchecked(&byte) };
            self.handle_text(text, 0);
        }
    }

    pub fn handle_text(&mut self, text: &str, replace_chars: usize) {
        let Some(idx) = self.focused else {
            return;
        };
        let mut value = self.nodes[idx].attr("text").to_string();
        for _ in 0..replace_chars {
            value.pop();
        }
        value.push_str(text);
        set_attr(&mut self.nodes[idx], "text", &value);
        self.dirty = true;
    }

    fn hit(&self, x: i32, y: i32) -> Option<usize> {
        (0..self.nodes.len()).rev().find(|idx| {
            let n = &self.nodes[*idx];
            n.visible() && interactive(n) && x >= n.x && y >= n.y && x < n.x + n.w && y < n.y + n.h
        })
    }

    fn execute(&mut self, actions: &[Action]) {
        for action in actions.iter().take(MAX_ACTIONS) {
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
            }
        }
    }

    fn command(&mut self, name: &str, target: &str, raw: &str) {
        let value = self.value(raw);
        match name {
            "var.set" | "var.edit" | "const.set" => self.set_state(target, &value),
            "fun" | "for" => {
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
            "WarpUI.text" => {
                if let Some(i) = self.find(target) {
                    set_attr(&mut self.nodes[i], "text", &value);
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
                }
            }
            "WarpUI.visibility" => {
                if let Some(i) = self.find(target) {
                    set_attr(&mut self.nodes[i], "visibility", &value);
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
                if let Some(uri) = value.strip_prefix("app://") {
                    self.last_command = Some(format!("app://{uri}"));
                }
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn find(&self, id: &str) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.id() == id.trim_start_matches("@+id/"))
    }
    fn set_state(&mut self, key: &str, value: &str) {
        if let Some((_, v)) = self.state.iter_mut().find(|(k, _)| k == key) {
            *v = value.into();
        } else if self.state.len() < 256 {
            self.state.push((key.into(), value.into()));
        }
    }
    fn state(&self, key: &str) -> String {
        self.state
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }
    fn value(&self, raw: &str) -> String {
        let mut s = raw
            .trim()
            .trim_matches('(')
            .trim_matches(')')
            .trim()
            .to_string();
        if s == "()" {
            return String::new();
        }
        loop {
            let Some(start) = s.find("var[") else {
                break;
            };
            let Some(end) = s[start + 4..].find(']') else {
                break;
            };
            let end = start + 4 + end;
            let v = self.state(&s[start + 4..end]);
            s.replace_range(start..=end, &v);
        }
        loop {
            let Some(start) = s.find("const[") else {
                break;
            };
            let Some(end) = s[start + 6..].find(']') else {
                break;
            };
            let end = start + 6 + end;
            let v = self.state(&s[start + 6..end]);
            s.replace_range(start..=end, &v);
        }
        if s.starts_with("calc[") && s.ends_with(']') {
            return eval_calc(&s[5..s.len() - 1]).to_string();
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

    fn refresh_visibility(&mut self) {
        for n in &mut self.nodes {
            n.hidden = n.attr("visibility") == "gone";
        }
    }

    fn layout(
        &mut self,
        idx: usize,
        x: i32,
        y: i32,
        available_w: i32,
        forced_h: Option<i32>,
        parent_orientation: &str,
    ) -> i32 {
        let margin = edges(&self.nodes[idx], "layout_margin");
        let pad = edges(&self.nodes[idx], "padding");
        let width_attr = self.nodes[idx].attr("layout_width");
        let weighted_width = parent_orientation == "LinearLayout"
            && parse_i32(self.nodes[idx].attr("layout_weight")) > 0
            && width_attr == "0dp";
        let w = if weighted_width {
            available_w
        } else {
            dimension(width_attr, available_w, self.intrinsic_w(idx, available_w))
        }
        .max(1);
        let tag = self.nodes[idx].tag.clone();
        let own_h = forced_h.or_else(|| {
            let raw = self.nodes[idx].attr("layout_height");
            if raw == "match_parent" {
                Some((self.height - TITLE_BAR).max(1))
            } else if raw != "wrap_content" && !raw.is_empty() {
                Some(parse_dim(raw, 0))
            } else {
                None
            }
        });
        self.nodes[idx].x = x + margin.left;
        self.nodes[idx].y = y + margin.top;
        self.nodes[idx].w = w - margin.left - margin.right;
        if tag == "LinearLayout" || tag == "RadioGroup" {
            let horizontal = self.nodes[idx].attr("orientation") == "horizontal";
            let inner_w = (self.nodes[idx].w - pad.left - pad.right).max(1);
            let inner_x = self.nodes[idx].x + pad.left;
            let inner_y = self.nodes[idx].y + pad.top;
            let children = self.nodes[idx].children.clone();
            let mut fixed = 0;
            let mut weights = 0i32;
            for &child in &children {
                if !self.nodes[child].visible() {
                    continue;
                }
                let e = edges(&self.nodes[child], "layout_margin");
                let weight = parse_i32(self.nodes[child].attr("layout_weight"));
                if weight > 0 {
                    weights += weight;
                } else if horizontal {
                    fixed += self.intrinsic_w(child, inner_w) + e.left + e.right;
                } else {
                    fixed += self.intrinsic_h(child, inner_w) + e.top + e.bottom;
                }
            }
            let inner_h = own_h.unwrap_or_else(|| {
                if horizontal {
                    self.intrinsic_h(idx, available_w)
                } else {
                    fixed + pad.top + pad.bottom
                }
            });
            self.nodes[idx].h = inner_h.max(1);
            let free = (if horizontal {
                inner_w
            } else {
                inner_h - pad.top - pad.bottom
            })
            .saturating_sub(fixed)
            .max(0);
            let mut cursor = if horizontal { inner_x } else { inner_y };
            for child in children {
                if !self.nodes[child].visible() {
                    continue;
                }
                let e = edges(&self.nodes[child], "layout_margin");
                let weight = parse_i32(self.nodes[child].attr("layout_weight"));
                let allocated = if horizontal {
                    if weight > 0 {
                        free * weight / weights.max(1)
                    } else {
                        self.intrinsic_w(child, inner_w)
                    }
                } else if weight > 0 {
                    free * weight / weights.max(1)
                } else {
                    self.intrinsic_h(child, inner_w)
                };
                if horizontal {
                    cursor += e.left;
                    self.layout(child, cursor, inner_y, allocated.max(1), None, &tag);
                    cursor += allocated + e.right;
                } else {
                    cursor += e.top;
                    self.layout(
                        child,
                        inner_x,
                        cursor,
                        inner_w,
                        Some(allocated.max(1)),
                        &tag,
                    );
                    cursor += allocated + e.bottom;
                }
            }
        } else {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            let children = self.nodes[idx].children.clone();
            let mut cy = self.nodes[idx].y + pad.top;
            for child in children {
                if self.nodes[child].visible() {
                    let ch = self.layout(
                        child,
                        self.nodes[idx].x + pad.left,
                        cy,
                        (self.nodes[idx].w - pad.left - pad.right).max(1),
                        None,
                        &tag,
                    );
                    cy += ch;
                }
            }
        }
        let _ = parent_orientation;
        self.nodes[idx].h + margin.top + margin.bottom
    }

    fn intrinsic_w(&self, idx: usize, available: i32) -> i32 {
        let n = &self.nodes[idx];
        if n.is("Space") {
            return 0;
        }
        if !n.attr("text").is_empty() {
            return measure(n.attr("text")) + 20;
        }
        if n.is("Button") || n.is("EditText") {
            return available.min(180).max(64);
        }
        if (n.is("LinearLayout") || n.is("RadioGroup")) && n.attr("orientation") == "horizontal" {
            let pad = edges(n, "padding");
            return (pad.left
                + pad.right
                + n.children
                    .iter()
                    .filter(|c| self.nodes[**c].visible())
                    .map(|c| self.intrinsic_w(*c, available))
                    .sum::<i32>())
            .min(available);
        }
        available
    }
    fn intrinsic_h(&self, idx: usize, available: i32) -> i32 {
        let n = &self.nodes[idx];
        if n.is("Space") {
            return 0;
        }
        if n.is("Button")
            || n.is("EditText")
            || n.is("Switch")
            || n.is("CheckBox")
            || n.is("RadioButton")
            || n.is("ToggleButton")
        {
            return 36;
        }
        if n.is("ProgressBar") {
            return 10;
        }
        if n.is("TextView") {
            return if n.attr("text").is_empty() { 20 } else { 24 };
        }
        let pad = edges(n, "padding");
        let child_h = if (n.is("LinearLayout") || n.is("RadioGroup"))
            && n.attr("orientation") == "horizontal"
        {
            n.children
                .iter()
                .filter(|c| self.nodes[**c].visible())
                .map(|c| self.intrinsic_h(*c, available))
                .max()
                .unwrap_or(0)
        } else {
            n.children
                .iter()
                .filter(|c| self.nodes[**c].visible())
                .map(|c| self.intrinsic_h(*c, available))
                .sum()
        };
        (pad.top + pad.bottom + child_h).max(1)
    }

    fn paint(&self, layer: &mut LayerSystem, idx: usize, ox: i32, oy: i32) {
        let n = &self.nodes[idx];
        if !n.visible() {
            return;
        }
        let x = n.x + ox;
        let y = n.y + oy;
        let w = n.w.max(1) as usize;
        let h = n.h.max(1) as usize;
        let fill = parse_color(n.attr("background")).unwrap_or(if n.is("LinearLayout") {
            Color::rgb(245, 248, 251)
        } else {
            Color::TRANSPARENT
        });
        if fill != Color::TRANSPARENT {
            layer.fill_rounded_rect(x.max(0) as usize, y.max(0) as usize, w, h, 6, fill);
        }
        if n.is("Button") {
            let hover = self.hovered == Some(idx);
            layer.fill_rounded_rect(
                x.max(0) as usize,
                y.max(0) as usize,
                w,
                h,
                6,
                if hover {
                    Color::rgb(220, 232, 248)
                } else {
                    config::get_color("ui-theme/color/btn_bg", Color::BTN_BG)
                },
            );
        } else if n.is("EditText") {
            layer.rounded_rect_outline(
                x.max(0) as usize,
                y.max(0) as usize,
                w,
                h,
                5,
                config::get_color("ui-theme/color/border", Color::BORDER),
                Color::rgb(255, 255, 255),
            );
        } else if n.is("Switch") {
            let on = n.attr("checked") == "true";
            layer.fill_rounded_rect(
                x.max(0) as usize,
                y.max(0) as usize,
                42,
                22,
                11,
                if on {
                    Color::rgb(50, 120, 70)
                } else {
                    Color::rgb(145, 150, 155)
                },
            );
            layer.fill_circle(
                (x + (if on { 31 } else { 11 })).max(0) as usize,
                (y + 11).max(0) as usize,
                8,
                Color::rgb(255, 255, 255),
            );
        } else if n.is("ProgressBar") {
            layer.fill_rounded_rect(
                x.max(0) as usize,
                (y + 2).max(0) as usize,
                w,
                6,
                3,
                Color::rgb(210, 215, 220),
            );
            let max = parse_i32(n.attr("max")).max(1);
            let progress = parse_i32(n.attr("progress")).clamp(0, max);
            layer.fill_rounded_rect(
                x.max(0) as usize,
                (y + 2).max(0) as usize,
                (w as i32 * progress / max) as usize,
                6,
                3,
                Color::rgb(55, 120, 210),
            );
        }
        let text = n.attr("text");
        if !text.is_empty() {
            let color = parse_color(n.attr("textColor")).unwrap_or(if n.is("TextView") {
                Color::rgb(40, 40, 40)
            } else {
                Color::TEXT
            });
            let size = text_size(n);
            let tx = if n.is("Button") {
                x + (n.w - measure_size(text, size)).max(0) / 2
            } else {
                x + 8
            };
            let ty = y + 8;
            put_str_size(layer, tx, ty, text, color, size);
        } else if n.is("EditText") && !n.attr("hint").is_empty() {
            layer.put_str(
                (x + 8).max(0) as usize,
                (y + 8).max(0) as usize,
                n.attr("hint"),
                Color::MUTED,
            );
        }
        for &child in &n.children {
            self.paint(layer, child, ox, oy);
        }
    }
}

struct XmlParser {
    chars: Vec<char>,
    pos: usize,
}
impl XmlParser {
    fn new(s: &str) -> Self {
        Self {
            chars: s.chars().collect(),
            pos: 0,
        }
    }
    fn parse_element(&mut self, parent: Option<usize>, nodes: &mut Vec<Node>) -> Option<usize> {
        while self.pos < self.chars.len() && self.chars[self.pos] != '<' {
            self.pos += 1;
        }
        if self.pos >= self.chars.len() {
            return None;
        }
        self.pos += 1;
        if self.chars.get(self.pos) == Some(&'?') {
            while self.pos < self.chars.len() && self.chars[self.pos] != '>' {
                self.pos += 1;
            }
            self.pos += 1;
            return self.parse_element(parent, nodes);
        }
        if self.chars.get(self.pos) == Some(&'!') {
            while self.pos < self.chars.len() && self.chars[self.pos] != '>' {
                self.pos += 1;
            }
            self.pos += 1;
            return self.parse_element(parent, nodes);
        }
        let tag = self.ident();
        if tag.is_empty() {
            return None;
        }
        let attrs = self.attrs();
        let self_close = self.chars.get(self.pos.saturating_sub(2)) == Some(&'/');
        if nodes.len() >= MAX_NODES {
            return None;
        }
        let idx = nodes.len();
        nodes.push(Node {
            tag,
            attrs,
            parent,
            ..Node::default()
        });
        if self_close {
            return Some(idx);
        }
        loop {
            self.skip();
            if self.pos >= self.chars.len() {
                break;
            }
            if self.chars[self.pos] == '<' && self.chars.get(self.pos + 1) == Some(&'/') {
                while self.pos < self.chars.len() && self.chars[self.pos] != '>' {
                    self.pos += 1;
                }
                self.pos += 1;
                break;
            }
            if self.chars[self.pos] == '<' {
                if let Some(c) = self.parse_element(Some(idx), nodes) {
                    nodes[idx].children.push(c);
                }
            } else {
                let start = self.pos;
                while self.pos < self.chars.len() && self.chars[self.pos] != '<' {
                    self.pos += 1;
                }
                let s: String = self.chars[start..self.pos].iter().collect();
                let s = s.trim();
                if !s.is_empty() {
                    nodes[idx].text.push_str(s);
                    set_attr(&mut nodes[idx], "text", s);
                }
            }
        }
        Some(idx)
    }
    fn ident(&mut self) -> String {
        let s = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_alphanumeric()
                || matches!(self.chars[self.pos], ':' | '_' | '-'))
        {
            self.pos += 1;
        }
        self.chars[s..self.pos].iter().collect()
    }
    fn attrs(&mut self) -> Vec<Attr> {
        let mut out = Vec::new();
        loop {
            self.skip();
            if self.pos >= self.chars.len() || self.chars[self.pos] == '>' {
                self.pos += 1;
                break;
            }
            if self.chars[self.pos] == '/' {
                self.pos += 1;
                self.skip();
                if self.chars.get(self.pos) == Some(&'>') {
                    self.pos += 1;
                }
                break;
            }
            let raw = self.ident();
            self.skip();
            if self.chars.get(self.pos) != Some(&'=') {
                continue;
            }
            self.pos += 1;
            self.skip();
            let q = self.chars.get(self.pos).copied().unwrap_or('"');
            if q == '"' || q == '\'' {
                self.pos += 1;
                let s = self.pos;
                while self.pos < self.chars.len() && self.chars[self.pos] != q {
                    self.pos += 1;
                }
                let value: String = self.chars[s..self.pos].iter().collect();
                self.pos += 1;
                out.push(Attr {
                    key: raw.rsplit(':').next().unwrap_or(&raw).into(),
                    value: decode(&value),
                });
            }
        }
        out
    }
    fn skip(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }
}

fn parse_script(source: &str) -> Script {
    let mut script = Script::default();
    let mut stack: Vec<(String, Vec<Action>)> = Vec::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if line.starts_with("WarpUI.OnClick") && line.ends_with('{') {
            stack.push((
                line["WarpUI.OnClick".len()..line.len() - 1].trim().into(),
                Vec::new(),
            ));
            continue;
        }
        if line.starts_with("fun") && line.ends_with('{') {
            let header = line[3..line.len() - 1].trim();
            let name = header.trim_start_matches('(').trim_end_matches(')').trim();
            if !name.is_empty() {
                stack.push((format!("fun:{name}"), Vec::new()));
            }
            continue;
        }
        if line.starts_with("if ") && line.ends_with('{') {
            if let Some((l, o, r)) = condition(&line[3..line.len() - 1]) {
                stack.push((format!("if\u{1f}{l}\u{1f}{o}\u{1f}{r}"), Vec::new()));
            }
            continue;
        }
        if line == "}" {
            if let Some((name, body)) = stack.pop() {
                if let Some(rest) = name.strip_prefix("if\u{1f}") {
                    let mut p = rest.split('\u{1f}');
                    let a = Action::If {
                        left: p.next().unwrap_or("").into(),
                        op: p.next().unwrap_or("").into(),
                        right: p.next().unwrap_or("").into(),
                        body,
                    };
                    if let Some((_, parent)) = stack.last_mut() {
                        parent.push(a);
                    } else {
                        script.init.push(a);
                    }
                } else if let Some(function) = name.strip_prefix("fun:") {
                    script.functions.push((function.into(), body));
                } else if let Some((_, parent)) = stack.last_mut() {
                    parent.push(Action::Command {
                        name: "WarpUI.OnClick".into(),
                        target: name,
                        value: String::new(),
                    });
                    parent.extend(body);
                } else {
                    script.clicks.push((name, body));
                }
            }
            continue;
        }
        if let Some(action) = parse_command(line) {
            if let Some((_, body)) = stack.last_mut() {
                body.push(action);
            } else {
                script.init.push(action);
            }
        }
    }
    script
}
fn parse_command(line: &str) -> Option<Action> {
    let (name, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(Action::Command {
            name: name.into(),
            target: String::new(),
            value: String::new(),
        });
    }
    if name == "fun" && rest.starts_with('(') {
        let (value, _) = balanced(rest, 0)?;
        return Some(Action::Call(value.trim().into()));
    }
    let (target, value) = if let Some(open) = rest.find('(') {
        let target = rest[..open].trim();
        let (v, _) = balanced(rest, open)?;
        (target.into(), v)
    } else {
        (String::new(), rest.into())
    };
    Some(Action::Command {
        name: name.into(),
        target,
        value,
    })
}
fn balanced(s: &str, open: usize) -> Option<(String, usize)> {
    let mut depth = 0;
    for (i, c) in s.char_indices().skip(open) {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                return Some((s[open + 1..i].into(), i + 1));
            }
        }
    }
    None
}
fn condition(s: &str) -> Option<(String, String, String)> {
    for op in ["!=", "=", "<", ">"] {
        if let Some(i) = s.find(op) {
            return Some((
                s[..i].trim().into(),
                op.into(),
                s[i + op.len()..].trim().into(),
            ));
        }
    }
    None
}
fn set_attr(n: &mut Node, key: &str, value: &str) {
    if let Some(a) = n.attrs.iter_mut().find(|a| a.key == key) {
        a.value = value.into();
    } else {
        n.attrs.push(Attr {
            key: key.into(),
            value: value.into(),
        });
    }
}
fn decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}
fn ini(s: &str, key: &str) -> Option<String> {
    s.lines().find_map(|l| {
        let (a, b) = l.split_once('=')?;
        (a.trim() == key).then(|| b.trim().into())
    })
}
fn parse_i32(s: &str) -> i32 {
    s.trim().parse().unwrap_or(0)
}
fn parse_dim(s: &str, default: i32) -> i32 {
    s.trim()
        .trim_end_matches("dp")
        .trim_end_matches("sp")
        .trim_end_matches("px")
        .parse()
        .unwrap_or(default)
}
fn dimension(raw: &str, available: i32, intrinsic: i32) -> i32 {
    match raw {
        "match_parent" | "fill_parent" => available,
        "wrap_content" | "" => intrinsic,
        "0dp" => 0,
        _ => parse_dim(raw, intrinsic),
    }
}
fn edges(n: &Node, base: &str) -> Edges {
    let style = n.attr("style");
    let (style_top, style_bottom) = if base == "layout_margin" && style.contains("SectionTitle") {
        (22, 8)
    } else if base == "layout_margin" && style.contains("SectionDescription") {
        (0, 14)
    } else if base == "layout_margin" && style.contains("ComponentLabel") {
        (14, 6)
    } else if base == "layout_margin" && style.contains("ComponentBox") {
        (0, 3)
    } else {
        (0, 0)
    };
    let all = parse_dim(n.attr(base), 0);
    let h = parse_dim(
        if n.attr(&format!("{base}Horizontal")).is_empty() {
            n.attr(base)
        } else {
            n.attr(&format!("{base}Horizontal"))
        },
        all,
    );
    let v = parse_dim(
        if n.attr(&format!("{base}Vertical")).is_empty() {
            n.attr(base)
        } else {
            n.attr(&format!("{base}Vertical"))
        },
        all,
    );
    Edges {
        top: parse_dim(
            n.attr(&format!("{base}Top")),
            if v == 0 { style_top } else { v },
        ),
        right: parse_dim(n.attr(&format!("{base}Right")), h),
        bottom: parse_dim(
            n.attr(&format!("{base}Bottom")),
            if v == 0 { style_bottom } else { v },
        ),
        left: parse_dim(n.attr(&format!("{base}Left")), h),
    }
}
fn interactive(n: &Node) -> bool {
    n.is("Button")
        || n.is("EditText")
        || n.is("AutoCompleteTextView")
        || n.is("MultiAutoCompleteTextView")
        || n.is("Switch")
        || n.is("CheckBox")
        || n.is("RadioButton")
        || n.is("ToggleButton")
}
fn measure(s: &str) -> i32 {
    if ttf_font::is_available() {
        s.chars().map(ttf_font::advance).sum()
    } else {
        s.len() as i32 * 8
    }
}
fn measure_size(s: &str, size: f32) -> i32 {
    if !ttf_font::is_available() {
        return measure(s);
    }
    let mut total = 0;
    for ch in s.chars() {
        let mut advance = 0;
        ttf_font::with_glyph_at_size(ch, size, |_data, _w, _h, glyph_advance, _y_off| {
            advance = glyph_advance;
        });
        total += advance.max(8);
    }
    total
}
fn text_size(n: &Node) -> f32 {
    if !n.attr("textSize").is_empty() {
        return parse_dim(n.attr("textSize"), 16) as f32;
    }
    let style = n.attr("style");
    if style.contains("SectionTitle") {
        24.0
    } else if style.contains("SectionDescription") {
        13.0
    } else if style.contains("ComponentLabel") {
        15.0
    } else {
        16.0
    }
}
fn put_str_size(layer: &mut LayerSystem, mut x: i32, y: i32, text: &str, color: Color, size: f32) {
    if !ttf_font::is_available() {
        if x >= 0 && y >= 0 {
            layer.put_str(x as usize, y as usize, text, color);
        }
        return;
    }
    let baseline = y + ttf_font::ascent_at_size(size);
    let layer_w = layer.width();
    let layer_h = layer.height();
    let (clip_x0, clip_y0, clip_x1, clip_y1) = layer.clip_bounds();
    for ch in text.chars() {
        let mut advance = 0;
        ttf_font::with_glyph_at_size(ch, size, |data, w, h, glyph_advance, y_off| {
            advance = glyph_advance;
            for row in 0..h {
                let py = baseline + y_off + row;
                if py < clip_y0 as i32 || py >= clip_y1.min(layer_h) as i32 {
                    continue;
                }
                for col in 0..w {
                    let px = x + col;
                    if px < clip_x0 as i32 || px >= clip_x1.min(layer_w) as i32 {
                        continue;
                    }
                    let alpha = data[row as usize * w as usize + col as usize] as f32 / 255.0;
                    if alpha > 0.0 {
                        let index = py as usize * layer_w + px as usize;
                        let background = layer.buf_ref()[index];
                        layer.buf_mut()[index] =
                            LayerSystem::blend_alpha(background, color.0, alpha);
                    }
                }
            }
        });
        x += advance.max(8);
    }
}
fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim().strip_prefix('#')?;
    let v = u32::from_str_radix(s, 16).ok()?;
    if s.len() == 6 {
        Some(Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    } else {
        None
    }
}
fn eval_calc(s: &str) -> i64 {
    let c = s.chars().filter(|c| !c.is_whitespace()).collect::<Vec<_>>();
    let mut total = 0i64;
    let mut current = 0i64;
    let mut sign = 1i64;
    for ch in c {
        if ch.is_ascii_digit() {
            current = current * 10 + (ch as i64 - '0' as i64);
        } else {
            total += sign * current;
            current = 0;
            sign = if ch == '-' { -1 } else { 1 };
        }
    }
    total + sign * current
}
fn bg() -> Color {
    config::get_color("ui-theme/color/bg", Color::BG)
}
