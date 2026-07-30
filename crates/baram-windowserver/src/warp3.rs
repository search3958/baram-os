//! Native Warp 3 UI renderer.
//!
//! Warp 3 applications stay as `config.ini`, `.w3u`, and `.w3s` files on the
//! EFI volume.  This module deliberately does not depend on a browser or JS.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use baram_bsd::{config, vfs};
use baram_core::{Color, LayerSystem};
use baram_font::LayerFontExt;

const MAX_NODES: usize = 2048;
const MAX_ACTION_DEPTH: usize = 24;

#[derive(Clone, Default)]
struct Node {
    tags: Vec<String>,
    classes: Vec<String>,
    props: Vec<(String, String)>,
    children: Vec<usize>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    tab: usize,
}

impl Node {
    fn is(&self, tag: &str) -> bool {
        self.tags.iter().any(|item| item == tag)
    }

    fn prop(&self, name: &str) -> &str {
        self.props
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .unwrap_or("")
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SectionKind {
    Click,
    Function,
}

#[derive(Clone)]
struct ScriptSection {
    kind: SectionKind,
    name: String,
    actions: Vec<(String, String)>,
}

#[derive(Default)]
struct Parser {
    chars: Vec<char>,
    pos: usize,
    nodes: Vec<Node>,
}

impl Parser {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            ..Self::default()
        }
    }

    fn parse(mut self) -> Vec<Node> {
        let _ = self.parse_nodes(false);
        self.nodes
    }

    fn parse_nodes(&mut self, nested: bool) -> Vec<usize> {
        let mut out = Vec::new();
        loop {
            self.skip();
            if self.pos >= self.chars.len() {
                break;
            }
            if self.peek() == Some('}') {
                self.pos += 1;
                break;
            }
            let (tags, classes) = self.selector();
            if tags.is_empty() {
                self.pos += 1;
                continue;
            }
            self.skip();
            if self.peek() != Some('{') {
                self.skip_call_or_line();
                continue;
            }
            self.pos += 1;
            if self.nodes.len() >= MAX_NODES {
                break;
            }
            let idx = self.nodes.len();
            self.nodes.push(Node {
                tags,
                classes,
                ..Node::default()
            });
            loop {
                self.skip();
                if self.peek() == Some('}') {
                    self.pos += 1;
                    break;
                }
                if self.pos >= self.chars.len() {
                    break;
                }
                let saved = self.pos;
                let (names, inner_classes) = self.selector();
                self.skip();
                if names.len() == 1 && inner_classes.is_empty() && self.peek() == Some('(') {
                    let value = self.call_value();
                    self.nodes[idx].props.push((names[0].clone(), value));
                } else {
                    self.pos = saved;
                    let children = self.parse_nodes(true);
                    self.nodes[idx].children.extend(children);
                    break;
                }
            }
            out.push(idx);
            if nested && self.peek() == Some('}') {
                self.pos += 1;
                break;
            }
        }
        out
    }

    fn selector(&mut self) -> (Vec<String>, Vec<String>) {
        let mut tags = Vec::new();
        let mut classes = Vec::new();
        let first = self.ident();
        if first.is_empty() {
            return (tags, classes);
        }
        tags.push(first);
        loop {
            if self.peek() == Some('.') {
                self.pos += 1;
                let class = self.ident();
                if !class.is_empty() {
                    classes.push(class);
                }
            } else if self.peek() == Some(',') {
                self.pos += 1;
                self.skip();
                let tag = self.ident();
                if !tag.is_empty() {
                    tags.push(tag);
                }
            } else {
                break;
            }
        }
        (tags, classes)
    }

    fn ident(&mut self) -> String {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_alphanumeric() || c == '-' || c == '_') {
            self.pos += 1;
        }
        self.chars[start..self.pos].iter().collect()
    }

    fn call_value(&mut self) -> String {
        self.pos += 1;
        self.skip();
        if self.peek() != Some('"') {
            self.skip_call_or_line();
            return String::new();
        }
        self.pos += 1;
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            self.pos += 1;
            match ch {
                '"' => break,
                '\\' => {
                    if let Some(escaped) = self.peek() {
                        self.pos += 1;
                        value.push(match escaped {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            other => other,
                        });
                    }
                }
                other => value.push(other),
            }
        }
        while self.pos < self.chars.len() && self.peek() != Some(')') {
            self.pos += 1;
        }
        if self.peek() == Some(')') {
            self.pos += 1;
        }
        value
    }

    fn skip_call_or_line(&mut self) {
        while let Some(ch) = self.peek() {
            self.pos += 1;
            if ch == '\n' || ch == ')' || ch == '}' {
                break;
            }
        }
    }

    fn skip(&mut self) {
        loop {
            while matches!(self.peek(), Some(ch) if ch.is_whitespace()) {
                self.pos += 1;
            }
            if self.peek() == Some('/') && self.chars.get(self.pos + 1) == Some(&'/') {
                while !matches!(self.peek(), None | Some('\n')) {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
}

pub struct Warp3Engine {
    config_name: String,
    screen: String,
    app_title: String,
    nodes: Vec<Node>,
    roots: Vec<usize>,
    scripts: Vec<ScriptSection>,
    state: Vec<(String, String)>,
    hovered: Option<usize>,
    focused_input: Option<usize>,
    width: i32,
    height: i32,
    scroll: i32,
    pub content_height: i32,
    scroll_request: Option<i32>,
    dirty: bool,
}

impl Warp3Engine {
    pub fn new(config_name: &str) -> Self {
        let config_source = vfs::read_file_str(&alloc::format!("apps/{config_name}"));
        let screen = ini_value(&config_source, "screen").unwrap_or_else(|| "main".to_string());
        let title = ini_value(&config_source, "name").unwrap_or_else(|| "Warp 3".to_string());
        let mut engine = Self {
            config_name: config_name.to_string(),
            screen,
            app_title: title,
            nodes: Vec::new(),
            roots: Vec::new(),
            scripts: Vec::new(),
            state: Vec::new(),
            hovered: None,
            focused_input: None,
            width: 0,
            height: 0,
            scroll: 0,
            content_height: 0,
            scroll_request: None,
            dirty: true,
        };
        engine.load_screen();
        engine
    }

    pub fn title(&self) -> &str {
        &self.app_title
    }

    pub fn update(&mut self, width: i32, height: i32) {
        if !self.dirty && self.width == width && self.height == height {
            return;
        }
        self.width = width.max(1);
        self.height = height.max(1);
        let mut y = crate::window::title_bar_h() as i32 + 20;
        let roots = self.roots.clone();
        for idx in roots {
            if self.nodes[idx].is("config") || self.nodes[idx].is("toolbar") {
                continue;
            }
            let h = self.layout(idx, 24, y, self.width - 48);
            y += h + 10;
        }
        self.content_height = (y + 80).max(self.height);
        let toolbar_y = self.scroll + self.height - 58;
        let toolbars: Vec<usize> = self
            .roots
            .iter()
            .copied()
            .filter(|idx| self.nodes[*idx].is("toolbar"))
            .collect();
        for idx in toolbars {
            self.layout(idx, 16, toolbar_y, self.width - 32);
        }
        self.dirty = false;
    }

    pub fn set_scroll(&mut self, scroll: i32) {
        if self.scroll != scroll {
            self.scroll = scroll.max(0);
            self.dirty = true;
        }
    }

    pub fn hovered_node(&self) -> Option<usize> {
        self.hovered
    }

    pub fn has_focused_input(&self) -> bool {
        self.focused_input.is_some()
    }

    pub fn take_scroll_request(&mut self) -> Option<i32> {
        self.scroll_request.take()
    }

    pub fn set_hover(&mut self, x: i32, y: i32) {
        let next = (0..self.nodes.len()).rev().find(|idx| {
            let node = &self.nodes[*idx];
            interactive(node) && !self.hidden_by_tab(*idx) && contains(node, x, y)
        });
        if self.hovered != next {
            self.hovered = next;
            self.dirty = true;
        }
    }

    pub fn clear_hover(&mut self) {
        if self.hovered.take().is_some() {
            self.dirty = true;
        }
    }

    pub fn click(&mut self, x: i32, y: i32) {
        let hit = (0..self.nodes.len()).rev().find(|idx| {
            interactive(&self.nodes[*idx])
                && !self.hidden_by_tab(*idx)
                && contains(&self.nodes[*idx], x, y)
        });
        let Some(idx) = hit else {
            self.focused_input = None;
            self.dirty = true;
            return;
        };
        if self.nodes[idx].is("input") || self.nodes[idx].is("textarea") {
            self.focused_input = Some(idx);
        } else if self.nodes[idx].is("switch") {
            let current = self.nodes[idx].prop("default") == "true";
            set_prop(
                &mut self.nodes[idx],
                "default",
                if current { "false" } else { "true" },
            );
            self.run_click(idx);
        } else if self.nodes[idx].is("content") {
            if let Some(parent) = self.find_parent(idx) {
                let siblings = self.nodes[parent].children.clone();
                if let Some(tab) = siblings.iter().position(|item| *item == idx) {
                    self.nodes[parent].tab = tab;
                }
            }
        } else {
            self.focused_input = None;
            self.run_click(idx);
        }
        self.dirty = true;
    }

    pub fn handle_key(&mut self, key: u8) {
        let Some(idx) = self.focused_input else {
            return;
        };
        let mut value = self.nodes[idx].prop("text").to_string();
        if key == 0x08 || key == 0x7f {
            value.pop();
        } else if (0x20..0x7f).contains(&key) {
            value.push(key as char);
        }
        set_prop(&mut self.nodes[idx], "text", &value);
        self.dirty = true;
    }

    pub fn draw_to_layer(&self, layer: &mut LayerSystem, ox: i32, oy: i32) {
        for (idx, node) in self.nodes.iter().enumerate() {
            if node.is("config")
                || node.is("space")
                || (node.is("scroll-point") && node.tags.len() == 1)
            {
                continue;
            }
            if self.hidden_by_tab(idx) {
                continue;
            }
            let x = node.x + ox;
            let y = node.y + oy;
            if x + node.w <= 0
                || y + node.h <= crate::window::title_bar_h() as i32
                || x >= layer.width() as i32
                || y >= layer.height() as i32
            {
                continue;
            }
            self.draw_node(layer, idx, x, y);
        }
    }

    fn load_screen(&mut self) {
        let base = config_base(&self.config_name);
        let ui_path = if base.is_empty() {
            alloc::format!("apps/{}.w3u", self.screen)
        } else {
            alloc::format!("apps/{base}/{}.w3u", self.screen)
        };
        let source = vfs::read_file_str(&ui_path);
        self.nodes = Parser::new(&source).parse();
        self.roots = root_indices(&self.nodes);
        self.scripts.clear();
        let mut script_names = Vec::new();
        for node in &self.nodes {
            if node.is("config") {
                for (key, value) in &node.props {
                    if key == "title" && !value.is_empty() {
                        self.app_title = value.clone();
                    } else if key == "script" {
                        script_names.push(value.clone());
                    }
                }
            }
        }
        for name in script_names {
            let path = if base.is_empty() {
                alloc::format!("apps/{name}")
            } else {
                alloc::format!("apps/{base}/{name}")
            };
            self.scripts
                .extend(parse_script(&vfs::read_file_str(&path)));
        }
        self.hovered = None;
        self.focused_input = None;
        self.scroll = 0;
        self.scroll_request = Some(0);
        self.dirty = true;
    }

    fn layout(&mut self, idx: usize, x: i32, y: i32, width: i32) -> i32 {
        self.nodes[idx].x = x;
        self.nodes[idx].y = y;
        self.nodes[idx].w = width.max(1);
        let tag = self.nodes[idx].tags.first().cloned().unwrap_or_default();
        match tag.as_str() {
            "text" | "detail" | "head" | "code" | "scroll-point" => {
                let lines = self.nodes[idx].prop("text").split('\n').count().max(1) as i32;
                self.nodes[idx].h = if tag == "head" {
                    38
                } else {
                    lines * 22 + if tag == "code" { 22 } else { 0 }
                };
            }
            "button" => {
                self.nodes[idx].w = (measure(self.nodes[idx].prop("text")) + 30).clamp(70, width);
                self.nodes[idx].h = 36;
            }
            "input" => self.nodes[idx].h = 38,
            "textarea" => self.nodes[idx].h = 104,
            "switch" => {
                self.nodes[idx].w = 42;
                self.nodes[idx].h = 24;
            }
            "space" => {
                self.nodes[idx].h = 1;
                self.nodes[idx].w = width;
            }
            "flex" | "toolbar" => {
                let children = self.nodes[idx].children.clone();
                let mut cx = x + if tag == "toolbar" { 10 } else { 0 };
                let mut max_h = 36;
                let available = width - if tag == "toolbar" { 20 } else { 0 };
                let fixed: i32 = children
                    .iter()
                    .filter(|child| !self.nodes[**child].is("space"))
                    .map(|child| (measure(self.nodes[*child].prop("text")) + 38).max(70))
                    .sum();
                let gaps = (children.len().saturating_sub(1) as i32) * 8;
                let spaces = children
                    .iter()
                    .filter(|child| self.nodes[**child].is("space"))
                    .count() as i32;
                let space_w = if spaces > 0 {
                    (available - fixed - gaps).max(8) / spaces
                } else {
                    0
                };
                for child in children {
                    let child_w = if self.nodes[child].is("space") {
                        space_w
                    } else {
                        available
                    };
                    let h =
                        self.layout(child, cx, y + if tag == "toolbar" { 9 } else { 0 }, child_w);
                    cx += self.nodes[child].w + 8;
                    max_h = max_h.max(h);
                }
                self.nodes[idx].h = if tag == "toolbar" { 54 } else { max_h };
            }
            "tab" => {
                let children = self.nodes[idx].children.clone();
                let mut control_x = x + 10;
                let mut page_h = 0;
                for (tab, child) in children.iter().copied().enumerate() {
                    let label_w = (measure(self.nodes[child].prop("text")) + 24).max(64);
                    self.nodes[child].x = control_x;
                    self.nodes[child].y = y + 8;
                    self.nodes[child].w = label_w;
                    self.nodes[child].h = 34;
                    control_x += label_w + 4;
                    if tab == self.nodes[idx].tab {
                        let mut cy = y + 56;
                        for grandchild in self.nodes[child].children.clone() {
                            let h = self.layout(grandchild, x + 18, cy, width - 36);
                            cy += h + 8;
                        }
                        page_h = cy - y + 10;
                    }
                }
                self.nodes[idx].h = page_h.max(100);
            }
            "content" => {}
            "card" | "list-box" => {
                let mut cy = y + 18;
                if !self.nodes[idx].prop("text").is_empty() {
                    cy += 30;
                }
                for child in self.nodes[idx].children.clone() {
                    let h = self.layout(child, x + 16, cy, width - 32);
                    cy += h + 8;
                }
                self.nodes[idx].h = cy - y + 10;
            }
            "list" => {
                let mut cy = y + 12;
                for child in self.nodes[idx].children.clone() {
                    let h = self.layout(child, x + 8, cy + 20, width - 16);
                    cy += h;
                }
                self.nodes[idx].h = (cy - y + 10).max(46);
            }
            _ => {
                let mut cy = y;
                for child in self.nodes[idx].children.clone() {
                    let h = self.layout(child, x, cy, width);
                    cy += h + 8;
                }
                self.nodes[idx].h = (cy - y).max(1);
            }
        }
        self.nodes[idx].h
    }

    fn draw_node(&self, layer: &mut LayerSystem, idx: usize, x: i32, y: i32) {
        let node = &self.nodes[idx];
        let text = node.prop("text");
        let bg = config::get_color("ui-theme/color/win_bg", Color::WIN_BG);
        let panel = config::get_color("ui-theme/color/panel", Color::PANEL);
        let border = config::get_color("ui-theme/color/border", Color::BORDER);
        let fg = config::get_color("ui-theme/color/text", Color::TEXT);
        let muted = config::get_color("ui-theme/color/muted", Color::MUTED);
        let accent = config::get_color("ui-theme/color/btn_primary", Color::BTN_PRIMARY);
        let hover = config::get_color("ui-theme/color/btn_tonal_hover", Color::BTN_TONAL_HOVER);
        let xu = x.max(0) as usize;
        let yu = y.max(0) as usize;
        let wu = node.w.max(1) as usize;
        let hu = node.h.max(1) as usize;

        if node.is("toolbar") || node.is("card") || node.is("tab") || node.is("list-box") {
            layer.fill_rounded_rect(xu, yu, wu, hu, 9, panel);
            layer.rounded_rect_outline(xu, yu, wu, hu, 9, border, panel);
        } else if node.is("list") {
            layer.fill_rect(xu, (y + node.h - 1).max(0) as usize, wu, 1, border);
        } else if node.is("button") || node.is("content") {
            let primary = node.prop("type") == "primary";
            let text_only = node.prop("type") == "text";
            let color = if self.hovered == Some(idx) {
                hover
            } else if primary {
                accent
            } else if text_only {
                bg
            } else {
                panel
            };
            layer.fill_rounded_rect(xu, yu, wu, hu, 5, color);
            if !text_only {
                layer.rounded_rect_outline(xu, yu, wu, hu, 5, border, color);
            }
        } else if node.is("input") || node.is("textarea") {
            layer.fill_rounded_rect(xu, yu, wu, hu, 5, bg);
            let focus = if self.focused_input == Some(idx) {
                accent
            } else {
                border
            };
            layer.rounded_rect_outline(xu, yu, wu, hu, 5, focus, bg);
        } else if node.is("code") {
            layer.fill_rounded_rect(xu, yu, wu, hu, 6, Color::rgb(32, 32, 32));
        } else if node.is("switch") {
            let on = node.prop("default") == "true";
            let switch_bg = if on { accent } else { border };
            layer.fill_rounded_rect(xu, yu, wu, hu, 12, switch_bg);
            let knob_x = if on { x + node.w - 20 } else { x + 4 };
            layer.fill_circle(knob_x.max(0) as usize + 8, yu + 12, 8, bg);
        }

        if text.is_empty() {
            return;
        }
        let (tx, ty, color) = if node.is("button") || node.is("content") {
            (x + 12, y + 9, fg)
        } else if node.is("input") || node.is("textarea") {
            (x + 10, y + 10, fg)
        } else if node.is("card") || node.is("list-box") {
            (x + 16, y + 14, fg)
        } else if node.is("list") {
            (x + 8, y + 13, fg)
        } else if node.is("code") {
            (x + 12, y + 11, Color::rgb(246, 246, 246))
        } else {
            (
                x,
                y + if node.is("head") { 8 } else { 2 },
                if node.is("detail") { muted } else { fg },
            )
        };
        for (line, value) in text.split('\n').enumerate() {
            if ty + line as i32 * 22 >= 0 {
                layer.put_str(
                    tx.max(0) as usize,
                    (ty + line as i32 * 22) as usize,
                    value,
                    color,
                );
            }
        }
    }

    fn hidden_by_tab(&self, idx: usize) -> bool {
        for node in &self.nodes {
            if !node.is("tab") {
                continue;
            }
            for (tab, child) in node.children.iter().copied().enumerate() {
                if tab != node.tab && descendant(&self.nodes, child, idx) {
                    return true;
                }
            }
        }
        false
    }

    fn find_parent(&self, child: usize) -> Option<usize> {
        self.nodes
            .iter()
            .position(|node| node.children.contains(&child))
    }

    fn run_click(&mut self, idx: usize) {
        let classes = self.nodes[idx].classes.clone();
        for class in classes {
            let sections = self.scripts.clone();
            for section in sections {
                if section.kind == SectionKind::Click && section.name == class {
                    self.execute(section.actions, 0);
                }
            }
        }
    }

    fn execute(&mut self, actions: Vec<(String, String)>, depth: usize) {
        if depth >= MAX_ACTION_DEPTH {
            return;
        }
        for (left, right) in actions {
            match left.as_str() {
                "screen" => {
                    self.screen = unquote(&right);
                    self.load_screen();
                }
                "scroll" => self.request_scroll(&right),
                "print" | "wait" => {}
                "fun" => {
                    let name = unquote(&right);
                    let sections = self.scripts.clone();
                    for section in sections {
                        if section.kind == SectionKind::Function && section.name == name {
                            self.execute(section.actions, depth + 1);
                        }
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
                    let value = if right.trim() == "+1" || right.trim() == "-1" {
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
        } else {
            let state = self.state(trimmed);
            if state.is_empty() {
                trimmed.to_string()
            } else {
                state
            }
        }
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
        if let Some(node) = self
            .nodes
            .iter_mut()
            .find(|node| node.classes.iter().any(|item| item == class))
        {
            set_prop(node, "text", value);
            self.dirty = true;
        }
    }
}

fn parse_script(source: &str) -> Vec<ScriptSection> {
    let mut sections = Vec::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if let Some((kind, name)) = line[1..line.len() - 1].split_once('=') {
                let kind = match kind.trim() {
                    "onClick" => SectionKind::Click,
                    "fun" => SectionKind::Function,
                    _ => continue,
                };
                sections.push(ScriptSection {
                    kind,
                    name: name.trim().to_string(),
                    actions: Vec::new(),
                });
            }
        } else if let Some((left, right)) = line.split_once('=') {
            if let Some(section) = sections.last_mut() {
                section
                    .actions
                    .push((left.trim().to_string(), right.trim().to_string()));
            }
        }
    }
    sections
}

fn ini_value(source: &str, key: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let (left, right) = line.trim().split_once('=')?;
        (left.trim() == key).then(|| right.trim().to_string())
    })
}

fn config_base(name: &str) -> &str {
    name.rsplit_once('/').map(|(base, _)| base).unwrap_or("")
}

fn root_indices(nodes: &[Node]) -> Vec<usize> {
    (0..nodes.len())
        .filter(|idx| !nodes.iter().any(|node| node.children.contains(idx)))
        .collect()
}

fn set_prop(node: &mut Node, key: &str, value: &str) {
    if let Some((_, current)) = node.props.iter_mut().find(|(name, _)| name == key) {
        *current = value.to_string();
    } else {
        node.props.push((key.to_string(), value.to_string()));
    }
}

fn interactive(node: &Node) -> bool {
    node.is("button")
        || node.is("switch")
        || node.is("input")
        || node.is("textarea")
        || node.is("content")
}

fn contains(node: &Node, x: i32, y: i32) -> bool {
    x >= node.x && x < node.x + node.w && y >= node.y && y < node.y + node.h
}

fn descendant(nodes: &[Node], parent: usize, wanted: usize) -> bool {
    nodes[parent]
        .children
        .iter()
        .any(|child| *child == wanted || descendant(nodes, *child, wanted))
}

fn measure(text: &str) -> i32 {
    text.chars().count() as i32 * 9
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_reference_ui_without_preprocessing() {
        let nodes = Parser::new(include_str!("../../../app/main.w3u")).parse();
        assert!(nodes.len() > 100);
        assert!(nodes.iter().any(|node| node.is("toolbar")));
        assert!(nodes.iter().any(|node| node.is("tab")));
        assert!(nodes.iter().any(|node| node.is("switch")));
        assert!(nodes.iter().any(|node| node.is("textarea")));
        assert!(nodes.iter().any(|node| {
            node.is("button") && node.classes.iter().any(|class| class == "vardemo")
        }));
    }

    #[test]
    fn parses_reference_script_commands_and_functions() {
        let nav = parse_script(include_str!("../../../app/nav.w3s"));
        let variables = parse_script(include_str!("../../../app/var-demo.w3s"));
        assert!(nav.iter().any(|section| {
            section.kind == SectionKind::Click
                && section.name == "vardemo"
                && section
                    .actions
                    .iter()
                    .any(|(left, right)| left == "screen" && right == "var")
        }));
        assert!(variables.iter().any(|section| {
            section.kind == SectionKind::Function && section.name == "updateDisplay"
        }));
        assert!(variables.iter().any(|section| {
            section.name == "set-btn"
                && section
                    .actions
                    .iter()
                    .any(|(left, right)| left == "getText myVar" && right == "set-input")
        }));
    }
}
