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
use baram_graphics::svg;
use uefi::runtime;

const MAX_NODES: usize = 2048;
const MAX_ACTIONS: usize = 2048;
const SWITCH_DURATION_NS: u64 = 220_000_000;
const RADIO_DURATION_NS: u64 = 180_000_000;
const CHECK_ICON_SVG: &str = include_str!("../../../data/check-icon.svg");
const WARP4_INPUT_RADIUS: usize = 11;
// Warp3 control palette.  Keep Warp4's native controls visually identical to
// the established Warp3 surface instead of maintaining a second theme.
const WARP3_BG: Color = Color::rgb(243, 243, 243);
const WARP3_SURFACE: Color = Color::rgb(251, 251, 251);
const WARP3_TEXT: Color = Color::rgb(26, 26, 26);
const WARP3_MUTED: Color = Color::rgb(93, 93, 93);
const WARP3_BORDER: Color = Color::rgb(211, 211, 211);
const WARP3_ACCENT: Color = Color::rgb(0, 106, 255);
const WARP4_BG: Color = Color::rgb(250, 250, 252);
const WARP4_PRIMARY: Color = Color::rgb(0, 106, 255);
const WARP4_BUTTON_BG: Color = Color::rgb(238, 238, 239);
const WARP4_INPUT_BG: Color = Color::rgb(255, 255, 255);
const WARP4_INPUT_BORDER: Color = Color::rgb(242, 242, 246);
const WARP4_RADIO_OFF: Color = Color::rgb(231, 230, 230);
const WARP4_WHITE: Color = Color::rgb(255, 255, 255);
const WARP4_BLACK: Color = Color::rgb(0, 0, 0);
const SCROLLBAR_TRACK: Color = Color::rgb(241, 241, 241);
const SCROLLBAR_THUMB: Color = Color::rgb(184, 184, 184);
const SCROLLBAR_RADIUS: usize = 3;

fn title_bar_h() -> i32 {
    config::get_usize("ui-theme/window/title_bar_h", 30) as i32
}

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
    /// The native equivalent of the generated DOM's overflow/position state.
    /// These are computed by the layout pass; they are deliberately kept on
    /// the view instead of being global renderer state so nested viewports can
    /// be clipped and hit-tested independently.
    content_w: i32,
    content_h: i32,
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
    Break,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PaintPass {
    Flow,
    Fixed,
}

#[derive(Clone, Copy)]
struct ControlAnimation {
    idx: usize,
    to_on: bool,
    started_ns: u64,
    duration_ns: u64,
}

#[derive(Clone, Copy)]
struct SpinnerFade {
    idx: usize,
    started_ns: u64,
}

pub struct Warp4Engine {
    archive: Warp4Archive,
    origin: String,
    title: String,
    screen: String,
    nodes: Vec<Node>,
    roots: Vec<usize>,
    fixed_subtree: Vec<bool>,
    script: Script,
    state: Vec<(String, String)>,
    focused: Option<usize>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    spinner_open: Option<usize>,
    spinner_fade: Option<SpinnerFade>,
    width: i32,
    height: i32,
    scroll: i32,
    pub content_height: i32,
    pub last_command: Option<String>,
    dirty: bool,
    now_ns: u64,
    wait_until_ns: Option<u64>,
    pending: Vec<Action>,
    break_requested: bool,
    flip_elapsed_ns: u64,
    last_tick_ns: Option<u64>,
    control_animations: Vec<ControlAnimation>,
    last_clicked_id: Option<String>,
    runtime_fps: u32,
    runtime_windows: usize,
    runtime_keys: u32,
    runtime_mouse: u32,
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
            fixed_subtree: Vec::new(),
            script: Script::default(),
            state: Vec::new(),
            focused: None,
            hovered: None,
            pressed: None,
            spinner_open: None,
            spinner_fade: None,
            width: 0,
            height: 0,
            scroll: 0,
            content_height: 0,
            last_command: None,
            dirty: true,
            now_ns: 0,
            wait_until_ns: None,
            pending: Vec::new(),
            break_requested: false,
            flip_elapsed_ns: 0,
            last_tick_ns: None,
            control_animations: Vec::new(),
            last_clicked_id: None,
            runtime_fps: 0,
            runtime_windows: 0,
            runtime_keys: 0,
            runtime_mouse: 0,
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
        let max = self
            .content_height
            .saturating_sub(self.height + title_bar_h());
        let next = scroll.max(0).min(max.max(0));
        if self.scroll != next {
            self.scroll = next;
            if let Some(idx) = self.spinner_open {
                self.close_spinner(idx);
            }
            self.hovered = None;
            self.dirty = true;
        }
    }
    pub fn is_animating(&self) -> bool {
        !self.control_animations.is_empty()
    }
    pub fn window_damage(&self) -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn has_focused_input(&self) -> bool {
        self.focused.is_some()
    }
    pub fn set_runtime_metrics(&mut self, fps: u32, windows: usize, keys: u32, mouse: u32) {
        self.runtime_fps = fps;
        self.runtime_windows = windows;
        self.runtime_keys = keys;
        self.runtime_mouse = mouse;
    }
    pub fn hovered_node(&self) -> Option<usize> {
        self.hovered
    }
    pub fn clear_hover(&mut self) {
        if self.hovered.take().is_some() {
            self.dirty = true;
        }
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
    pub fn tick(&mut self, now_ns: u64) -> bool {
        self.now_ns = now_ns;
        let delta = self
            .last_tick_ns
            .replace(now_ns)
            .map(|previous| now_ns.saturating_sub(previous).min(100_000_000))
            .unwrap_or(0);
        self.flip_elapsed_ns = self.flip_elapsed_ns.saturating_add(delta);

        let mut changed = false;
        let mut flip_interval_ns: Option<u64> = None;
        for node in &self.nodes {
            if (node.is("ViewFlipper") || node.is("ViewAnimator"))
                && truth(node.attr("autoStart"))
                && node.children.len() > 1
            {
                let interval_ms = parse_i32(node.attr("flipInterval")).max(16) as u64;
                flip_interval_ns =
                    Some(flip_interval_ns.map_or(interval_ms * 1_000_000, |current| {
                        current.min(interval_ms * 1_000_000)
                    }));
            }
        }
        if let Some(interval_ns) = flip_interval_ns {
            if self.flip_elapsed_ns >= interval_ns {
                self.flip_elapsed_ns %= interval_ns;
                if let Some(idx) = self.nodes.iter().position(|node| {
                    (node.is("ViewFlipper") || node.is("ViewAnimator"))
                        && truth(node.attr("autoStart"))
                        && node.children.len() > 1
                }) {
                    let count = self.nodes[idx].children.len() as i32;
                    let next =
                        (parse_i32(self.nodes[idx].attr("displayedChild")).max(0) + 1) % count;
                    set_attr(&mut self.nodes[idx], "displayedChild", &next.to_string());
                    self.hovered = None;
                    self.pressed = None;
                    self.dirty = true;
                    changed = true;
                }
            }
        }

        if let Some(until) = self.wait_until_ns {
            if now_ns >= until {
                self.wait_until_ns = None;
                if !self.pending.is_empty() {
                    let pending = core::mem::take(&mut self.pending);
                    self.execute(&pending);
                    changed = true;
                }
            }
        }
        let mut controls_animating = false;
        self.control_animations.retain(|animation| {
            let active = now_ns.saturating_sub(animation.started_ns) < animation.duration_ns;
            controls_animating |= active;
            active
        });
        if controls_animating {
            self.dirty = true;
            changed = true;
        }
        // Keep a focused text field alive while its native caret blinks.  The
        // caret is painted independently from the field's text value.
        if self.focused.is_some() {
            self.dirty = true;
            changed = true;
        }
        if let Some(fade) = self.spinner_fade {
            if now_ns.saturating_sub(fade.started_ns) >= 140_000_000 {
                self.spinner_fade = None;
                // The last translucent frame must be invalidated as well so
                // the underlying content is painted back over the popup.
                self.dirty = true;
                changed = true;
            } else {
                self.dirty = true;
                changed = true;
            }
        }
        changed
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
        // Rebuilding the native view tree must not reset a running app. Keep
        // script variables and user-editable view values across navigation or
        // any other screen reload.
        let previous_state = self.state.clone();
        let mut previous_attrs: Vec<(String, String, String)> = Vec::new();
        for node in &self.nodes {
            let id = node.id();
            if id.is_empty() {
                continue;
            }
            for key in [
                "text",
                "checked",
                "progress",
                "rating",
                "selectedIndex",
                "value",
                "visibility",
                "enabled",
            ] {
                let value = node.attr(key);
                if !value.is_empty() {
                    previous_attrs.push((id.to_string(), key.into(), value.into()));
                }
            }
        }

        self.nodes.clear();
        self.roots.clear();
        self.fixed_subtree.clear();
        self.focused = None;
        self.hovered = None;
        self.pressed = None;
        self.spinner_open = None;
        self.spinner_fade = None;
        self.control_animations.clear();
        self.flip_elapsed_ns = 0;
        self.last_tick_ns = None;
        self.wait_until_ns = None;
        self.pending.clear();
        let mut layout = self.archive.read_text(&format!("{}.w4u", self.screen));
        if layout.is_empty() {
            layout = self.archive.read_text(&format!("{}.w3u", self.screen));
        }
        // A few early Warp 4 examples were distributed as plain XML instead
        // of using the `.w4u` member name. They are the same native view-tree
        // format; accepting the suffix here keeps those archives native while
        // avoiding an HTML conversion path.
        if layout.is_empty() {
            layout = self.archive.read_text(&format!("{}.xml", self.screen));
        }
        if layout.is_empty() && self.screen != "main" {
            layout = self.archive.read_text("main.xml");
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
        // Init provides defaults on the first load. When this is a reload,
        // restore the values owned by the already-running application.
        for (key, value) in previous_state {
            self.set_state(&key, &value);
        }
        for (id, key, value) in previous_attrs {
            if let Some(idx) = self.find(&id) {
                set_attr(&mut self.nodes[idx], &key, &value);
            }
        }
        let chrome_mode = self.document_has_scroll();
        self.rebuild_fixed_subtree(chrome_mode);
        self.refresh_visibility();
        self.dirty = true;
    }

    fn rebuild_fixed_subtree(&mut self, chrome_mode: bool) {
        self.fixed_subtree.clear();
        self.fixed_subtree.resize(self.nodes.len(), false);
        let roots = self.roots.clone();
        for root in roots {
            self.mark_fixed_subtree(root, chrome_mode);
        }
    }

    fn mark_fixed_subtree(&mut self, idx: usize, chrome_mode: bool) -> bool {
        let mut has_fixed = self.node_is_fixed(idx, chrome_mode);
        let children = self.nodes[idx].children.clone();
        for child in children {
            has_fixed |= self.mark_fixed_subtree(child, chrome_mode);
        }
        self.fixed_subtree[idx] = has_fixed;
        has_fixed
    }

    pub fn update(&mut self, width: i32, height: i32) {
        self.width = width.max(1);
        self.height = height.max(1);
        if !self.dirty {
            return;
        }
        self.refresh_visibility();
        let roots = self.roots.clone();
        // `height` is the viewport below the window title bar.  Coordinates
        // remain full-layer coordinates so the compositor can apply its
        // window scroll offset without a second layout coordinate system.
        // The generated HTML has no implicit document margin, so the native
        // root starts at the content edge and uses the complete width.
        let mut y = title_bar_h();
        let usable = self.height;
        for root in roots {
            let forced = if is_match_parent(self.nodes[root].attr("layout_height")) {
                Some(usable)
            } else {
                None
            };
            let h = self.layout(root, 0, y, self.width, forced, "");
            y += h;
        }
        let mut internal_overflow = 0;
        for idx in 0..self.nodes.len() {
            if is_scroll_container(&self.nodes[idx]) {
                internal_overflow = internal_overflow
                    .max(self.nodes[idx].content_h.saturating_sub(self.nodes[idx].h));
            }
        }
        self.content_height = (y + internal_overflow).max(self.height + title_bar_h());
        self.scroll = self.scroll.min(
            self.content_height
                .saturating_sub(self.height + title_bar_h())
                .max(0),
        );
        self.dirty = false;
    }

    pub fn draw_to_layer(&mut self, layer: &mut LayerSystem, ox: i32, oy: i32) {
        if self.dirty {
            self.update(layer.width() as i32, layer.height() as i32 - title_bar_h());
        }
        layer.fill_rect(
            0,
            title_bar_h() as usize,
            layer.width(),
            layer.height().saturating_sub(title_bar_h() as usize),
            bg(),
        );
        let chrome_mode = self.document_has_scroll();
        for &root in &self.roots {
            // The compositor supplies the window-manager offset.  The view
            // tree gets two passes: normal content first, then fixed/sticky
            // chrome.  This is what the reference CSS achieves with a
            // viewport and `position:fixed`, without ever creating HTML.
            self.paint(layer, root, ox, oy, false, chrome_mode, PaintPass::Flow);
            if self.fixed_subtree.get(root).copied().unwrap_or(true) {
                self.paint(layer, root, ox, oy, false, chrome_mode, PaintPass::Fixed);
            }
        }
        if let Some(idx) = self.spinner_open {
            self.paint_spinner_popup(layer, idx, 255);
        } else if let Some(fade) = self.spinner_fade {
            let elapsed = self.now_ns.saturating_sub(fade.started_ns);
            let remaining = 140_000_000u64.saturating_sub(elapsed);
            let opacity = (remaining.saturating_mul(255) / 140_000_000) as u8;
            self.paint_spinner_popup(layer, fade.idx, opacity);
        }
    }

    pub fn set_hover(&mut self, x: i32, y: i32) {
        let popup_hit = self.spinner_popup_hit(x, y);
        let next = popup_hit.map(|(idx, _)| idx).or_else(|| self.hit(x, y));
        if self.hovered != next {
            self.hovered = next;
            self.dirty = true;
        }
    }

    fn close_spinner(&mut self, idx: usize) {
        self.spinner_open = None;
        self.spinner_fade = Some(SpinnerFade {
            idx,
            started_ns: self.now_ns,
        });
        self.dirty = true;
    }

    fn start_control_animation(&mut self, idx: usize, from_on: bool, to_on: bool) {
        if from_on == to_on {
            return;
        }
        self.control_animations
            .retain(|animation| animation.idx != idx);
        self.control_animations.push(ControlAnimation {
            idx,
            to_on,
            started_ns: self.now_ns,
            duration_ns: if self.nodes[idx].is("Switch") {
                SWITCH_DURATION_NS
            } else {
                RADIO_DURATION_NS
            },
        });
        self.dirty = true;
    }

    fn control_amount(&self, idx: usize, on: bool) -> f32 {
        let Some(animation) = self
            .control_animations
            .iter()
            .rev()
            .find(|animation| animation.idx == idx)
        else {
            return if on { 1.0 } else { 0.0 };
        };
        let t = (self.now_ns.saturating_sub(animation.started_ns) as f32
            / animation.duration_ns.max(1) as f32)
            .clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        if animation.to_on {
            eased
        } else {
            1.0 - eased
        }
    }

    pub fn click(&mut self, x: i32, y: i32) {
        if let Some(open_idx) = self.spinner_open {
            if let Some((idx, item)) = self.spinner_popup_hit(x, y) {
                set_attr(&mut self.nodes[idx], "selectedIndex", &item.to_string());
                self.close_spinner(idx);
                self.pressed = Some(idx);
                self.focused = None;
                self.run_click_actions(idx);
                self.dirty = true;
                return;
            }
            // A native dropdown dismisses when the pointer lands outside its
            // menu. Do not leak that same click into a control underneath.
            if self.hit(x, y) != Some(open_idx) {
                self.close_spinner(open_idx);
                self.pressed = None;
                self.dirty = true;
                return;
            }
            self.close_spinner(open_idx);
            self.pressed = Some(open_idx);
            self.dirty = true;
            return;
        }
        let Some(idx) = self.hit(x, y) else {
            self.focused = None;
            self.pressed = None;
            self.dirty = true;
            return;
        };
        self.pressed = Some(idx);
        self.last_clicked_id = Some(self.nodes[idx].id().to_string());
        if self.nodes[idx].is("EditText")
            || self.nodes[idx].is("AutoCompleteTextView")
            || self.nodes[idx].is("MultiAutoCompleteTextView")
        {
            self.focused = Some(idx);
            self.dirty = true;
            return;
        }
        // Clicking another control follows normal native focus semantics: an
        // EditText loses focus as soon as a different view is activated.
        self.focused = None;
        if self.nodes[idx].is("Switch")
            || self.nodes[idx].is("CheckBox")
            || self.nodes[idx].is("RadioButton")
            || self.nodes[idx].is("ToggleButton")
        {
            if self.nodes[idx].is("RadioButton") {
                if let Some(parent) = self.nodes[idx].parent {
                    for sibling in self.nodes[parent].children.clone() {
                        if self.nodes[sibling].is("RadioButton") {
                            let old = self.nodes[sibling].attr("checked") == "true";
                            let next = sibling == idx;
                            self.start_control_animation(sibling, old, next);
                            set_attr(
                                &mut self.nodes[sibling],
                                "checked",
                                if next { "true" } else { "false" },
                            );
                        }
                    }
                } else {
                    let old = self.nodes[idx].attr("checked") == "true";
                    self.start_control_animation(idx, old, true);
                    set_attr(&mut self.nodes[idx], "checked", "true");
                }
            } else {
                let old = self.nodes[idx].attr("checked") == "true";
                let value = !old;
                if self.nodes[idx].is("Switch") {
                    self.start_control_animation(idx, old, value);
                }
                set_attr(
                    &mut self.nodes[idx],
                    "checked",
                    if value { "true" } else { "false" },
                );
            }
        } else if self.nodes[idx].is("SeekBar") {
            self.set_seek_progress(idx, x);
        } else if self.nodes[idx].is("RatingBar") {
            self.set_rating(idx, x);
        } else if self.nodes[idx].is("Spinner") {
            self.spinner_open = Some(idx);
            self.spinner_fade = None;
        }
        self.run_click_actions(idx);
        self.dirty = true;
    }

    pub fn take_clicked_id(&mut self) -> Option<String> {
        self.last_clicked_id.take()
    }

    pub fn set_text(&mut self, id: &str, text: &str) {
        if let Some(idx) = self.find(id) {
            set_attr(&mut self.nodes[idx], "text", text);
            self.dirty = true;
        }
    }

    /// Update a pointer-controlled widget while the primary pointer is held.
    /// The caller keeps the window drag state separate from this capture, just
    /// like the browser range/rating controls do.
    pub fn pointer_move(&mut self, x: i32, y: i32) -> bool {
        let Some(idx) = self.pressed else {
            return false;
        };
        if self.nodes.get(idx).map_or(true, |n| {
            !n.visible() || (!n.is("SeekBar") && !n.is("RatingBar"))
        }) {
            return false;
        }
        let changed = if self.nodes[idx].is("SeekBar") {
            self.set_seek_progress(idx, x)
        } else {
            self.set_rating(idx, x)
        };
        if changed {
            self.dirty = true;
        }
        let _ = y;
        true
    }

    pub fn has_pointer_capture(&self) -> bool {
        self.pressed.is_some_and(|idx| {
            self.nodes
                .get(idx)
                .is_some_and(|n| n.is("SeekBar") || n.is("RatingBar"))
        })
    }

    pub fn release(&mut self) {
        if self.pressed.take().is_some() {
            self.dirty = true;
        }
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
            n.visible()
                && self.active_child(*idx)
                && (interactive(n) || self.script.clicks.iter().any(|(id, _)| id == n.id()))
                && self.hit_visible(*idx, x, y)
        })
    }

    fn hit_visible(&self, idx: usize, x: i32, y: i32) -> bool {
        // The compositor gives us document coordinates (`y` already includes
        // the window scroll). Convert once to the viewport coordinate used by
        // paint, then test the control itself.
        let screen_y = y - self.scroll;
        let node = &self.nodes[idx];
        let node_y = self.node_screen_y(idx);
        if x < node.x || screen_y < node_y || x >= node.x + node.w || screen_y >= node_y + node.h {
            return false;
        }

        // Ordinary layout parents do not clip in the native painter. Only a
        // real scroll viewport is a hit-test boundary. Checking every parent
        // here makes a fixed child of a scrolled root appear unclickable once
        // the root's own document rectangle has moved off-screen.
        let mut current = node.parent;
        while let Some(i) = current {
            let viewport = &self.nodes[i];
            if is_scroll_container(viewport) {
                let viewport_y = self.node_screen_y(i);
                if x < viewport.x
                    || screen_y < viewport_y
                    || x >= viewport.x + viewport.w
                    || screen_y >= viewport_y + viewport.h
                {
                    return false;
                }
            }
            current = viewport.parent;
        }
        true
    }

    fn node_is_fixed(&self, idx: usize, chrome_mode: bool) -> bool {
        let node = &self.nodes[idx];
        let in_scroll = self.ancestor_is_scroll(idx);
        let is_document_root = self.roots.contains(&idx);
        is_fixed(node)
            || (!in_scroll && chrome_mode && (!is_document_root || is_scroll_container(node)))
    }

    fn node_screen_y(&self, idx: usize) -> i32 {
        let chrome_mode = self.document_has_scroll();
        self.nodes[idx].y
            + if self.node_is_fixed(idx, chrome_mode) {
                0
            } else {
                -self.scroll
            }
    }

    fn ancestor_is_scroll(&self, idx: usize) -> bool {
        let mut current = self.nodes[idx].parent;
        while let Some(i) = current {
            if is_scroll_container(&self.nodes[i]) {
                return true;
            }
            current = self.nodes[i].parent;
        }
        false
    }

    fn set_seek_progress(&mut self, idx: usize, x: i32) -> bool {
        let n = &self.nodes[idx];
        let max = parse_i32(n.attr("max")).max(1);
        let next = ((x - n.x).clamp(0, n.w.max(1)) * max / n.w.max(1)).clamp(0, max);
        let current = parse_i32(n.attr("progress")).clamp(0, max);
        if current == next {
            return false;
        }
        set_attr(&mut self.nodes[idx], "progress", &next.to_string());
        true
    }

    fn set_rating(&mut self, idx: usize, x: i32) -> bool {
        let n = &self.nodes[idx];
        let stars = parse_i32(n.attr("numStars")).clamp(1, 10);
        let step = n.attr("stepSize").parse::<f32>().unwrap_or(1.0).max(0.1);
        let raw =
            ((x - n.x).max(0) as f32 / n.w.max(1) as f32 * stars as f32).clamp(0.0, stars as f32);
        let next = ((raw / step + 0.5) as i32).max(0) as f32 * step;
        let integral = (next + 0.5) as i32;
        let value = if (next - integral as f32).abs() < 0.001 {
            integral.to_string()
        } else {
            next.to_string()
        };
        if n.attr("rating") == value {
            return false;
        }
        set_attr(&mut self.nodes[idx], "rating", &value);
        true
    }

    fn run_click_actions(&mut self, idx: usize) {
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
    }

    fn spinner_item_count(&self, idx: usize) -> usize {
        let count = self.nodes[idx]
            .attr("items")
            .split(',')
            .filter(|item| !item.trim().is_empty())
            .count();
        count.max(3)
    }

    fn spinner_popup_rect(&self, idx: usize) -> (i32, i32, i32, i32) {
        let node = &self.nodes[idx];
        let row_h = 36;
        let h = self.spinner_item_count(idx) as i32 * row_h + 8;
        let bottom = self.node_screen_y(idx) + node.h;
        let layer_h = self.height + title_bar_h();
        let y = if bottom + h <= layer_h {
            bottom
        } else {
            (self.node_screen_y(idx) - h).max(title_bar_h())
        };
        (node.x, y, node.w.max(1), h)
    }

    fn spinner_popup_hit(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        let idx = self.spinner_open?;
        let node = self.nodes.get(idx)?;
        if !node.visible() || !node.is("Spinner") {
            return None;
        }
        let (_, popup_y, popup_w, popup_h) = self.spinner_popup_rect(idx);
        let screen_y = y - self.scroll;
        if x < node.x
            || x >= node.x + popup_w
            || screen_y < popup_y
            || screen_y >= popup_y + popup_h
        {
            return None;
        }
        let local_y = screen_y - popup_y - 4;
        if local_y < 0 {
            return None;
        }
        let item = (local_y / 36) as usize;
        (item < self.spinner_item_count(idx)).then_some((idx, item))
    }

    fn execute(&mut self, actions: &[Action]) {
        for (pos, action) in actions.iter().take(MAX_ACTIONS).enumerate() {
            if self.wait_until_ns.is_some() {
                self.pending.extend_from_slice(&actions[pos..]);
                return;
            }
            if self.break_requested {
                return;
            }
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
                Action::Break => {
                    self.break_requested = true;
                    return;
                }
            }
        }
    }

    fn command(&mut self, name: &str, target: &str, raw: &str) {
        if matches!(name, "var.edit" | "var.set") && raw.trim().starts_with("append ") {
            let suffix = self.value(raw.trim().strip_prefix("append ").unwrap_or(""));
            let current = self.state(target);
            self.set_state(target, &format!("{current}{suffix}"));
            self.dirty = true;
            return;
        }
        let value = self.value(raw);
        match name {
            "var.set" | "var.edit" | "const.set" => self.set_state(target, &value),
            "fun" => {
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
            "for" => {
                if let Some((_, body)) = self
                    .script
                    .functions
                    .iter()
                    .find(|(key, _)| key == &value)
                    .cloned()
                {
                    for _ in 0..MAX_ACTIONS {
                        self.break_requested = false;
                        self.execute(&body);
                        if self.wait_until_ns.is_some() {
                            return;
                        }
                        if self.break_requested {
                            self.break_requested = false;
                            break;
                        }
                    }
                }
            }
            "break" => self.break_requested = true,
            "print" => {}
            "wait" => {
                if let Some(duration) = duration_ns(&value) {
                    self.wait_until_ns = Some(self.now_ns.saturating_add(duration));
                }
            }
            "run" => {
                // Keep the Warp 3 `run = os://...` form and also accept the
                // compact `run (os://...)` form in native Warp 4 scripts.
                let command = value.trim().trim_start_matches('=').trim();
                if !command.is_empty() {
                    self.last_command = Some(command.into());
                }
            }
            "BaramOS.get" => {
                let path = value
                    .trim()
                    .strip_prefix("os://")
                    .unwrap_or(value.trim())
                    .trim_start_matches("--");
                let current = if let Some(now_path) = path.strip_prefix("now://") {
                    self.now_value(now_path).unwrap_or_default()
                } else {
                    config::get_config().get(path).unwrap_or("").to_string()
                };
                self.set_state(target.trim(), &current);
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
                if let Some(i) = self.find(target) {
                    let key = name.strip_prefix("WarpUI.").unwrap_or(name);
                    set_attr(&mut self.nodes[i], key, &value);
                }
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
        for _ in 0..128 {
            let mut next = String::new();
            let mut changed = false;
            let chars: Vec<char> = s.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                let kind = if chars[i..].starts_with(&['v', 'a', 'r', '[']) {
                    Some(("var", 4))
                } else if chars[i..].starts_with(&['c', 'o', 'n', 's', 't', '[']) {
                    Some(("const", 6))
                } else if chars[i..].starts_with(&['c', 'a', 'l', 'c', '[']) {
                    Some(("calc", 5))
                } else {
                    None
                };
                let Some((kind, open)) = kind else {
                    next.push(chars[i]);
                    i += 1;
                    continue;
                };
                let mut depth = 1i32;
                let mut end = i + open;
                while end < chars.len() {
                    if chars[end] == '[' {
                        depth += 1;
                    }
                    if chars[end] == ']' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    end += 1;
                }
                if end >= chars.len() {
                    next.push(chars[i]);
                    i += 1;
                    continue;
                }
                let inner: String = chars[i + open..end].iter().collect();
                let expanded = self.value(&inner);
                let replacement = if kind == "calc" {
                    eval_calc(&expanded)
                } else {
                    self.state(expanded.trim())
                };
                if kind == "calc" || !replacement.is_empty() || self.state_contains(expanded.trim())
                {
                    next.push_str(&replacement);
                    changed = true;
                } else {
                    next.extend(chars[i..=end].iter());
                }
                i = end + 1;
            }
            if !changed || next == s {
                break;
            }
            s = next;
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

    fn now_value(&self, path: &str) -> Option<String> {
        let time = runtime::get_time().ok()?;
        let timezone_minutes = config::timezone_offset_minutes();
        let utc_seconds =
            time.hour() as i32 * 3600 + time.minute() as i32 * 60 + time.second() as i32;
        let local_seconds = (utc_seconds + timezone_minutes * 60).rem_euclid(24 * 3600);
        let hour = (local_seconds / 3600) as u8;
        let minute = ((local_seconds / 60) % 60) as u8;
        let second = (local_seconds % 60) as u8;
        Some(match path.trim_matches('/') {
            "fps" => self.runtime_fps.to_string(),
            "window" | "windows" => self.runtime_windows.to_string(),
            "key" | "keys" => self.runtime_keys.to_string(),
            "mouse" => self.runtime_mouse.to_string(),
            "h" => hour.to_string(),
            "m" => minute.to_string(),
            "s" => second.to_string(),
            "hh" => format!("{hour:02}"),
            "mm" => format!("{minute:02}"),
            "ss" => format!("{second:02}"),
            "hhmm" => format!("{hour:02}:{minute:02}"),
            "hhmmss" => format!("{hour:02}:{minute:02}:{second:02}"),
            _ => return None,
        })
    }

    fn state_contains(&self, key: &str) -> bool {
        self.state.iter().any(|(name, _)| name == key)
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
            && is_zero_dimension(width_attr);
        let w = if weighted_width {
            available_w
        } else {
            dimension(width_attr, available_w, self.intrinsic_w(idx, available_w))
        }
        .max(1);
        let tag = self.nodes[idx].tag.clone();
        let own_h = forced_h.or_else(|| {
            let raw = self.nodes[idx].attr("layout_height");
            if is_match_parent(raw) {
                Some(self.height.max(1))
            } else if raw != "wrap_content" && !raw.is_empty() {
                Some(parse_dim(raw, 0))
            } else {
                None
            }
        });
        self.nodes[idx].x = x + margin.left;
        self.nodes[idx].y = y + margin.top;
        self.nodes[idx].w = (w - margin.left - margin.right).max(1);
        self.nodes[idx].content_w = (self.nodes[idx].w - pad.left - pad.right).max(1);
        if tag == "LinearLayout" || tag == "RadioGroup" {
            let horizontal = self.nodes[idx].attr("orientation") == "horizontal";
            let inner_w = (self.nodes[idx].w - pad.left - pad.right).max(1);
            let inner_x = self.nodes[idx].x + pad.left;
            let inner_y = self.nodes[idx].y + pad.top;
            let children = self.nodes[idx].children.clone();
            // Match Warp3's native row/section spacing when an XML layout
            // does not specify a gap explicitly.
            let layout_gap = parse_dim(self.nodes[idx].attr("layout_gap"), 8).max(0);
            let visible_children = children
                .iter()
                .filter(|child| self.nodes[**child].visible())
                .count();
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
            fixed += layout_gap * visible_children.saturating_sub(1) as i32;
            let inner_h = own_h.unwrap_or_else(|| {
                if horizontal {
                    self.intrinsic_h(idx, available_w)
                } else {
                    fixed + pad.top + pad.bottom
                }
            });
            self.nodes[idx].h = inner_h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
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
                    let child_h = if is_match_parent(self.nodes[child].attr("layout_height")) {
                        Some(self.nodes[idx].content_h)
                    } else {
                        None
                    };
                    self.layout(child, cursor, inner_y, allocated.max(1), child_h, &tag);
                    let child_h = self.nodes[child].h;
                    let cross = cross_offset(
                        self.nodes[child].attr("layout_gravity"),
                        self.nodes[idx].h - pad.top - pad.bottom,
                        child_h + e.top + e.bottom,
                        true,
                    );
                    self.nodes[child].y = inner_y + cross + e.top;
                    cursor += self.nodes[child].w + e.left + e.right + layout_gap;
                } else {
                    self.layout(
                        child,
                        inner_x,
                        cursor,
                        inner_w,
                        Some(allocated.max(1)),
                        &tag,
                    );
                    let cross = cross_offset(
                        self.nodes[child].attr("layout_gravity"),
                        inner_w,
                        self.nodes[child].w + e.left + e.right,
                        false,
                    );
                    self.nodes[child].x = inner_x + cross + e.left;
                    cursor += self.nodes[child].h + e.top + e.bottom + layout_gap;
                }
            }
        } else if tag == "ScrollView" || tag == "HorizontalScrollView" {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            let inner_w = (self.nodes[idx].w - pad.left - pad.right).max(1);
            let inner_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let child = self.nodes[idx]
                .children
                .iter()
                .copied()
                .find(|c| self.nodes[*c].visible());
            let mut content_w = inner_w;
            let mut content_h = inner_h;
            if let Some(child) = child {
                let fill = self.nodes[idx].attr("fillViewport") == "true";
                let forced = if fill && tag == "ScrollView" {
                    Some(inner_h)
                } else {
                    None
                };
                self.layout(
                    child,
                    self.nodes[idx].x + pad.left,
                    self.nodes[idx].y + pad.top,
                    if tag == "HorizontalScrollView" {
                        self.intrinsic_w(child, inner_w).max(inner_w)
                    } else {
                        inner_w
                    },
                    forced,
                    &tag,
                );
                content_w = if tag == "HorizontalScrollView" {
                    (self.subtree_right(child) - self.nodes[idx].x + pad.right).max(inner_w)
                } else {
                    inner_w
                };
                content_h = if tag == "ScrollView" {
                    (self.subtree_bottom(child) - self.nodes[idx].y + pad.bottom).max(inner_h)
                } else {
                    inner_h
                };
            }
            self.nodes[idx].content_w = content_w;
            self.nodes[idx].content_h = content_h;
        } else if tag == "RelativeLayout" {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            self.layout_relative(idx, pad);
        } else if tag == "FrameLayout"
            || tag == "ViewFlipper"
            || tag == "ViewAnimator"
            || tag == "ViewSwitcher"
            || tag == "TextSwitcher"
        {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let children = self.nodes[idx].children.clone();
            let active = if tag == "ViewFlipper"
                || tag == "ViewAnimator"
                || tag == "ViewSwitcher"
                || tag == "TextSwitcher"
            {
                parse_i32(self.nodes[idx].attr("displayedChild")).max(0) as usize
            } else {
                usize::MAX
            };
            for (pos, child) in children.into_iter().enumerate() {
                if !self.nodes[child].visible() || pos != active && active != usize::MAX {
                    continue;
                }
                let child_w = dimension(
                    self.nodes[child].attr("layout_width"),
                    self.nodes[idx].content_w,
                    self.intrinsic_w(child, self.nodes[idx].content_w),
                );
                let child_h = dimension(
                    self.nodes[child].attr("layout_height"),
                    self.nodes[idx].content_h,
                    self.intrinsic_h(child, self.nodes[idx].content_w),
                );
                self.layout(
                    child,
                    self.nodes[idx].x + pad.left,
                    self.nodes[idx].y + pad.top,
                    child_w.max(1),
                    Some(child_h.max(1)),
                    &tag,
                );
                let e = edges(&self.nodes[child], "layout_margin");
                let (dx, dy) = gravity_offset(
                    self.nodes[child].attr("layout_gravity"),
                    self.nodes[idx].content_w,
                    self.nodes[idx].content_h,
                    self.nodes[child].w + e.left + e.right,
                    self.nodes[child].h + e.top + e.bottom,
                );
                self.nodes[child].x = self.nodes[idx].x + pad.left + dx + e.left;
                self.nodes[child].y = self.nodes[idx].y + pad.top + dy + e.top;
            }
        } else if tag == "AbsoluteLayout" {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let children = self.nodes[idx].children.clone();
            for child in children {
                if !self.nodes[child].visible() {
                    continue;
                }
                let child_w = dimension(
                    self.nodes[child].attr("layout_width"),
                    self.nodes[idx].content_w,
                    self.intrinsic_w(child, self.nodes[idx].content_w),
                );
                self.layout(
                    child,
                    self.nodes[idx].x + pad.left + parse_dim(self.nodes[child].attr("layout_x"), 0),
                    self.nodes[idx].y + pad.top + parse_dim(self.nodes[child].attr("layout_y"), 0),
                    child_w.max(1),
                    is_match_parent(self.nodes[child].attr("layout_height"))
                        .then_some(self.nodes[idx].content_h),
                    &tag,
                );
            }
        } else if tag == "GridLayout" {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            self.layout_grid(idx, pad);
        } else if tag == "TableLayout" || tag == "TableRow" {
            let horizontal = tag == "TableRow";
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let mut cursor = if horizontal {
                self.nodes[idx].x + pad.left
            } else {
                self.nodes[idx].y + pad.top
            };
            let children = self.nodes[idx].children.clone();
            let table_stretch = self.nodes[idx]
                .parent
                .filter(|parent| self.nodes[*parent].is("TableLayout"))
                .map(|parent| self.nodes[parent].attr("stretchColumns").to_string())
                .unwrap_or_default();
            let stretch_all = table_stretch.contains('*');
            let visible_count = children
                .iter()
                .filter(|c| self.nodes[**c].visible())
                .count()
                .max(1) as i32;
            let mut fixed_width = 0;
            let mut stretch_count = 0;
            for (column, child) in children.iter().enumerate() {
                if !horizontal || !self.nodes[*child].visible() {
                    continue;
                }
                let stretched = stretch_all
                    || table_stretch
                        .split(',')
                        .any(|value| value.trim().parse::<usize>().ok() == Some(column));
                let e = edges(&self.nodes[*child], "layout_margin");
                if stretched {
                    stretch_count += 1;
                } else {
                    fixed_width +=
                        self.intrinsic_w(*child, self.nodes[idx].content_w) + e.left + e.right;
                }
            }
            let stretch_width = (self.nodes[idx].content_w.saturating_sub(fixed_width)
                / stretch_count.max(1))
            .max(1);
            for (column, child) in children.into_iter().enumerate() {
                if !self.nodes[child].visible() {
                    continue;
                }
                let e = edges(&self.nodes[child], "layout_margin");
                if !horizontal
                    && self.nodes[idx].attr("stretchColumns").contains('*')
                    && self.nodes[child].is("TableRow")
                    && self.nodes[child].attr("stretchColumns").is_empty()
                {
                    set_attr(&mut self.nodes[child], "stretchColumns", "*");
                }
                let stretched = horizontal
                    && (stretch_all
                        || table_stretch
                            .split(',')
                            .any(|value| value.trim().parse::<usize>().ok() == Some(column)));
                let allocated = if horizontal && stretched {
                    if stretch_all {
                        (self.nodes[idx].content_w / visible_count).max(1)
                    } else {
                        stretch_width
                    }
                } else if horizontal {
                    self.intrinsic_w(child, self.nodes[idx].content_w)
                } else {
                    self.nodes[idx].content_w
                };
                if horizontal {
                    self.layout(
                        child,
                        cursor,
                        self.nodes[idx].y + pad.top,
                        allocated,
                        None,
                        &tag,
                    );
                    if stretched {
                        self.nodes[child].w = (allocated - e.left - e.right).max(1);
                        self.nodes[child].content_w = (self.nodes[child].w
                            - edges(&self.nodes[child], "padding").left
                            - edges(&self.nodes[child], "padding").right)
                            .max(1);
                    }
                    cursor += allocated + e.left + e.right;
                } else {
                    self.layout(
                        child,
                        self.nodes[idx].x + pad.left,
                        cursor,
                        allocated,
                        None,
                        &tag,
                    );
                    cursor += self.nodes[child].h + e.top + e.bottom;
                }
            }
        } else {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let children = self.nodes[idx].children.clone();
            let mut cy = self.nodes[idx].y + pad.top;
            for child in children {
                if self.nodes[child].visible() {
                    let ch = self.layout(
                        child,
                        self.nodes[idx].x + pad.left,
                        cy,
                        (self.nodes[idx].w - pad.left - pad.right).max(1),
                        is_match_parent(self.nodes[child].attr("layout_height"))
                            .then_some(self.nodes[idx].content_h),
                        &tag,
                    );
                    cy += ch;
                }
            }
        }
        self.nodes[idx].h + margin.top + margin.bottom
    }

    fn layout_relative(&mut self, idx: usize, pad: Edges) {
        let parent_x = self.nodes[idx].x + pad.left;
        let parent_y = self.nodes[idx].y + pad.top;
        let parent_w = self.nodes[idx].content_w;
        let parent_h = self.nodes[idx].content_h;
        let children = self.nodes[idx].children.clone();
        for child in &children {
            if !self.nodes[*child].visible() {
                continue;
            }
            let cw = dimension(
                self.nodes[*child].attr("layout_width"),
                parent_w,
                self.intrinsic_w(*child, parent_w),
            );
            let ch = dimension(
                self.nodes[*child].attr("layout_height"),
                parent_h,
                self.intrinsic_h(*child, parent_w),
            );
            self.layout(
                *child,
                parent_x,
                parent_y,
                cw.max(1),
                Some(ch.max(1)),
                "RelativeLayout",
            );
        }
        for &child in &children {
            if !self.nodes[child].visible() {
                continue;
            }
            let n = self.nodes[child].clone();
            let e = edges(&n, "layout_margin");
            let mut x = e.left;
            let mut y = e.top;
            if truth(n.attr("layout_alignParentRight")) || truth(n.attr("layout_alignParentEnd")) {
                x = parent_w - n.w - e.right;
            }
            if truth(n.attr("layout_centerHorizontal")) || truth(n.attr("layout_centerInParent")) {
                x = (parent_w - n.w) / 2;
            }
            if truth(n.attr("layout_alignParentBottom")) {
                y = parent_h - n.h - e.bottom;
            }
            if truth(n.attr("layout_centerVertical")) || truth(n.attr("layout_centerInParent")) {
                y = (parent_h - n.h) / 2;
            }
            let sibling = |key: &str, nodes: &Vec<Node>, children: &Vec<usize>| -> Option<Node> {
                let id = nodes[child].attr(key);
                if id.is_empty() {
                    return None;
                }
                let id = id.trim_start_matches("@+id/").trim_start_matches("@id/");
                children
                    .iter()
                    .find_map(|other| (nodes[*other].id() == id).then(|| nodes[*other].clone()))
            };
            if let Some(ref q) = sibling("layout_below", &self.nodes, &children) {
                y = q.y - parent_y + q.h + e.top;
            }
            if let Some(ref q) = sibling("layout_above", &self.nodes, &children) {
                y = q.y - parent_y - n.h - e.bottom;
            }
            if let Some(ref q) = sibling("layout_toRightOf", &self.nodes, &children)
                .or_else(|| sibling("layout_toEndOf", &self.nodes, &children))
            {
                x = q.x - parent_x + q.w + e.left;
            }
            if let Some(ref q) = sibling("layout_toLeftOf", &self.nodes, &children)
                .or_else(|| sibling("layout_toStartOf", &self.nodes, &children))
            {
                x = q.x - parent_x - n.w - e.right;
            }
            if let Some(ref q) = sibling("layout_alignLeft", &self.nodes, &children)
                .or_else(|| sibling("layout_alignStart", &self.nodes, &children))
            {
                x = q.x - parent_x + e.left;
            }
            if let Some(ref q) = sibling("layout_alignRight", &self.nodes, &children)
                .or_else(|| sibling("layout_alignEnd", &self.nodes, &children))
            {
                x = q.x - parent_x + q.w - n.w - e.right;
            }
            if let Some(ref q) = sibling("layout_alignTop", &self.nodes, &children) {
                y = q.y - parent_y + e.top;
            }
            if let Some(ref q) = sibling("layout_alignBottom", &self.nodes, &children) {
                y = q.y - parent_y + q.h - n.h - e.bottom;
            }
            self.nodes[child].x = (parent_x + x).max(parent_x);
            self.nodes[child].y = (parent_y + y).max(parent_y);
        }
    }

    fn layout_grid(&mut self, idx: usize, pad: Edges) {
        let columns = parse_i32(self.nodes[idx].attr("columnCount")).max(1);
        let gap = 8;
        let cell_w = ((self.nodes[idx].content_w - gap * (columns - 1)) / columns).max(1);
        let mut row_y = self.nodes[idx].y + pad.top;
        let mut row_h = 0;
        let mut column = 0;
        for child in self.nodes[idx].children.clone() {
            if !self.nodes[child].visible() {
                continue;
            }
            let explicit_col = self.nodes[child].attr("layout_column");
            if !explicit_col.is_empty() {
                column = parse_i32(explicit_col).max(0);
            }
            if column >= columns {
                row_y += row_h + gap;
                row_h = 0;
                column = 0;
            }
            let span = parse_i32(self.nodes[child].attr("layout_columnSpan"))
                .max(1)
                .min(columns - column);
            let allocated = cell_w * span + gap * (span - 1);
            self.layout(
                child,
                self.nodes[idx].x + pad.left + column * (cell_w + gap),
                row_y,
                allocated,
                None,
                "GridLayout",
            );
            row_h = row_h.max(self.nodes[child].h);
            column += span;
            if column >= columns {
                row_y += row_h + gap;
                row_h = 0;
                column = 0;
            }
        }
    }

    fn subtree_bottom(&self, idx: usize) -> i32 {
        self.nodes[idx]
            .children
            .iter()
            .fold(self.nodes[idx].y + self.nodes[idx].h, |bottom, child| {
                bottom.max(self.subtree_bottom(*child))
            })
    }
    fn subtree_right(&self, idx: usize) -> i32 {
        self.nodes[idx]
            .children
            .iter()
            .fold(self.nodes[idx].x + self.nodes[idx].w, |right, child| {
                right.max(self.subtree_right(*child))
            })
    }

    fn intrinsic_w(&self, idx: usize, available: i32) -> i32 {
        let n = &self.nodes[idx];
        let raw_width = n.attr("layout_width");
        if !raw_width.is_empty()
            && !is_match_parent(raw_width)
            && raw_width != "wrap_content"
            && !is_zero_dimension(raw_width)
        {
            return parse_dim(raw_width, available).max(0);
        }
        if n.is("Space") {
            return parse_dim(n.attr("layout_width"), 0).max(0);
        }
        if !n.attr("text").is_empty() {
            let pad = edges(n, "padding");
            let control_width = if n.is("Switch") {
                55
            } else if interactive(n) {
                32
            } else {
                0
            };
            return measure_size(n.attr("text"), text_size(n))
                + pad.left
                + pad.right
                + control_width;
        }
        if is_button_like(n) {
            return (measure_size(
                if n.is("ToggleButton") {
                    n.attr("textOn")
                } else {
                    n.attr("text")
                },
                text_size(n),
            ) + 32)
                .max(64)
                .min(available.max(64));
        }
        if n.is("RatingBar") {
            let stars = parse_i32(n.attr("numStars")).clamp(1, 10);
            return stars * 22 + (stars - 1).max(0);
        }
        if n.is("Switch") {
            return if n.attr("text").is_empty() {
                44
            } else {
                55 + measure_size(n.attr("text"), text_size(n))
            };
        }
        if n.is("EditText") || n.is("AutoCompleteTextView") || n.is("MultiAutoCompleteTextView") {
            return available.min(240).max(80);
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
        if n.is("RelativeLayout")
            || n.is("FrameLayout")
            || n.is("AbsoluteLayout")
            || n.is("GridLayout")
        {
            return available;
        }
        available
    }
    fn intrinsic_h(&self, idx: usize, available: i32) -> i32 {
        let n = &self.nodes[idx];
        let raw_height = n.attr("layout_height");
        if !raw_height.is_empty()
            && !is_match_parent(raw_height)
            && raw_height != "wrap_content"
            && !is_zero_dimension(raw_height)
        {
            return parse_dim(raw_height, available).max(0);
        }
        if n.is("Space") {
            return parse_dim(n.attr("layout_height"), 0).max(0);
        }
        if is_button_like(n) {
            return 48;
        }
        if n.is("EditText") || n.is("AutoCompleteTextView") {
            return 38;
        }
        if n.is("MultiAutoCompleteTextView") {
            return 58;
        }
        if n.is("Switch") || n.is("CheckBox") || n.is("RadioButton") {
            return 44;
        }
        if n.is("SeekBar") {
            return 30;
        }
        if n.is("RatingBar") {
            return 32;
        }
        if n.is("Spinner") || n.is("SearchView") || n.is("DatePicker") || n.is("TimePicker") {
            return 38;
        }
        if n.is("ProgressBar") {
            return if n.attr("style").contains("progressBarStyleHorizontal") {
                6
            } else {
                28
            };
        }
        if n.is("TextView") {
            let pad = edges(n, "padding");
            let size = text_size(n);
            let line = (size * 1.25) as i32;
            let chars_per_line =
                (available.max(1) / (size.max(8.0) as i32 / 2).max(4)).max(1) as usize;
            let lines = n
                .attr("text")
                .split('\n')
                .map(|s| (s.chars().count().max(1) + chars_per_line - 1) / chars_per_line)
                .sum::<usize>()
                .max(1);
            return pad.top + pad.bottom + line * lines as i32;
        }
        let pad = edges(n, "padding");
        let child_h = if (n.is("LinearLayout") || n.is("RadioGroup") || n.is("TableRow"))
            && (n.attr("orientation") == "horizontal" || n.is("TableRow"))
        {
            n.children
                .iter()
                .filter(|c| self.nodes[**c].visible())
                .map(|c| self.intrinsic_h(*c, available))
                .max()
                .unwrap_or(0)
        } else if n.is("GridLayout") {
            let columns = parse_i32(n.attr("columnCount")).max(1) as usize;
            let mut row_h = 0;
            let mut rows = 0usize;
            let mut column = 0usize;
            for child in n.children.iter().filter(|c| self.nodes[**c].visible()) {
                let span = parse_i32(self.nodes[*child].attr("layout_columnSpan")).max(1) as usize;
                row_h = row_h.max(self.intrinsic_h(*child, available));
                column += span;
                if column >= columns {
                    rows += 1;
                    column = 0;
                    row_h = 0;
                }
            }
            if column > 0 {
                rows += 1;
            }
            let pad = edges(n, "padding");
            return (pad.top + pad.bottom + rows as i32 * 48 + rows.saturating_sub(1) as i32 * 8)
                .max(1);
        } else if n.is("FrameLayout")
            || n.is("RelativeLayout")
            || n.is("AbsoluteLayout")
            || n.is("GridLayout")
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
                .map(|c| {
                    self.intrinsic_h(*c, available)
                        + edges(&self.nodes[*c], "layout_margin").top
                        + edges(&self.nodes[*c], "layout_margin").bottom
                })
                .sum()
        };
        (pad.top + pad.bottom + child_h).max(1)
    }

    fn paint(
        &self,
        layer: &mut LayerSystem,
        idx: usize,
        ox: i32,
        oy: i32,
        in_scroll: bool,
        chrome_mode: bool,
        pass: PaintPass,
    ) {
        if pass == PaintPass::Fixed && !self.fixed_subtree.get(idx).copied().unwrap_or(true) {
            return;
        }
        let n = &self.nodes[idx];
        if !n.visible() || !self.active_child(idx) {
            return;
        }
        let fixed = self.node_is_fixed(idx, chrome_mode);
        if pass == PaintPass::Flow && fixed {
            let clipped_scroll = is_scroll_container(n);
            if clipped_scroll {
                // Paint the viewport background before its scrolling
                // descendants. The fixed pass runs after the flow pass, so
                // doing this later would cover a white ScrollView's content.
                let x = n.x + ox;
                let y = n.y;
                if let Some(fill) = parse_color(n.attr("background")) {
                    let radius = parse_dim(n.attr("cornerRadius"), 0).max(0) as usize;
                    if radius > 0 {
                        layer.fill_rounded_rect(
                            x.max(0) as usize,
                            y.max(title_bar_h()).max(0) as usize,
                            n.w.max(1) as usize,
                            n.h.max(1) as usize,
                            radius
                                .min(n.w.max(1) as usize / 2)
                                .min(n.h.max(1) as usize / 2),
                            fill,
                        );
                    } else {
                        layer.fill_rect(
                            x.max(0) as usize,
                            y.max(title_bar_h()).max(0) as usize,
                            n.w.max(1) as usize,
                            n.h.max(1) as usize,
                            fill,
                        );
                    }
                }
                let clip_x0 = n.x + ox;
                let clip_y0 = n.y;
                layer.push_clip(
                    clip_x0.max(0) as usize,
                    clip_y0.max(title_bar_h()).max(0) as usize,
                    (clip_x0 + n.w).max(0) as usize,
                    (clip_y0 + n.h).max(0) as usize,
                );
            }
            let child_scroll = in_scroll || is_scroll_container(n);
            for &child in &n.children {
                self.paint(layer, child, ox, oy, child_scroll, chrome_mode, pass);
            }
            if clipped_scroll {
                layer.pop_clip();
            }
            return;
        }
        if pass == PaintPass::Fixed && !fixed {
            let child_scroll = in_scroll || is_scroll_container(n);
            for &child in &n.children {
                self.paint(layer, child, ox, oy, child_scroll, chrome_mode, pass);
            }
            return;
        }
        let x = n.x + ox;
        let y = n.y + if fixed { 0 } else { oy };
        let w = n.w.max(1) as usize;
        let h = n.h.max(1) as usize;
        if !(pass == PaintPass::Fixed && fixed && is_scroll_container(n)) {
            if let Some(fill) = parse_color(n.attr("background")) {
                let radius = parse_dim(n.attr("cornerRadius"), 0).max(0) as usize;
                if radius > 0 {
                    layer.fill_rounded_rect(
                        x.max(0) as usize,
                        y.max(0) as usize,
                        w,
                        h,
                        radius.min(w / 2).min(h / 2),
                        fill,
                    );
                } else {
                    layer.fill_rect(x.max(0) as usize, y.max(0) as usize, w, h, fill);
                }
            }
        }
        if is_button_like(n) {
            let active = self.pressed == Some(idx);
            let hover = self.hovered == Some(idx);
            let primary = n.is("PrimaryButton");
            let primary_color = if active {
                Color::rgb(0, 96, 196)
            } else if hover {
                Color::rgb(0, 112, 232)
            } else {
                WARP4_PRIMARY
            };
            layer.fill_rounded_rect(
                x.max(0) as usize,
                y.max(0) as usize,
                w,
                h,
                w.min(h) / 2,
                if primary {
                    primary_color
                } else if active {
                    Color::rgb(224, 224, 226)
                } else if hover {
                    Color::rgb(244, 244, 245)
                } else {
                    WARP4_BUTTON_BG
                },
            );
        } else if n.is("EditText")
            || n.is("AutoCompleteTextView")
            || n.is("MultiAutoCompleteTextView")
        {
            layer.rounded_rect_outline(
                x.max(0) as usize,
                y.max(0) as usize,
                w,
                h,
                WARP4_INPUT_RADIUS.min(w / 2).min(h / 2),
                WARP4_INPUT_BORDER,
                WARP4_INPUT_BG,
            );
        } else if n.is("CheckBox") || n.is("RadioButton") {
            let checked = n.attr("checked") == "true";
            let hover = self.hovered == Some(idx);
            let mark_x = x + 2;
            let mark_y = y + (n.h - if n.is("RadioButton") { 18 } else { 22 }).max(0) / 2;
            if n.is("CheckBox") {
                let border = if checked || hover {
                    WARP3_ACCENT
                } else {
                    WARP3_MUTED
                };
                layer.rounded_rect_outline(
                    mark_x.max(0) as usize,
                    mark_y.max(0) as usize,
                    22,
                    22,
                    4,
                    border,
                    if checked { WARP3_ACCENT } else { WARP3_SURFACE },
                );
                if checked {
                    draw_check_icon(layer, mark_x + 5, mark_y + 5);
                }
            } else {
                let amount = self.control_amount(idx, checked);
                let outer = if self
                    .control_animations
                    .iter()
                    .any(|animation| animation.idx == idx)
                {
                    mix_color(WARP4_RADIO_OFF, WARP4_PRIMARY, amount)
                } else if checked {
                    WARP4_PRIMARY
                } else {
                    WARP4_RADIO_OFF
                };
                layer.fill_circle(
                    (mark_x + 9).max(0) as usize,
                    (mark_y + 9).max(0) as usize,
                    9,
                    outer,
                );
                let inner_radius = (4.0 * amount + 0.5) as usize;
                if inner_radius > 0 {
                    layer.fill_circle(
                        (mark_x + 9).max(0) as usize,
                        (mark_y + 9).max(0) as usize,
                        inner_radius,
                        WARP4_WHITE,
                    );
                }
            }
        } else if n.is("Switch") {
            let on = n.attr("checked") == "true";
            let amount = self.control_amount(idx, on);
            let track = mix_color(WARP3_BG, WARP3_ACCENT, amount);
            let sy = y + (n.h - 22).max(0) / 2;
            let track_w = 44usize;
            layer.rounded_rect_outline(
                x.max(0) as usize,
                sy.max(0) as usize,
                track_w,
                22,
                11,
                mix_color(WARP3_MUTED, WARP3_ACCENT, amount),
                track,
            );
            // Warp3 uses a compact 14px knob inside the 22px track.  The
            // previous 28px knob made the white state dominate the control.
            let knob_x = x + 10 + ((track_w as f32 - 20.0) * amount + 0.5) as i32;
            layer.fill_circle(
                knob_x.max(0) as usize,
                (sy + 11).max(0) as usize,
                7,
                mix_color(Color::rgb(102, 102, 102), Color::rgb(255, 255, 255), amount),
            );
        } else if n.is("Spinner") || n.is("SearchView") {
            layer.fill_rect(
                x.max(0) as usize,
                (y + h as i32 - 2).max(0) as usize,
                w,
                2,
                if self.hovered == Some(idx) {
                    WARP3_ACCENT
                } else {
                    WARP3_BORDER
                },
            );
            if n.is("Spinner") {
                let selected = parse_i32(n.attr("selectedIndex")).max(0) as usize;
                let value = if !n.attr("text").is_empty() {
                    n.attr("text")
                } else if !n.attr("value").is_empty() {
                    n.attr("value")
                } else if let Some(item) = n
                    .attr("items")
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .nth(selected)
                {
                    item
                } else {
                    match selected % 3 {
                        1 => "Item 2",
                        2 => "Item 3",
                        _ => "Item 1",
                    }
                };
                put_str_size(
                    layer,
                    x + 7,
                    y + ((h as i32 - 19).max(0) / 2),
                    value,
                    WARP3_TEXT,
                    15.0,
                );
                // Native equivalent of the CSS select arrow.
                let ax = x + n.w - 14;
                let ay = y + h as i32 / 2 - 2;
                for row in 0..5 {
                    let width = 2 + row * 2;
                    layer.fill_rect(
                        (ax - row).max(0) as usize,
                        (ay + row).max(0) as usize,
                        width as usize,
                        1,
                        WARP3_MUTED,
                    );
                }
            }
        } else if n.is("SeekBar") {
            let cy = y + h as i32 / 2;
            layer.fill_rect(
                x.max(0) as usize,
                cy.max(0) as usize,
                w,
                3,
                if self.hovered == Some(idx) {
                    WARP3_ACCENT
                } else {
                    WARP3_BORDER
                },
            );
            let max = parse_i32(n.attr("max")).max(1);
            let progress = parse_i32(n.attr("progress")).clamp(0, max);
            let px = x + w as i32 * progress / max;
            if progress > 0 {
                layer.fill_rect(
                    x.max(0) as usize,
                    cy.max(0) as usize,
                    (px - x).max(0) as usize,
                    3,
                    WARP3_ACCENT,
                );
            }
            layer.fill_circle(
                px.max(0) as usize,
                cy.max(0) as usize,
                if self.hovered == Some(idx) { 10 } else { 9 },
                WARP3_ACCENT,
            );
        } else if n.is("RatingBar") {
            let stars = parse_i32(n.attr("numStars")).max(1).min(10);
            let rating = n.attr("rating").parse::<f32>().unwrap_or(0.0);
            let hover = self.hovered == Some(idx);
            for star in 0..stars {
                let color = if star as f32 + 0.5 <= rating {
                    WARP3_ACCENT
                } else if hover {
                    Color::rgb(158, 190, 220)
                } else {
                    Color::rgb(183, 183, 183)
                };
                put_str_size(layer, x + star * 23, y, "★", color, 25.0);
            }
        } else if n.is("ProgressBar") {
            if n.attr("style").contains("progressBarStyleHorizontal") || w > 80 {
                layer.fill_rounded_rect(
                    x.max(0) as usize,
                    (y + 2).max(0) as usize,
                    w,
                    6,
                    3,
                    WARP3_BORDER,
                );
                let max = parse_i32(n.attr("max")).max(1);
                let progress = parse_i32(n.attr("progress")).clamp(0, max);
                layer.fill_rounded_rect(
                    x.max(0) as usize,
                    (y + 2).max(0) as usize,
                    (w as i32 * progress / max) as usize,
                    6,
                    3,
                    WARP3_ACCENT,
                );
            } else {
                let cx = (x + w as i32 / 2).max(0);
                let cy = (y + h as i32 / 2).max(0);
                layer.fill_circle(cx as usize, cy as usize, 14, Color::rgb(207, 207, 207));
                layer.fill_circle(cx as usize, cy as usize, 9, Color::rgb(255, 255, 255));
                for dy in -14..=14 {
                    for dx in -14..=14 {
                        let radius = dx * dx + dy * dy;
                        if radius <= 14 * 14 && radius >= 9 * 9 && (dx >= 0 || dy <= 0) {
                            let px = cx + dx;
                            let py = cy + dy;
                            if px >= 0 && py >= 0 {
                                layer.fill_rect(px as usize, py as usize, 1, 1, WARP3_ACCENT);
                            }
                        }
                    }
                }
            }
        } else if n.is("ImageView") {
            layer.rect_outline(x.max(0) as usize, y.max(0) as usize, w, h, WARP3_BORDER);
        } else if n.is("ListView") || n.is("ExpandableListView") {
            for row in 0..(h / 44) {
                let ry = y + row as i32 * 44;
                layer.fill_rect(x.max(0) as usize, ry.max(0) as usize, w, 43, WARP3_SURFACE);
                layer.fill_rect(
                    x.max(0) as usize,
                    (ry + 43).max(0) as usize,
                    w,
                    1,
                    WARP3_BORDER,
                );
                put_str_size(
                    layer,
                    x + 12,
                    ry + 12,
                    &format!("Item {}", row + 1),
                    WARP3_TEXT,
                    14.0,
                );
            }
        }
        let text = if !n.attr("text").is_empty() {
            n.attr("text")
        } else if n.is("ToggleButton") {
            if n.attr("checked") == "true" {
                n.attr("textOn")
            } else {
                n.attr("textOff")
            }
        } else {
            ""
        };
        if !text.is_empty() {
            let color = text_color(n);
            let size = text_size(n);
            let pad = edges(n, "padding");
            let text_w = measure_size(text, size);
            let tx = if is_text_button(n) {
                x + (n.w - text_w).max(0) / 2
            } else if n.is("EditText")
                || n.is("AutoCompleteTextView")
                || n.is("MultiAutoCompleteTextView")
            {
                // Warp3 inputs use a fixed 10px text inset; using the XML
                // padding here made the value appear vertically/horizontally
                // displaced between focused and unfocused states.
                x + 10
            } else if ascii_contains_ignore_case(n.attr("gravity"), "right")
                || ascii_contains_ignore_case(n.attr("gravity"), "end")
            {
                x + n.w - text_w - pad.right
            } else if ascii_contains_ignore_case(n.attr("gravity"), "center") {
                x + (n.w - text_w) / 2
            } else {
                x + pad.left
                    + if n.is("CheckBox") {
                        32
                    } else if n.is("RadioButton") {
                        28
                    } else if n.is("Switch") {
                        55
                    } else {
                        0
                    }
            };
            let line_h = (size * 1.25) as i32;
            let line_count = text.split('\n').count().max(1) as i32;
            let block_h = line_h * line_count;
            let gravity = n.attr("gravity");
            let ty = if is_text_button(n) {
                y + (n.h - block_h).max(0) / 2
            } else if n.is("EditText")
                || n.is("AutoCompleteTextView")
                || n.is("MultiAutoCompleteTextView")
            {
                y + 8
            } else if ascii_contains_ignore_case(gravity, "bottom") {
                y + n.h - pad.bottom - block_h
            } else if ascii_contains_ignore_case(gravity, "center_vertical")
                || gravity.eq_ignore_ascii_case("center")
                || ascii_contains_ignore_case(gravity, "center|vertical")
            {
                y + (n.h - block_h).max(0) / 2
            } else {
                y + pad.top
            };
            for (line, part) in text.split('\n').enumerate() {
                let line_y = ty + line as i32 * (size * 1.25) as i32;
                put_str_size(layer, tx, line_y, part, color, size);
                if text_bold(n) {
                    put_str_size(layer, tx + 1, line_y, part, color, size);
                }
            }
        } else if (n.is("EditText")
            || n.is("AutoCompleteTextView")
            || n.is("MultiAutoCompleteTextView"))
            && !n.attr("hint").is_empty()
        {
            layer.put_str(
                (x + 10).max(0) as usize,
                (y + 8).max(0) as usize,
                n.attr("hint"),
                WARP3_MUTED,
            );
        }
        if (n.is("EditText") || n.is("AutoCompleteTextView") || n.is("MultiAutoCompleteTextView"))
            && self.focused == Some(idx)
            && (self.now_ns / 500_000_000) % 2 == 0
        {
            let size = text_size(n);
            let caret_x = x + 10 + measure_size(text, size);
            let caret_y = y + 7;
            let caret_h = (size * 1.25).max(1.0) as usize;
            if caret_x >= 0 && caret_y >= 0 {
                layer.fill_rect(caret_x as usize, caret_y as usize, 1, caret_h, WARP4_BLACK);
            }
        }
        let child_scroll = in_scroll || is_scroll_container(n);
        if is_scroll_container(n) {
            layer.push_clip(
                x.max(0) as usize,
                y.max(title_bar_h()).max(0) as usize,
                (x + n.w).max(0) as usize,
                (y + n.h).max(0) as usize,
            );
        }
        for &child in &n.children {
            self.paint(layer, child, ox, oy, child_scroll, chrome_mode, pass);
        }
        if is_scroll_container(n) {
            if n.is("ScrollView") && n.content_h > n.h {
                let track_h = (n.h - 2).max(1);
                let thumb_h = (track_h * n.h / n.content_h).max(12).min(track_h);
                let max_thumb_y = track_h - thumb_h;
                let max_scroll = n.content_h.saturating_sub(n.h).max(1);
                let thumb_y = max_thumb_y * self.scroll / max_scroll;
                let bar_x = (x + n.w - 8).max(0) as usize;
                layer.fill_rounded_rect(
                    bar_x,
                    (y + 1).max(0) as usize,
                    6,
                    track_h as usize,
                    SCROLLBAR_RADIUS,
                    SCROLLBAR_TRACK,
                );
                layer.fill_rounded_rect(
                    bar_x,
                    (y + 1 + thumb_y).max(0) as usize,
                    6,
                    thumb_h as usize,
                    SCROLLBAR_RADIUS,
                    SCROLLBAR_THUMB,
                );
            } else if n.is("HorizontalScrollView") && n.content_w > n.w {
                let track_w = (n.w - 2).max(1);
                let thumb_w = (track_w * n.w / n.content_w).max(12).min(track_w);
                let max_thumb_x = track_w - thumb_w;
                let max_scroll = n.content_w.saturating_sub(n.w).max(1);
                let thumb_x = max_thumb_x * self.scroll / max_scroll;
                layer.fill_rounded_rect(
                    (x + 1).max(0) as usize,
                    (y + n.h - 6).max(0) as usize,
                    track_w as usize,
                    6,
                    SCROLLBAR_RADIUS,
                    SCROLLBAR_TRACK,
                );
                layer.fill_rounded_rect(
                    (x + 1 + thumb_x).max(0) as usize,
                    (y + n.h - 6).max(0) as usize,
                    thumb_w as usize,
                    6,
                    SCROLLBAR_RADIUS,
                    SCROLLBAR_THUMB,
                );
            }
            layer.pop_clip();
        }
    }

    fn paint_spinner_popup(&self, layer: &mut LayerSystem, idx: usize, opacity: u8) {
        let Some(node) = self.nodes.get(idx) else {
            return;
        };
        if !node.visible() || !node.is("Spinner") {
            return;
        }
        let (x, y, w, h) = self.spinner_popup_rect(idx);
        let popup_w = w.max(1) as usize;
        let popup_h = h.max(1) as usize;
        let radius = 8usize;
        let shadow_pad = 16usize;
        if opacity < 255 {
            // Seed the temporary layer with the pixels underneath the menu so
            // fading does not turn its transparent margins into black.
            let popup_x = x.max(0) as usize;
            let popup_y = y.max(title_bar_h()) as usize;
            let sx = popup_x.saturating_sub(shadow_pad);
            let sy = popup_y.saturating_sub(shadow_pad);
            let ex = (sx + popup_w + shadow_pad * 2).min(layer.width());
            let ey = (sy + popup_h + shadow_pad * 2).min(layer.height());
            let copy_w = ex.saturating_sub(sx).max(1);
            let copy_h = ey.saturating_sub(sy).max(1);
            let mut popup = LayerSystem::new(copy_w, copy_h);
            for row in 0..copy_h {
                let src = (sy + row) * layer.width() + sx;
                let dst = row * copy_w;
                popup.buf_mut()[dst..dst + copy_w]
                    .copy_from_slice(&layer.buf_ref()[src..src + copy_w]);
            }
            let local_x = popup_x.saturating_sub(sx).min(copy_w.saturating_sub(1));
            let local_y = popup_y.saturating_sub(sy).min(copy_h.saturating_sub(1));
            draw_spinner_shadow(&mut popup, local_x, local_y, popup_w, popup_h, radius);
            self.paint_spinner_popup_content(&mut popup, local_x, local_y, popup_w, popup_h, idx);
            layer.composit_rect_global_alpha(
                popup.buf_ref(),
                popup.width(),
                popup.height(),
                sx,
                sy,
                opacity,
            );
            return;
        }
        let x = x.max(0) as usize;
        let y = y.max(title_bar_h()) as usize;
        draw_spinner_shadow(layer, x, y, popup_w, popup_h, radius);
        self.paint_spinner_popup_content(layer, x, y, popup_w, popup_h, idx);
    }

    fn paint_spinner_popup_content(
        &self,
        layer: &mut LayerSystem,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        idx: usize,
    ) {
        let Some(node) = self.nodes.get(idx) else {
            return;
        };
        layer.fill_rounded_rect(x, y, w, h, 8, Color::rgb(255, 255, 255));
        let selected = parse_i32(node.attr("selectedIndex")).max(0) as usize;
        let items = node.attr("items");
        for item in 0..self.spinner_item_count(idx) {
            let row_y = y + 4 + item * 36;
            let highlighted = item == selected;
            if highlighted {
                layer.fill_rounded_rect(
                    x + 4,
                    row_y,
                    w.saturating_sub(8),
                    36.min(h.saturating_sub(4 + item * 36)),
                    6,
                    WARP3_ACCENT,
                );
            }
            let label = items
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .nth(item)
                .unwrap_or(match item {
                    1 => "Item 2",
                    2 => "Item 3",
                    _ => "Item 1",
                });
            put_str_size(
                layer,
                x as i32 + 16,
                row_y as i32 + 9,
                label,
                if highlighted {
                    Color::rgb(255, 255, 255)
                } else {
                    WARP3_TEXT
                },
                15.0,
            );
        }
    }

    fn document_has_scroll(&self) -> bool {
        self.roots
            .iter()
            .any(|root| contains_scroll(&self.nodes, *root))
    }

    fn active_child(&self, idx: usize) -> bool {
        let Some(parent) = self.nodes[idx].parent else {
            return true;
        };
        let p = &self.nodes[parent];
        if !(p.is("ViewFlipper")
            || p.is("ViewAnimator")
            || p.is("ViewSwitcher")
            || p.is("TextSwitcher"))
        {
            return true;
        }
        let active = parse_i32(p.attr("displayedChild")).max(0) as usize;
        p.children.iter().position(|child| *child == idx) == Some(active)
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

#[derive(Clone)]
enum ScriptNode {
    Raw(String),
    Block {
        header: String,
        body: Vec<ScriptNode>,
    },
}

struct ScriptParser {
    chars: Vec<char>,
    pos: usize,
}

impl ScriptParser {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
        }
    }
    fn parse_program(&mut self, stop_on_brace: bool) -> Vec<ScriptNode> {
        let mut nodes = Vec::new();
        while self.pos < self.chars.len() {
            while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
                self.pos += 1;
            }
            if self.pos >= self.chars.len() {
                break;
            }
            let start = self.pos;
            let mut square = 0i32;
            let mut paren = 0i32;
            let mut consumed = false;
            while self.pos < self.chars.len() {
                let c = self.chars[self.pos];
                match c {
                    '[' => square += 1,
                    ']' => square -= 1,
                    '(' => paren += 1,
                    ')' => paren -= 1,
                    '{' if square == 0 && paren == 0 => {
                        let header: String = self.chars[start..self.pos]
                            .iter()
                            .collect::<String>()
                            .trim()
                            .into();
                        self.pos += 1;
                        let body = self.parse_program(true);
                        nodes.push(ScriptNode::Block { header, body });
                        consumed = true;
                        break;
                    }
                    '\n' if square == 0 && paren == 0 => {
                        let raw: String = self.chars[start..self.pos]
                            .iter()
                            .collect::<String>()
                            .trim()
                            .into();
                        self.pos += 1;
                        if !raw.is_empty() {
                            nodes.push(ScriptNode::Raw(raw));
                        }
                        consumed = true;
                        break;
                    }
                    '}' if square == 0 && paren == 0 => {
                        if stop_on_brace {
                            self.pos += 1;
                            return nodes;
                        }
                        self.pos += 1;
                        consumed = true;
                        break;
                    }
                    _ => {}
                }
                self.pos += 1;
            }
            if !consumed && self.pos >= self.chars.len() {
                let raw: String = self.chars[start..self.pos]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .into();
                if !raw.is_empty() {
                    nodes.push(ScriptNode::Raw(raw));
                }
            }
        }
        nodes
    }
}

fn parse_script(source: &str) -> Script {
    let mut parser = ScriptParser::new(source);
    let nodes = parser.parse_program(false);
    let mut script = Script::default();
    for node in nodes {
        match node {
            ScriptNode::Raw(raw) => {
                if let Some(action) = parse_script_raw(&raw) {
                    script.init.push(action);
                }
            }
            ScriptNode::Block { header, body } => {
                let header = header.trim();
                if let Some(target) = header.strip_prefix("WarpUI.OnClick") {
                    let target = target.trim();
                    if !target.is_empty() {
                        script
                            .clicks
                            .push((target.into(), compile_script_nodes(&body)));
                    }
                } else if header.starts_with("fun") {
                    let name = header[3..]
                        .trim()
                        .trim_start_matches('(')
                        .trim_end_matches(')')
                        .trim();
                    if !name.is_empty() {
                        script
                            .functions
                            .push((name.into(), compile_script_nodes(&body)));
                    }
                }
            }
        }
    }
    script
}

fn compile_script_nodes(nodes: &[ScriptNode]) -> Vec<Action> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            ScriptNode::Raw(raw) => {
                if let Some(action) = parse_script_raw(raw) {
                    out.push(action);
                }
            }
            ScriptNode::Block { header, body } => {
                if let Some(condition_text) = header.strip_prefix("if ") {
                    if let Some((left, op, right)) = condition(condition_text.trim()) {
                        out.push(Action::If {
                            left,
                            op,
                            right,
                            body: compile_script_nodes(body),
                        });
                    }
                }
            }
        }
    }
    out
}

fn parse_script_raw(raw: &str) -> Option<Action> {
    let line = raw.split_once('#').map_or(raw, |(before, _)| before).trim();
    if line.is_empty() || line.starts_with("//") {
        return None;
    }
    if line == "break" {
        return Some(Action::Break);
    }
    if let Some(rest) = line.strip_prefix("if ") {
        if let Some(open) = find_top_level(rest, '(') {
            if let Some((body, _)) = balanced(rest, open) {
                if let Some((left, op, right)) = condition(rest[..open].trim()) {
                    let mut parser = ScriptParser::new(&body);
                    return Some(Action::If {
                        left,
                        op,
                        right,
                        body: compile_script_nodes(&parser.parse_program(false)),
                    });
                }
            }
        }
    }
    parse_command(line)
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
    if name == "BaramOS" {
        let rest = rest.strip_prefix("run").map(str::trim).unwrap_or(rest);
        let value = rest.trim_start_matches('=').trim();
        if value.starts_with('(') {
            let (value, _) = balanced(value, 0)?;
            return Some(Action::Command {
                name: "run".into(),
                target: String::new(),
                value,
            });
        }
        return Some(Action::Command {
            name: "run".into(),
            target: String::new(),
            value: value.into(),
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
fn find_top_level(s: &str, wanted: char) -> Option<usize> {
    let mut square = 0i32;
    let mut paren = 0i32;
    for (i, c) in s.char_indices() {
        if c == wanted && square == 0 && paren == 0 {
            return Some(i);
        }
        match c {
            '[' => square += 1,
            ']' => square -= 1,
            '(' => paren += 1,
            ')' => paren -= 1,
            _ => {}
        }
    }
    None
}
fn condition(s: &str) -> Option<(String, String, String)> {
    let chars: Vec<char> = s.chars().collect();
    let mut square = 0i32;
    let mut paren = 0i32;
    let mut i = 0;
    while i < chars.len() {
        let op = if square == 0
            && paren == 0
            && i + 1 < chars.len()
            && chars[i] == '!'
            && chars[i + 1] == '='
        {
            Some(("!=", 2))
        } else if square == 0 && paren == 0 && chars[i] == '=' {
            Some(("=", 1))
        } else if square == 0 && paren == 0 && chars[i] == '<' {
            Some(("<", 1))
        } else if square == 0 && paren == 0 && chars[i] == '>' {
            Some((">", 1))
        } else {
            None
        };
        if let Some((op, width)) = op {
            return Some((
                chars[..i].iter().collect::<String>().trim().into(),
                op.into(),
                chars[i + width..].iter().collect::<String>().trim().into(),
            ));
        }
        match chars[i] {
            '[' => square += 1,
            ']' => square -= 1,
            '(' => paren += 1,
            ')' => paren -= 1,
            _ => {}
        }
        i += 1;
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
fn is_match_parent(s: &str) -> bool {
    matches!(s.trim(), "match_parent" | "fill_parent")
}
fn is_zero_dimension(s: &str) -> bool {
    let raw = s.trim();
    !raw.is_empty() && parse_dim(raw, i32::MIN) == 0
}
fn duration_ns(s: &str) -> Option<u64> {
    let value = s.trim();
    let (number, multiplier) = if let Some(v) = value.strip_suffix("ns") {
        (v, 1u64)
    } else if let Some(v) = value.strip_suffix("us") {
        (v, 1_000)
    } else if let Some(v) = value.strip_suffix("ms") {
        (v, 1_000_000)
    } else if let Some(v) = value.strip_suffix('s') {
        (v, 1_000_000_000)
    } else if let Some(v) = value.strip_suffix('m') {
        (v, 60_000_000_000)
    } else if let Some(v) = value.strip_suffix('h') {
        (v, 3_600_000_000_000)
    } else {
        return None;
    };
    let n = number.trim().parse::<f64>().ok()?;
    Some((n * multiplier as f64).max(0.0) as u64)
}
fn parse_dim(s: &str, default: i32) -> i32 {
    s.trim()
        .trim_end_matches("dip")
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
fn truth(value: &str) -> bool {
    let value = value.trim();
    value.eq_ignore_ascii_case("true") || value == "1" || value.eq_ignore_ascii_case("yes")
}

#[inline]
fn ascii_contains_ignore_case(value: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    !needle.is_empty()
        && value.as_bytes().windows(needle.len()).any(|window| {
            window
                .iter()
                .zip(needle.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
        })
}

fn is_scroll_container(n: &Node) -> bool {
    n.is("ScrollView")
        || n.is("HorizontalScrollView")
        || n.is("ListView")
        || n.is("GridView")
        || n.is("ExpandableListView")
}
fn contains_scroll(nodes: &[Node], idx: usize) -> bool {
    is_scroll_container(&nodes[idx])
        || nodes[idx]
            .children
            .iter()
            .any(|child| contains_scroll(nodes, *child))
}
fn is_fixed(n: &Node) -> bool {
    let position = n.attr("position");
    position.eq_ignore_ascii_case("fixed")
        || position.eq_ignore_ascii_case("sticky")
        || truth(n.attr("fixed"))
        || truth(n.attr("sticky"))
        || n.attr("layout_position").eq_ignore_ascii_case("fixed")
}
fn gravity_offset(
    gravity: &str,
    parent_w: i32,
    parent_h: i32,
    child_w: i32,
    child_h: i32,
) -> (i32, i32) {
    let x = if ascii_contains_ignore_case(gravity, "center_horizontal")
        || gravity.eq_ignore_ascii_case("center")
        || ascii_contains_ignore_case(gravity, "center|horizontal")
    {
        (parent_w - child_w) / 2
    } else if ascii_contains_ignore_case(gravity, "right")
        || ascii_contains_ignore_case(gravity, "end")
    {
        parent_w - child_w
    } else {
        0
    };
    let y = if ascii_contains_ignore_case(gravity, "center_vertical")
        || gravity.eq_ignore_ascii_case("center")
    {
        (parent_h - child_h) / 2
    } else if ascii_contains_ignore_case(gravity, "bottom") {
        parent_h - child_h
    } else {
        0
    };
    (x.max(0), y.max(0))
}
fn cross_offset(gravity: &str, parent_size: i32, child_size: i32, horizontal_axis: bool) -> i32 {
    if (horizontal_axis
        && (ascii_contains_ignore_case(gravity, "center_vertical")
            || gravity.eq_ignore_ascii_case("center")))
        || (!horizontal_axis
            && (ascii_contains_ignore_case(gravity, "center_horizontal")
                || gravity.eq_ignore_ascii_case("center")))
    {
        return (parent_size - child_size).max(0) / 2;
    }
    if (horizontal_axis && ascii_contains_ignore_case(gravity, "bottom"))
        || (!horizontal_axis
            && (ascii_contains_ignore_case(gravity, "right")
                || ascii_contains_ignore_case(gravity, "end")))
    {
        return (parent_size - child_size).max(0);
    }
    0
}
fn is_button_like(n: &Node) -> bool {
    n.is("Button") || n.is("PrimaryButton") || n.is("ImageButton") || n.is("ToggleButton")
}

fn is_text_button(n: &Node) -> bool {
    n.is("Button") || n.is("PrimaryButton") || n.is("ToggleButton")
}

fn interactive(n: &Node) -> bool {
    n.attr("enabled") != "false"
        && (is_button_like(n)
            || n.is("EditText")
            || n.is("AutoCompleteTextView")
            || n.is("MultiAutoCompleteTextView")
            || n.is("Switch")
            || n.is("CheckBox")
            || n.is("RadioButton")
            || n.is("ToggleButton")
            || n.is("Spinner")
            || n.is("SeekBar")
            || n.is("RatingBar")
            || n.is("SearchView")
            || n.is("NumberPicker")
            || n.is("DatePicker")
            || n.is("TimePicker"))
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
        total += advance.max(1);
    }
    total
}
fn text_size(n: &Node) -> f32 {
    if !n.attr("textSize").is_empty() {
        return parse_dim(n.attr("textSize"), 16) as f32;
    }
    if is_button_like(n) {
        return 18.0;
    }
    if n.is("CheckBox") || n.is("RadioButton") || n.is("Switch") {
        return 14.0;
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
fn text_color(n: &Node) -> Color {
    if let Some(color) = parse_color(n.attr("textColor")) {
        return color;
    }
    if n.is("PrimaryButton") {
        return WARP4_WHITE;
    }
    if n.is("Button") {
        return WARP4_PRIMARY;
    }
    if n.is("EditText") || n.is("AutoCompleteTextView") || n.is("MultiAutoCompleteTextView") {
        return WARP4_BLACK;
    }
    let style = n.attr("style");
    if style.contains("SectionDescription") {
        WARP3_MUTED
    } else if style.contains("ComponentLabel") {
        WARP3_MUTED
    } else {
        WARP3_TEXT
    }
}
fn text_bold(_n: &Node) -> bool {
    false
}

fn mix_color(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount) as u8;
    Color::rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

fn draw_spinner_shadow(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    radius: usize,
) {
    // Build a smooth rounded mask and blur its alpha, matching the app-list
    // shadow treatment without introducing a backdrop blur behind the menu.
    let pad = 16usize;
    let offset_y = 4usize;
    let shadow_w = width.saturating_add(pad * 2);
    let shadow_h = height.saturating_add(pad * 2 + offset_y);
    let mut mask = LayerSystem::new_transparent(shadow_w, shadow_h);
    mask.fill_rounded_rect(pad, pad + offset_y, width, height, radius, Color::BLACK);
    let mut alpha = alloc::vec![0u8; shadow_w * shadow_h];
    for (dst, src) in alpha.iter_mut().zip(mask.buf_ref()) {
        *dst = if *src == Color::TRANSPARENT.0 { 0 } else { 52 };
    }
    blur_shadow_alpha(&mut alpha, shadow_w, shadow_h, 6);

    let base_x = x.saturating_sub(pad);
    let base_y = y.saturating_sub(pad);
    for sy in 0..shadow_h {
        let dy = base_y + sy;
        if dy >= layer.height() {
            continue;
        }
        for sx in 0..shadow_w {
            let dx = base_x + sx;
            let opacity = alpha[sy * shadow_w + sx];
            if dx >= layer.width() || opacity == 0 {
                continue;
            }
            let pos = dy * layer.width() + dx;
            let old = layer.buf_ref()[pos];
            layer.buf_mut()[pos] =
                LayerSystem::blend_alpha(old, Color::BLACK.0, opacity as f32 / 255.0);
        }
    }
}

fn blur_shadow_alpha(alpha: &mut [u8], width: usize, height: usize, radius: usize) {
    if width == 0 || height == 0 || radius == 0 {
        return;
    }
    let mut scratch = alloc::vec![0u8; alpha.len()];
    let diameter = radius * 2 + 1;
    for y in 0..height {
        for x in 0..width {
            let start = x.saturating_sub(radius);
            let end = (x + radius + 1).min(width);
            let mut sum = 0usize;
            for px in start..end {
                sum += alpha[y * width + px] as usize;
            }
            scratch[y * width + x] = (sum / diameter.min(end - start)) as u8;
        }
    }
    for y in 0..height {
        for x in 0..width {
            let start = y.saturating_sub(radius);
            let end = (y + radius + 1).min(height);
            let mut sum = 0usize;
            for py in start..end {
                sum += scratch[py * width + x] as usize;
            }
            alpha[y * width + x] = (sum / diameter.min(end - start)) as u8;
        }
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
        x += advance.max(1);
    }
}

fn draw_check_icon(layer: &mut LayerSystem, x: i32, y: i32) {
    // Use the shared SVG asset as a mask, then tint it white for the checked
    // state.  The source asset is black because it is also usable on light
    // surfaces; the native checkbox needs the same white mark as Warp3.
    const ICON_SIZE: usize = 12;
    let pixels = svg::rasterize_svg_to_buffer(CHECK_ICON_SVG, ICON_SIZE, ICON_SIZE);
    let (clip_x0, clip_y0, clip_x1, clip_y1) = layer.clip_bounds();
    let layer_w = layer.width();
    let layer_h = layer.height();
    for sy in 0..ICON_SIZE as i32 {
        let py = y + sy;
        if py < clip_y0 as i32 || py >= clip_y1.min(layer_h) as i32 {
            continue;
        }
        for sx in 0..ICON_SIZE as i32 {
            let px = x + sx;
            if px < clip_x0 as i32 || px >= clip_x1.min(layer_w) as i32 {
                continue;
            }
            let alpha = pixels[(sy as usize * ICON_SIZE + sx as usize) * 4 + 3];
            if alpha == 0 {
                continue;
            }
            let index = py as usize * layer_w + px as usize;
            let background = layer.buf_ref()[index];
            layer.buf_mut()[index] = LayerSystem::blend_alpha(
                background,
                Color::rgb(255, 255, 255).0,
                alpha as f32 / 255.0,
            );
        }
    }
}

fn parse_color(s: &str) -> Option<Color> {
    let raw = s.trim();
    if raw.eq_ignore_ascii_case("transparent") {
        return Some(Color::TRANSPARENT);
    }
    if raw.eq_ignore_ascii_case("white") {
        return Some(Color::rgb(255, 255, 255));
    }
    if raw.eq_ignore_ascii_case("black") {
        return Some(Color::rgb(0, 0, 0));
    }
    if raw.eq_ignore_ascii_case("gray") || raw.eq_ignore_ascii_case("grey") {
        return Some(Color::rgb(128, 128, 128));
    }
    if raw.eq_ignore_ascii_case("red") {
        return Some(Color::rgb(255, 0, 0));
    }
    if raw.eq_ignore_ascii_case("green") {
        return Some(Color::rgb(0, 128, 0));
    }
    if raw.eq_ignore_ascii_case("blue") {
        return Some(Color::rgb(0, 0, 255));
    }
    let named = raw;
    let hex = named.strip_prefix('#')?;
    let hex = if hex.len() == 3 {
        let mut expanded = String::new();
        for c in hex.chars() {
            expanded.push(c);
            expanded.push(c);
        }
        expanded
    } else {
        hex.into()
    };
    let v = u32::from_str_radix(&hex, 16).ok()?;
    if hex.len() == 6 {
        Some(Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    } else if hex.len() == 8 {
        Some(Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    } else {
        None
    }
}

fn eval_calc(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut parser = CalcParser { chars, pos: 0 };
    let value = parser.expr();
    if (value as i64) as f64 == value {
        (value as i64).to_string()
    } else {
        value.to_string()
    }
}

struct CalcParser {
    chars: Vec<char>,
    pos: usize,
}
impl CalcParser {
    fn skip(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }
    fn expr(&mut self) -> f64 {
        let mut value = self.term();
        loop {
            self.skip();
            let op = self.chars.get(self.pos).copied();
            if op != Some('+') && op != Some('-') {
                break;
            }
            self.pos += 1;
            let rhs = self.term();
            value = if op == Some('+') {
                value + rhs
            } else {
                value - rhs
            };
        }
        value
    }
    fn term(&mut self) -> f64 {
        let mut value = self.factor();
        loop {
            self.skip();
            let op = self.chars.get(self.pos).copied();
            if !matches!(op, Some('*') | Some('/') | Some('%')) {
                break;
            }
            self.pos += 1;
            let rhs = self.factor();
            value = match op {
                Some('*') => value * rhs,
                Some('/') => {
                    if rhs == 0.0 {
                        0.0
                    } else {
                        value / rhs
                    }
                }
                Some('%') => value % rhs,
                _ => value,
            };
        }
        value
    }
    fn factor(&mut self) -> f64 {
        self.skip();
        if self.chars.get(self.pos) == Some(&'-') {
            self.pos += 1;
            return -self.factor();
        }
        if self.chars.get(self.pos) == Some(&'(') {
            self.pos += 1;
            let value = self.expr();
            self.skip();
            if self.chars.get(self.pos) == Some(&')') {
                self.pos += 1;
            }
            return value;
        }
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_digit() || self.chars[self.pos] == '.')
        {
            self.pos += 1;
        }
        self.chars[start..self.pos]
            .iter()
            .collect::<String>()
            .parse()
            .unwrap_or(0.0)
    }
}
fn bg() -> Color {
    WARP4_BG
}
