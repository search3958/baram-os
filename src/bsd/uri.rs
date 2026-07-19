//! URI command parser and executor for BaramOS.
//!
//! Supports `os://` scheme for controlling system settings.
//! Example URIs:
//!   os://display/pointer?size=1
//!   os://display/hud?enabled=1
//!   os://display/wallpaper?file=baram.png
//!   os://display/wallpaper?color=#990000

use alloc::string::String;
use alloc::vec::Vec;
use crate::bsd::vfs;
use baram_bsd::config;

pub struct UriCommand {
    pub category: String,
    pub action: String,
    pub params: Vec<(String, String)>,
}

pub fn parse(uri: &str) -> Option<UriCommand> {
    let uri = uri.trim();
    let uri = uri.strip_prefix("os://")?;

    let (path, query) = match uri.find('?') {
        Some(i) => (&uri[..i], &uri[i + 1..]),
        None => (uri, ""),
    };

    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() < 2 {
        return None;
    }

    let category = String::from(parts[0]);
    let action = String::from(parts[1]);

    let mut params = Vec::new();
    if !query.is_empty() {
        for pair in query.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                params.push((String::from(k), String::from(v)));
            }
        }
    }

    Some(UriCommand { category, action, params })
}

pub struct DisplayState {
    pub pointer_size: f32,
    pub hud_enabled: bool,
    pub wallpaper_color: Option<u32>,
    pub wallpaper_index: usize,
}

impl DisplayState {
    pub fn new() -> Self {
        Self {
            pointer_size: 1.0,
            hud_enabled: true,
            wallpaper_color: None,
            wallpaper_index: 0,
        }
    }
}

pub fn get_param<'a>(cmd: &'a UriCommand, key: &str) -> Option<&'a str> {
    cmd.params.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

pub fn execute(uri: &str, state: &mut DisplayState) -> bool {
    let cmd = match parse(uri) {
        Some(c) => c,
        None => return false,
    };

    match cmd.category.as_str() {
        "display" => execute_display(&cmd, state),
        "system" => execute_system(&cmd, state),
        _ => false,
    }
}

pub fn wallpaper_changed(state: &DisplayState) -> bool {
    state.wallpaper_color.is_some() || state.wallpaper_index != 0
}

fn execute_display(cmd: &UriCommand, state: &mut DisplayState) -> bool {
    let cfg = config::get_config_mut();
    match cmd.action.as_str() {
        "pointer" => {
            if let Some(size_str) = get_param(cmd, "size") {
                if let Ok(v) = size_str.parse::<i32>() {
                    let size = v as f32 / 10.0;
                    if size >= 0.5 && size <= 5.0 {
                        state.pointer_size = size;
                        cfg.set("display", "pointer.size", &alloc::format!("{}", v));
                        config::save_config();
                        return true;
                    }
                }
            }
            false
        }
        "hud" => {
            if let Some(enabled_str) = get_param(cmd, "enabled") {
                match enabled_str {
                    "1" | "true" | "on" => {
                        state.hud_enabled = true;
                        cfg.set("display", "hud.enabled", "1");
                        config::save_config();
                        return true;
                    }
                    "0" | "false" | "off" => {
                        state.hud_enabled = false;
                        cfg.set("display", "hud.enabled", "0");
                        config::save_config();
                        return true;
                    }
                    _ => {}
                }
            }
            false
        }
        "wallpaper" => {
            if let Some(color_str) = get_param(cmd, "color") {
                let hex = color_str.trim_start_matches('#');
                if hex.len() == 6 {
                    if let (Ok(r), Ok(g), Ok(b)) = (
                        u8::from_str_radix(&hex[0..2], 16),
                        u8::from_str_radix(&hex[2..4], 16),
                        u8::from_str_radix(&hex[4..6], 16),
                    ) {
                        state.wallpaper_color = Some(crate::pexpert::gop::Color::rgb(r, g, b).0);
                        state.wallpaper_index = 0;
                        cfg.set("display", "wallpaper.file", "");
                        cfg.set("display", "wallpaper.color", color_str);
                        config::save_config();
                        return true;
                    }
                }
            }
            if let Some(file_str) = get_param(cmd, "file") {
                let idx = match file_str {
                    "baram.png" | "baram" => Some(0),
                    "hanul.png" | "hanul" => Some(1),
                    "reflect.png" | "reflect" => Some(2),
                    _ => None,
                };
                if let Some(i) = idx {
                    state.wallpaper_color = None;
                    state.wallpaper_index = i;
                    cfg.set("display", "wallpaper.file", file_str);
                    cfg.set("display", "wallpaper.color", "");
                    config::save_config();
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

fn execute_system(cmd: &UriCommand, state: &mut DisplayState) -> bool {
    match cmd.action.as_str() {
        "reset" => {
            if let Some(option) = get_param(cmd, "option") {
                if option == "all" {
                    *state = DisplayState::new();
                    vfs::remove_file("apps/.setup_done");
                    uefi::runtime::reset(
                        uefi_raw::table::runtime::ResetType::COLD,
                        uefi::Status::SUCCESS,
                        None,
                    );
                }
            }
            false
        }
        _ => false,
    }
}

pub fn load_settings_from_config(state: &mut DisplayState) {
    let cfg = config::get_config();
    if let Some(v) = cfg.get("display", "pointer.size") {
        if let Ok(size) = v.parse::<i32>() {
            state.pointer_size = size as f32 / 10.0;
        }
    }
    if let Some(v) = cfg.get("display", "hud.enabled") {
        state.hud_enabled = v == "1" || v == "true" || v == "on";
    }
    if let Some(v) = cfg.get("display", "wallpaper.color") {
        if !v.is_empty() {
            let hex = v.trim_start_matches('#');
            if hex.len() == 6 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                ) {
                    state.wallpaper_color = Some(crate::pexpert::gop::Color::rgb(r, g, b).0);
                    state.wallpaper_index = 0;
                }
            }
        }
    }
    if let Some(v) = cfg.get("display", "wallpaper.file") {
        if !v.is_empty() {
            state.wallpaper_color = None;
            state.wallpaper_index = match v {
                "baram.png" | "baram" => 0,
                "hanul.png" | "hanul" => 1,
                "reflect.png" | "reflect" => 2,
                _ => 0,
            };
        }
    }
}
