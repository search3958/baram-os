use super::vfs;

pub const FALLBACK_INDEX: &str = include_str!("../../../files/app/index.yaml");

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

/// An in-memory Warp 3 application archive.
///
/// Every archive is loaded through its registered `.w3a` application name,
/// so common members such as `config.ini` and `main.w3u` never enter the
/// global VFS namespace.
pub struct Warp3Archive {
    app_name: alloc::string::String,
    data: alloc::vec::Vec<u8>,
    embedded: Option<alloc::vec::Vec<(alloc::string::String, alloc::string::String)>>,
}

/// The archive transport is shared by the native Warp runtimes.  The alias
/// keeps the Warp 4 crate independent from Warp 3's renderer while preserving
/// one safe TAR/VFS reader in the OS layer.
pub type Warp4Archive = Warp3Archive;

impl Warp3Archive {
    pub fn open(app_name: &str) -> Self {
        let path = alloc::format!("apps/{app_name}");
        let alias = if let Some(stem) = app_name.strip_suffix(".w4a") {
            Some(alloc::format!("apps/{stem}.s4a"))
        } else if let Some(stem) = app_name.strip_suffix(".s4a") {
            Some(alloc::format!("apps/{stem}.w4a"))
        } else {
            None
        };
        let paths = if let Some(alias) = alias.as_deref() {
            alloc::vec![path.as_str(), alias]
        } else {
            alloc::vec![path.as_str()]
        };
        Self {
            app_name: alloc::string::String::from(app_name),
            data: vfs::read_file_candidates(&paths),
            embedded: None,
        }
    }

    /// Builds an OS-owned Warp 3 resource set without registering an app or
    /// placing files in the application VFS.
    pub fn from_embedded(name: &str, sources: &[(&str, &str)]) -> Self {
        Self {
            app_name: alloc::string::String::from(name),
            data: alloc::vec::Vec::new(),
            embedded: Some(
                sources
                    .iter()
                    .map(|(name, source)| {
                        (
                            alloc::string::String::from(*name),
                            alloc::string::String::from(*source),
                        )
                    })
                    .collect(),
            ),
        }
    }

    pub fn app_name(&self) -> &str {
        &self.app_name
    }

    pub fn read_text(&self, member_name: &str) -> alloc::string::String {
        if !is_safe_archive_member(member_name) {
            return alloc::string::String::new();
        }
        let wanted = member_name.trim_start_matches("./");
        if let Some(sources) = &self.embedded {
            return sources
                .iter()
                .find(|(name, _)| name.trim_start_matches("./") == wanted)
                .map(|(_, source)| source.clone())
                .unwrap_or_default();
        }
        let Some(bytes) = self.read(member_name) else {
            return alloc::string::String::new();
        };
        alloc::string::String::from_utf8(bytes.to_vec()).unwrap_or_default()
    }

    fn read(&self, member_name: &str) -> Option<&[u8]> {
        if !is_safe_archive_member(member_name) {
            return None;
        }
        let wanted = member_name.trim_start_matches("./");
        let mut offset = 0usize;
        while offset.checked_add(512)? <= self.data.len() {
            let header = &self.data[offset..offset + 512];
            if header.iter().all(|byte| *byte == 0) {
                return None;
            }
            let name = tar_string(&header[0..100]);
            let prefix = tar_string(&header[345..500]);
            let full_name = if prefix.is_empty() {
                name
            } else {
                alloc::format!("{prefix}/{name}")
            };
            let size = tar_octal(&header[124..136])?;
            let data_start = offset + 512;
            let data_end = data_start.checked_add(size)?;
            if data_end > self.data.len() {
                return None;
            }
            if full_name.trim_start_matches("./") == wanted {
                return Some(&self.data[data_start..data_end]);
            }
            let padded = size.checked_add(511)? / 512 * 512;
            offset = data_start.checked_add(padded)?;
        }
        None
    }
}

fn is_safe_archive_member(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.split('/').any(|part| part == "..")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'))
}

fn tar_string(bytes: &[u8]) -> alloc::string::String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    alloc::string::String::from_utf8_lossy(&bytes[..end])
        .trim()
        .into()
}

fn tar_octal(bytes: &[u8]) -> Option<usize> {
    let mut value = 0usize;
    let mut found = false;
    for byte in bytes {
        match byte {
            b'0'..=b'7' => {
                value = value.checked_mul(8)?.checked_add((byte - b'0') as usize)?;
                found = true;
            }
            0 | b' ' => {}
            _ => return None,
        }
    }
    found.then_some(value)
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
            || name.ends_with(".w3a")
            || name.ends_with(".w4a")
            || name.ends_with(".s4a")
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
        assert_eq!(parse_app_uri("app://settings.w3a"), Some("settings.w3a"));
        assert_eq!(
            parse_app_uri("app://web-demo.html#top"),
            Some("web-demo.html")
        );
        assert_eq!(parse_app_uri("app://warp3demo.w3a"), Some("warp3demo.w3a"));
        assert_eq!(parse_app_uri("app://example.w4a"), Some("example.w4a"));
        assert_eq!(parse_app_uri("app://example.s4a"), Some("example.s4a"));
        assert_eq!(parse_app_uri("app://config.ini"), None);
        assert_eq!(parse_app_uri("app://../config.xml"), None);
        assert_eq!(parse_app_uri("app://folder/demo.warp"), None);
    }
}
