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

/// Load an HTML application and any local stylesheets referenced with
/// `<link rel="stylesheet" href="...">`.
///
/// Stylesheets are deliberately restricted to simple filenames in `/apps`.
/// This keeps an HTML app from reading arbitrary files from the EFI volume.
pub fn load_html_document(name: &str) -> (alloc::string::String, alloc::string::String) {
    use alloc::string::String;

    let html = load_app_source(name);
    let mut css = String::new();
    let mut cursor = 0;
    while let Some(link_start_rel) = html[cursor..].find("<link") {
        let link_start = cursor + link_start_rel;
        let Some(link_end_rel) = html[link_start..].find('>') else {
            break;
        };
        let link_end = link_start + link_end_rel;
        let tag = &html[link_start..=link_end];
        if tag.to_ascii_lowercase().contains("stylesheet") {
            if let Some(href) = find_html_attr(tag, "href") {
                if is_safe_app_filename(href) && href.to_ascii_lowercase().ends_with(".css") {
                    let stylesheet = load_app_source(href);
                    if !stylesheet.is_empty() {
                        css.push_str(&stylesheet);
                        css.push('\n');
                    }
                }
            }
        }
        cursor = link_end + 1;
    }
    (html, css)
}

/// Parse an `app://name.ext` URI into a safe application filename.
pub fn parse_app_uri(uri: &str) -> Option<&str> {
    let name = uri.trim().strip_prefix("app://")?;
    let name = name.split(['?', '#']).next().unwrap_or(name);
    if is_safe_app_filename(name)
        && (name.ends_with(".warp")
            || name.ends_with(".html")
            || name.ends_with(".htm")
            || name.ends_with(".u1"))
    {
        Some(name)
    } else {
        None
    }
}

pub fn is_safe_app_filename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn find_html_attr<'a>(tag: &'a str, attr_name: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let pos = lower.find(attr_name)?;
    let mut cursor = pos + attr_name.len();
    let bytes = tag.as_bytes();
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    if cursor >= bytes.len() || bytes[cursor] != b'=' {
        return None;
    }
    cursor += 1;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    if cursor >= bytes.len() {
        return None;
    }
    if bytes[cursor] == b'"' || bytes[cursor] == b'\'' {
        let quote = bytes[cursor];
        cursor += 1;
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor] != quote {
            cursor += 1;
        }
        Some(&tag[start..cursor])
    } else {
        let start = cursor;
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'>'
        {
            cursor += 1;
        }
        Some(&tag[start..cursor])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_uri_accepts_only_local_app_filenames() {
        assert_eq!(parse_app_uri("app://settings.warp"), Some("settings.warp"));
        assert_eq!(
            parse_app_uri("app://web-demo.html#top"),
            Some("web-demo.html")
        );
        assert_eq!(parse_app_uri("app://../config.xml"), None);
        assert_eq!(parse_app_uri("app://folder/demo.warp"), None);
    }
}
