use alloc::string::ToString;

pub fn load_shift_key() -> u8 {
    crate::config::get_config()
        .get("keyboard/shift_key")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

pub fn save_shift_key(code: u8) {
    crate::config::get_config_mut().set("keyboard/shift_key", &code.to_string());
    crate::config::save_config();
}
