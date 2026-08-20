//! Native Warp 4 application runtime.
//!
//! Warp 4 layouts look like Android XML, but they are not rendered by making
//! an HTML document.  This crate keeps the XML parser deliberately small and
//! `no_std`, builds a native view tree, lays that tree out, paints controls to
//! `LayerSystem`, and executes the `.w4s` program directly.

#![no_std]
#![allow(unexpected_cfgs)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use baram_bsd::{app::Warp4Archive, config, vfs};
use baram_core::{Color, LayerSystem};
#[cfg(feature = "ttf")]
use baram_font::ttf_font;
use baram_font::{bdf_font, LayerFontExt};
#[cfg(feature = "ttf")]
use baram_graphics::svg;
use uefi::runtime;

mod components;
mod engine;
mod helpers;
mod input;
mod layout;
mod model;
mod parser;
mod render;
mod script;
mod ui;

use components::*;
use helpers::*;
pub use model::Warp4Engine;
use model::{Attr, ControlAnimation, Edges, Node, PaintPass, SpinnerFade};
use parser::{
    duration_ns, ini, is_match_parent, is_safe_script_name, is_zero_dimension, parse_i32,
    parse_script, set_attr, XmlParser,
};
use script::{bg, Action, Script};
use ui::{is_xiao, palette, ui_px, ui_px_usize, ui_size};
pub use ui::{set_ui_mode, set_ui_scale_percent, UiMode};

const MAX_NODES: usize = 2048;
const MAX_ACTIONS: usize = 2048;
const SWITCH_DURATION_NS: u64 = 220_000_000;
const RADIO_DURATION_NS: u64 = 180_000_000;
// Keep the standalone Xiao viewport in lockstep with the normal window
// server's scroll animation. The owner differs, but the motion must not.
const XIAO_SCROLL_ANIMATION_NS: u64 = 180_000_000;
const XIAO_TRANSITION_NS: u64 = 180_000_000;
const KEYBOARD_PRESS_NS: u64 = 120_000_000;
#[cfg(feature = "ttf")]
const CHECK_ICON_SVG: &str = include_str!("../../../files/data/ui/check-icon.svg");
const WARP4_INPUT_RADIUS: usize = 11;
const WARP4_WHITE: Color = Color::rgb(255, 255, 255);
const WARP4_BLACK: Color = Color::rgb(0, 0, 0);
const SCROLLBAR_RADIUS: usize = 3;

// The renderer selects the visual profile after boot. Xiao uses the same
// smooth rounded primitives as the normal renderer, with a compact dark
// palette and solid control faces. Both profiles use the same parser, layout,
// and paint pipeline.

fn title_bar_h() -> i32 {
    ui_px(config::get_usize("ui-theme/window/title_bar_h", 30) as i32)
}
