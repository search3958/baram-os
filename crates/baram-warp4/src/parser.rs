use super::*;

pub(crate) struct XmlParser {
    chars: Vec<char>,
    pos: usize,
}
impl XmlParser {
    pub(crate) fn new(s: &str) -> Self {
        Self {
            chars: s.chars().collect(),
            pos: 0,
        }
    }
    pub(crate) fn parse_element(
        &mut self,
        parent: Option<usize>,
        nodes: &mut Vec<Node>,
    ) -> Option<usize> {
        while self.pos < self.chars.len() && self.chars[self.pos] != '<' {
            self.pos += 1;
        }
        if self.pos >= self.chars.len() {
            return None;
        }
        self.pos += 1;
        if self.chars.get(self.pos) == Some(&'?') {
            while self.pos < self.chars.len() && self.chars[self.pos] != '>' {
                self.pos += 1;
            }
            self.pos += 1;
            return self.parse_element(parent, nodes);
        }
        if self.chars.get(self.pos) == Some(&'!') {
            while self.pos < self.chars.len() && self.chars[self.pos] != '>' {
                self.pos += 1;
            }
            self.pos += 1;
            return self.parse_element(parent, nodes);
        }
        let tag = self.ident();
        if tag.is_empty() {
            return None;
        }
        let attrs = self.attrs();
        let self_close = self.chars.get(self.pos.saturating_sub(2)) == Some(&'/');
        if nodes.len() >= MAX_NODES {
            return None;
        }
        let idx = nodes.len();
        nodes.push(Node {
            tag,
            attrs,
            parent,
            ..Node::default()
        });
        if self_close {
            return Some(idx);
        }
        loop {
            self.skip();
            if self.pos >= self.chars.len() {
                break;
            }
            if self.chars[self.pos] == '<' && self.chars.get(self.pos + 1) == Some(&'/') {
                while self.pos < self.chars.len() && self.chars[self.pos] != '>' {
                    self.pos += 1;
                }
                self.pos += 1;
                break;
            }
            if self.chars[self.pos] == '<' {
                if let Some(c) = self.parse_element(Some(idx), nodes) {
                    nodes[idx].children.push(c);
                }
            } else {
                let start = self.pos;
                while self.pos < self.chars.len() && self.chars[self.pos] != '<' {
                    self.pos += 1;
                }
                let s: String = self.chars[start..self.pos].iter().collect();
                let s = s.trim();
                if !s.is_empty() {
                    nodes[idx].text.push_str(s);
                    set_attr(&mut nodes[idx], "text", s);
                }
            }
        }
        Some(idx)
    }
    fn ident(&mut self) -> String {
        let s = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_alphanumeric()
                || matches!(self.chars[self.pos], ':' | '_' | '-'))
        {
            self.pos += 1;
        }
        self.chars[s..self.pos].iter().collect()
    }
    fn attrs(&mut self) -> Vec<Attr> {
        let mut out = Vec::new();
        loop {
            self.skip();
            if self.pos >= self.chars.len() || self.chars[self.pos] == '>' {
                self.pos += 1;
                break;
            }
            if self.chars[self.pos] == '/' {
                self.pos += 1;
                self.skip();
                if self.chars.get(self.pos) == Some(&'>') {
                    self.pos += 1;
                }
                break;
            }
            let raw = self.ident();
            self.skip();
            if self.chars.get(self.pos) != Some(&'=') {
                continue;
            }
            self.pos += 1;
            self.skip();
            let q = self.chars.get(self.pos).copied().unwrap_or('"');
            if q == '"' || q == '\'' {
                self.pos += 1;
                let s = self.pos;
                while self.pos < self.chars.len() && self.chars[self.pos] != q {
                    self.pos += 1;
                }
                let value: String = self.chars[s..self.pos].iter().collect();
                self.pos += 1;
                out.push(Attr {
                    key: raw.rsplit(':').next().unwrap_or(&raw).into(),
                    value: decode(&value),
                });
            }
        }
        out
    }
    fn skip(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }
}

#[derive(Clone)]
enum ScriptNode {
    Raw(String),
    Block {
        header: String,
        body: Vec<ScriptNode>,
    },
}

struct ScriptParser {
    chars: Vec<char>,
    pos: usize,
}

impl ScriptParser {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
        }
    }
    fn parse_program(&mut self, stop_on_brace: bool) -> Vec<ScriptNode> {
        let mut nodes = Vec::new();
        while self.pos < self.chars.len() {
            while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
                self.pos += 1;
            }
            if self.pos >= self.chars.len() {
                break;
            }
            let start = self.pos;
            let mut square = 0i32;
            let mut paren = 0i32;
            let mut consumed = false;
            while self.pos < self.chars.len() {
                let c = self.chars[self.pos];
                match c {
                    '[' => square += 1,
                    ']' => square -= 1,
                    '(' => paren += 1,
                    ')' => paren -= 1,
                    '{' if square == 0 && paren == 0 => {
                        let header: String = self.chars[start..self.pos]
                            .iter()
                            .collect::<String>()
                            .trim()
                            .into();
                        self.pos += 1;
                        let body = self.parse_program(true);
                        nodes.push(ScriptNode::Block { header, body });
                        consumed = true;
                        break;
                    }
                    '\n' if square == 0 && paren == 0 => {
                        let raw: String = self.chars[start..self.pos]
                            .iter()
                            .collect::<String>()
                            .trim()
                            .into();
                        self.pos += 1;
                        if !raw.is_empty() {
                            nodes.push(ScriptNode::Raw(raw));
                        }
                        consumed = true;
                        break;
                    }
                    '}' if square == 0 && paren == 0 => {
                        if stop_on_brace {
                            self.pos += 1;
                            return nodes;
                        }
                        self.pos += 1;
                        consumed = true;
                        break;
                    }
                    _ => {}
                }
                self.pos += 1;
            }
            if !consumed && self.pos >= self.chars.len() {
                let raw: String = self.chars[start..self.pos]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .into();
                if !raw.is_empty() {
                    nodes.push(ScriptNode::Raw(raw));
                }
            }
        }
        nodes
    }
}

pub(crate) fn parse_script(source: &str) -> Script {
    let mut parser = ScriptParser::new(source);
    let nodes = parser.parse_program(false);
    let mut script = Script::default();
    for node in nodes {
        match node {
            ScriptNode::Raw(raw) => {
                if let Some(action) = parse_script_raw(&raw) {
                    script.init.push(action);
                }
            }
            ScriptNode::Block { header, body } => {
                let header = header.trim();
                if let Some(target) = header.strip_prefix("WarpUI.OnClick") {
                    let target = target.trim();
                    if !target.is_empty() {
                        script
                            .clicks
                            .push((target.into(), compile_script_nodes(&body)));
                    }
                } else if header.starts_with("fun") {
                    let name = header[3..]
                        .trim()
                        .trim_start_matches('(')
                        .trim_end_matches(')')
                        .trim();
                    if !name.is_empty() {
                        script
                            .functions
                            .push((name.into(), compile_script_nodes(&body)));
                    }
                }
            }
        }
    }
    script
}

fn compile_script_nodes(nodes: &[ScriptNode]) -> Vec<Action> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            ScriptNode::Raw(raw) => {
                if let Some(action) = parse_script_raw(raw) {
                    out.push(action);
                }
            }
            ScriptNode::Block { header, body } => {
                if let Some(condition_text) = header.strip_prefix("if ") {
                    if let Some((left, op, right)) = condition(condition_text.trim()) {
                        out.push(Action::If {
                            left,
                            op,
                            right,
                            body: compile_script_nodes(body),
                        });
                    }
                }
            }
        }
    }
    out
}

fn parse_script_raw(raw: &str) -> Option<Action> {
    let line = raw.split_once('#').map_or(raw, |(before, _)| before).trim();
    if line.is_empty() || line.starts_with("//") {
        return None;
    }
    if line == "break" {
        return Some(Action::Break);
    }
    if let Some(rest) = line.strip_prefix("if ") {
        if let Some(open) = find_top_level(rest, '(') {
            if let Some((body, _)) = balanced(rest, open) {
                if let Some((left, op, right)) = condition(rest[..open].trim()) {
                    let mut parser = ScriptParser::new(&body);
                    return Some(Action::If {
                        left,
                        op,
                        right,
                        body: compile_script_nodes(&parser.parse_program(false)),
                    });
                }
            }
        }
    }
    parse_command(line)
}
fn parse_command(line: &str) -> Option<Action> {
    let (name, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(Action::Command {
            name: name.into(),
            target: String::new(),
            value: String::new(),
        });
    }
    if name == "BaramOS" {
        if let Some(command) = parse_baram_file_command(rest) {
            return Some(command);
        }
        let rest = rest.strip_prefix("run").map(str::trim).unwrap_or(rest);
        let value = rest.trim_start_matches('=').trim();
        if value.starts_with('(') {
            let (value, _) = balanced(value, 0)?;
            return Some(Action::Command {
                name: "run".into(),
                target: String::new(),
                value,
            });
        }
        return Some(Action::Command {
            name: "run".into(),
            target: String::new(),
            value: value.into(),
        });
    }
    if name == "fun" && rest.starts_with('(') {
        let (value, _) = balanced(rest, 0)?;
        return Some(Action::Call(value.trim().into()));
    }
    let (target, value) = if let Some(open) = rest.find('(') {
        let target = rest[..open].trim();
        let (v, _) = balanced(rest, open)?;
        (target.into(), v)
    } else {
        (String::new(), rest.into())
    };
    Some(Action::Command {
        name: name.into(),
        target,
        value,
    })
}

fn parse_baram_file_command(rest: &str) -> Option<Action> {
    let (operation, args) = rest.split_once(char::is_whitespace)?;
    if operation != "getFile" && operation != "uploadFile" {
        return None;
    }
    let args = args.trim();
    let (target, value) = args.split_once(char::is_whitespace)?;
    let target = target.trim();
    let remainder = value.trim();
    let open = remainder.find('(')?;
    let (value, _) = balanced(remainder, open)?;
    Some(Action::Command {
        name: format!("BaramOS.{operation}"),
        target: target.into(),
        value,
    })
}

pub(crate) fn is_safe_script_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn balanced(s: &str, open: usize) -> Option<(String, usize)> {
    let mut depth = 0;
    for (i, c) in s.char_indices().skip(open) {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                return Some((s[open + 1..i].into(), i + 1));
            }
        }
    }
    None
}
fn find_top_level(s: &str, wanted: char) -> Option<usize> {
    let mut square = 0i32;
    let mut paren = 0i32;
    for (i, c) in s.char_indices() {
        if c == wanted && square == 0 && paren == 0 {
            return Some(i);
        }
        match c {
            '[' => square += 1,
            ']' => square -= 1,
            '(' => paren += 1,
            ')' => paren -= 1,
            _ => {}
        }
    }
    None
}
fn condition(s: &str) -> Option<(String, String, String)> {
    let chars: Vec<char> = s.chars().collect();
    let mut square = 0i32;
    let mut paren = 0i32;
    let mut i = 0;
    while i < chars.len() {
        let op = if square == 0
            && paren == 0
            && i + 1 < chars.len()
            && chars[i] == '!'
            && chars[i + 1] == '='
        {
            Some(("!=", 2))
        } else if square == 0 && paren == 0 && chars[i] == '=' {
            Some(("=", 1))
        } else if square == 0 && paren == 0 && chars[i] == '<' {
            Some(("<", 1))
        } else if square == 0 && paren == 0 && chars[i] == '>' {
            Some((">", 1))
        } else {
            None
        };
        if let Some((op, width)) = op {
            return Some((
                chars[..i].iter().collect::<String>().trim().into(),
                op.into(),
                chars[i + width..].iter().collect::<String>().trim().into(),
            ));
        }
        match chars[i] {
            '[' => square += 1,
            ']' => square -= 1,
            '(' => paren += 1,
            ')' => paren -= 1,
            _ => {}
        }
        i += 1;
    }
    None
}
pub(crate) fn set_attr(n: &mut Node, key: &str, value: &str) {
    if let Some(a) = n.attrs.iter_mut().find(|a| a.key == key) {
        a.value = value.into();
    } else {
        n.attrs.push(Attr {
            key: key.into(),
            value: value.into(),
        });
    }
}
fn decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}
pub(crate) fn ini(s: &str, key: &str) -> Option<String> {
    s.lines().find_map(|l| {
        let (a, b) = l.split_once('=')?;
        (a.trim() == key).then(|| b.trim().into())
    })
}
pub(crate) fn parse_i32(s: &str) -> i32 {
    s.trim().parse().unwrap_or(0)
}
pub(crate) fn is_match_parent(s: &str) -> bool {
    matches!(s.trim(), "match_parent" | "fill_parent")
}
pub(crate) fn is_zero_dimension(s: &str) -> bool {
    let raw = s.trim();
    !raw.is_empty() && parse_dim(raw, i32::MIN) == 0
}
pub(crate) fn duration_ns(s: &str) -> Option<u64> {
    let value = s.trim();
    let (number, multiplier) = if let Some(v) = value.strip_suffix("ns") {
        (v, 1u64)
    } else if let Some(v) = value.strip_suffix("us") {
        (v, 1_000)
    } else if let Some(v) = value.strip_suffix("ms") {
        (v, 1_000_000)
    } else if let Some(v) = value.strip_suffix('s') {
        (v, 1_000_000_000)
    } else if let Some(v) = value.strip_suffix('m') {
        (v, 60_000_000_000)
    } else if let Some(v) = value.strip_suffix('h') {
        (v, 3_600_000_000_000)
    } else {
        return None;
    };
    let n = number.trim().parse::<f64>().ok()?;
    Some((n * multiplier as f64).max(0.0) as u64)
}
