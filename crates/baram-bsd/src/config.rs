use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

const DEFAULT_CONFIG_XML: &[u8] = include_bytes!("../../../config.xml");

#[derive(Clone)]
pub enum XmlNode {
    Element { tag: String, children: Vec<XmlNode> },
    Text(String),
}

impl XmlNode {
    pub fn element(tag: &str) -> Self {
        XmlNode::Element {
            tag: tag.to_string(),
            children: Vec::new(),
        }
    }

    pub fn text(content: &str) -> Self {
        XmlNode::Text(content.to_string())
    }

    pub fn tag(&self) -> &str {
        match self {
            XmlNode::Element { tag, .. } => tag,
            XmlNode::Text(_) => "",
        }
    }

    pub fn text_content(&self) -> &str {
        match self {
            XmlNode::Text(s) => s,
            XmlNode::Element { children, .. } => {
                if children.len() == 1 {
                    if let XmlNode::Text(s) = &children[0] {
                        return s;
                    }
                }
                ""
            }
        }
    }

    pub fn children(&self) -> &[XmlNode] {
        match self {
            XmlNode::Element { children, .. } => children,
            XmlNode::Text(_) => &[],
        }
    }

    pub fn children_mut(&mut self) -> &mut Vec<XmlNode> {
        match self {
            XmlNode::Element { children, .. } => children,
            XmlNode::Text(_) => panic!("Text nodes have no children"),
        }
    }
}

#[derive(Clone)]
pub struct Config {
    root: XmlNode,
}

impl Config {
    pub fn new() -> Self {
        Self {
            root: XmlNode::element("baram-os-config"),
        }
    }

    pub fn load_from_vfs(path: &str) -> Self {
        let data = super::vfs::read_file(path);
        if data.is_empty() {
            return Self::load_from_bytes(DEFAULT_CONFIG_XML);
        }
        let text = String::from_utf8(data.to_vec()).unwrap_or_default();
        if let Some(node) = parse_xml(&text) {
            return Self { root: node };
        }
        // A hand-edited or previously corrupted file must not leave every
        // setting absent. Use the embedded known-good document for this boot.
        Self::load_from_bytes(DEFAULT_CONFIG_XML)
    }

    pub fn load_from_bytes(data: &[u8]) -> Self {
        let mut config = Self::new();
        let text = String::from_utf8(data.to_vec()).unwrap_or_default();
        if let Some(node) = parse_xml(&text) {
            config.root = node;
        }
        config
    }

    pub fn get(&self, path: &str) -> Option<&str> {
        let normalized = path.replace('.', "/");
        let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
        self.find_node(&parts).map(|n| n.text_content())
    }

    pub fn get_u32(&self, path: &str) -> Option<u32> {
        self.get(path)?.parse().ok()
    }

    pub fn get_usize(&self, path: &str) -> Option<usize> {
        self.get(path)?.parse().ok()
    }

    pub fn get_i32(&self, path: &str) -> Option<i32> {
        self.get(path)?.parse().ok()
    }

    pub fn get_f32(&self, path: &str) -> Option<f32> {
        self.get(path)?.parse().ok()
    }

    pub fn get_color(&self, path: &str) -> Option<baram_core::Color> {
        let hex = self.get(path)?;
        let hex = hex.trim_start_matches('#');
        let val = u32::from_str_radix(hex, 16).ok()?;
        Some(baram_core::Color::rgb(
            ((val >> 16) & 0xFF) as u8,
            ((val >> 8) & 0xFF) as u8,
            (val & 0xFF) as u8,
        ))
    }

    pub fn set(&mut self, path: &str, value: &str) {
        let normalized = path.replace('.', "/");
        let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return;
        }
        let node = self.find_or_create_node(&parts);
        if let XmlNode::Element { children, .. } = node {
            children.clear();
            children.push(XmlNode::text(value));
        }
    }

    pub fn remove(&mut self, path: &str) -> bool {
        let normalized = path.replace('.', "/");
        let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return false;
        }
        self.remove_node(&parts)
    }

    pub fn to_xml(&self) -> String {
        let mut buf = String::new();
        serialize_node(&self.root, &mut buf, 0);
        buf
    }

    pub fn save_to_vfs(&self, path: &str) {
        let xml = self.to_xml();
        super::vfs::write_file(path, xml.as_bytes());
    }

    fn find_node(&self, parts: &[&str]) -> Option<&XmlNode> {
        let mut current = &self.root;
        for &part in parts {
            current = current.children().iter().find(|c| c.tag() == part)?;
        }
        Some(current)
    }

    fn find_or_create_node(&mut self, parts: &[&str]) -> &mut XmlNode {
        let mut current = &mut self.root;
        for &part in parts {
            let idx = current.children().iter().position(|c| c.tag() == part);
            match idx {
                Some(i) => {
                    current = &mut current.children_mut()[i];
                }
                None => {
                    let len = current.children_mut().len();
                    current.children_mut().push(XmlNode::element(part));
                    current = &mut current.children_mut()[len];
                }
            }
        }
        current
    }

    fn remove_node(&mut self, parts: &[&str]) -> bool {
        if parts.len() == 1 {
            let children = match &mut self.root {
                XmlNode::Element { children, .. } => children,
                _ => return false,
            };
            if let Some(i) = children.iter().position(|c| c.tag() == parts[0]) {
                children.remove(i);
                return true;
            }
            return false;
        }
        let parent_parts = &parts[..parts.len() - 1];
        let target_tag = parts[parts.len() - 1];
        if let Some(parent) = self.find_node_mut(parent_parts) {
            let children = match parent {
                XmlNode::Element { children, .. } => children,
                _ => return false,
            };
            if let Some(i) = children.iter().position(|c| c.tag() == target_tag) {
                children.remove(i);
                return true;
            }
        }
        false
    }

    fn find_node_mut(&mut self, parts: &[&str]) -> Option<&mut XmlNode> {
        let mut current = &mut self.root;
        for &part in parts {
            let idx = current.children().iter().position(|c| c.tag() == part)?;
            current = &mut current.children_mut()[idx];
        }
        Some(current)
    }
}

fn serialize_node(node: &XmlNode, buf: &mut String, indent: usize) {
    let pad = "    ".repeat(indent);
    match node {
        XmlNode::Text(s) => {
            buf.push_str(&pad);
            push_escaped_text(buf, s);
            buf.push('\n');
        }
        XmlNode::Element { tag, children } => {
            if children.is_empty() {
                buf.push_str(&pad);
                buf.push('<');
                buf.push_str(tag);
                buf.push_str("/>\n");
            } else if children.len() == 1 {
                if let XmlNode::Text(s) = &children[0] {
                    buf.push_str(&pad);
                    buf.push('<');
                    buf.push_str(tag);
                    buf.push('>');
                    push_escaped_text(buf, s);
                    buf.push_str("</");
                    buf.push_str(tag);
                    buf.push_str(">\n");
                    return;
                }
                buf.push_str(&pad);
                buf.push('<');
                buf.push_str(tag);
                buf.push_str(">\n");
                for child in children {
                    serialize_node(child, buf, indent + 1);
                }
                buf.push_str(&pad);
                buf.push_str("</");
                buf.push_str(tag);
                buf.push_str(">\n");
            } else {
                buf.push_str(&pad);
                buf.push('<');
                buf.push_str(tag);
                buf.push_str(">\n");
                for child in children {
                    serialize_node(child, buf, indent + 1);
                }
                buf.push_str(&pad);
                buf.push_str("</");
                buf.push_str(tag);
                buf.push_str(">\n");
            }
        }
    }
}

fn push_escaped_text(buf: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => buf.push_str("&amp;"),
            '<' => buf.push_str("&lt;"),
            '>' => buf.push_str("&gt;"),
            '"' => buf.push_str("&quot;"),
            '\'' => buf.push_str("&apos;"),
            _ => buf.push(ch),
        }
    }
}

fn decode_text(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn parse_xml(text: &str) -> Option<XmlNode> {
    let bytes = text.as_bytes();
    let mut pos = 0;
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        pos = 3;
    }
    skip_xml_misc(bytes, &mut pos)?;
    let (node, new_pos) = parse_element(bytes, pos)?;
    pos = new_pos;
    skip_xml_misc(bytes, &mut pos)?;
    (pos == bytes.len()).then_some(node)
}

fn parse_element(bytes: &[u8], start: usize) -> Option<(XmlNode, usize)> {
    if bytes[start] != b'<' {
        return None;
    }

    let mut pos = start + 1;
    let tag = read_tag_name(bytes, &mut pos)?;

    skip_whitespace(bytes, &mut pos);

    if pos < bytes.len() && bytes[pos] == b'/' && pos + 1 < bytes.len() && bytes[pos + 1] == b'>' {
        return Some((XmlNode::element(&tag), pos + 2));
    }

    if pos < bytes.len() && bytes[pos] == b'>' {
        pos += 1;
    } else {
        return None;
    }

    let mut children = Vec::new();

    loop {
        skip_whitespace(bytes, &mut pos);

        if pos >= bytes.len() {
            return None;
        }

        if bytes[pos] == b'<' {
            if bytes[pos..].starts_with(b"<!--") {
                skip_xml_comment(bytes, &mut pos)?;
                continue;
            }
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'/' {
                pos += 2;
                let close_tag = read_tag_name(bytes, &mut pos)?;
                skip_whitespace(bytes, &mut pos);
                if pos >= bytes.len() || bytes[pos] != b'>' {
                    return None;
                }
                pos += 1;
                if close_tag != tag {
                    return None;
                }
                break;
            }
            let (child, new_pos) = parse_element(bytes, pos)?;
            children.push(child);
            pos = new_pos;
        } else {
            let text_start = pos;
            while pos < bytes.len() && bytes[pos] != b'<' {
                pos += 1;
            }
            let text_content = core::str::from_utf8(&bytes[text_start..pos])
                .unwrap_or("")
                .trim();
            if !text_content.is_empty() {
                children.push(XmlNode::text(&decode_text(text_content)));
            }
        }
    }

    Some((XmlNode::Element { tag, children }, pos))
}

fn read_tag_name(bytes: &[u8], pos: &mut usize) -> Option<String> {
    let start = *pos;
    while *pos < bytes.len()
        && bytes[*pos] != b' '
        && bytes[*pos] != b'\t'
        && bytes[*pos] != b'\n'
        && bytes[*pos] != b'\r'
        && bytes[*pos] != b'>'
        && bytes[*pos] != b'/'
    {
        *pos += 1;
    }
    if *pos == start {
        return None;
    }
    let name = core::str::from_utf8(&bytes[start..*pos]).ok()?;
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_')
        || chars.any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')))
    {
        return None;
    }
    Some(name.to_string())
}

fn skip_whitespace(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len()
        && (bytes[*pos] == b' '
            || bytes[*pos] == b'\t'
            || bytes[*pos] == b'\n'
            || bytes[*pos] == b'\r')
    {
        *pos += 1;
    }
}

fn skip_xml_comment(bytes: &[u8], pos: &mut usize) -> Option<()> {
    if !bytes[*pos..].starts_with(b"<!--") {
        return None;
    }
    *pos += 4;
    while *pos + 2 < bytes.len() {
        if bytes[*pos..].starts_with(b"-->") {
            *pos += 3;
            return Some(());
        }
        *pos += 1;
    }
    None
}

fn skip_xml_misc(bytes: &[u8], pos: &mut usize) -> Option<()> {
    loop {
        skip_whitespace(bytes, pos);
        if *pos >= bytes.len() {
            return Some(());
        }
        if bytes[*pos..].starts_with(b"<!--") {
            skip_xml_comment(bytes, pos)?;
            continue;
        }
        if bytes[*pos..].starts_with(b"<?xml") {
            *pos += 5;
            while *pos + 1 < bytes.len() && !bytes[*pos..].starts_with(b"?>") {
                *pos += 1;
            }
            if *pos + 1 >= bytes.len() {
                return None;
            }
            *pos += 2;
            continue;
        }
        return Some(());
    }
}

pub static mut GLOBAL_CONFIG: Option<Config> = None;
static CONFIG_REVISION: AtomicUsize = AtomicUsize::new(0);

pub fn init_config() {
    let config = Config::load_from_vfs("EFI/BOOT/config.xml");
    unsafe {
        GLOBAL_CONFIG = Some(config);
    }
    CONFIG_REVISION.fetch_add(1, Ordering::Release);
}

pub fn reset_to_default() {
    let default_config = Config::load_from_bytes(DEFAULT_CONFIG_XML);
    unsafe {
        GLOBAL_CONFIG = Some(default_config);
    }
    save_config();
}

pub fn get_config() -> &'static Config {
    unsafe { GLOBAL_CONFIG.as_ref().expect("Config not initialized") }
}

pub fn get_config_mut() -> &'static mut Config {
    unsafe { GLOBAL_CONFIG.as_mut().expect("Config not initialized") }
}

pub fn save_config() -> bool {
    let xml = get_config().to_xml();
    let saved = super::vfs::write_file("EFI/BOOT/config.xml", xml.as_bytes());
    if saved {
        CONFIG_REVISION.fetch_add(1, Ordering::Release);
    }
    saved
}

/// Apply a configuration mutation and keep it in memory only when the FAT
/// write is durable. This prevents a failed real-hardware write from making
/// the running OS believe a setting was saved successfully.
pub fn update_and_save(update: impl FnOnce(&mut Config)) -> bool {
    let backup = get_config().clone();
    update(get_config_mut());
    if save_config() {
        true
    } else {
        unsafe {
            GLOBAL_CONFIG = Some(backup);
        }
        false
    }
}

/// Changes whenever settings are loaded or saved. Long-lived device drivers
/// use this to refresh cached values without parsing the config every report.
pub fn revision() -> usize {
    CONFIG_REVISION.load(Ordering::Acquire)
}

pub fn get_usize(path: &str, default: usize) -> usize {
    get_config().get_usize(path).unwrap_or(default)
}

pub fn get_i32(path: &str, default: i32) -> i32 {
    get_config().get_i32(path).unwrap_or(default)
}

pub fn get_f32(path: &str, default: f32) -> f32 {
    get_config().get_f32(path).unwrap_or(default)
}

pub fn get_color(path: &str, default: baram_core::Color) -> baram_core::Color {
    get_config().get_color(path).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_closing_tags_round_trip_without_becoming_text() {
        let source = "<baram-os-config><system><done>1</done><reset><option/></reset></system></baram-os-config>";
        let config = Config::load_from_bytes(source.as_bytes());

        assert_eq!(config.get("system/done"), Some("1"));
        assert_eq!(config.get("system/reset/option"), Some(""));
        assert_eq!(
            config.to_xml(),
            "<baram-os-config>\n    <system>\n        <done>1</done>\n        <reset>\n            <option/>\n        </reset>\n    </system>\n</baram-os-config>\n"
        );
    }

    #[test]
    fn mismatched_closing_tag_is_rejected() {
        assert!(parse_xml("<system><done>1</reset></system>").is_none());
    }

    #[test]
    fn comments_declaration_entities_and_edits_round_trip() {
        let source = "<?xml version=\"1.0\"?><!-- settings --><baram-os-config><trackpad><speed>5.0</speed><!-- curve --></trackpad><label>A &amp; B</label></baram-os-config>";
        let mut config = Config::load_from_bytes(source.as_bytes());
        assert_eq!(config.get("trackpad/speed"), Some("5.0"));
        assert_eq!(config.get("label"), Some("A & B"));

        config.set("trackpad/speed", "6.0");
        let saved = config.to_xml();
        assert!(saved.contains("<speed>6.0</speed>"));
        assert!(saved.contains("<label>A &amp; B</label>"));
        let loaded_again = Config::load_from_bytes(saved.as_bytes());
        assert_eq!(loaded_again.get("trackpad/speed"), Some("6.0"));
        assert_eq!(loaded_again.get("label"), Some("A & B"));
    }

    #[test]
    fn incomplete_or_trailing_xml_is_rejected() {
        assert!(parse_xml("<system><done>1</done>").is_none());
        assert!(parse_xml("<system></system>garbage").is_none());
        assert!(parse_xml("<!-- unterminated").is_none());
    }

    #[test]
    fn missing_config_uses_embedded_defaults() {
        let config = Config::load_from_vfs("missing.xml");
        assert_eq!(config.get("system/done"), Some("0"));
        assert!(config.get("trackpad/speed").is_some());
    }
}
