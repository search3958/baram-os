// Small, dependency-free HTML/CSS application renderer for BaramOS.
//
// This is intentionally a document/application-view engine rather than a
// networked web browser.  It supports the common structural HTML elements,
// a practical CSS subset, BaramOS links, and live config values while
// remaining usable in the UEFI `no_std` environment.

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
    warp4: Option<crate::warp::WarpEngine>,
    origin: String,
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


