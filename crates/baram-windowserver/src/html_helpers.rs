fn point_in(px: i32, py: i32, x: i32, y: i32, w: i32, h: i32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn fill_box(layer: &mut LayerSystem, x: i32, y: i32, w: i32, h: i32, radius: i32, color: Color) {
    if w <= 0 || h <= 0 {
        return;
    }
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(layer.width() as i32);
    let y1 = (y + h).min(layer.height() as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    if x >= 0 && y >= 0 && x + w <= layer.width() as i32 && y + h <= layer.height() as i32 {
        if radius > 0 {
            layer.fill_rounded_rect(
                x as usize,
                y as usize,
                w as usize,
                h as usize,
                radius.min(w / 2).min(h / 2) as usize,
                color,
            );
        } else {
            layer.fill_rect(x as usize, y as usize, w as usize, h as usize, color);
        }
    } else {
        layer.fill_rect(
            x0 as usize,
            y0 as usize,
            (x1 - x0) as usize,
            (y1 - y0) as usize,
            color,
        );
    }
}

fn draw_border(
    layer: &mut LayerSystem,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
    color: Color,
    border_width: i32,
) {
    for inset in 0..border_width.min(4) {
        let iw = w - inset * 2;
        let ih = h - inset * 2;
        if iw <= 0 || ih <= 0 || x + inset < 0 || y + inset < 0 {
            continue;
        }
        // Keep the already-painted interior intact. Rounded clipping is used
        // for the background; a compact rectangular outline is preferable to
        // clearing that interior with a second rounded fill.
        let _ = radius;
        layer.rect_outline(
            (x + inset) as usize,
            (y + inset) as usize,
            iw as usize,
            ih as usize,
            color,
        );
    }
}

fn resolve_width(length: Option<Length>, available: i32) -> Option<i32> {
    match length? {
        Length::Px(value) => Some(value),
        Length::Percent(value) => Some(available * value / 100),
    }
}

fn is_textual_tag(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "a"
            | "button"
            | "label"
            | "li"
            | "pre"
            | "code"
            | "small"
            | "strong"
            | "b"
            | "em"
            | "i"
            | "span"
    )
}

fn apply_tag_defaults(tag: &str, style: &mut Style) {
    match tag {
        "html" => {}
        "body" => {
            style.padding = Edges {
                top: 18,
                right: 18,
                bottom: 18,
                left: 18,
            };
            style.gap = 12;
        }
        "h1" => {
            style.font_size = 30;
            style.bold = true;
            style.margin.bottom = 12;
        }
        "h2" => {
            style.font_size = 25;
            style.bold = true;
            style.margin.bottom = 10;
        }
        "h3" => {
            style.font_size = 21;
            style.bold = true;
            style.margin.bottom = 8;
        }
        "h4" | "h5" | "h6" => {
            style.font_size = 18;
            style.bold = true;
            style.margin.bottom = 6;
        }
        "p" => {
            style.margin.bottom = 8;
        }
        "a" => {
            style.color = Color::rgb(10, 96, 255);
            style.underline = true;
            style.display = Display::Inline;
            style.margin.bottom = 4;
        }
        "button" => {
            style.background = Some(config::get_color(
                "ui-theme/color/btn_primary",
                Color::BTN_PRIMARY,
            ));
            style.color = config::get_color("ui-theme/color/btn_text", Color::BTN_TEXT);
            style.radius = config::get_i32("ui-theme/button/corner", 20);
            style.padding = Edges {
                top: 10,
                right: 18,
                bottom: 10,
                left: 18,
            };
            style.align = TextAlign::Center;
        }
        "strong" | "b" => style.bold = true,
        "small" => style.font_size = 13,
        "code" | "pre" => {
            style.background = Some(Color::rgb(238, 240, 244));
            style.padding = Edges {
                top: 8,
                right: 10,
                bottom: 8,
                left: 10,
            };
            style.radius = 6;
        }
        "ul" | "ol" => style.padding.left = 18,
        "li" => style.margin.bottom = 5,
        _ => {}
    }
}

fn apply_declarations(style: &mut Style, declarations: &[(String, String)]) {
    for (name, value) in declarations {
        match name.as_str() {
            "display" => {
                style.display = match value.as_str() {
                    "none" => Display::None,
                    "inline" | "inline-block" => Display::Inline,
                    "flex" => Display::Flex,
                    _ => Display::Block,
                }
            }
            "flex-direction" => {
                style.flex_direction = if value == "column" {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                }
            }
            "color" => {
                if let Some(color) = parse_color(value) {
                    style.color = color;
                }
            }
            "background" | "background-color" => {
                style.background = parse_color(value);
            }
            "border-color" => {
                if let Some(color) = parse_color(value) {
                    style.border_color = color;
                }
            }
            "border-width" => style.border_width = parse_px(value).unwrap_or(0).max(0),
            "border" => parse_border(value, style),
            "border-radius" => style.radius = parse_px(value).unwrap_or(0).max(0),
            "margin" => style.margin = parse_edges(value),
            "margin-top" => style.margin.top = parse_px(value).unwrap_or(0),
            "margin-right" => style.margin.right = parse_px(value).unwrap_or(0),
            "margin-bottom" => style.margin.bottom = parse_px(value).unwrap_or(0),
            "margin-left" => style.margin.left = parse_px(value).unwrap_or(0),
            "padding" => style.padding = parse_edges(value),
            "padding-top" => style.padding.top = parse_px(value).unwrap_or(0),
            "padding-right" => style.padding.right = parse_px(value).unwrap_or(0),
            "padding-bottom" => style.padding.bottom = parse_px(value).unwrap_or(0),
            "padding-left" => style.padding.left = parse_px(value).unwrap_or(0),
            "gap" => style.gap = parse_px(value).unwrap_or(8).max(0),
            "width" => style.width = parse_length(value),
            "height" | "min-height" => style.height = parse_px(value),
            "font-size" => style.font_size = parse_px(value).unwrap_or(16).clamp(10, 48),
            "font-weight" => {
                style.bold = value == "bold"
                    || value
                        .parse::<u32>()
                        .map(|weight| weight >= 600)
                        .unwrap_or(false)
            }
            "text-decoration" => style.underline = value.contains("underline"),
            "text-align" => {
                style.align = match value.as_str() {
                    "center" => TextAlign::Center,
                    "right" | "end" => TextAlign::Right,
                    _ => TextAlign::Left,
                }
            }
            _ => {}
        }
    }
}

fn parse_border(value: &str, style: &mut Style) {
    for token in value.split_ascii_whitespace() {
        if let Some(width) = parse_px(token) {
            style.border_width = width.max(0);
        } else if let Some(color) = parse_color(token) {
            style.border_color = color;
        }
    }
}

fn parse_edges(value: &str) -> Edges {
    let values: Vec<i32> = value
        .split_ascii_whitespace()
        .filter_map(parse_px)
        .collect();
    match values.as_slice() {
        [all] => Edges {
            top: *all,
            right: *all,
            bottom: *all,
            left: *all,
        },
        [vertical, horizontal] => Edges {
            top: *vertical,
            right: *horizontal,
            bottom: *vertical,
            left: *horizontal,
        },
        [top, horizontal, bottom] => Edges {
            top: *top,
            right: *horizontal,
            bottom: *bottom,
            left: *horizontal,
        },
        [top, right, bottom, left, ..] => Edges {
            top: *top,
            right: *right,
            bottom: *bottom,
            left: *left,
        },
        _ => Edges::default(),
    }
}

fn parse_px(value: &str) -> Option<i32> {
    let value = value.trim();
    if value == "auto" {
        return None;
    }
    let number = value
        .strip_suffix("px")
        .or_else(|| value.strip_suffix("rem").map(|v| v))
        .unwrap_or(value);
    if value.ends_with("rem") {
        number
            .parse::<f32>()
            .ok()
            .map(|parsed| (parsed * 16.0) as i32)
    } else {
        number.parse::<f32>().ok().map(|parsed| parsed as i32)
    }
}

fn parse_length(value: &str) -> Option<Length> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent.parse::<i32>().ok().map(Length::Percent);
    }
    parse_px(value).map(Length::Px)
}

fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim().trim_end_matches("!important").trim();
    if value.eq_ignore_ascii_case("transparent") || value.eq_ignore_ascii_case("none") {
        return None;
    }
    let named = match value.to_ascii_lowercase().as_str() {
        "black" => Some(Color::rgb(0, 0, 0)),
        "white" => Some(Color::rgb(255, 255, 255)),
        "red" => Some(Color::rgb(255, 0, 0)),
        "green" => Some(Color::rgb(0, 128, 0)),
        "blue" => Some(Color::rgb(0, 102, 255)),
        "gray" | "grey" => Some(Color::rgb(128, 128, 128)),
        "yellow" => Some(Color::rgb(255, 204, 0)),
        "orange" => Some(Color::rgb(255, 136, 0)),
        "purple" => Some(Color::rgb(128, 64, 192)),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some(Color::rgb(r, g, b));
        }
        if hex.len() >= 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::rgb(r, g, b));
        }
    }
    if let Some(args) = value.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let channels: Vec<u8> = args
            .split(',')
            .filter_map(|part| part.trim().parse::<u8>().ok())
            .collect();
        if channels.len() == 3 {
            return Some(Color::rgb(channels[0], channels[1], channels[2]));
        }
    }
    None
}

fn parse_html(html: &str) -> (Vec<Node>, usize) {
    let mut nodes = Vec::new();
    nodes.push(Node {
        tag: "html".to_string(),
        ..Node::default()
    });
    let root = 0;
    let mut stack = alloc::vec![root];
    let mut cursor = 0;

    while cursor < html.len() && nodes.len() < MAX_NODES {
        let Some(open_rel) = html[cursor..].find('<') else {
            append_text(&mut nodes, *stack.last().unwrap_or(&root), &html[cursor..]);
            break;
        };
        let open = cursor + open_rel;
        if open > cursor {
            append_text(
                &mut nodes,
                *stack.last().unwrap_or(&root),
                &html[cursor..open],
            );
        }
        if html[open..].starts_with("<!--") {
            cursor = html[open + 4..]
                .find("-->")
                .map(|end| open + 4 + end + 3)
                .unwrap_or(html.len());
            continue;
        }
        let Some(close_rel) = html[open + 1..].find('>') else {
            break;
        };
        let close = open + 1 + close_rel;
        let raw = html[open + 1..close].trim();
        cursor = close + 1;
        if raw.is_empty() || raw.starts_with('!') || raw.starts_with('?') {
            continue;
        }
        if let Some(end_tag) = raw.strip_prefix('/') {
            let end_tag = end_tag
                .split_ascii_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            while stack.len() > 1 {
                let popped = stack.pop().unwrap_or(root);
                if nodes[popped].tag == end_tag {
                    break;
                }
            }
            continue;
        }

        let self_closing = raw.ends_with('/');
        let (tag, attrs) = parse_tag(raw.trim_end_matches('/').trim());
        if tag.is_empty() {
            continue;
        }
        let parent = *stack.last().unwrap_or(&root);
        let idx = nodes.len();
        nodes.push(Node {
            tag: tag.clone(),
            attrs,
            text: String::new(),
            children: Vec::new(),
            parent: Some(parent),
        });
        nodes[parent].children.push(idx);
        if !self_closing && !is_void_tag(&tag) {
            stack.push(idx);
        }
    }
    (nodes, root)
}

fn append_text(nodes: &mut Vec<Node>, parent: usize, text: &str) {
    if parent >= nodes.len() || text.is_empty() || nodes.len() >= MAX_NODES {
        return;
    }
    let idx = nodes.len();
    nodes.push(Node {
        tag: "#text".to_string(),
        text: decode_entities(text),
        parent: Some(parent),
        ..Node::default()
    });
    nodes[parent].children.push(idx);
}

fn parse_tag(raw: &str) -> (String, Vec<Attr>) {
    let mut chars = raw.char_indices().peekable();
    let mut name_end = raw.len();
    while let Some((idx, ch)) = chars.next() {
        if ch.is_ascii_whitespace() {
            name_end = idx;
            break;
        }
    }
    let tag = raw[..name_end].trim().to_ascii_lowercase();
    let attrs = if name_end < raw.len() {
        parse_attrs(&raw[name_end..])
    } else {
        Vec::new()
    };
    (tag, attrs)
}

fn parse_attrs(raw: &str) -> Vec<Attr> {
    let bytes = raw.as_bytes();
    let mut attrs = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let name_start = cursor;
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'='
        {
            cursor += 1;
        }
        if name_start == cursor {
            cursor += 1;
            continue;
        }
        let name = raw[name_start..cursor].to_ascii_lowercase();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let mut value = String::new();
        if cursor < bytes.len() && bytes[cursor] == b'=' {
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor < bytes.len() && (bytes[cursor] == b'"' || bytes[cursor] == b'\'') {
                let quote = bytes[cursor];
                cursor += 1;
                let value_start = cursor;
                while cursor < bytes.len() && bytes[cursor] != quote {
                    cursor += 1;
                }
                value = decode_entities(&raw[value_start..cursor]);
                cursor = (cursor + 1).min(bytes.len());
            } else {
                let value_start = cursor;
                while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
                value = decode_entities(&raw[value_start..cursor]);
            }
        }
        attrs.push(Attr { name, value });
    }
    attrs
}

fn is_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn decode_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn collect_style_text(nodes: &[Node]) -> String {
    let mut output = String::new();
    for (idx, node) in nodes.iter().enumerate() {
        if node.tag == "style" {
            collect_raw_text(nodes, idx, &mut output);
            output.push('\n');
        }
    }
    output
}

fn collect_raw_text(nodes: &[Node], idx: usize, output: &mut String) {
    output.push_str(&nodes[idx].text);
    for child in &nodes[idx].children {
        collect_raw_text(nodes, *child, output);
    }
}

fn parse_css(css: &str) -> Vec<CssRule> {
    let css = strip_css_comments(css);
    let mut rules = Vec::new();
    let mut cursor = 0;
    while cursor < css.len() && rules.len() < MAX_RULES {
        let Some(open_rel) = css[cursor..].find('{') else {
            break;
        };
        let open = cursor + open_rel;
        let Some(close_rel) = css[open + 1..].find('}') else {
            break;
        };
        let close = open + 1 + close_rel;
        let selectors = css[cursor..open].trim();
        let declarations = parse_declarations(&css[open + 1..close]);
        for selector_text in selectors.split(',') {
            if let Some(selector) = parse_selector(selector_text) {
                rules.push(CssRule {
                    selector,
                    declarations: declarations.clone(),
                    order: rules.len(),
                });
            }
        }
        cursor = close + 1;
    }
    rules
}

fn strip_css_comments(css: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(start_rel) = css[cursor..].find("/*") {
        let start = cursor + start_rel;
        output.push_str(&css[cursor..start]);
        let Some(end_rel) = css[start + 2..].find("*/") else {
            return output;
        };
        cursor = start + 2 + end_rel + 2;
    }
    output.push_str(&css[cursor..]);
    output
}

fn parse_selector(selector: &str) -> Option<Selector> {
    let simple = selector.split_ascii_whitespace().last()?.trim();
    if simple.is_empty() || simple.starts_with('@') {
        return None;
    }
    let hover = simple.contains(":hover");
    let simple = simple.replace(":hover", "");
    let bytes = simple.as_bytes();
    let mut selector = Selector {
        hover,
        ..Selector::default()
    };
    let mut cursor = 0;
    let tag_start = cursor;
    while cursor < bytes.len() && bytes[cursor] != b'.' && bytes[cursor] != b'#' {
        cursor += 1;
    }
    if cursor > tag_start {
        selector.tag = simple[tag_start..cursor].to_ascii_lowercase();
    }
    while cursor < bytes.len() {
        let kind = bytes[cursor];
        cursor += 1;
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor] != b'.' && bytes[cursor] != b'#' {
            cursor += 1;
        }
        if start == cursor {
            continue;
        }
        let value = simple[start..cursor].to_string();
        if kind == b'#' {
            selector.id = value;
        } else {
            selector.classes.push(value);
        }
    }
    Some(selector)
}

fn parse_declarations(input: &str) -> Vec<(String, String)> {
    let mut declarations = Vec::new();
    for declaration in input.split(';') {
        if let Some((name, value)) = declaration.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_ascii_lowercase();
            if !name.is_empty() && !value.is_empty() {
                declarations.push((name, value));
            }
        }
    }
    declarations
}

fn normalize_whitespace(input: &str) -> String {
    let mut output = String::new();
    let mut was_space = false;
    for ch in input.chars() {
        if ch == '\n' {
            while output.ends_with(' ') {
                output.pop();
            }
            if !output.ends_with('\n') {
                output.push('\n');
            }
            was_space = false;
        } else if ch.is_whitespace() {
            if !was_space && !output.is_empty() && !output.ends_with('\n') {
                output.push(' ');
            }
            was_space = true;
        } else {
            output.push(ch);
            was_space = false;
        }
    }
    output.trim().to_string()
}

fn wrap_text(text: &str, width: i32, font_size: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_width = 0;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(line);
            line = String::new();
            line_width = 0;
            continue;
        }
        let char_width = measure_char(ch, font_size);
        if !line.is_empty() && line_width + char_width > width {
            lines.push(line.trim_end().to_string());
            line = String::new();
            line_width = 0;
            if ch == ' ' {
                continue;
            }
        }
        line.push(ch);
        line_width += char_width;
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line.trim_end().to_string());
    }
    lines
}

fn measure_text(text: &str, font_size: i32) -> i32 {
    text.chars().map(|ch| measure_char(ch, font_size)).sum()
}

fn measure_char(ch: char, font_size: i32) -> i32 {
    let base = if ttf_font::is_available() {
        let glyph = ttf_font::glyph(ch);
        if glyph.w > 0 {
            glyph.advance.max(1) as i32
        } else {
            8
        }
    } else {
        8
    };
    if font_size >= 22 {
        base * 5 / 4
    } else if font_size <= 13 {
        base * 4 / 5
    } else {
        base
    }
}

fn line_height(font_size: i32) -> i32 {
    if font_size >= 26 {
        34
    } else if font_size >= 22 {
        29
    } else if font_size <= 13 {
        18
    } else {
        22
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_attributes_and_entities() {
        let (nodes, _) =
            parse_html(r#"<body><a href="os://display/hud?enabled=1&amp;mode=x">Open</a></body>"#);
        let link = nodes.iter().find(|node| node.tag == "a").unwrap();
        assert_eq!(link.attr("href"), "os://display/hud?enabled=1&mode=x");
    }

    #[test]
    fn css_specificity_prefers_ids() {
        let rules = parse_css("p { color: red; } .x { color: blue; } #main { color: white; }");
        assert_eq!(rules.len(), 3);
        assert!(rules[2].selector.specificity() > rules[1].selector.specificity());
    }

    #[test]
    fn wraps_japanese_without_spaces() {
        let lines = wrap_text("これは長い日本語のテキストです", 40, 16);
        assert!(lines.len() > 1);
    }
}

