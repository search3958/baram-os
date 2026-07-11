use crate::vfs;

pub const FALLBACK_INDEX: &str = include_str!("app/index.yaml");

pub fn load_app_source(name: &str) -> alloc::string::String {
    let path = alloc::format!("apps/{}", name);
    vfs::read_file_str(&path)
}
