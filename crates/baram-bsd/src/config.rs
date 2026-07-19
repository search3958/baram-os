use alloc::string::{String, ToString};
use alloc::vec::Vec;

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
        let mut config = Self::new();
        let data = super::vfs::read_file(path);
        if data.is_empty() {
            return config;
        }
        let text = String::from_utf8(data.to_vec()).unwrap_or_default();
        if let Some(node) = parse_xml(&text) {
            config.root = node;
        }
        config
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
            buf.push_str(s);
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
                    buf.push_str(s);
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

fn parse_xml(text: &str) -> Option<XmlNode> {
    let bytes = text.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        if bytes[pos] == b'<' {
            let (node, _new_pos) = parse_element(bytes, pos)?;
            return Some(node);
        }
        pos += 1;
    }
    None
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
            break;
        }

        if bytes[pos] == b'<' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'/' {
                let close_tag = read_tag_name(bytes, &mut {
                    pos += 2;
                    pos
                })?;
                skip_whitespace(bytes, &mut pos);
                if pos < bytes.len() && bytes[pos] == b'>' {
                    pos += 1;
                }
                if close_tag == tag {
                    break;
                }
                children.push(XmlNode::element(&close_tag));
                continue;
            }
            if let Some((child, new_pos)) = parse_element(bytes, pos) {
                children.push(child);
                pos = new_pos;
            } else {
                break;
            }
        } else {
            let text_start = pos;
            while pos < bytes.len() && bytes[pos] != b'<' {
                pos += 1;
            }
            let text_content = core::str::from_utf8(&bytes[text_start..pos])
                .unwrap_or("")
                .trim();
            if !text_content.is_empty() {
                children.push(XmlNode::text(text_content));
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
    core::str::from_utf8(&bytes[start..*pos])
        .ok()
        .map(|s| s.to_string())
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

pub static mut GLOBAL_CONFIG: Option<Config> = None;

pub fn init_config() {
    let config = Config::load_from_vfs("EFI/BOOT/config.xml");
    unsafe {
        GLOBAL_CONFIG = Some(config);
    }
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

pub fn save_config() {
    let xml = get_config().to_xml();
    super::vfs::write_file("EFI/BOOT/config.xml", xml.as_bytes());
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
