use baram_core::Color;
use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Normal = 0,
    Xiao = 1,
}

static UI_MODE: AtomicU8 = AtomicU8::new(UiMode::Normal as u8);
static UI_SCALE_PERCENT: AtomicU32 = AtomicU32::new(100);

pub fn set_ui_mode(mode: UiMode) {
    UI_MODE.store(mode as u8, Ordering::Relaxed);
}

#[inline]
pub fn is_xiao() -> bool {
    UI_MODE.load(Ordering::Relaxed) == UiMode::Xiao as u8
}

pub fn set_ui_scale_percent(percent: u32) {
    UI_SCALE_PERCENT.store(percent.clamp(1, 100), Ordering::Relaxed);
}

#[inline]
pub fn ui_px(value: i32) -> i32 {
    let percent = UI_SCALE_PERCENT.load(Ordering::Relaxed) as i64;
    ((value as i64 * percent) / 100) as i32
}

#[inline]
pub fn ui_px_usize(value: usize) -> usize {
    ui_px(value as i32).max(0) as usize
}

#[inline]
pub fn ui_size(value: f32) -> f32 {
    value * (UI_SCALE_PERCENT.load(Ordering::Relaxed) as f32 / 100.0)
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub warp3_bg: Color,
    pub warp3_surface: Color,
    pub warp3_text: Color,
    pub warp3_muted: Color,
    pub warp3_border: Color,
    pub warp3_accent: Color,
    pub warp4_bg: Color,
    pub warp4_primary: Color,
    pub warp4_button_bg: Color,
    pub warp4_input_bg: Color,
    pub warp4_input_border: Color,
    pub warp4_radio_off: Color,
    pub warp9_highlight: Color,
    pub warp9_mid: Color,
    pub warp9_shadow: Color,
    pub warp9_dark_shadow: Color,
    pub button_radius: usize,
    pub button_border: Color,
    pub button_inner_edge: Color,
    pub button_highlight: Color,
    pub button_face: Color,
    pub button_shadow_1: Color,
    pub button_shadow_2: Color,
    pub scrollbar_track: Color,
    pub scrollbar_thumb: Color,
}

pub fn palette() -> Palette {
    if is_xiao() {
        Palette {
            warp3_bg: Color::rgb(0xD8, 0xD8, 0xD8),
            warp3_surface: Color::rgb(0xF0, 0xF0, 0xF0),
            warp3_text: Color::rgb(0x00, 0x00, 0x00),
            warp3_muted: Color::rgb(0x55, 0x55, 0x55),
            warp3_border: Color::rgb(0x78, 0x78, 0x78),
            warp3_accent: Color::rgb(0x33, 0x66, 0xCC),
            warp4_bg: Color::rgb(0xEE, 0xEE, 0xEE),
            warp4_primary: Color::rgb(0x33, 0x66, 0xCC),
            warp4_button_bg: Color::rgb(0xDD, 0xDD, 0xDD),
            warp4_input_bg: Color::rgb(0xFF, 0xFF, 0xFF),
            warp4_input_border: Color::rgb(0x7A, 0x7A, 0x7A),
            warp4_radio_off: Color::rgb(0xC8, 0xC8, 0xC8),
            warp9_highlight: Color::rgb(0xFF, 0xFF, 0xFF),
            warp9_mid: Color::rgb(0xD0, 0xD0, 0xD0),
            warp9_shadow: Color::rgb(0x8A, 0x8A, 0x8A),
            warp9_dark_shadow: Color::rgb(0x5F, 0x5F, 0x5F),
            button_radius: 2,
            button_border: Color::rgb(0, 0, 0),
            button_inner_edge: Color::rgb(221, 221, 221),
            button_highlight: Color::rgb(255, 255, 255),
            button_face: Color::rgb(221, 221, 221),
            button_shadow_1: Color::rgb(119, 119, 119),
            button_shadow_2: Color::rgb(170, 170, 170),
            scrollbar_track: Color::rgb(0xC8, 0xC8, 0xC8),
            scrollbar_thumb: Color::rgb(0x8A, 0x8A, 0x8A),
        }
    } else {
        Palette {
            warp3_bg: Color::rgb(243, 243, 243),
            warp3_surface: Color::rgb(251, 251, 251),
            warp3_text: Color::rgb(26, 26, 26),
            warp3_muted: Color::rgb(93, 93, 93),
            warp3_border: Color::rgb(211, 211, 211),
            warp3_accent: Color::rgb(0, 106, 255),
            warp4_bg: Color::rgb(250, 250, 252),
            warp4_primary: Color::rgb(0, 106, 255),
            warp4_button_bg: Color::rgb(238, 238, 239),
            warp4_input_bg: Color::rgb(255, 255, 255),
            warp4_input_border: Color::rgb(242, 242, 246),
            warp4_radio_off: Color::rgb(231, 230, 230),
            warp9_highlight: Color::rgb(255, 255, 255),
            warp9_mid: Color::rgb(208, 208, 208),
            warp9_shadow: Color::rgb(138, 138, 138),
            warp9_dark_shadow: Color::rgb(95, 95, 95),
            button_radius: 0,
            button_border: Color::rgb(0, 0, 0),
            button_inner_edge: Color::rgb(221, 221, 221),
            button_highlight: Color::rgb(255, 255, 255),
            button_face: Color::rgb(221, 221, 221),
            button_shadow_1: Color::rgb(119, 119, 119),
            button_shadow_2: Color::rgb(170, 170, 170),
            scrollbar_track: Color::rgb(241, 241, 241),
            scrollbar_thumb: Color::rgb(184, 184, 184),
        }
    }
}
