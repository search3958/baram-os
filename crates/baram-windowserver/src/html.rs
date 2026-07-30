//! Small, dependency-free HTML/CSS application renderer for BaramOS.
//!
//! This is intentionally a document/application-view engine rather than a
//! networked web browser.  It supports the common structural HTML elements,
//! a practical CSS subset, BaramOS links, and live config values while
//! remaining usable in the UEFI `no_std` environment.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use baram_bsd::config;
use baram_core::{Color, LayerSystem};
use baram_font::{ttf_font, LayerFontExt};

const MAX_NODES: usize = 2048;
const MAX_RULES: usize = 512;
const MAX_ITEMS: usize = 4096;

#[derive(Clone, Default)]
struct Attr {
    name: String,
    value: String,
}

#[derive(Clone, Default)]
struct Node {
    tag: String,
    attrs: Vec<Attr>,
    text: String,
    children: Vec<usize>,
    parent: Option<usize>,
}

impl Node {
    fn attr(&self, name: &str) -> &str {
        self.attrs
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.value.as_str())
            .unwrap_or("")
    }

    fn has_class(&self, class_name: &str) -> bool {
        self.attr("class")
            .split_ascii_whitespace()
            .any(|class| class == class_name)
    }
}

#[derive(Clone, Default)]
struct Selector {
    tag: String,
    id: String,
    classes: Vec<String>,
    hover: bool,
}

impl Selector {
    fn specificity(&self) -> usize {
        (!self.id.is_empty()) as usize * 100
            + (self.classes.len() + self.hover as usize) * 10
            + (!self.tag.is_empty()) as usize
    }

    fn matches(&self, node: &Node, hovered: bool) -> bool {
        (self.tag.is_empty() || self.tag == "*" || self.tag == node.tag)
            && (self.id.is_empty() || self.id == node.attr("id"))
            && self.classes.iter().all(|class| node.has_class(class))
            && (!self.hover || hovered)
    }
}

#[derive(Clone, Default)]
struct CssRule {
    selector: Selector,
    declarations: Vec<(String, String)>,
    order: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Display {
    Block,
    Inline,
    Flex,
    None,
}

#[derive(Clone, Copy, PartialEq)]
enum FlexDirection {
    Row,
    Column,
}

#[derive(Clone, Copy, PartialEq)]
enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Default)]
struct Edges {
    top: i32,
    right: i32,
    bottom: i32,
    left: i32,
}

#[derive(Clone, Copy)]
enum Length {
    Px(i32),
    Percent(i32),
}

#[derive(Clone)]
struct Style {
    display: Display,
    flex_direction: FlexDirection,
    color: Color,
    background: Option<Color>,
    border_color: Color,
    border_width: i32,
    radius: i32,
    margin: Edges,
    padding: Edges,
    gap: i32,
    width: Option<Length>,
    height: Option<i32>,
    font_size: i32,
    bold: bool,
    underline: bool,
    align: TextAlign,
}

impl Style {
    fn inherited(parent: Option<&Style>) -> Self {
        let (color, font_size, bold, align) = if let Some(style) = parent {
            (style.color, style.font_size, style.bold, style.align)
        } else {
            (
                config::get_color("ui-theme/color/text", Color::TEXT),
                16,
                false,
                TextAlign::Left,
            )
        };
        Self {
            display: Display::Block,
            flex_direction: FlexDirection::Row,
            color,
            background: None,
            border_color: config::get_color("ui-theme/color/border", Color::BORDER),
            border_width: 0,
            radius: 0,
            margin: Edges::default(),
            padding: Edges::default(),
            gap: 8,
            width: None,
            height: None,
            font_size,
            bold,
            underline: false,
            align,
        }
    }
}

#[derive(Clone)]
enum PaintKind {
    Box {
        background: Option<Color>,
        border: Color,
        border_width: i32,
        radius: i32,
    },
    Text {
        text: String,
        color: Color,
        large: bool,
        underline: bool,
    },
}

#[derive(Clone)]
struct PaintItem {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    kind: PaintKind,
}

#[derive(Clone)]
struct HitArea {
    node: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    href: String,
}

pub struct HtmlEngine {
    warp3: Option<crate::warp3::Warp3Engine>,
    nodes: Vec<Node>,
    rules: Vec<CssRule>,
    root: usize,
    items: Vec<PaintItem>,
    hits: Vec<HitArea>,
    hovered_node: Option<usize>,
    width: i32,
    height: i32,
    layout_dirty: bool,
    pub content_height: i32,
    pub last_command: Option<String>,
}

impl HtmlEngine {
    pub fn new(html: &str, external_css: &str) -> Self {
        let (nodes, root) = parse_html(html);
        let mut css = collect_style_text(&nodes);
        if !external_css.trim().is_empty() {
            css.push('\n');
            css.push_str(external_css);
        }
        Self {
            warp3: None,
            nodes,
            rules: parse_css(&css),
            root,
            items: Vec::new(),
            hits: Vec::new(),
            hovered_node: None,
            width: 0,
            height: 0,
            layout_dirty: true,
            content_height: 0,
            last_command: None,
        }
    }

    pub fn new_warp3(config_name: &str) -> Self {
        let warp3 = crate::warp3::Warp3Engine::new(config_name);
        Self {
            warp3: Some(warp3),
            nodes: Vec::new(),
            rules: Vec::new(),
            root: 0,
            items: Vec::new(),
            hits: Vec::new(),
            hovered_node: None,
            width: 0,
            height: 0,
            layout_dirty: true,
            content_height: 0,
            last_command: None,
        }
    }

    pub fn update(&mut self, width: i32, height: i32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.update(width, height);
            self.content_height = engine.content_height;
            return;
        }
        if !self.layout_dirty && self.width == width && self.height == height {
            return;
        }
        self.width = width.max(1);
        self.height = height.max(1);
        self.items.clear();
        self.hits.clear();

        let styles = self.compute_styles();
        let root = self.body_node().unwrap_or(self.root);
        let start_y = crate::window::title_bar_h() as i32;
        let content_h = self.layout_node(root, 0, start_y, self.width, &styles);
        self.content_height = (start_y + content_h + 16).max(self.height);
        self.layout_dirty = false;
    }

    pub fn refresh_config(&mut self) {
        if self.warp3.is_some() {
            return;
        }
        self.layout_dirty = true;
    }

    pub fn click(&mut self, x: i32, y: i32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.click(x, y);
            return;
        }
        for hit in self.hits.iter().rev() {
            if point_in(x, y, hit.x, hit.y, hit.w, hit.h) {
                if hit.href.starts_with("os://") || hit.href.starts_with("app://") {
                    self.last_command = Some(hit.href.clone());
                }
                return;
            }
        }
    }

    pub fn set_hover(&mut self, x: i32, y: i32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.set_hover(x, y);
            return;
        }
        let hovered = self
            .hits
            .iter()
            .rev()
            .find(|hit| point_in(x, y, hit.x, hit.y, hit.w, hit.h))
            .map(|hit| hit.node);
        if self.hovered_node != hovered {
            self.hovered_node = hovered;
            self.layout_dirty = true;
        }
    }

    pub fn clear_hover(&mut self) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.clear_hover();
            return;
        }
        if self.hovered_node.take().is_some() {
            self.layout_dirty = true;
        }
    }

    pub fn hovered_node(&self) -> Option<usize> {
        if let Some(engine) = self.warp3.as_ref() {
            return engine.hovered_node();
        }
        self.hovered_node
    }

    pub fn draw_to_layer(&mut self, layer: &mut LayerSystem, ox: i32, oy: i32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.draw_to_layer(layer, ox, oy);
            return;
        }
        for item in &self.items {
            let x = item.x + ox;
            let y = item.y + oy;
            if x + item.w <= 0
                || y + item.h <= 0
                || x >= layer.width() as i32
                || y >= layer.height() as i32
            {
                continue;
            }
            match &item.kind {
                PaintKind::Box {
                    background,
                    border,
                    border_width,
                    radius,
                } => {
                    if let Some(bg) = background {
                        fill_box(layer, x, y, item.w, item.h, *radius, *bg);
                    }
                    if *border_width > 0 {
                        draw_border(layer, x, y, item.w, item.h, *radius, *border, *border_width);
                    }
                }
                PaintKind::Text {
                    text,
                    color,
                    large,
                    underline,
                } => {
                    if y < 0 {
                        continue;
                    }
                    // The large HUD font is Latin-focused. Keep mixed/Japanese
                    // headings complete by falling back to the CJK-capable UI
                    // font instead of dropping unsupported glyphs.
                    if *large && text.is_ascii() {
                        layer.put_str_hud(x.max(0) as usize, y as usize, text, *color);
                    } else {
                        layer.put_str(x.max(0) as usize, y as usize, text, *color);
                    }
                    if *underline {
                        let underline_y = y + item.h - 3;
                        fill_box(layer, x, underline_y, item.w, 1, 0, *color);
                    }
                }
            }
        }
    }

    pub fn set_scroll(&mut self, scroll: i32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.set_scroll(scroll);
        }
    }

    pub fn take_scroll_request(&mut self) -> Option<i32> {
        self.warp3.as_mut().and_then(|engine| engine.take_scroll_request())
    }

    pub fn window_damage(&self) -> Option<(i32, i32, i32, i32)> {
        self.warp3.as_ref().and_then(|engine| engine.window_damage())
    }

    pub fn tick(&mut self, now_ns: u64) -> bool {
        self.warp3.as_mut().map_or(false, |engine| engine.tick(now_ns))
    }

    pub fn has_focused_input(&self) -> bool {
        self.warp3
            .as_ref()
            .map_or(false, |engine| engine.has_focused_input())
    }

    pub fn handle_key(&mut self, key: u8) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.handle_key(key);
        }
    }

    fn body_node(&self) -> Option<usize> {
        self.nodes.iter().position(|node| node.tag == "body")
    }

    fn compute_styles(&self) -> Vec<Style> {
        let mut styles = Vec::with_capacity(self.nodes.len());
        for idx in 0..self.nodes.len() {
            let parent_style = self.nodes[idx].parent.and_then(|parent| styles.get(parent));
            let mut style = Style::inherited(parent_style);
            apply_tag_defaults(&self.nodes[idx].tag, &mut style);

            let mut matching: Vec<&CssRule> = self
                .rules
                .iter()
                .filter(|rule| {
                    rule.selector
                        .matches(&self.nodes[idx], self.hovered_node == Some(idx))
                })
                .collect();
            matching.sort_by_key(|rule| (rule.selector.specificity(), rule.order));
            for rule in matching {
                apply_declarations(&mut style, &rule.declarations);
            }
            let inline = parse_declarations(self.nodes[idx].attr("style"));
            apply_declarations(&mut style, &inline);
            styles.push(style);
        }
        styles
    }

    fn layout_node(
        &mut self,
        idx: usize,
        x: i32,
        y: i32,
        available_w: i32,
        styles: &[Style],
    ) -> i32 {
        if idx >= self.nodes.len() || self.items.len() >= MAX_ITEMS {
            return 0;
        }
        let style = styles[idx].clone();
        if style.display == Display::None
            || matches!(
                self.nodes[idx].tag.as_str(),
                "head" | "style" | "script" | "title" | "meta" | "link"
            )
        {
            return 0;
        }
        if self.nodes[idx].tag == "#text" {
            let text = normalize_whitespace(&self.nodes[idx].text);
            return self.layout_text(idx, &text, x, y, available_w, &style, String::new());
        }
        if self.nodes[idx].tag == "br" {
            return line_height(style.font_size);
        }
        if self.nodes[idx].tag == "hr" {
            self.items.push(PaintItem {
                x,
                y: y + 7,
                w: available_w,
                h: 1,
                kind: PaintKind::Box {
                    background: Some(style.border_color),
                    border: style.border_color,
                    border_width: 0,
                    radius: 0,
                },
            });
            return 16;
        }

        let outer_x = x + style.margin.left;
        let outer_y = y + style.margin.top;
        let max_w = (available_w - style.margin.left - style.margin.right).max(1);
        let box_w = resolve_width(style.width, max_w)
            .unwrap_or(max_w)
            .min(max_w)
            .max(1);
        let inner_x = outer_x + style.padding.left + style.border_width;
        let inner_y = outer_y + style.padding.top + style.border_width;
        let inner_w =
            (box_w - style.padding.left - style.padding.right - style.border_width * 2).max(1);

        let paint_index = self.items.len();
        self.items.push(PaintItem {
            x: outer_x,
            y: outer_y,
            w: box_w,
            h: 0,
            kind: PaintKind::Box {
                background: style.background,
                border: style.border_color,
                border_width: style.border_width,
                radius: style.radius,
            },
        });

        let tag = self.nodes[idx].tag.clone();
        let href = self.effective_href(idx);
        let textual = is_textual_tag(&tag);
        let mut content_h = 0;

        if textual {
            let text = self.collect_text(idx);
            content_h =
                self.layout_text(idx, &text, inner_x, inner_y, inner_w, &style, href.clone());
        } else {
            let own_text = normalize_whitespace(&self.nodes[idx].text);
            if !own_text.is_empty() {
                content_h += self.layout_text(
                    idx,
                    &own_text,
                    inner_x,
                    inner_y,
                    inner_w,
                    &style,
                    href.clone(),
                );
            }

            let children = self.nodes[idx].children.clone();
            if style.display == Display::Flex
                && style.flex_direction == FlexDirection::Row
                && !children.is_empty()
            {
                let visible_count = children
                    .iter()
                    .filter(|child| self.is_layout_visible(**child, styles))
                    .count()
                    .max(1) as i32;
                let child_w = ((inner_w - style.gap * (visible_count - 1)) / visible_count).max(1);
                let mut child_x = inner_x;
                let mut max_h = 0;
                for child in children {
                    if !self.is_layout_visible(child, styles) {
                        continue;
                    }
                    let h = self.layout_node(child, child_x, inner_y, child_w, styles);
                    max_h = max_h.max(h);
                    child_x += child_w + style.gap;
                }
                content_h = content_h.max(max_h);
            } else {
                let mut child_y = inner_y + content_h;
                for child in children {
                    let h = self.layout_node(child, inner_x, child_y, inner_w, styles);
                    if h > 0 {
                        child_y += h + style.gap;
                        content_h = child_y - inner_y - style.gap;
                    }
                }
            }
        }

        let natural_h =
            content_h + style.padding.top + style.padding.bottom + style.border_width * 2;
        let box_h = style.height.unwrap_or(natural_h).max(natural_h).max(1);
        self.items[paint_index].h = box_h;

        if !href.is_empty() {
            self.hits.push(HitArea {
                node: idx,
                x: outer_x,
                y: outer_y,
                w: box_w,
                h: box_h,
                href,
            });
        }

        style.margin.top + box_h + style.margin.bottom
    }

    fn is_layout_visible(&self, idx: usize, styles: &[Style]) -> bool {
        styles[idx].display != Display::None
            && (self.nodes[idx].tag != "#text"
                || !normalize_whitespace(&self.nodes[idx].text).is_empty())
    }

    fn layout_text(
        &mut self,
        idx: usize,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        style: &Style,
        href: String,
    ) -> i32 {
        if text.trim().is_empty() {
            return 0;
        }
        let lines = wrap_text(text, width.max(1), style.font_size);
        let line_h = line_height(style.font_size);
        let large = style.font_size >= 22 || style.bold;
        for (line_index, line) in lines.iter().enumerate() {
            let text_w = measure_text(line, style.font_size).min(width);
            let text_x = match style.align {
                TextAlign::Left => x,
                TextAlign::Center => x + (width - text_w) / 2,
                TextAlign::Right => x + width - text_w,
            };
            self.items.push(PaintItem {
                x: text_x,
                y: y + line_index as i32 * line_h,
                w: text_w,
                h: line_h,
                kind: PaintKind::Text {
                    text: line.clone(),
                    color: style.color,
                    large,
                    underline: style.underline,
                },
            });
        }
        let h = lines.len() as i32 * line_h;
        if !href.is_empty() {
            self.hits.push(HitArea {
                node: idx,
                x,
                y,
                w: width,
                h,
                href,
            });
        }
        h
    }

    fn collect_text(&self, idx: usize) -> String {
        let mut output = String::new();
        self.collect_text_into(idx, &mut output);
        normalize_whitespace(&output)
    }

    fn collect_text_into(&self, idx: usize, output: &mut String) {
        let node = &self.nodes[idx];
        if node.tag == "span" && !node.attr("config").is_empty() {
            if let Some(path) = node.attr("config").strip_prefix("os://") {
                let path = path.split('?').next().unwrap_or(path).trim_matches('/');
                if let Some(value) = config::get_config().get(path) {
                    output.push_str(value);
                    return;
                }
            }
        }
        output.push_str(&node.text);
        for child in &node.children {
            if self.nodes[*child].tag == "br" {
                output.push('\n');
            } else {
                self.collect_text_into(*child, output);
            }
        }
    }

    fn effective_href(&self, idx: usize) -> String {
        let href = self.nodes[idx].attr("href");
        if href.starts_with("os://") || href.starts_with("app://") {
            href.to_string()
        } else {
            String::new()
        }
    }
}

fn point_in(px: i32, py: i32, x: i32, y: i32, w: i32, h: i32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn fill_box(layer: &mut LayerSystem, x: i32, y: i32, w: i32, h: i32, radius: i32, color: Color) {
    if w <= 0 || h <= 0 {
        return;
    }
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(layer.width() as i32);
    let y1 = (y + h).min(layer.height() as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    if x >= 0 && y >= 0 && x + w <= layer.width() as i32 && y + h <= layer.height() as i32 {
        if radius > 0 {
            layer.fill_rounded_rect(
                x as usize,
                y as usize,
                w as usize,
                h as usize,
                radius.min(w / 2).min(h / 2) as usize,
                color,
            );
        } else {
            layer.fill_rect(x as usize, y as usize, w as usize, h as usize, color);
        }
    } else {
        layer.fill_rect(
            x0 as usize,
            y0 as usize,
            (x1 - x0) as usize,
            (y1 - y0) as usize,
            color,
        );
    }
}

fn draw_border(
    layer: &mut LayerSystem,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
    color: Color,
    border_width: i32,
) {
    for inset in 0..border_width.min(4) {
        let iw = w - inset * 2;
        let ih = h - inset * 2;
        if iw <= 0 || ih <= 0 || x + inset < 0 || y + inset < 0 {
            continue;
        }
        // Keep the already-painted interior intact. Rounded clipping is used
        // for the background; a compact rectangular outline is preferable to
        // clearing that interior with a second rounded fill.
        let _ = radius;
        layer.rect_outline(
            (x + inset) as usize,
            (y + inset) as usize,
            iw as usize,
            ih as usize,
            color,
        );
    }
}

fn resolve_width(length: Option<Length>, available: i32) -> Option<i32> {
    match length? {
        Length::Px(value) => Some(value),
        Length::Percent(value) => Some(available * value / 100),
    }
}

fn is_textual_tag(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "a"
            | "button"
            | "label"
            | "li"
            | "pre"
            | "code"
            | "small"
            | "strong"
            | "b"
            | "em"
            | "i"
            | "span"
    )
}

fn apply_tag_defaults(tag: &str, style: &mut Style) {
    match tag {
        "html" => {}
        "body" => {
            style.padding = Edges {
                top: 18,
                right: 18,
                bottom: 18,
                left: 18,
            };
            style.gap = 12;
        }
        "h1" => {
            style.font_size = 30;
            style.bold = true;
            style.margin.bottom = 12;
        }
        "h2" => {
            style.font_size = 25;
            style.bold = true;
            style.margin.bottom = 10;
        }
        "h3" => {
            style.font_size = 21;
            style.bold = true;
            style.margin.bottom = 8;
        }
        "h4" | "h5" | "h6" => {
            style.font_size = 18;
            style.bold = true;
            style.margin.bottom = 6;
        }
        "p" => {
            style.margin.bottom = 8;
        }
        "a" => {
            style.color = Color::rgb(10, 96, 255);
            style.underline = true;
            style.display = Display::Inline;
            style.margin.bottom = 4;
        }
        "button" => {
            style.background = Some(config::get_color(
                "ui-theme/color/btn_primary",
                Color::BTN_PRIMARY,
            ));
            style.color = config::get_color("ui-theme/color/btn_text", Color::BTN_TEXT);
            style.radius = config::get_i32("ui-theme/button/corner", 20);
            style.padding = Edges {
                top: 10,
                right: 18,
                bottom: 10,
                left: 18,
            };
            style.align = TextAlign::Center;
        }
        "strong" | "b" => style.bold = true,
        "small" => style.font_size = 13,
        "code" | "pre" => {
            style.background = Some(Color::rgb(238, 240, 244));
            style.padding = Edges {
                top: 8,
                right: 10,
                bottom: 8,
                left: 10,
            };
            style.radius = 6;
        }
        "ul" | "ol" => style.padding.left = 18,
        "li" => style.margin.bottom = 5,
        _ => {}
    }
}

fn apply_declarations(style: &mut Style, declarations: &[(String, String)]) {
    for (name, value) in declarations {
        match name.as_str() {
            "display" => {
                style.display = match value.as_str() {
                    "none" => Display::None,
                    "inline" | "inline-block" => Display::Inline,
                    "flex" => Display::Flex,
                    _ => Display::Block,
                }
            }
            "flex-direction" => {
                style.flex_direction = if value == "column" {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                }
            }
            "color" => {
                if let Some(color) = parse_color(value) {
                    style.color = color;
                }
            }
            "background" | "background-color" => {
                style.background = parse_color(value);
            }
            "border-color" => {
                if let Some(color) = parse_color(value) {
                    style.border_color = color;
                }
            }
            "border-width" => style.border_width = parse_px(value).unwrap_or(0).max(0),
            "border" => parse_border(value, style),
            "border-radius" => style.radius = parse_px(value).unwrap_or(0).max(0),
            "margin" => style.margin = parse_edges(value),
            "margin-top" => style.margin.top = parse_px(value).unwrap_or(0),
            "margin-right" => style.margin.right = parse_px(value).unwrap_or(0),
            "margin-bottom" => style.margin.bottom = parse_px(value).unwrap_or(0),
            "margin-left" => style.margin.left = parse_px(value).unwrap_or(0),
            "padding" => style.padding = parse_edges(value),
            "padding-top" => style.padding.top = parse_px(value).unwrap_or(0),
            "padding-right" => style.padding.right = parse_px(value).unwrap_or(0),
            "padding-bottom" => style.padding.bottom = parse_px(value).unwrap_or(0),
            "padding-left" => style.padding.left = parse_px(value).unwrap_or(0),
            "gap" => style.gap = parse_px(value).unwrap_or(8).max(0),
            "width" => style.width = parse_length(value),
            "height" | "min-height" => style.height = parse_px(value),
            "font-size" => style.font_size = parse_px(value).unwrap_or(16).clamp(10, 48),
            "font-weight" => {
                style.bold = value == "bold"
                    || value
                        .parse::<u32>()
                        .map(|weight| weight >= 600)
                        .unwrap_or(false)
            }
            "text-decoration" => style.underline = value.contains("underline"),
            "text-align" => {
                style.align = match value.as_str() {
                    "center" => TextAlign::Center,
                    "right" | "end" => TextAlign::Right,
                    _ => TextAlign::Left,
                }
            }
            _ => {}
        }
    }
}

fn parse_border(value: &str, style: &mut Style) {
    for token in value.split_ascii_whitespace() {
        if let Some(width) = parse_px(token) {
            style.border_width = width.max(0);
        } else if let Some(color) = parse_color(token) {
            style.border_color = color;
        }
    }
}

fn parse_edges(value: &str) -> Edges {
    let values: Vec<i32> = value
        .split_ascii_whitespace()
        .filter_map(parse_px)
        .collect();
    match values.as_slice() {
        [all] => Edges {
            top: *all,
            right: *all,
            bottom: *all,
            left: *all,
        },
        [vertical, horizontal] => Edges {
            top: *vertical,
            right: *horizontal,
            bottom: *vertical,
            left: *horizontal,
        },
        [top, horizontal, bottom] => Edges {
            top: *top,
            right: *horizontal,
            bottom: *bottom,
            left: *horizontal,
        },
        [top, right, bottom, left, ..] => Edges {
            top: *top,
            right: *right,
            bottom: *bottom,
            left: *left,
        },
        _ => Edges::default(),
    }
}

fn parse_px(value: &str) -> Option<i32> {
    let value = value.trim();
    if value == "auto" {
        return None;
    }
    let number = value
        .strip_suffix("px")
        .or_else(|| value.strip_suffix("rem").map(|v| v))
        .unwrap_or(value);
    if value.ends_with("rem") {
        number
            .parse::<f32>()
            .ok()
            .map(|parsed| (parsed * 16.0) as i32)
    } else {
        number.parse::<f32>().ok().map(|parsed| parsed as i32)
    }
}

fn parse_length(value: &str) -> Option<Length> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent.parse::<i32>().ok().map(Length::Percent);
    }
    parse_px(value).map(Length::Px)
}

fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim().trim_end_matches("!important").trim();
    if value.eq_ignore_ascii_case("transparent") || value.eq_ignore_ascii_case("none") {
        return None;
    }
    let named = match value.to_ascii_lowercase().as_str() {
        "black" => Some(Color::rgb(0, 0, 0)),
        "white" => Some(Color::rgb(255, 255, 255)),
        "red" => Some(Color::rgb(255, 0, 0)),
        "green" => Some(Color::rgb(0, 128, 0)),
        "blue" => Some(Color::rgb(0, 102, 255)),
        "gray" | "grey" => Some(Color::rgb(128, 128, 128)),
        "yellow" => Some(Color::rgb(255, 204, 0)),
        "orange" => Some(Color::rgb(255, 136, 0)),
        "purple" => Some(Color::rgb(128, 64, 192)),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some(Color::rgb(r, g, b));
        }
        if hex.len() >= 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::rgb(r, g, b));
        }
    }
    if let Some(args) = value.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let channels: Vec<u8> = args
            .split(',')
            .filter_map(|part| part.trim().parse::<u8>().ok())
            .collect();
        if channels.len() == 3 {
            return Some(Color::rgb(channels[0], channels[1], channels[2]));
        }
    }
    None
}

fn parse_html(html: &str) -> (Vec<Node>, usize) {
    let mut nodes = Vec::new();
    nodes.push(Node {
        tag: "html".to_string(),
        ..Node::default()
    });
    let root = 0;
    let mut stack = alloc::vec![root];
    let mut cursor = 0;

    while cursor < html.len() && nodes.len() < MAX_NODES {
        let Some(open_rel) = html[cursor..].find('<') else {
            append_text(&mut nodes, *stack.last().unwrap_or(&root), &html[cursor..]);
            break;
        };
        let open = cursor + open_rel;
        if open > cursor {
            append_text(
                &mut nodes,
                *stack.last().unwrap_or(&root),
                &html[cursor..open],
            );
        }
        if html[open..].starts_with("<!--") {
            cursor = html[open + 4..]
                .find("-->")
                .map(|end| open + 4 + end + 3)
                .unwrap_or(html.len());
            continue;
        }
        let Some(close_rel) = html[open + 1..].find('>') else {
            break;
        };
        let close = open + 1 + close_rel;
        let raw = html[open + 1..close].trim();
        cursor = close + 1;
        if raw.is_empty() || raw.starts_with('!') || raw.starts_with('?') {
            continue;
        }
        if let Some(end_tag) = raw.strip_prefix('/') {
            let end_tag = end_tag
                .split_ascii_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            while stack.len() > 1 {
                let popped = stack.pop().unwrap_or(root);
                if nodes[popped].tag == end_tag {
                    break;
                }
            }
            continue;
        }

        let self_closing = raw.ends_with('/');
        let (tag, attrs) = parse_tag(raw.trim_end_matches('/').trim());
        if tag.is_empty() {
            continue;
        }
        let parent = *stack.last().unwrap_or(&root);
        let idx = nodes.len();
        nodes.push(Node {
            tag: tag.clone(),
            attrs,
            text: String::new(),
            children: Vec::new(),
            parent: Some(parent),
        });
        nodes[parent].children.push(idx);
        if !self_closing && !is_void_tag(&tag) {
            stack.push(idx);
        }
    }
    (nodes, root)
}

fn append_text(nodes: &mut Vec<Node>, parent: usize, text: &str) {
    if parent >= nodes.len() || text.is_empty() || nodes.len() >= MAX_NODES {
        return;
    }
    let idx = nodes.len();
    nodes.push(Node {
        tag: "#text".to_string(),
        text: decode_entities(text),
        parent: Some(parent),
        ..Node::default()
    });
    nodes[parent].children.push(idx);
}

fn parse_tag(raw: &str) -> (String, Vec<Attr>) {
    let mut chars = raw.char_indices().peekable();
    let mut name_end = raw.len();
    while let Some((idx, ch)) = chars.next() {
        if ch.is_ascii_whitespace() {
            name_end = idx;
            break;
        }
    }
    let tag = raw[..name_end].trim().to_ascii_lowercase();
    let attrs = if name_end < raw.len() {
        parse_attrs(&raw[name_end..])
    } else {
        Vec::new()
    };
    (tag, attrs)
}

fn parse_attrs(raw: &str) -> Vec<Attr> {
    let bytes = raw.as_bytes();
    let mut attrs = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let name_start = cursor;
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'='
        {
            cursor += 1;
        }
        if name_start == cursor {
            cursor += 1;
            continue;
        }
        let name = raw[name_start..cursor].to_ascii_lowercase();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let mut value = String::new();
        if cursor < bytes.len() && bytes[cursor] == b'=' {
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor < bytes.len() && (bytes[cursor] == b'"' || bytes[cursor] == b'\'') {
                let quote = bytes[cursor];
                cursor += 1;
                let value_start = cursor;
                while cursor < bytes.len() && bytes[cursor] != quote {
                    cursor += 1;
                }
                value = decode_entities(&raw[value_start..cursor]);
                cursor = (cursor + 1).min(bytes.len());
            } else {
                let value_start = cursor;
                while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
                value = decode_entities(&raw[value_start..cursor]);
            }
        }
        attrs.push(Attr { name, value });
    }
    attrs
}

fn is_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn decode_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn collect_style_text(nodes: &[Node]) -> String {
    let mut output = String::new();
    for (idx, node) in nodes.iter().enumerate() {
        if node.tag == "style" {
            collect_raw_text(nodes, idx, &mut output);
            output.push('\n');
        }
    }
    output
}

fn collect_raw_text(nodes: &[Node], idx: usize, output: &mut String) {
    output.push_str(&nodes[idx].text);
    for child in &nodes[idx].children {
        collect_raw_text(nodes, *child, output);
    }
}

fn parse_css(css: &str) -> Vec<CssRule> {
    let css = strip_css_comments(css);
    let mut rules = Vec::new();
    let mut cursor = 0;
    while cursor < css.len() && rules.len() < MAX_RULES {
        let Some(open_rel) = css[cursor..].find('{') else {
            break;
        };
        let open = cursor + open_rel;
        let Some(close_rel) = css[open + 1..].find('}') else {
            break;
        };
        let close = open + 1 + close_rel;
        let selectors = css[cursor..open].trim();
        let declarations = parse_declarations(&css[open + 1..close]);
        for selector_text in selectors.split(',') {
            if let Some(selector) = parse_selector(selector_text) {
                rules.push(CssRule {
                    selector,
                    declarations: declarations.clone(),
                    order: rules.len(),
                });
            }
        }
        cursor = close + 1;
    }
    rules
}

fn strip_css_comments(css: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(start_rel) = css[cursor..].find("/*") {
        let start = cursor + start_rel;
        output.push_str(&css[cursor..start]);
        let Some(end_rel) = css[start + 2..].find("*/") else {
            return output;
        };
        cursor = start + 2 + end_rel + 2;
    }
    output.push_str(&css[cursor..]);
    output
}

fn parse_selector(selector: &str) -> Option<Selector> {
    let simple = selector.split_ascii_whitespace().last()?.trim();
    if simple.is_empty() || simple.starts_with('@') {
        return None;
    }
    let hover = simple.contains(":hover");
    let simple = simple.replace(":hover", "");
    let bytes = simple.as_bytes();
    let mut selector = Selector {
        hover,
        ..Selector::default()
    };
    let mut cursor = 0;
    let tag_start = cursor;
    while cursor < bytes.len() && bytes[cursor] != b'.' && bytes[cursor] != b'#' {
        cursor += 1;
    }
    if cursor > tag_start {
        selector.tag = simple[tag_start..cursor].to_ascii_lowercase();
    }
    while cursor < bytes.len() {
        let kind = bytes[cursor];
        cursor += 1;
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor] != b'.' && bytes[cursor] != b'#' {
            cursor += 1;
        }
        if start == cursor {
            continue;
        }
        let value = simple[start..cursor].to_string();
        if kind == b'#' {
            selector.id = value;
        } else {
            selector.classes.push(value);
        }
    }
    Some(selector)
}

fn parse_declarations(input: &str) -> Vec<(String, String)> {
    let mut declarations = Vec::new();
    for declaration in input.split(';') {
        if let Some((name, value)) = declaration.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_ascii_lowercase();
            if !name.is_empty() && !value.is_empty() {
                declarations.push((name, value));
            }
        }
    }
    declarations
}

fn normalize_whitespace(input: &str) -> String {
    let mut output = String::new();
    let mut was_space = false;
    for ch in input.chars() {
        if ch == '\n' {
            while output.ends_with(' ') {
                output.pop();
            }
            if !output.ends_with('\n') {
                output.push('\n');
            }
            was_space = false;
        } else if ch.is_whitespace() {
            if !was_space && !output.is_empty() && !output.ends_with('\n') {
                output.push(' ');
            }
            was_space = true;
        } else {
            output.push(ch);
            was_space = false;
        }
    }
    output.trim().to_string()
}

fn wrap_text(text: &str, width: i32, font_size: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_width = 0;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(line);
            line = String::new();
            line_width = 0;
            continue;
        }
        let char_width = measure_char(ch, font_size);
        if !line.is_empty() && line_width + char_width > width {
            lines.push(line.trim_end().to_string());
            line = String::new();
            line_width = 0;
            if ch == ' ' {
                continue;
            }
        }
        line.push(ch);
        line_width += char_width;
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line.trim_end().to_string());
    }
    lines
}

fn measure_text(text: &str, font_size: i32) -> i32 {
    text.chars().map(|ch| measure_char(ch, font_size)).sum()
}

fn measure_char(ch: char, font_size: i32) -> i32 {
    let base = if ttf_font::is_available() {
        let glyph = ttf_font::glyph(ch);
        if glyph.w > 0 {
            glyph.advance.max(1) as i32
        } else {
            8
        }
    } else {
        8
    };
    if font_size >= 22 {
        base * 5 / 4
    } else if font_size <= 13 {
        base * 4 / 5
    } else {
        base
    }
}

fn line_height(font_size: i32) -> i32 {
    if font_size >= 26 {
        34
    } else if font_size >= 22 {
        29
    } else if font_size <= 13 {
        18
    } else {
        22
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_attributes_and_entities() {
        let (nodes, _) =
            parse_html(r#"<body><a href="os://display/hud?enabled=1&amp;mode=x">Open</a></body>"#);
        let link = nodes.iter().find(|node| node.tag == "a").unwrap();
        assert_eq!(link.attr("href"), "os://display/hud?enabled=1&mode=x");
    }

    #[test]
    fn css_specificity_prefers_ids() {
        let rules = parse_css("p { color: red; } .x { color: blue; } #main { color: white; }");
        assert_eq!(rules.len(), 3);
        assert!(rules[2].selector.specificity() > rules[1].selector.specificity());
    }

    #[test]
    fn wraps_japanese_without_spaces() {
        let lines = wrap_text("これは長い日本語のテキストです", 40, 16);
        assert!(lines.len() > 1);
    }
}
