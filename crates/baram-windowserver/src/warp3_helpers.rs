fn eval_math(expression: &str) -> i64 {
    let chars: Vec<char> = expression.chars().collect();
    let mut index = 0usize;
    let mut result = parse_math_integer(&chars, &mut index);
    while index < chars.len() {
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }
        let operator = chars[index];
        index += 1;
        let value = parse_math_integer(&chars, &mut index);
        match operator {
            '+' => result = result.saturating_add(value),
            '-' => result = result.saturating_sub(value),
            '*' => result = result.saturating_mul(value),
            '/' if value != 0 => result /= value,
            _ => {}
        }
    }
    result
}

fn parse_math_integer(chars: &[char], index: &mut usize) -> i64 {
    while *index < chars.len() && chars[*index].is_whitespace() {
        *index += 1;
    }
    let negative = chars.get(*index) == Some(&'-');
    if negative {
        *index += 1;
    }
    let mut value = 0i64;
    while *index < chars.len() && chars[*index].is_ascii_digit() {
        value = value
            .saturating_mul(10)
            .saturating_add(chars[*index].to_digit(10).unwrap_or(0) as i64);
        *index += 1;
    }
    if negative {
        -value
    } else {
        value
    }
}

fn format_now_value(
    path: &str,
    now: NowValues,
    hour: u8,
    minute: u8,
    second: u8,
) -> Option<String> {
    let value = match path.trim_matches('/') {
        "fps" => now.fps.to_string(),
        "window" | "windows" => now.windows.to_string(),
        "key" | "keys" => now.keys.to_string(),
        "mouse" => now.mouse.to_string(),
        "h" => hour.to_string(),
        "m" => minute.to_string(),
        "s" => second.to_string(),
        "hh" => alloc::format!("{hour:02}"),
        "mm" => alloc::format!("{minute:02}"),
        "ss" => alloc::format!("{second:02}"),
        "hhmm" => alloc::format!("{hour:02}:{minute:02}"),
        "hhmmss" => alloc::format!("{hour:02}:{minute:02}:{second:02}"),
        _ => return None,
    };
    Some(value)
}

fn parse_script(source: &str) -> Vec<ScriptSection> {
    let mut sections = Vec::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if let Some((kind, name)) = line[1..line.len() - 1].split_once('=') {
                let kind = match kind.trim() {
                    "onClick" => SectionKind::Click,
                    "fun" => SectionKind::Function,
                    _ => continue,
                };
                sections.push(ScriptSection {
                    kind,
                    name: name.trim().to_string(),
                    actions: Vec::new(),
                });
            }
        } else if let Some((left, right)) = line.split_once('=') {
            if let Some(section) = sections.last_mut() {
                section
                    .actions
                    .push((left.trim().to_string(), right.trim().to_string()));
            }
        }
    }
    sections
}

fn ini_value(source: &str, key: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let (left, right) = line.trim().split_once('=')?;
        (left.trim() == key).then(|| right.trim().to_string())
    })
}

fn root_indices(nodes: &[Node]) -> Vec<usize> {
    (0..nodes.len())
        .filter(|idx| !nodes.iter().any(|node| node.children.contains(idx)))
        .collect()
}

fn set_prop(node: &mut Node, key: &str, value: &str) {
    if let Some((_, current)) = node.props.iter_mut().find(|(name, _)| name == key) {
        *current = value.to_string();
    } else {
        node.props.push((key.to_string(), value.to_string()));
    }
}

fn interactive(node: &Node) -> bool {
    (node.is("button") && !node.classes.iter().any(|class| class == "candidate-mode"))
        || node.is("switch")
        || node.is("input")
        || node.is("textarea")
        || node.is("content")
}

fn contains(node: &Node, x: i32, y: i32) -> bool {
    x >= node.x && x < node.x + node.w && y >= node.y && y < node.y + node.h
}

fn mark_overlay_tree(nodes: &mut [Node], idx: usize) {
    nodes[idx].overlay = true;
    let children = nodes[idx].children.clone();
    for child in children {
        mark_overlay_tree(nodes, child);
    }
}

/// One separable box blur. Two invocations provide a fast softening
/// approximation and need only integer additions/subtractions per pixel.
fn box_blur_alpha(alpha: &mut [u8], width: usize, height: usize, radius: usize) {
    if width == 0 || height == 0 {
        return;
    }
    let mut tmp = alloc::vec![0u8; alpha.len()];
    let diameter = radius * 2 + 1;
    for y in 0..height {
        let row = y * width;
        let mut sum = 0u32;
        for x in 0..width + radius {
            if x < width {
                sum += alpha[row + x] as u32;
            }
            if x >= diameter && x - diameter < width {
                sum -= alpha[row + x - diameter] as u32;
            }
            if x >= radius && x - radius < width {
                tmp[row + x - radius] = (sum / diameter as u32) as u8;
            }
        }
    }
    for x in 0..width {
        let mut sum = 0u32;
        for y in 0..height + radius {
            if y < height {
                sum += tmp[y * width + x] as u32;
            }
            if y >= diameter && y - diameter < height {
                sum -= tmp[(y - diameter) * width + x] as u32;
            }
            if y >= radius && y - radius < height {
                alpha[(y - radius) * width + x] = (sum / diameter as u32) as u8;
            }
        }
    }
}

fn measure(text: &str) -> i32 {
    text.chars().map(ttf_font::advance).sum()
}

fn fit_button_width(desired: i32, available: i32) -> i32 {
    desired.max(44).min(available.max(1))
}

/// Keep wrapping and layout on the exact same glyph advances.  A character
/// count approximation breaks mixed CJK/Latin labels and shifts every sibling.
fn wrap_lines(text: &str, width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut line = String::new();
        let mut line_width = 0;
        for ch in paragraph.chars() {
            let advance = ttf_font::advance(ch);
            if !line.is_empty() && line_width + advance > width.max(1) {
                lines.push(line);
                line = String::new();
                line_width = 0;
            }
            line.push(ch);
            line_width += advance;
        }
        lines.push(line);
    }
    lines
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

fn parse_wait_ns(value: &str) -> u64 {
    let value = value.trim();
    if let Some(ms) = value.strip_suffix("ms") {
        return ms
            .trim()
            .parse::<u64>()
            .unwrap_or(0)
            .saturating_mul(1_000_000);
    }
    if let Some(seconds) = value.strip_suffix('s') {
        return seconds
            .trim()
            .parse::<u64>()
            .unwrap_or(0)
            .saturating_mul(1_000_000_000);
    }
    value.parse::<u64>().unwrap_or(0).saturating_mul(1_000_000)
}

fn html_bg() -> Color {
    config::get_color("ui-theme/color/win_bg", Color::rgb(243, 243, 243))
}

fn html_layer() -> Color {
    Color::rgb(249, 249, 249)
}

fn html_layer_solid() -> Color {
    Color::rgb(251, 251, 251)
}

fn html_text() -> Color {
    Color::rgb(26, 26, 26)
}

fn html_muted() -> Color {
    Color::rgb(93, 93, 93)
}

fn html_border() -> Color {
    Color::rgb(211, 211, 211)
}

fn html_accent() -> Color {
    Color::rgb(0, 103, 192)
}

fn html_accent_hover() -> Color {
    Color::rgb(25, 117, 197)
}

fn blend_color(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
    Color::rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn rounded_box(
    layer: &mut LayerSystem,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    radius: usize,
    border: Color,
    fill: Color,
) {
    if x < 0 || y < 0 || width == 0 || height == 0 {
        return;
    }
    let x = x as usize;
    let y = y as usize;
    layer.fill_rounded_rect(x, y, width, height, radius, border);
    if width > 2 && height > 2 {
        layer.fill_rounded_rect(
            x + 1,
            y + 1,
            width - 2,
            height - 2,
            radius.saturating_sub(1),
            fill,
        );
    }
}

fn rounded_fill(
    layer: &mut LayerSystem,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    radius: usize,
    fill: Color,
) {
    if x >= 0 && y >= 0 && width > 0 && height > 0 {
        layer.fill_rounded_rect(x as usize, y as usize, width, height, radius, fill);
    }
}

fn keyboard_key_width(node: &Node) -> i32 {
    let fit = fit_button_width(measure(node.prop("text")) + 28, i32::MAX);
    if node
        .classes
        .iter()
        .any(|class| class == "backspace" || class == "enter")
    {
        fit.max(76)
    } else if node.classes.iter().any(|class| class == "shift") {
        fit.max(76)
    } else if node
        .classes
        .iter()
        .any(|class| class == "symbols" || class == "letters")
    {
        fit.max(64)
    } else if node.classes.iter().any(|class| class == "close") {
        fit.max(68)
    } else {
        fit
    }
}

fn keyboard_row_natural_width(nodes: &[Node], row: usize) -> i32 {
    let children = &nodes[row].children;
    let gap = 6 * children.len().saturating_sub(1) as i32;
    children
        .iter()
        .map(|child| keyboard_key_width(&nodes[*child]))
        .sum::<i32>()
        + gap
}

fn put_str_size(layer: &mut LayerSystem, mut x: i32, y: i32, text: &str, color: Color, size: f32) {
    if !ttf_font::is_available() {
        if x >= 0 && y >= 0 {
            layer.put_str(x as usize, y as usize, text, color);
        }
        return;
    }
    let baseline = y + ttf_font::ascent_at_size(size);
    let layer_w = layer.width();
    let layer_h = layer.height();
    let (clip_x0, clip_y0, clip_x1, clip_y1) = layer.clip_bounds();
    for ch in text.chars() {
        let mut advance = 0;
        ttf_font::with_glyph_at_size(ch, size, |data, w, h, glyph_advance, y_off| {
            advance = glyph_advance;
            let top = baseline + y_off;
            for row in 0..h {
                let py = top + row;
                if py < crate::window::title_bar_h() as i32
                    || py < clip_y0 as i32
                    || py >= clip_y1.min(layer_h) as i32
                {
                    continue;
                }
                for col in 0..w {
                    let px = x + col;
                    if px < clip_x0 as i32 || px >= clip_x1.min(layer_w) as i32 {
                        continue;
                    }
                    let alpha = data[row as usize * w as usize + col as usize] as f32 / 255.0;
                    if alpha <= 0.0 {
                        continue;
                    }
                    let index = py as usize * layer_w + px as usize;
                    let background = layer.buf_ref()[index];
                    layer.buf_mut()[index] = LayerSystem::blend_alpha(background, color.0, alpha);
                }
            }
        });
        x += advance.max(8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_reference_ui_without_preprocessing() {
        let nodes = Parser::new(include_str!("../../../files/app/warp3demo.w3a/main.w3u")).parse();
        assert!(nodes.len() > 100);
        assert!(nodes.iter().any(|node| node.is("toolbar")));
        assert!(nodes.iter().any(|node| node.is("tab")));
        assert!(nodes.iter().any(|node| node.is("switch")));
        assert!(nodes.iter().any(|node| node.is("textarea")));
        assert!(nodes.iter().any(|node| {
            node.is("button") && node.classes.iter().any(|class| class == "vardemo")
        }));
    }

    #[test]
    fn parses_the_task_manager_w3a_ui() {
        let nodes = Parser::new(include_str!("../../../files/app/task.w3a/main.w3u")).parse();
        assert!(nodes.iter().any(|node| {
            node.is("button") && node.classes.iter().any(|class| class == "refresh-values")
        }));
        assert!(nodes.iter().any(|node| {
            node.is("detail") && node.classes.iter().any(|class| class == "hhmmss-value")
        }));
    }

    #[test]
    fn parses_the_converted_warp3_apps() {
        for source in [
            include_str!("../../../files/app/calc.w3a/main.w3u"),
            include_str!("../../../files/app/mousekeydialog.w3a/main.w3u"),
            include_str!("../../../files/app/settings.w3a/main.w3u"),
            include_str!("../../../files/app/settings.w3a/theme.w3u"),
            include_str!("../../../files/app/settings.w3a/pointer.w3u"),
            include_str!("../../../files/app/settings.w3a/hud.w3u"),
            include_str!("../../../files/app/settings.w3a/system.w3u"),
            include_str!("../../../files/app/theme.w3a/main.w3u"),
            include_str!("../../../files/app/ospermission.w3a/main.w3u"),
        ] {
            assert!(!Parser::new(source).parse().is_empty());
        }
        assert!(!parse_script(include_str!("../../../files/app/calc.w3a/calc.w3s")).is_empty());
        assert!(
            !parse_script(include_str!("../../../files/app/settings.w3a/settings.w3s")).is_empty()
        );
        assert!(!parse_script(include_str!("../../../files/app/theme.w3a/theme.w3s")).is_empty());
        let permission = parse_script(include_str!(
            "../../../files/app/ospermission.w3a/permission.w3s"
        ));
        assert!(permission.iter().any(|section| {
            section.name == "permission-always"
                && section
                    .actions
                    .iter()
                    .any(|(left, right)| left == "run" && right == "security://always")
        }));
    }

    #[test]
    fn evaluates_calculator_expressions_like_warp2() {
        assert_eq!(eval_math("12+3"), 15);
        assert_eq!(eval_math("7+3*2"), 20);
        assert_eq!(eval_math("20/4-2"), 3);
        assert_eq!(eval_math("9/0"), 9);
    }

    #[test]
    fn formats_now_runtime_values_and_time_tokens() {
        let now = NowValues {
            fps: 60,
            windows: 3,
            keys: 12,
            mouse: 34,
        };
        assert_eq!(format_now_value("fps", now, 4, 5, 6).as_deref(), Some("60"));
        assert_eq!(
            format_now_value("window", now, 4, 5, 6).as_deref(),
            Some("3")
        );
        assert_eq!(format_now_value("key", now, 4, 5, 6).as_deref(), Some("12"));
        assert_eq!(
            format_now_value("mouse", now, 4, 5, 6).as_deref(),
            Some("34")
        );
        assert_eq!(format_now_value("h", now, 4, 5, 6).as_deref(), Some("4"));
        assert_eq!(format_now_value("hh", now, 4, 5, 6).as_deref(), Some("04"));
        assert_eq!(
            format_now_value("hhmm", now, 4, 5, 6).as_deref(),
            Some("04:05")
        );
        assert_eq!(
            format_now_value("hhmmss", now, 4, 5, 6).as_deref(),
            Some("04:05:06")
        );
        assert!(format_now_value("unknown", now, 4, 5, 6).is_none());
    }

    #[test]
    fn parses_reference_script_commands_and_functions() {
        let nav = parse_script(include_str!("../../../files/app/warp3demo.w3a/nav.w3s"));
        let variables = parse_script(include_str!(
            "../../../files/app/warp3demo.w3a/var-demo.w3s"
        ));
        assert!(nav.iter().any(|section| {
            section.kind == SectionKind::Click
                && section.name == "vardemo"
                && section
                    .actions
                    .iter()
                    .any(|(left, right)| left == "screen" && right == "var")
        }));
        assert!(variables.iter().any(|section| {
            section.kind == SectionKind::Function && section.name == "updateDisplay"
        }));
        assert!(variables.iter().any(|section| {
            section.name == "set-btn"
                && section
                    .actions
                    .iter()
                    .any(|(left, right)| left == "getText myVar" && right == "set-input")
        }));
    }

    #[test]
    fn parses_wait_durations_as_absolute_nanoseconds() {
        assert_eq!(parse_wait_ns("50ms"), 50_000_000);
        assert_eq!(parse_wait_ns("2s"), 2_000_000_000);
        assert_eq!(parse_wait_ns("25"), 25_000_000);
        assert_eq!(parse_wait_ns("invalid"), 0);
    }

    #[test]
    fn button_layout_accepts_widths_below_its_normal_minimum() {
        assert_eq!(fit_button_width(120, 20), 20);
        assert_eq!(fit_button_width(120, 0), 1);
        assert_eq!(fit_button_width(30, 100), 44);
    }

    #[test]
    fn parses_wait_run_and_config_text_actions() {
        let sections = parse_script(
            "[onClick = demo]\nwait = 50ms\nrun = os://display/hud?enabled=0\nsetText display = os://display/hud/enabled\n",
        );
        let actions = &sections[0].actions;
        assert_eq!(actions[0], ("wait".to_string(), "50ms".to_string()));
        assert_eq!(actions[1].0, "run");
        assert_eq!(actions[2].0, "setText display");
    }
}

