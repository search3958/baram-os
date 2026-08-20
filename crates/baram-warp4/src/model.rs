use super::*;
#[derive(Clone)]
pub(crate) struct Attr {
    pub(crate) key: String,
    pub(crate) value: String,
}

#[derive(Clone, Default)]
pub(crate) struct Node {
    pub(crate) tag: String,
    pub(crate) attrs: Vec<Attr>,
    pub(crate) children: Vec<usize>,
    pub(crate) parent: Option<usize>,
    pub(crate) text: String,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) hidden: bool,
    /// The native equivalent of the generated DOM's overflow/position state.
    /// These are computed by the layout pass; they are deliberately kept on
    /// the view instead of being global renderer state so nested viewports can
    /// be clipped and hit-tested independently.
    pub(crate) content_w: i32,
    pub(crate) content_h: i32,
}

impl Node {
    pub(crate) fn attr(&self, key: &str) -> &str {
        self.attrs
            .iter()
            .find(|a| a.key == key)
            .map(|a| a.value.as_str())
            .unwrap_or("")
    }
    pub(crate) fn id(&self) -> &str {
        let value = self.attr("id");
        value
            .strip_prefix("@+id/")
            .or_else(|| value.strip_prefix("@id/"))
            .unwrap_or(value)
    }
    pub(crate) fn is(&self, tag: &str) -> bool {
        self.tag.eq_ignore_ascii_case(tag)
    }
    pub(crate) fn visible(&self) -> bool {
        !self.hidden && self.attr("visibility") != "gone" && self.attr("visibility") != "invisible"
    }
}

#[derive(Clone)]
pub(crate) struct Edges {
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
    pub(crate) left: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaintPass {
    Flow,
    Fixed,
}

#[derive(Clone, Copy)]
pub(crate) struct ControlAnimation {
    pub(crate) idx: usize,
    pub(crate) to_on: bool,
    pub(crate) started_ns: u64,
    pub(crate) duration_ns: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct SpinnerFade {
    pub(crate) idx: usize,
    pub(crate) started_ns: u64,
}

pub struct Warp4Engine {
    pub(crate) archive: Warp4Archive,
    pub(crate) origin: String,
    pub(crate) title: String,
    pub(crate) screen: String,
    pub(crate) nodes: Vec<Node>,
    pub(crate) roots: Vec<usize>,
    pub(crate) fixed_subtree: Vec<bool>,
    pub(crate) script: Script,
    pub(crate) state: Vec<(String, String)>,
    pub(crate) focused: Option<usize>,
    pub(crate) keyboard_focus: Option<usize>,
    pub(crate) hovered: Option<usize>,
    pub(crate) pressed: Option<usize>,
    pub(crate) keyboard_press_until_ns: Option<u64>,
    pub(crate) spinner_open: Option<usize>,
    pub(crate) spinner_fade: Option<SpinnerFade>,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) chrome_height: i32,
    pub(crate) scroll: i32,
    pub(crate) scroll_target: i32,
    pub(crate) scroll_start: i32,
    pub(crate) scroll_elapsed_ns: u64,
    pub content_height: i32,
    pub last_command: Option<String>,
    pub(crate) dirty: bool,
    pub(crate) layout_dirty: bool,
    pub(crate) now_ns: u64,
    pub(crate) wait_until_ns: Option<u64>,
    pub(crate) pending: Vec<Action>,
    pub(crate) break_requested: bool,
    pub(crate) flip_elapsed_ns: u64,
    pub(crate) last_tick_ns: Option<u64>,
    pub(crate) control_animations: Vec<ControlAnimation>,
    pub(crate) transition_elapsed_ns: Option<u64>,
    pub(crate) last_clicked_id: Option<String>,
    pub(crate) runtime_fps: u32,
    pub(crate) runtime_windows: usize,
    pub(crate) runtime_keys: u32,
    pub(crate) runtime_mouse: u32,
}
