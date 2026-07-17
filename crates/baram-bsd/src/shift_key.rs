pub fn load_shift_key() -> u8 {
    let data = crate::vfs::read_file("apps/.shift_key");
    if data.len() >= 1 { data[0] } else { 0 }
}

pub fn save_shift_key(code: u8) {
    crate::vfs::write_file("apps/.shift_key", &[code]);
}
