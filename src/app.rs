use crate::vfs;

pub const FALLBACK_SETTINGS: &str = include_str!("app/settings.warp");
pub const FALLBACK_WARPDEMO: &str = include_str!("app/warpdemo.warp");
pub const FALLBACK_BLANK: &str = include_str!("app/blank.warp");
pub const FALLBACK_DEMO_U1: &str = include_str!("app/demo.u1");
pub const FALLBACK_INDEX: &str = include_str!("app/index.yaml");

pub fn load_app_source(name: &str) -> alloc::string::String {
    let path = alloc::format!("apps/{}", name);
    let content = vfs::read_file_str(&path);
    if !content.is_empty() {
        return content;
    }
    match name {
        "settings.warp" => alloc::string::String::from(FALLBACK_SETTINGS),
        "warpdemo.warp" => alloc::string::String::from(FALLBACK_WARPDEMO),
        "blank.warp" => alloc::string::String::from(FALLBACK_BLANK),
        "demo.u1" => alloc::string::String::from(FALLBACK_DEMO_U1),
        _ => alloc::string::String::new(),
    }
}
