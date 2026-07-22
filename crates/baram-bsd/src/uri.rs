//! URI command parser and executor for BaramOS.
//!
//! Supports `os://` scheme for controlling system settings.
//! The URI path maps directly to the XML config structure.
//! Query parameters set child values under that path.
//!
//! Example URIs:
//!   os://display/pointer/size?10
//!   os://display/hud/enabled?1
//!   os://display/wallpaper?file=baram&mode=file
//!   os://display/wallpaper?color=#990000&mode=color
//!   os://ui-theme/color?btn_primary=BB0000

use crate::config;
use crate::vfs;
use alloc::string::String;
use alloc::vec::Vec;

pub struct UriCommand {
    pub path: String,
    pub params: Vec<(String, String)>,
}

pub fn parse(uri: &str) -> Option<UriCommand> {
    let uri = uri.trim();
    let uri = uri.strip_prefix("os://")?;

    let (path, query) = match uri.find('?') {
        Some(i) => (&uri[..i], &uri[i + 1..]),
        None => (uri, ""),
    };

    let path = path.trim_end_matches('/');

    let mut params = Vec::new();
    if !query.is_empty() {
        for pair in query.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                params.push((String::from(k), String::from(v)));
            } else if !pair.is_empty() {
                params.push((String::new(), String::from(pair)));
            }
        }
    }

    Some(UriCommand {
        path: String::from(path),
        params,
    })
}

pub struct DisplayState {
    pub pointer_size: f32,
    pub hud_enabled: bool,
    pub wallpaper_color: Option<u32>,
    pub wallpaper_index: usize,
    pub wallpaper_mode: WallpaperMode,
}

#[derive(Clone, Copy, PartialEq)]
pub enum WallpaperMode {
    File,
    Color,
}

impl DisplayState {
    pub fn new() -> Self {
        Self {
            pointer_size: 1.0,
            hud_enabled: true,
            wallpaper_color: None,
            wallpaper_index: 0,
            wallpaper_mode: WallpaperMode::File,
        }
    }
}

pub fn get_param<'a>(cmd: &'a UriCommand, key: &str) -> Option<&'a str> {
    cmd.params
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

pub fn execute(uri: &str, state: &mut DisplayState) -> bool {
    let cmd = match parse(uri) {
        Some(c) => c,
        None => return false,
    };

    let mut shared_pointer_speed = None;
    {
        let cfg = config::get_config_mut();
        for (key, value) in &cmd.params {
            let full_path = if key.is_empty() {
                cmd.path.clone()
            } else if cmd.path.is_empty() {
                String::from(key)
            } else {
                alloc::format!("{}/{}", cmd.path, key)
            };
            cfg.set(&full_path, value);
            if key == "speed" && (cmd.path == "mouse" || cmd.path == "trackpad") {
                shared_pointer_speed = Some(String::from(value.as_str()));
            }
        }
        // UEFI SimplePointer does not identify an integrated trackpad as a
        // trackpad. Keep the speed value shared so that either input path
        // receives the setting even when hardware identification is absent.
        if let Some(speed) = &shared_pointer_speed {
            cfg.set("mouse/speed", speed);
            cfg.set("trackpad/speed", speed);
        }
    }

    config::save_config();
    load_settings_from_config(state);
    true
}

pub enum SystemCommand {
    None,
    ResetAll,
}

pub fn check_system_commands(state: &mut DisplayState) -> SystemCommand {
    let result = {
        let cfg = config::get_config();
        match cfg.get("system/reset/option") {
            Some("all") => SystemCommand::ResetAll,
            _ => SystemCommand::None,
        }
    };

    if let SystemCommand::ResetAll = &result {
        config::reset_to_default();
        *state = DisplayState::new();
    }

    result
}

pub fn wallpaper_changed(state: &DisplayState) -> bool {
    state.wallpaper_mode == WallpaperMode::Color && state.wallpaper_color.is_some()
        || state.wallpaper_mode == WallpaperMode::File
}

pub fn load_settings_from_config(state: &mut DisplayState) {
    let cfg = config::get_config();
    if let Some(v) = cfg.get("display/pointer/size") {
        if let Ok(size) = v.parse::<i32>() {
            state.pointer_size = size as f32 / 10.0;
        }
    }
    if let Some(v) = cfg.get("display/hud/enabled") {
        state.hud_enabled = v == "1" || v == "true" || v == "on";
    }
    if let Some(v) = cfg.get("display/wallpaper/mode") {
        state.wallpaper_mode = match v {
            "color" => WallpaperMode::Color,
            _ => WallpaperMode::File,
        };
    }
    if let Some(v) = cfg.get("display/wallpaper/color") {
        if !v.is_empty() {
            let hex = v.trim_start_matches('#');
            if hex.len() == 6 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                ) {
                    state.wallpaper_color = Some(baram_core::Color::rgb(r, g, b).0);
                }
            }
        }
    }
    if let Some(v) = cfg.get("display/wallpaper/file") {
        if !v.is_empty() {
            state.wallpaper_index = match v {
                "baram.png" | "baram" => 0,
                "hanul.png" | "hanul" => 1,
                "reflect.png" | "reflect" => 2,
                _ => 0,
            };
        }
    }
}
