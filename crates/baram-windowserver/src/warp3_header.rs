// Native Warp 3 UI renderer.
//
// Warp 3 applications stay as `config.ini`, `.w3u`, and `.w3s` files on the
// EFI volume.  This module deliberately does not depend on a browser or JS.

extern crate alloc;

use crate::text_cursor;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use baram_bsd::{app::Warp3Archive, config};
use baram_core::{Color, LayerSystem};
use baram_font::ttf_font;
use baram_font::LayerFontExt;
use uefi::runtime;

// Progress is derived directly from monotonic time. There are deliberately no
// animation steps: the compositor samples the exact state whenever it draws.
const HOVER_DURATION_NS: u64 = 6_500_000;
const SWITCH_DURATION_NS: u64 = 18_000_000;
const HOVER_DAMAGE_PAD: i32 = 14;

#[derive(Clone, Copy, Default)]
struct NowValues {
    fps: u32,
    windows: usize,
    keys: u32,
    mouse: u32,
}

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
                let dx = if px < left + r as usize {
                    left as i32 + r - px as i32
                } else if px >= right - r as usize {
                    px as i32 - (right as i32 - r - 1)
                } else {
                    0
                };
                let dy = if py < top + r as usize {
                    top as i32 + r - py as i32
                } else if py >= bottom - r as usize {
                    py as i32 - (bottom as i32 - r - 1)
                } else {
                    0
                };
                if dx == 0 || dy == 0 || dx * dx + dy * dy <= r * r {
                    alpha[py * sw + px] = 34;
                }
            }
        }
        for _ in 0..2 {
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
            if dy < 0 || dy >= target.height() as i32 {
                continue;
            }
            for sx in 0..self.layer.width() {
                let dx = x - self.pad + sx as i32;
                if dx < 0 || dx >= target.width() as i32 {
                    continue;
                }
                let alpha = self.layer.buf_ref()[sy * self.layer.width() + sx] & 0xff;
                if alpha == 0 {
                    continue;
                }
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
    /// Resolved at layout time so dense lists do not repeatedly wrap their
    /// labels during every paint.
    text_lines: Vec<String>,
    tab: usize,
    overlay: bool,
    hidden: bool,
    manual_hidden: bool,
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
    script_frames: Vec<(Vec<(String, String)>, usize, Option<String>)>,
    script_wait_until_ns: Option<u64>,
    command_queue: Vec<String>,
    state: Vec<(String, String)>,
    hovered: Option<usize>,
    focused_input: Option<usize>,
    width: i32,
    height: i32,
    scroll: i32,
    pub content_height: i32,
    scroll_request: Option<i32>,
    dirty: bool,
    /// First document y that must be repainted after a reflow. Keeping the
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
    hover_transition: Option<(Option<usize>, Option<usize>)>,
    switch_transition: Option<(usize, bool)>,
    hover_started_ns: Option<u64>,
    switch_started_ns: Option<u64>,
    screen_transition_active: bool,
    screen_transition_started_ns: Option<u64>,
    screen_transition_offset_y: f32,
    animation_now_ns: u64,
    caret_visible: bool,
    shadows: Vec<ShadowMask>,
    now: NowValues,
    last_clicked_class: Option<String>,
    candidate_nodes: Vec<usize>,
}


