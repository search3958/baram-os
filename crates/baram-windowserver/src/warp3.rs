//! Native Warp 3 UI renderer.
//!
//! Warp 3 applications stay as `config.ini`, `.w3u`, and `.w3s` files on the
//! EFI volume.  This module deliberately does not depend on a browser or JS.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use baram_bsd::app::Warp3Archive;
use baram_core::{Color, LayerSystem};
use baram_font::ttf_font;
use baram_font::LayerFontExt;

const HOVER_STEPS: u8 = 7;
const SWITCH_STEPS: u8 = 10;
const HOVER_DURATION_NS: u64 = 100_000_000;
const SWITCH_DURATION_NS: u64 = 150_000_000;

struct ShadowMask {
    w: usize,
    h: usize,
    radius: usize,
    pad: i32,
    layer: LayerSystem,
}

impl ShadowMask {
    fn new(w: usize, h: usize, radius: usize) -> Self {
        let pad = 12i32;
        let shadow_offset_y = 2i32;
        let sw = w + pad as usize * 2;
        let sh = h + pad as usize * 2;
        let mut layer = LayerSystem::new_transparent(sw, sh);
        // Rasterize a cheap integer rounded-rectangle mask, then approximate a
        // Gaussian with three box blurs.  This replaces per-pixel sqrt work at
        // application startup with linear, cache-friendly scanline passes.
        let mut alpha = alloc::vec![0u8; sw * sh];
        let left = pad as usize;
        let top = (pad + shadow_offset_y) as usize;
        let right = left + w;
        let bottom = top + h;
        let r = radius.min(w / 2).min(h / 2) as i32;
        for py in top..bottom {
            for px in left..right {
                let dx = if px < left + r as usize { left as i32 + r - px as i32 }
                    else if px >= right - r as usize { px as i32 - (right as i32 - r - 1) } else { 0 };
                let dy = if py < top + r as usize { top as i32 + r - py as i32 }
                    else if py >= bottom - r as usize { py as i32 - (bottom as i32 - r - 1) } else { 0 };
                if dx == 0 || dy == 0 || dx * dx + dy * dy <= r * r {
                    alpha[py * sw + px] = 34;
                }
            }
        }
        for _ in 0..3 {
            box_blur_alpha(&mut alpha, sw, sh, 4);
        }
        for (dst, value) in layer.buf_mut().iter_mut().zip(alpha) {
            *dst = value as u32;
        }
        Self {
            w,
            h,
            radius,
            pad,
            layer,
        }
    }

    fn composite(&self, target: &mut LayerSystem, x: i32, y: i32) {
        let shadow_x = x - self.pad;
        let shadow_y = y - self.pad;
        let src_x = (-shadow_x).max(0) as usize;
        let src_y = (-shadow_y).max(0) as usize;
        let dst_x = shadow_x.max(0) as usize;
        let dst_y = shadow_y.max(0) as usize;
        let width = self.layer.width().saturating_sub(src_x);
        let height = self.layer.height().saturating_sub(src_y);
        if width > 0 && height > 0 {
            target.composit_shadow_alpha(&self.layer, dst_x, dst_y, src_x, src_y, width, height);
        }
    }

    /// Store a black shadow as straight alpha in a transparent overlay.  It
    /// must not be blended against transparent black first, otherwise it later
    /// becomes an opaque dark rectangle when the toolbar layer is composited.
    fn composite_transparent(&self, target: &mut LayerSystem, x: i32, y: i32) {
        for sy in 0..self.layer.height() {
            let dy = y - self.pad + sy as i32;
            if dy < 0 || dy >= target.height() as i32 { continue; }
            for sx in 0..self.layer.width() {
                let dx = x - self.pad + sx as i32;
                if dx < 0 || dx >= target.width() as i32 { continue; }
                let alpha = self.layer.buf_ref()[sy * self.layer.width() + sx] & 0xff;
                if alpha == 0 { continue; }
                let dst = dy as usize * target.width() + dx as usize;
                let old = (target.buf_ref()[dst] >> 24) & 0xff;
                target.buf_mut()[dst] = old.max(alpha) << 24;
            }
        }
    }
}

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
    overlay: bool,
    hidden: bool,
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
    archive: Warp3Archive,
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
    /// First document y that must be repainted after a reflow.  Keeping the
    /// already-valid prefix is important for long Warp 3 pages.
    repaint_from: i32,
    repaint_to: i32,
    document_layer: Option<LayerSystem>,
    toolbar_layer: Option<LayerSystem>,
    toolbar_dirty: bool,
    document_paint: Vec<usize>,
    toolbar_paint: Vec<usize>,
    window_damage: Option<(i32, i32, i32, i32)>,
    full_window_redraw: bool,
    hover_transition: Option<(Option<usize>, Option<usize>, u8)>,
    switch_transition: Option<(usize, bool, u8)>,
    hover_started_ns: Option<u64>,
    switch_started_ns: Option<u64>,
    shadows: Vec<ShadowMask>,
}

impl Warp3Engine {
    pub fn new(app_name: &str) -> Self {
        let archive = Warp3Archive::open(app_name);
        let config_source = archive.read_text("config.ini");
        let screen = ini_value(&config_source, "screen").unwrap_or_else(|| "main".to_string());
        let title = ini_value(&config_source, "name").unwrap_or_else(|| "Warp 3".to_string());
        let mut engine = Self {
            archive,
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
            repaint_from: 0,
            repaint_to: i32::MAX,
            document_layer: None,
            toolbar_layer: None,
            toolbar_dirty: true,
            document_paint: Vec::new(),
            toolbar_paint: Vec::new(),
            window_damage: None,
            full_window_redraw: false,
            hover_transition: None,
            switch_transition: None,
            hover_started_ns: None,
            switch_started_ns: None,
            shadows: Vec::new(),
        };
        engine.load_screen();
        engine
    }

    pub fn title(&self) -> &str {
        &self.app_title
    }

    pub fn update(&mut self, width: i32, height: i32) {
        if !self.dirty && self.width == width && self.height == height {
            // Hover changes do not affect geometry.  Repaint only the cached
            // damage rectangle and skip the document layout walk.
            if self.repaint_from < self.content_height || self.toolbar_dirty {
                self.paint_cached_layers();
            }
            return;
        }
        if self.width != width || self.height != height {
            self.toolbar_dirty = true;
        }
        self.width = width.max(1);
        self.height = height.max(1);
        let title_bar = crate::window::title_bar_h() as i32;
        let document_width = self.width.min(960);
        let document_x = (self.width - document_width) / 2;
        let content_x = document_x + 28;
        let content_width = document_width - 56;
        let mut y = title_bar + 32;
        let roots = self.roots.clone();
        for idx in roots {
            if self.nodes[idx].is("config") || self.nodes[idx].is("toolbar") {
                continue;
            }
            let h = self.layout(idx, content_x, y, content_width);
            y += h + 12;
        }
        self.content_height = (y + 96).max(title_bar + self.height);
        let toolbar_width = (self.width - 32).min(900);
        let toolbar_x = (self.width - toolbar_width) / 2;
        // Toolbars live in viewport coordinates.  They are deliberately not
        // part of the document, so scrolling never moves or reflows them.
        let toolbar_y = title_bar + self.height - 18 - 54;
        let toolbars: Vec<usize> = self
            .roots
            .iter()
            .copied()
            .filter(|idx| self.nodes[*idx].is("toolbar"))
            .collect();
        for idx in toolbars {
            self.layout(idx, toolbar_x, toolbar_y, toolbar_width);
        }
        self.refresh_visibility();
        self.rebuild_paint_lists();
        self.prepare_shadows();
        self.paint_cached_layers();
        self.dirty = false;
    }

    pub fn set_scroll(&mut self, scroll: i32) {
        if self.scroll != scroll {
            self.scroll = scroll.max(0);
        }
    }

    pub fn hovered_node(&self) -> Option<usize> {
        self.hovered
    }

    pub fn has_focused_input(&self) -> bool {
        self.focused_input.is_some()
    }

    pub fn window_damage(&self) -> Option<(i32, i32, i32, i32)> {
        (!self.full_window_redraw).then_some(self.window_damage).flatten()
    }

    pub fn take_scroll_request(&mut self) -> Option<i32> {
        self.scroll_request.take()
    }

    pub fn set_hover(&mut self, x: i32, y: i32) {
        let next = self.hit_test(x, y);
        if self.hovered != next {
            let old = self.hovered;
            self.hover_transition = Some((old, next, 0));
            self.hover_started_ns = None;
            self.invalidate_nodes(old, next);
            self.hovered = next;
        }
    }

    pub fn clear_hover(&mut self) {
        if let Some(old) = self.hovered.take() {
            self.hover_transition = Some((Some(old), None, 0));
            self.hover_started_ns = None;
            self.invalidate_nodes(Some(old), None);
        }
    }

    pub fn click(&mut self, x: i32, y: i32) {
        let hit = self.hit_test(x, y);
        let Some(idx) = hit else {
            self.focused_input = None;
            self.invalidate_from(0);
            return;
        };
        if self.nodes[idx].is("input") || self.nodes[idx].is("textarea") {
            self.focused_input = Some(idx);
        } else if self.nodes[idx].is("switch") {
            let current = self.nodes[idx].prop("default") == "true";
            let next = !current;
            set_prop(
                &mut self.nodes[idx],
                "default",
                if next { "true" } else { "false" },
            );
            self.switch_transition = Some((idx, next, 0));
            self.switch_started_ns = None;
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
        self.invalidate_from(self.nodes[idx].y);
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
        self.invalidate_from(self.nodes[idx].y);
    }

    /// Advance 90 ms control transitions in 1 ms timer ticks.  No layout is
    /// involved; only the old/new control damage rectangle is requested.
    pub fn tick(&mut self, now_ns: u64) -> bool {
        if self.hover_transition.is_none() && self.switch_transition.is_none() {
            return false;
        }
        let mut changed = false;
        if let Some((old, new, frame)) = self.hover_transition {
            let started = self.hover_started_ns.get_or_insert(now_ns);
            let next = (((now_ns.saturating_sub(*started) * HOVER_STEPS as u64) / HOVER_DURATION_NS)
                .min(HOVER_STEPS as u64)) as u8;
            if next != frame {
                self.hover_transition = Some((old, new, next));
                self.invalidate_nodes(old, new);
                changed = true;
            }
            if next >= HOVER_STEPS {
                self.hover_transition = None;
                self.hover_started_ns = None;
            }
        }
        if let Some((idx, on, frame)) = self.switch_transition {
            let started = self.switch_started_ns.get_or_insert(now_ns);
            let next = (((now_ns.saturating_sub(*started) * SWITCH_STEPS as u64) / SWITCH_DURATION_NS)
                .min(SWITCH_STEPS as u64)) as u8;
            if next != frame {
                self.switch_transition = Some((idx, on, next));
                self.invalidate_nodes(Some(idx), None);
                changed = true;
            }
            if next >= SWITCH_STEPS {
                self.switch_transition = None;
                self.switch_started_ns = None;
            }
        }
        changed
    }

    pub fn draw_to_layer(&mut self, layer: &mut LayerSystem, ox: i32, _oy: i32) {
        // The window may be drawn before its next regular update tick.  Apply
        // hover-only damage here as well, without paying for a layout pass.
        if !self.dirty && (self.repaint_from < self.content_height || self.toolbar_dirty) {
            self.paint_cached_layers();
        }
        let title_bar = crate::window::title_bar_h();
        layer.fill_rect(
            0,
            title_bar,
            layer.width(),
            layer.height().saturating_sub(title_bar),
            html_bg(),
        );
        if let Some(document) = &self.document_layer {
            let source_y = (self.scroll + title_bar as i32).max(0) as usize;
            let target_y = title_bar;
            let visible_h = layer.height().saturating_sub(target_y);
            if source_y < document.height() && visible_h > 0 {
                layer.composit_rect_opaque(document, ox.max(0) as usize, target_y, 0, source_y,
                    document.width(), visible_h.min(document.height() - source_y));
            }
        }
        // This is a separate transparent layer, composited after the scrolling
        // document.  It keeps both its pixels and its hit targets fixed.
        if let Some(toolbar) = &self.toolbar_layer {
            layer.composit_rect_alpha(toolbar, ox.max(0) as usize, 0, 0, 0,
                toolbar.width(), toolbar.height());
        }
        // Paint fixed controls only for a full frame or a toolbar-local hover.
        // Repainting TTF text outside the local damage would blend its edge
        // pixels again even though the document did not change.
        let paint_toolbar = self.window_damage.map_or(true, |(_, y0, _, y1)| {
            let toolbar_top = crate::window::title_bar_h() as i32 + self.height - 18 - 54 - 14;
            y1 >= toolbar_top && y0 < self.height + crate::window::title_bar_h() as i32
        });
        // Paint the fixed controls over the already-composited document. This
        // gives text and rounded corners their final-background antialiasing;
        // only the shadow itself needs transparent alpha storage.
        if paint_toolbar {
            for idx in self.toolbar_paint.clone() {
                let node = &self.nodes[idx];
                self.draw_node(layer, idx, node.x + ox, node.y);
            }
        }
        self.window_damage = None;
        // This draw is reached only after WindowManager selected the full
        // content path while `full_window_redraw` was set.
        self.full_window_redraw = false;
    }

    fn load_screen(&mut self) {
        let ui_path = alloc::format!("{}.w3u", self.screen);
        let source = self.archive.read_text(&ui_path);
        self.nodes = Parser::new(&source).parse();
        self.roots = root_indices(&self.nodes);
        let toolbar_roots: Vec<usize> = self
            .roots
            .iter()
            .copied()
            .filter(|idx| self.nodes[*idx].is("toolbar"))
            .collect();
        for root in toolbar_roots {
            mark_overlay_tree(&mut self.nodes, root);
        }
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
            self.scripts
                .extend(parse_script(&self.archive.read_text(&name)));
        }
        self.hovered = None;
        self.focused_input = None;
        self.scroll = 0;
        self.scroll_request = Some(0);
        self.dirty = true;
        self.repaint_from = 0;
        self.repaint_to = i32::MAX;
        self.document_layer = None;
        self.toolbar_layer = None;
        self.toolbar_dirty = true;
        self.document_paint.clear();
        self.toolbar_paint.clear();
        // Loading a screen invalidates every cached document pixel.  The
        // caller's normal `set_content_dirty` path therefore performs a full
        // window redraw, never a hover-sized patch.
        self.window_damage = None;
        self.full_window_redraw = true;
    }

    fn layout(&mut self, idx: usize, x: i32, y: i32, width: i32) -> i32 {
        self.nodes[idx].x = x;
        self.nodes[idx].y = y;
        self.nodes[idx].w = width.max(1);
        let tag = self.nodes[idx].tags.first().cloned().unwrap_or_default();
        match tag.as_str() {
            "text" | "detail" | "head" | "code" | "scroll-point" => {
                let lines = if tag == "code" {
                    self.nodes[idx].prop("text").split('\n').count().max(1)
                } else {
                    wrap_lines(self.nodes[idx].prop("text"), width).len().max(1)
                } as i32;
                self.nodes[idx].h = if tag == "head" {
                    38
                } else {
                    lines * 22 + if tag == "code" { 22 } else { 0 }
                };
            }
            "button" => {
                self.nodes[idx].w = (measure(self.nodes[idx].prop("text")) + 28).clamp(44, width);
                self.nodes[idx].h = 34;
            }
            "input" => {
                self.nodes[idx].w = width.min(420);
                self.nodes[idx].h = 34;
            }
            "textarea" => {
                self.nodes[idx].w = width.min(420);
                self.nodes[idx].h = 100;
            }
            "switch" => {
                self.nodes[idx].w = 40;
                self.nodes[idx].h = 20;
            }
            "space" => {
                self.nodes[idx].h = 1;
                self.nodes[idx].w = width;
            }
            "flex" | "toolbar" => {
                let children = self.nodes[idx].children.clone();
                let mut cx = x + if tag == "toolbar" { 10 } else { 0 };
                let mut max_h = 34;
                let available = width - if tag == "toolbar" { 20 } else { 0 };
                let fixed: i32 = children
                    .iter()
                    .filter(|child| !self.nodes[**child].is("space"))
                    .map(|child| (measure(self.nodes[*child].prop("text")) + 36).max(44))
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
                let control = self.nodes[idx].prop("control").to_string();
                let vertical = control == "left" || control == "right";
                let controls_w = if vertical { 104 } else { width };
                let mut control_x = if control == "right" {
                    x + width - controls_w + 8
                } else {
                    x + 8
                };
                let mut control_y = y + 8;
                let mut page_h = 0;
                for (tab, child) in children.iter().copied().enumerate() {
                    let label_w = (measure(self.nodes[child].prop("text")) + 24).max(64);
                    self.nodes[child].x = control_x;
                    self.nodes[child].y = control_y;
                    self.nodes[child].w = if vertical { controls_w - 16 } else { label_w };
                    self.nodes[child].h = 34;
                    if vertical {
                        control_y += 37;
                    } else {
                        control_x += label_w + 4;
                    }
                    if tab == self.nodes[idx].tab {
                        let page_x = if control == "left" {
                            x + controls_w + 18
                        } else {
                            x + 18
                        };
                        let page_width = if vertical {
                            width - controls_w - 36
                        } else {
                            width - 36
                        };
                        let mut cy = if vertical { y + 18 } else { y + 56 };
                        for grandchild in self.nodes[child].children.clone() {
                            let h = self.layout(grandchild, page_x, cy, page_width);
                            cy += h + 8;
                        }
                        page_h = cy - y + 10;
                    }
                }
                self.nodes[idx].h = page_h.max(if vertical { control_y - y + 8 } else { 100 });
            }
            "content" => {}
            "card" => {
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
            "list-box" => {
                let title_height = if self.nodes[idx].prop("text").is_empty() {
                    0
                } else {
                    36
                };
                let mut cy = y + title_height;
                for child in self.nodes[idx].children.clone() {
                    let h = self.layout(child, x, cy, width);
                    cy += h;
                }
                self.nodes[idx].h = cy - y;
            }
            "list" => {
                let detail_width = self.nodes[idx]
                    .children
                    .iter()
                    .find(|child| self.nodes[**child].is("detail"))
                    .map(|child| (measure(self.nodes[*child].prop("text")) + 8).min(width / 2))
                    .unwrap_or(0);
                let mut max_h = 22;
                for child in self.nodes[idx].children.clone() {
                    if self.nodes[child].is("detail") {
                        let h = self.layout(child, x + width - detail_width - 8, y + 12, detail_width);
                        max_h = max_h.max(h);
                    } else {
                        let text_width = (width - detail_width - if detail_width > 0 { 24 } else { 16 }).max(24);
                        let h = self.layout(child, x + 8, y + 12, text_width);
                        max_h = max_h.max(h);
                    }
                }
                self.nodes[idx].h = (max_h + 24).max(46);
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

    fn invalidate_from(&mut self, y: i32) {
        self.repaint_from = self.repaint_from.min(y.max(0));
        self.repaint_to = i32::MAX;
        self.dirty = true;
    }

    fn invalidate_nodes(&mut self, old: Option<usize>, new: Option<usize>) {
        let mut document_y = None;
        let mut document_bottom = None;
        for idx in [old, new].into_iter().flatten() {
            let node = &self.nodes[idx];
            let screen_y = if node.overlay { node.y } else { node.y - self.scroll };
            // Document patches may never clear title-bar pixels.  Those are
            // outside Warp3's ownership and would otherwise expose wallpaper.
            let y0 = if node.overlay {
                screen_y - 14
            } else {
                (screen_y - 14).max(crate::window::title_bar_h() as i32)
            };
            let next = (node.x - 14, y0, node.x + node.w + 14, screen_y + node.h + 14);
            self.window_damage = Some(match self.window_damage {
                Some(old) => (old.0.min(next.0), old.1.min(next.1), old.2.max(next.2), old.3.max(next.3)),
                None => next,
            });
            if self.is_toolbar_tree(idx) {
                // Fixed controls are painted directly over the final document;
                // their hover state therefore needs no cached-layer rebuild.
            } else {
                document_y = Some(document_y.map_or(self.nodes[idx].y, |y: i32| y.min(self.nodes[idx].y)));
                document_bottom = Some(document_bottom.map_or(
                    self.nodes[idx].y + self.nodes[idx].h,
                    |bottom: i32| bottom.max(self.nodes[idx].y + self.nodes[idx].h),
                ));
            }
        }
        if let Some(y) = document_y {
            // Shadows extend outside the node; repaint their full footprint.
            self.repaint_from = self.repaint_from.min((y - 12).max(0));
            self.repaint_to = self.repaint_to.max(document_bottom.unwrap_or(y) + 12);
        }
    }

    fn paint_cached_layers(&mut self) {
        let width = self.width.max(1) as usize;
        let document_h = self.content_height.max(self.height).max(1) as usize;
        let recreate_document = !matches!(&self.document_layer, Some(layer) if layer.width() == width && layer.height() == document_h);
        if recreate_document {
            self.document_layer = Some(LayerSystem::new_transparent(width, document_h));
            self.repaint_from = 0;
            self.repaint_to = i32::MAX;
        }
        let from = self.repaint_from.max(0) as usize;
        let to = self.repaint_to.min(document_h as i32).max(from as i32) as usize;
        if let Some(mut layer) = self.document_layer.take() {
            layer.fill_rect(0, from, width, to.saturating_sub(from), html_bg());
            layer.push_clip(0, from, width, to);
            for idx in self.document_paint.clone() {
                let node = &self.nodes[idx];
                if node.y + node.h + 12 <= from as i32 || node.y - 12 >= to as i32 { continue; }
                self.draw_node(&mut layer, idx, node.x, node.y);
            }
            layer.pop_clip();
            self.document_layer = Some(layer);
        }
        self.repaint_from = self.content_height;
        self.repaint_to = self.content_height;

        let toolbar_h = (self.height + crate::window::title_bar_h() as i32).max(1) as usize;
        let recreate_toolbar = !matches!(&self.toolbar_layer, Some(layer) if layer.width() == width && layer.height() == toolbar_h);
        if recreate_toolbar { self.toolbar_layer = Some(LayerSystem::new_transparent(width, toolbar_h)); }
        if self.toolbar_dirty || recreate_toolbar {
            if let Some(mut layer) = self.toolbar_layer.take() {
                layer.clear(Color::TRANSPARENT);
                for idx in self.toolbar_paint.clone() {
                    let node = &self.nodes[idx];
                    if node.is("toolbar") {
                        self.composite_shadow_transparent(&mut layer, idx);
                    }
                }
                self.toolbar_layer = Some(layer);
            }
            self.toolbar_dirty = false;
        }
    }

    fn prepare_shadows(&mut self) {
        let mut wanted: Vec<(usize, usize, usize)> = Vec::new();
        for node in &self.nodes {
            let dimensions = if node.is("list-box") {
                let title_height = if node.prop("text").is_empty() { 0 } else { 36 };
                Some((
                    node.w.max(1) as usize,
                    (node.h - title_height).max(1) as usize,
                    8,
                ))
            } else if node.is("toolbar") || node.is("card") || node.is("tab") {
                Some((node.w.max(1) as usize, node.h.max(1) as usize, 8))
            } else {
                None
            };
            if let Some(key) = dimensions {
                if !wanted.contains(&key) {
                    wanted.push(key);
                }
            }
        }
        self.shadows
            .retain(|mask| wanted.contains(&(mask.w, mask.h, mask.radius)));
        for (w, h, radius) in wanted {
            if !self
                .shadows
                .iter()
                .any(|mask| mask.w == w && mask.h == h && mask.radius == radius)
            {
                self.shadows.push(ShadowMask::new(w, h, radius));
            }
        }
    }

    fn composite_shadow(
        &self,
        layer: &mut LayerSystem,
        x: i32,
        y: i32,
        w: usize,
        h: usize,
        radius: usize,
    ) {
        if let Some(mask) = self
            .shadows
            .iter()
            .find(|mask| mask.w == w && mask.h == h && mask.radius == radius)
        {
            mask.composite(layer, x, y);
        }
    }

    fn composite_shadow_transparent(&self, layer: &mut LayerSystem, idx: usize) {
        let node = &self.nodes[idx];
        if let Some(mask) = self.shadows.iter().find(|mask| {
            mask.w == node.w.max(1) as usize && mask.h == node.h.max(1) as usize && mask.radius == 8
        }) {
            mask.composite_transparent(layer, node.x, node.y);
        }
    }

    fn draw_node(&self, layer: &mut LayerSystem, idx: usize, x: i32, y: i32) {
        let node = &self.nodes[idx];
        let text = node.prop("text");
        let bg = html_bg();
        let panel = html_layer();
        let solid = html_layer_solid();
        let border = html_border();
        let fg = html_text();
        let muted = html_muted();
        let accent = html_accent();
        let xu = x.max(0) as usize;
        let yu = y.max(0) as usize;
        let wu = node.w.max(1) as usize;
        let hu = node.h.max(1) as usize;

        if node.is("list-box") {
            let title_height = if text.is_empty() { 0 } else { 36 };
            let box_y = y + title_height;
            let box_h = (node.h - title_height).max(1) as usize;
            self.composite_shadow(layer, x, box_y, wu, box_h, 8);
            rounded_fill(layer, x, box_y, wu, box_h, 8, panel);
        } else if node.is("toolbar") || node.is("card") || node.is("tab") {
            if !node.is("toolbar") {
                self.composite_shadow(layer, x, y, wu, hu, 8);
            }
            rounded_fill(layer, x, y, wu, hu, 8, panel);
        } else if node.is("list") {
            layer.fill_rect(xu, (y + node.h - 1).max(0) as usize, wu, 1, border);
        } else if node.is("button") || node.is("content") {
            let primary = node.prop("type") == "primary";
            let text_only = node.prop("type") == "text";
            let active_tab = node.is("content") && self.is_active_tab(idx);
            let hover = self.hover_amount(idx);
            let color = if primary {
                blend_color(accent, html_accent_hover(), hover)
            } else if hover > 0.0 {
                blend_color(solid, Color::rgb(255, 255, 255), hover)
            } else if primary {
                accent
            } else if active_tab {
                solid
            } else if text_only {
                bg
            } else {
                solid
            };
            if !text_only && (!node.is("content") || active_tab || hover > 0.0) {
                rounded_box(layer, x, y, wu, hu, 4, border, color);
            }
        } else if node.is("input") || node.is("textarea") {
            rounded_box(layer, x, y, wu, hu, 4, border, solid);
            let bottom = if self.focused_input == Some(idx) {
                accent
            } else {
                Color::rgb(119, 119, 119)
            };
            layer.fill_rect(
                xu + 4,
                yu + hu.saturating_sub(2),
                wu.saturating_sub(8),
                2,
                bottom,
            );
        } else if node.is("code") {
            layer.fill_rounded_rect(xu, yu, wu, hu, 6, Color::rgb(32, 32, 32));
        } else if node.is("switch") {
            let on = node.prop("default") == "true";
            let progress = self.switch_amount(idx, on);
            let switch_bg = blend_color(bg, accent, progress);
            rounded_box(
                layer,
                x,
                y,
                wu,
                hu,
                10,
                blend_color(Color::rgb(119, 119, 119), accent, progress),
                switch_bg,
            );
            let knob_x = x + 3 + ((node.w - 21).max(0) as f32 * progress) as i32;
            layer.fill_circle(
                knob_x.max(0) as usize + 7,
                yu + 10,
                7,
                if on {
                    Color::rgb(255, 255, 255)
                } else {
                    Color::rgb(102, 102, 102)
                },
            );
        }

        if text.is_empty() {
            return;
        }
        let (tx, ty, color) = if node.is("button") || node.is("content") {
            let color = if node.prop("type") == "primary" {
                Color::rgb(255, 255, 255)
            } else if node.is("content") && self.is_active_tab(idx) {
                accent
            } else {
                fg
            };
            (x + 14, y + 8, color)
        } else if node.is("input") || node.is("textarea") {
            (x + 10, y + 8, fg)
        } else if node.is("card") {
            (x + 16, y + 14, fg)
        } else if node.is("list-box") {
            (x, y + 8, fg)
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
        let lines = if node.is("code") {
            text.split('\n').map(ToString::to_string).collect()
        } else {
            wrap_lines(text, node.w)
        };
        for (line, value) in lines.iter().enumerate() {
            if ty + line as i32 * 22 >= 0 {
                let line_y = ty + line as i32 * 22;
                if node.is("head") {
                    put_str_size(layer, tx, line_y - 5, value, color, 28.0);
                } else if node.is("list-box") {
                    put_str_size(layer, tx, line_y - 3, value, color, 20.0);
                } else {
                    layer.put_str(tx.max(0) as usize, line_y as usize, value, color);
                }
            }
        }
    }

    fn is_active_tab(&self, content_idx: usize) -> bool {
        self.nodes
            .iter()
            .any(|node| node.is("tab") && node.children.get(node.tab).copied() == Some(content_idx))
    }

    fn hover_amount(&self, idx: usize) -> f32 {
        if let Some((old, new, frame)) = self.hover_transition {
            let t = frame as f32 / HOVER_STEPS as f32;
            if new == Some(idx) { return t; }
            if old == Some(idx) { return 1.0 - t; }
        }
        (self.hovered == Some(idx)) as u8 as f32
    }

    fn switch_amount(&self, idx: usize, on: bool) -> f32 {
        if let Some((animated, target_on, frame)) = self.switch_transition {
            if animated == idx {
                let t = ease_out(frame as f32 / SWITCH_STEPS as f32);
                return if target_on { t } else { 1.0 - t };
            }
        }
        on as u8 as f32
    }

    fn is_toolbar_tree(&self, idx: usize) -> bool {
        self.nodes.get(idx).map_or(false, |node| node.overlay)
    }

    fn refresh_visibility(&mut self) {
        for node in &mut self.nodes {
            node.hidden = false;
        }
        let tabs: Vec<(usize, usize, Vec<usize>)> = self.nodes.iter().enumerate()
            .filter(|(_, node)| node.is("tab"))
            .map(|(idx, node)| (idx, node.tab, node.children.clone()))
            .collect();
        for (_idx, active, children) in tabs {
            for (tab, child) in children.into_iter().enumerate() {
                if tab != active {
                    // `content` itself is the tab control.  Keep every
                    // control visible and hide only its inactive page body.
                    let page_children = self.nodes[child].children.clone();
                    for page_child in page_children {
                        self.mark_hidden_tree(page_child);
                    }
                }
            }
        }
    }

    fn mark_hidden_tree(&mut self, idx: usize) {
        self.nodes[idx].hidden = true;
        let children = self.nodes[idx].children.clone();
        for child in children {
            self.mark_hidden_tree(child);
        }
    }

    /// Built only when layout/tab state changes.  Scroll and hover never walk
    /// the tree: they use these already-resolved paint lists.
    fn rebuild_paint_lists(&mut self) {
        self.document_paint.clear();
        self.toolbar_paint.clear();
        for (idx, node) in self.nodes.iter().enumerate() {
            if node.hidden || node.is("config") || node.is("space")
                || (node.is("scroll-point") && node.tags.len() == 1) {
                continue;
            }
            if node.overlay {
                self.toolbar_paint.push(idx);
            } else {
                self.document_paint.push(idx);
            }
        }
    }

    fn hit_test(&self, x: i32, y: i32) -> Option<usize> {
        for toolbar_pass in [true, false] {
            if let Some(idx) = (0..self.nodes.len()).rev().find(|idx| {
                self.is_toolbar_tree(*idx) == toolbar_pass
                    && interactive(&self.nodes[*idx])
                    && !self.nodes[*idx].hidden
                    && contains(
                        &self.nodes[*idx],
                        x,
                        if self.is_toolbar_tree(*idx) { y - self.scroll } else { y },
                    )
            }) {
                return Some(idx);
            }
        }
        None
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
                    self.execute(section.actions);
                }
            }
        }
    }

    fn execute(&mut self, actions: Vec<(String, String)>) {
        let mut frames: Vec<(Vec<(String, String)>, usize, Option<String>)> =
            alloc::vec![(actions, 0, None)];
        while !frames.is_empty() {
            let finished = {
                let frame = frames.last().unwrap();
                frame.1 >= frame.0.len()
            };
            if finished {
                frames.pop();
                continue;
            }
            let (left, right) = {
                let frame = frames.last_mut().unwrap();
                let action = frame.0[frame.1].clone();
                frame.1 += 1;
                action
            };
            match left.as_str() {
                "screen" => {
                    self.screen = unquote(&right);
                    self.load_screen();
                }
                "scroll" => self.request_scroll(&right),
                "print" | "wait" => {}
                "fun" => {
                    let name = unquote(&right);
                    if frames
                        .iter()
                        .any(|(_, _, active)| active.as_ref() == Some(&name))
                    {
                        continue;
                    }
                    if let Some(section) = self.scripts.iter().find(|section| {
                        section.kind == SectionKind::Function && section.name == name
                    }) {
                        frames.push((section.actions.clone(), 0, Some(name)));
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
        if let Some(idx) = self
            .nodes
            .iter_mut()
            .position(|node| node.classes.iter().any(|item| item == class))
        {
            let y = self.nodes[idx].y;
            set_prop(&mut self.nodes[idx], "text", value);
            self.invalidate_from(y);
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

fn mark_overlay_tree(nodes: &mut [Node], idx: usize) {
    nodes[idx].overlay = true;
    let children = nodes[idx].children.clone();
    for child in children {
        mark_overlay_tree(nodes, child);
    }
}

/// One separable box blur.  Three invocations are a fast Gaussian
/// approximation and need only integer additions/subtractions per pixel.
fn box_blur_alpha(alpha: &mut [u8], width: usize, height: usize, radius: usize) {
    if width == 0 || height == 0 { return; }
    let mut tmp = alloc::vec![0u8; alpha.len()];
    let diameter = radius * 2 + 1;
    for y in 0..height {
        let row = y * width;
        let mut sum = 0u32;
        for x in 0..width + radius {
            if x < width { sum += alpha[row + x] as u32; }
            if x > diameter && x - diameter - 1 < width { sum -= alpha[row + x - diameter - 1] as u32; }
            if x >= radius && x - radius < width { tmp[row + x - radius] = (sum / diameter as u32) as u8; }
        }
    }
    for x in 0..width {
        let mut sum = 0u32;
        for y in 0..height + radius {
            if y < height { sum += tmp[y * width + x] as u32; }
            if y > diameter && y - diameter - 1 < height { sum -= tmp[(y - diameter - 1) * width + x] as u32; }
            if y >= radius && y - radius < height { alpha[(y - radius) * width + x] = (sum / diameter as u32) as u8; }
        }
    }
}

fn measure(text: &str) -> i32 {
    text.chars().map(ttf_font::advance).sum()
}

/// Keep wrapping and layout on the exact same glyph advances.  A character
/// count approximation breaks mixed CJK/Latin labels and shifts every sibling.
fn wrap_lines(text: &str, width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut line = String::new();
        let mut line_width = 0;
        for ch in paragraph.chars() {
            let advance = ttf_font::advance(ch);
            if !line.is_empty() && line_width + advance > width.max(1) {
                lines.push(line);
                line = String::new();
                line_width = 0;
            }
            line.push(ch);
            line_width += advance;
        }
        lines.push(line);
    }
    lines
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

fn html_bg() -> Color {
    Color::rgb(243, 243, 243)
}

fn html_layer() -> Color {
    Color::rgb(249, 249, 249)
}

fn html_layer_solid() -> Color {
    Color::rgb(251, 251, 251)
}

fn html_text() -> Color {
    Color::rgb(26, 26, 26)
}

fn html_muted() -> Color {
    Color::rgb(93, 93, 93)
}

fn html_border() -> Color {
    Color::rgb(211, 211, 211)
}

fn html_accent() -> Color {
    Color::rgb(0, 103, 192)
}

fn html_accent_hover() -> Color {
    Color::rgb(25, 117, 197)
}

fn blend_color(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
    Color::rgb(mix(from.r(), to.r()), mix(from.g(), to.g()), mix(from.b(), to.b()))
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

fn rounded_box(
    layer: &mut LayerSystem,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    radius: usize,
    border: Color,
    fill: Color,
) {
    if x < 0 || y < 0 || width == 0 || height == 0 {
        return;
    }
    let x = x as usize;
    let y = y as usize;
    layer.fill_rounded_rect(x, y, width, height, radius, border);
    if width > 2 && height > 2 {
        layer.fill_rounded_rect(
            x + 1,
            y + 1,
            width - 2,
            height - 2,
            radius.saturating_sub(1),
            fill,
        );
    }
}

fn rounded_fill(layer: &mut LayerSystem, x: i32, y: i32, width: usize, height: usize, radius: usize, fill: Color) {
    if x >= 0 && y >= 0 && width > 0 && height > 0 {
        layer.fill_rounded_rect(x as usize, y as usize, width, height, radius, fill);
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
    for ch in text.chars() {
        let mut advance = 0;
        ttf_font::with_glyph_at_size(ch, size, |data, w, h, glyph_advance, y_off| {
            advance = glyph_advance;
            let top = baseline + y_off;
            for row in 0..h {
                let py = top + row;
                if py < crate::window::title_bar_h() as i32 || py >= layer_h as i32 { continue; }
                for col in 0..w {
                    let px = x + col;
                    if px < 0 || px >= layer_w as i32 { continue; }
                    let alpha = data[row as usize * w as usize + col as usize] as f32 / 255.0;
                    if alpha <= 0.0 { continue; }
                    let index = py as usize * layer_w + px as usize;
                    let background = layer.buf_ref()[index];
                    layer.buf_mut()[index] = LayerSystem::blend_alpha(background, color.0, alpha);
                }
            }
        });
        x += advance.max(8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_reference_ui_without_preprocessing() {
        let nodes = Parser::new(include_str!("../../../app/warp3demo.w3a/main.w3u")).parse();
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
        let nav = parse_script(include_str!("../../../app/warp3demo.w3a/nav.w3s"));
        let variables = parse_script(include_str!("../../../app/warp3demo.w3a/var-demo.w3s"));
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
