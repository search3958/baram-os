use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::collections::BTreeMap;

pub struct Config {
    sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            sections: BTreeMap::new(),
        }
    }

    pub fn load_from_vfs(path: &str) -> Self {
        let mut config = Self::new();
        let data = super::vfs::read_file(path);
        if data.is_empty() {
            return config;
        }
        config.parse(&data);
        config
    }

    fn parse(&mut self, data: &[u8]) {
        let text = String::from_utf8(data.to_vec()).unwrap_or_default();
        let mut current_section = String::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                continue;
            }

            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos + 1..].trim().to_string();
                self.sections
                    .entry(current_section.clone())
                    .or_insert_with(BTreeMap::new)
                    .insert(key, value);
            }
        }
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections.get(section)?.get(key).map(|s| s.as_str())
    }

    pub fn get_u32(&self, section: &str, key: &str) -> Option<u32> {
        self.get(section, key)?.parse().ok()
    }

    pub fn get_usize(&self, section: &str, key: &str) -> Option<usize> {
        self.get(section, key)?.parse().ok()
    }

    pub fn get_i32(&self, section: &str, key: &str) -> Option<i32> {
        self.get(section, key)?.parse().ok()
    }

    pub fn get_f32(&self, section: &str, key: &str) -> Option<f32> {
        self.get(section, key)?.parse().ok()
    }

    pub fn get_color(&self, section: &str, key: &str) -> Option<baram_core::Color> {
        let hex = self.get(section, key)?;
        let val = u32::from_str_radix(hex, 16).ok()?;
        Some(baram_core::Color::rgb(
            ((val >> 16) & 0xFF) as u8,
            ((val >> 8) & 0xFF) as u8,
            (val & 0xFF) as u8,
        ))
    }

    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        self.sections
            .entry(section.to_string())
            .or_insert_with(BTreeMap::new)
            .insert(key.to_string(), value.to_string());
    }

    pub fn save_to_vfs(&self, path: &str) {
        let mut buf = String::new();
        buf.push_str("# BaramOS Configuration\n");
        buf.push_str("# Format: section.key = value\n\n");

        for (section, keys) in &self.sections {
            buf.push_str(&alloc::format!("[{}]\n", section));
            for (key, value) in keys {
                buf.push_str(&alloc::format!("{} = {}\n", key, value));
            }
            buf.push('\n');
        }

        super::vfs::write_file(path, buf.as_bytes());
    }
}

pub static mut GLOBAL_CONFIG: Option<Config> = None;

pub fn init_config() {
    let config = Config::load_from_vfs("EFI/BOOT/config.txt");
    unsafe {
        GLOBAL_CONFIG = Some(config);
    }
}

pub fn get_config() -> &'static Config {
    unsafe { GLOBAL_CONFIG.as_ref().expect("Config not initialized") }
}

pub fn get_config_mut() -> &'static mut Config {
    unsafe { GLOBAL_CONFIG.as_mut().expect("Config not initialized") }
}

pub fn save_config() {
    get_config().save_to_vfs("EFI/BOOT/config.txt");
}

pub fn get_usize(section: &str, key: &str, default: usize) -> usize {
    get_config().get_usize(section, key).unwrap_or(default)
}

pub fn get_i32(section: &str, key: &str, default: i32) -> i32 {
    get_config().get_i32(section, key).unwrap_or(default)
}

pub fn get_f32(section: &str, key: &str, default: f32) -> f32 {
    get_config().get_f32(section, key).unwrap_or(default)
}

pub fn get_color(section: &str, key: &str, default: baram_core::Color) -> baram_core::Color {
    get_config().get_color(section, key).unwrap_or(default)
}
