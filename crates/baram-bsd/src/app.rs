use super::vfs;

pub const FALLBACK_INDEX: &str = include_str!("../../../app/index.yaml");

pub fn read_index_yaml() -> alloc::string::String {
    let content = vfs::read_file_str("apps/index.yaml");
    if !content.is_empty() {
        content
    } else {
        alloc::string::String::from(FALLBACK_INDEX)
    }
}

pub fn load_app_source(name: &str) -> alloc::string::String {
    let path = alloc::format!("apps/{}", name);
    vfs::read_file_str(&path)
}
