use alloc::string::ToString;

pub fn load_shift_key() -> u8 {
    crate::config::get_config()
        .get("keyboard/shift_key")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

pub fn save_shift_key(code: u8) {
    let value = code.to_string();
    crate::config::update_and_save(|config| config.set("keyboard/shift_key", &value));
}
