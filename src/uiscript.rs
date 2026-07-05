//! UI Script parser and renderer for BaramOS.
//!
//! Parses the custom UI Script markup language and renders it
//! inside a window's content area using the LayerSystem.
//! Uses clip_rect for pixel-level clipping.

use alloc::vec::Vec;
use alloc::string::String;
use crate::gop::Color;
use crate::window::LayerSystem;

#[derive(Clone, Debug)]
pub enum Command {
    Font(String),
    Color(String),
    Head(String),
    H2(String),
    H3(String),
    Text(String),
    Button { url: String, text: String, btn_type: String },
    Card { title: String, text: String, button: String },
    List { title: String, text: String, button: String },
    Hr,
    Br,
}

#[derive(Clone, Debug)]
enum InlineElement {
    Text { text: String, bold: bool, color: Option<String> },
}

fn parse_inline(text: &str) -> Vec<InlineElement> {
    let mut elements = Vec::new();
    let mut chars = text.chars().peekable();
    let mut current = String::new();
    let mut in_bold = false;

    while let Some(&ch) = chars.peek() {
        match ch {
            '*' => {
                if !current.is_empty() {
                    elements.push(InlineElement::Text {
                        text: current.clone(),
                        bold: in_bold,
                        color: None,
                    });
                    current.clear();
                }
                chars.next();
                if chars.peek() == Some(&'*') {
                    chars.next();
                    in_bold = !in_bold;
                }
            }
            '#' => {
                if !current.is_empty() {
                    elements.push(InlineElement::Text {
                        text: current.clone(),
                        bold: in_bold,
                        color: None,
                    });
                    current.clear();
                }
                chars.next();
                let mut color = String::new();
                let mut found_brace = false;
                for _ in 0..6 {
                    if let Some(&c) = chars.peek() {
                        if c.is_ascii_hexdigit() {
                            color.push(c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                if let Some(&'{') = chars.peek() {
                    chars.next();
                    let mut inner = String::new();
                    let mut depth = 1;
                    for c in chars.by_ref() {
                        match c {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 { break; }
                                inner.push(c);
                            }
                            _ => inner.push(c),
                        }
                    }
                    let inner_elements = parse_inline(&inner);
                    for elem in inner_elements {
                        match elem {
                            InlineElement::Text { text, bold, .. } => {
                                elements.push(InlineElement::Text {
                                    text,
                                    bold: in_bold || bold,
                                    color: Some(color.clone()),
                                });
                            }
                        }
                    }
                    found_brace = true;
                }
                if !found_brace && !color.is_empty() {
                    current.push('#');
                    current.push_str(&color);
                }
            }
            _ => {
                current.push(ch);
                chars.next();
            }
        }
    }

    if !current.is_empty() {
        elements.push(InlineElement::Text {
            text: current,
            bold: in_bold,
            color: None,
        });
    }

    elements
}

fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::TEXT;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    Color::rgb(r, g, b)
}

pub fn parse(source: &str) -> Vec<Command> {
    let valid_keys = ["font", "color", "head", "h2", "h3", "text", "button", "card", "list", "br"];
    let mut commands = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("<ui-script") || trimmed.starts_with("</ui-script") {
            continue;
        }

        for part in trimmed.split(';') {
            let part = part.trim();
            if part.is_empty() { continue; }

            let first_token = part.split(':').next().unwrap_or("").trim();
            if !valid_keys.contains(&first_token) { continue; }

            let parts: Vec<&str> = part.split(':').collect();
            let key = parts[0].trim();

            match key {
                "font" => {
                    let joined = parts[1..].join(":");
                    let val = joined.trim().trim_matches('"');
                    commands.push(Command::Font(String::from(val)));
                }
                "color" => {
                    let joined = parts[1..].join(":");
                    let val = joined.trim().trim_matches('"');
                    commands.push(Command::Color(String::from(val)));
                }
                "head" => {
                    let joined = parts[1..].join(":");
                    let val = joined.trim().trim_matches('"');
                    commands.push(Command::Head(String::from(val)));
                }
                "h2" => {
                    let joined = parts[1..].join(":");
                    let val = joined.trim().trim_matches('"');
                    commands.push(Command::H2(String::from(val)));
                }
                "h3" => {
                    let joined = parts[1..].join(":");
                    let val = joined.trim().trim_matches('"');
                    commands.push(Command::H3(String::from(val)));
                }
                "text" => {
                    let joined = parts[1..].join(":");
                    let val = joined.trim().trim_matches('"');
                    commands.push(Command::Text(String::from(val)));
                }
                "button" => {
                    let mut url = String::new();
                    let mut text = String::from("Button");
                    let mut btn_type = String::from("filled");
                    for item in parts[1..].iter() {
                        let item = item.trim();
                        if let Some(rest) = item.strip_prefix("url\"") {
                            url = String::from(rest.trim_end_matches('"'));
                        } else if let Some(rest) = item.strip_prefix("text\"") {
                            text = String::from(rest.trim_end_matches('"'));
                        } else if let Some(rest) = item.strip_prefix("type\"") {
                            btn_type = String::from(rest.trim_end_matches('"'));
                        }
                    }
                    commands.push(Command::Button { url, text, btn_type });
                }
                "card" => {
                    let mut title = String::new();
                    let mut text = String::new();
                    let mut button = String::new();
                    for item in parts[1..].iter() {
                        let item = item.trim();
                        if let Some(rest) = item.strip_prefix("title\"") {
                            title = String::from(rest.trim_end_matches('"'));
                        } else if let Some(rest) = item.strip_prefix("text\"") {
                            text = String::from(rest.trim_end_matches('"'));
                        } else if let Some(rest) = item.strip_prefix("button\"") {
                            button = String::from(rest.trim_end_matches('"'));
                        }
                    }
                    commands.push(Command::Card { title, text, button });
                }
                "list" => {
                    let mut title = String::new();
                    let mut text = String::new();
                    let mut button = String::new();
                    for item in parts[1..].iter() {
                        let item = item.trim();
                        if let Some(rest) = item.strip_prefix("title\"") {
                            title = String::from(rest.trim_end_matches('"'));
                        } else if let Some(rest) = item.strip_prefix("text\"") {
                            text = String::from(rest.trim_end_matches('"'));
                        } else if let Some(rest) = item.strip_prefix("button\"") {
                            button = String::from(rest.trim_end_matches('"'));
                        }
                    }
                    commands.push(Command::List { title, text, button });
                }
                "br" => {
                    let joined = parts[1..].join(":");
                    let val = joined.trim().trim_matches('"');
                    if val == "line" {
                        commands.push(Command::Hr);
                    } else {
                        commands.push(Command::Br);
                    }
                }
                _ => {}
            }
        }
    }

    commands
}

fn strip_icons(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '@' {
            if let Some('{') = chars.clone().next() {
                chars.next();
                for c in chars.by_ref() {
                    if c == '}' { break; }
                }
            } else {
                result.push(ch);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Render parsed UI Script commands.
/// Caller must set up clip_rect before calling.
pub fn render(
    layer: &mut LayerSystem,
    commands: &[Command],
    win_x: i32,
    win_y: i32,
    win_w: usize,
    win_h: usize,
    title_bar_h: usize,
    scroll_y: i32,
) {
    let accent = commands.iter().find_map(|c| {
        if let Command::Color(hex) = c { Some(hex_to_color(hex)) } else { None }
    }).unwrap_or(Color::ACCENT);

    let text_color = Color::TEXT;
    let muted = Color::MUTED;

    let content_x = win_x + 10;
    let content_base_y = win_y + title_bar_h as i32;
    let max_x = win_x + win_w as i32 - 10;

    let mut ly: i32 = 8;

    for cmd in commands {
        let sy = content_base_y + ly - scroll_y;

        match cmd {
            Command::Head(text) | Command::H2(text) | Command::H3(text) => {
                let plain = strip_icons(text);
                let elements = parse_inline(&plain);
                let mut x = content_x as usize;
                for elem in &elements {
                    match elem {
                        InlineElement::Text { text, bold, color: c } => {
                            let fg = c.as_ref().map(|h| hex_to_color(h)).unwrap_or(text_color);
                            if *bold {
                                layer.put_str(x + 1, sy as usize, text, fg);
                                layer.put_str(x, sy as usize, text, fg);
                            } else {
                                layer.put_str(x, sy as usize, text, fg);
                            }
                            x += text.len() * 8;
                        }
                    }
                }
                ly += 24;
            }
            Command::Text(text) => {
                let plain = strip_icons(text);
                let elements = parse_inline(&plain);
                let mut x = content_x as usize;
                for elem in &elements {
                    if x as i32 >= max_x { break; }
                    match elem {
                        InlineElement::Text { text, bold, color: c } => {
                            let fg = c.as_ref().map(|h| hex_to_color(h)).unwrap_or(text_color);
                            if *bold {
                                layer.put_str(x + 1, sy as usize, text, fg);
                                layer.put_str(x, sy as usize, text, fg);
                            } else {
                                layer.put_str(x, sy as usize, text, fg);
                            }
                            x += text.len() * 8;
                        }
                    }
                }
                ly += 20;
            }
            Command::Button { text, btn_type, .. } => {
                let plain = strip_icons(text);
                let w = (plain.len() * 8 + 20).min(win_w - 20);
                let h = 24usize;
                let bx = content_x as usize;
                let by = sy as usize;
                let radius = 6;

                match btn_type.as_str() {
                    "outline" => {
                        layer.rounded_rect_outline(bx, by, w, h, radius, accent);
                        layer.put_str(bx + 10, by + 5, &plain, accent);
                    }
                    "text" => {
                        layer.put_str(bx + 4, by + 5, &plain, accent);
                    }
                    _ => {
                        layer.fill_rounded_rect(bx, by, w, h, radius, accent);
                        layer.put_str(bx + 10, by + 5, &plain, Color::TEXT);
                    }
                }
                ly += 32;
            }
            Command::Card { title, text, button } => {
                let card_w = win_w.saturating_sub(20);
                let card_h = 80usize;
                let bx = content_x as usize;
                let by = sy as usize;
                let radius = 8;

                layer.fill_rounded_rect(bx, by, card_w, card_h, radius, Color::CARD_BG);
                layer.rounded_rect_outline(bx, by, card_w, card_h, radius, Color::BORDER);
                layer.put_str(bx + 8, by + 8, title, text_color);
                layer.put_str(bx + 8, by + 28, text, muted);
                if !button.is_empty() {
                    let btn_w = (button.len() * 8 + 20).min(card_w - 16);
                    layer.fill_rounded_rect(bx + 8, by + 48, btn_w, 24, 6, accent);
                    layer.put_str(bx + 18, by + 53, button, Color::TEXT);
                }
                ly += card_h as i32 + 8;
            }
            Command::List { title, text, button } => {
                let list_w = win_w.saturating_sub(20);
                let list_h = 80usize;
                let bx = content_x as usize;
                let by = sy as usize;
                let radius = 8;

                layer.fill_rounded_rect(bx, by, list_w, list_h, radius, Color::WIN_BG);
                layer.rounded_rect_outline(bx, by, list_w, list_h, radius, Color::BORDER);
                layer.put_str(bx + 8, by + 8, title, text_color);
                layer.put_str(bx + 8, by + 28, text, muted);
                if !button.is_empty() {
                    let btn_w = (button.len() * 8 + 20).min(list_w - 16);
                    layer.fill_rounded_rect(bx + 8, by + 48, btn_w, 24, 6, accent);
                    layer.put_str(bx + 18, by + 53, button, Color::TEXT);
                }
                ly += list_h as i32 + 8;
            }
            Command::Hr => {
                layer.fill_rect(content_x as usize, sy as usize, win_w.saturating_sub(20), 1, Color::BORDER);
                ly += 12;
            }
            Command::Br => {
                ly += 10;
            }
            _ => {}
        }
    }
}
