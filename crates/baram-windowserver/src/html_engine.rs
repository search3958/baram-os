impl HtmlEngine {
    pub fn new(html: &str, external_css: &str) -> Self {
        let (nodes, root) = parse_html(html);
        let mut css = collect_style_text(&nodes);
        if !external_css.trim().is_empty() {
            css.push('\n');
            css.push_str(external_css);
        }
        Self {
            warp3: None,
            warp4: None,
            origin: String::new(),
            nodes,
            rules: parse_css(&css),
            root,
            items: Vec::new(),
            hits: Vec::new(),
            hovered_node: None,
            width: 0,
            height: 0,
            layout_dirty: true,
            content_height: 0,
            last_command: None,
        }
    }

    pub fn new_warp3(config_name: &str) -> Self {
        let warp3 = crate::warp3::Warp3Engine::new(config_name);
        Self::from_warp3(warp3, config_name)
    }

    pub fn new_embedded_warp3(name: &str, sources: &[(&str, &str)]) -> Self {
        let warp3 = crate::warp3::Warp3Engine::new_embedded(name, sources);
        Self::from_warp3(warp3, name)
    }

    pub fn new_warp4(config_name: &str) -> Self {
        let warp4 = crate::warp::WarpEngine::new_warp4(config_name);
        Self::from_warp4(warp4, config_name)
    }

    pub fn new_embedded_warp4(name: &str, sources: &[(&str, &str)]) -> Self {
        let warp4 = crate::warp::WarpEngine::new_embedded_warp4(name, sources);
        Self::from_warp4(warp4, name)
    }

    fn from_warp3(warp3: crate::warp3::Warp3Engine, name: &str) -> Self {
        Self {
            warp3: Some(warp3),
            warp4: None,
            origin: String::from(name),
            nodes: Vec::new(),
            rules: Vec::new(),
            root: 0,
            items: Vec::new(),
            hits: Vec::new(),
            hovered_node: None,
            width: 0,
            height: 0,
            layout_dirty: true,
            content_height: 0,
            last_command: None,
        }
    }

    fn from_warp4(warp4: crate::warp::WarpEngine, name: &str) -> Self {
        Self {
            warp3: None,
            warp4: Some(warp4),
            origin: String::from(name),
            nodes: Vec::new(),
            rules: Vec::new(),
            root: 0,
            items: Vec::new(),
            hits: Vec::new(),
            hovered_node: None,
            width: 0,
            height: 0,
            layout_dirty: true,
            content_height: 0,
            last_command: None,
        }
    }

    pub fn set_origin(&mut self, app_name: &str) {
        self.origin = String::from(app_name);
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_origin(app_name);
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn set_warp3_text(&mut self, class: &str, value: &str) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.set_text(class, value);
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_text(class, value);
        }
    }

    pub fn set_warp3_screen(&mut self, screen: &str) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.set_screen(screen);
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_screen(screen);
        }
    }

    pub fn hold_warp3_command(&mut self) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.hold_command();
        }
    }

    pub fn complete_warp3_command(&mut self) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.complete_command();
            if self.last_command.is_none() {
                self.last_command = engine.take_command();
            }
        }
    }

    pub fn update(&mut self, width: i32, height: i32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.update(width, height);
            self.content_height = engine.content_height;
            return;
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.update(width, height);
            self.content_height = engine.content_height();
            return;
        }
        if !self.layout_dirty && self.width == width && self.height == height {
            return;
        }
        self.width = width.max(1);
        self.height = height.max(1);
        self.items.clear();
        self.hits.clear();

        let styles = self.compute_styles();
        let root = self.body_node().unwrap_or(self.root);
        let start_y = crate::window::title_bar_h() as i32;
        let content_h = self.layout_node(root, 0, start_y, self.width, &styles);
        self.content_height = (start_y + content_h + 16).max(self.height);
        self.layout_dirty = false;
    }

    pub fn refresh_config(&mut self) {
        if self.warp3.is_some() || self.warp4.is_some() {
            return;
        }
        self.layout_dirty = true;
    }

    pub fn click(&mut self, x: i32, y: i32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.click(x, y);
            if self.last_command.is_none() {
                self.last_command = engine.take_command();
            }
            return;
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.click(x, y);
            if self.last_command.is_none() {
                self.last_command = engine.last_command.take();
            }
            return;
        }
        for hit in self.hits.iter().rev() {
            if point_in(x, y, hit.x, hit.y, hit.w, hit.h) {
                if hit.href.starts_with("os://") || hit.href.starts_with("app://") {
                    self.last_command = Some(hit.href.clone());
                }
                return;
            }
        }
    }

    pub fn set_hover(&mut self, x: i32, y: i32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.set_hover(x, y);
            return;
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_hover(x, y);
            self.hovered_node = engine.hovered_node();
            return;
        }
        let hovered = self
            .hits
            .iter()
            .rev()
            .find(|hit| point_in(x, y, hit.x, hit.y, hit.w, hit.h))
            .map(|hit| hit.node);
        if self.hovered_node != hovered {
            self.hovered_node = hovered;
            self.layout_dirty = true;
        }
    }

    pub fn clear_hover(&mut self) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.clear_hover();
            return;
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.clear_hover();
            self.hovered_node = None;
            return;
        }
        if self.hovered_node.take().is_some() {
            self.layout_dirty = true;
        }
    }

    pub fn cancel_hover(&mut self) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.cancel_hover();
            return;
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.clear_hover();
            self.hovered_node = None;
            return;
        }
        if self.hovered_node.take().is_some() {
            self.layout_dirty = true;
        }
    }

    pub fn hovered_node(&self) -> Option<usize> {
        if let Some(engine) = self.warp3.as_ref() {
            return engine.hovered_node();
        }
        if let Some(engine) = self.warp4.as_ref() {
            return engine.hovered_node();
        }
        self.hovered_node
    }

    pub fn draw_to_layer(&mut self, layer: &mut LayerSystem, ox: i32, oy: i32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.draw_to_layer(layer, ox, oy);
            return;
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.draw_to_layer(layer, ox, oy);
            return;
        }
        for item in &self.items {
            let x = item.x + ox;
            let y = item.y + oy;
            if x + item.w <= 0
                || y + item.h <= 0
                || x >= layer.width() as i32
                || y >= layer.height() as i32
            {
                continue;
            }
            match &item.kind {
                PaintKind::Box {
                    background,
                    border,
                    border_width,
                    radius,
                } => {
                    if let Some(bg) = background {
                        fill_box(layer, x, y, item.w, item.h, *radius, *bg);
                    }
                    if *border_width > 0 {
                        draw_border(layer, x, y, item.w, item.h, *radius, *border, *border_width);
                    }
                }
                PaintKind::Text {
                    text,
                    color,
                    large,
                    underline,
                } => {
                    if y < 0 {
                        continue;
                    }
                    // The large HUD font is Latin-focused. Keep mixed/Japanese
                    // headings complete by falling back to the CJK-capable UI
                    // font instead of dropping unsupported glyphs.
                    if *large && text.is_ascii() {
                        layer.put_str_hud(x.max(0) as usize, y as usize, text, *color);
                    } else {
                        layer.put_str(x.max(0) as usize, y as usize, text, *color);
                    }
                    if *underline {
                        let underline_y = y + item.h - 3;
                        fill_box(layer, x, underline_y, item.w, 1, 0, *color);
                    }
                }
            }
        }
    }

    pub fn set_scroll(&mut self, scroll: i32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.set_scroll(scroll);
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_scroll(scroll);
        }
    }

    pub fn content_height(&self) -> i32 {
        self.warp3
            .as_ref()
            .map(|engine| engine.content_height)
            .or_else(|| {
                self.warp4
                    .as_ref()
                    .map(crate::warp::WarpEngine::content_height)
            })
            .unwrap_or(0)
    }

    pub fn set_runtime_metrics(&mut self, fps: u32, windows: usize, keys: u32, mouse: u32) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.set_runtime_metrics(fps, windows, keys, mouse);
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.set_runtime_metrics(fps, windows, keys, mouse);
        }
    }

    pub fn take_scroll_request(&mut self) -> Option<i32> {
        self.warp3
            .as_mut()
            .and_then(|engine| engine.take_scroll_request())
            .or_else(|| {
                self.warp4
                    .as_mut()
                    .and_then(|engine| engine.take_scroll_request())
            })
    }

    pub fn window_damage(&self) -> Option<(i32, i32, i32, i32)> {
        self.warp3
            .as_ref()
            .and_then(|engine| engine.window_damage())
            .or_else(|| {
                self.warp4
                    .as_ref()
                    .and_then(|engine| engine.window_damage())
            })
    }

    pub fn tick(&mut self, now_ns: u64) -> bool {
        if let Some(engine) = self.warp3.as_mut() {
            let changed = engine.tick(now_ns);
            if self.last_command.is_none() {
                self.last_command = engine.take_command();
            }
            changed
        } else if let Some(engine) = self.warp4.as_mut() {
            let changed = engine.tick(now_ns);
            if self.last_command.is_none() {
                self.last_command = engine.last_command.take();
            }
            changed
        } else {
            false
        }
    }

    pub fn has_focused_input(&self) -> bool {
        self.warp3.as_ref().map_or_else(
            || {
                self.warp4
                    .as_ref()
                    .is_some_and(|engine| engine.has_focused_input())
            },
            |engine| engine.has_focused_input(),
        )
    }

    pub fn is_warp3(&self) -> bool {
        self.warp3.is_some()
    }

    pub fn is_animating(&self) -> bool {
        self.warp3
            .as_ref()
            .is_some_and(crate::warp3::Warp3Engine::is_screen_transition_active)
    }

    pub fn handle_key(&mut self, key: u8) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.handle_key(key);
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.handle_key(key);
        }
    }

    pub fn handle_text(&mut self, text: &str, replace_chars: usize) {
        if let Some(engine) = self.warp3.as_mut() {
            engine.handle_text(text, replace_chars);
        }
        if let Some(engine) = self.warp4.as_mut() {
            engine.handle_text(text, replace_chars);
        }
    }

    fn body_node(&self) -> Option<usize> {
        self.nodes.iter().position(|node| node.tag == "body")
    }

    fn compute_styles(&self) -> Vec<Style> {
        let mut styles = Vec::with_capacity(self.nodes.len());
        for idx in 0..self.nodes.len() {
            let parent_style = self.nodes[idx].parent.and_then(|parent| styles.get(parent));
            let mut style = Style::inherited(parent_style);
            apply_tag_defaults(&self.nodes[idx].tag, &mut style);

            let mut matching: Vec<&CssRule> = self
                .rules
                .iter()
                .filter(|rule| {
                    rule.selector
                        .matches(&self.nodes[idx], self.hovered_node == Some(idx))
                })
                .collect();
            matching.sort_by_key(|rule| (rule.selector.specificity(), rule.order));
            for rule in matching {
                apply_declarations(&mut style, &rule.declarations);
            }
            let inline = parse_declarations(self.nodes[idx].attr("style"));
            apply_declarations(&mut style, &inline);
            styles.push(style);
        }
        styles
    }

    fn layout_node(
        &mut self,
        idx: usize,
        x: i32,
        y: i32,
        available_w: i32,
        styles: &[Style],
    ) -> i32 {
        if idx >= self.nodes.len() || self.items.len() >= MAX_ITEMS {
            return 0;
        }
        let style = styles[idx].clone();
        if style.display == Display::None
            || matches!(
                self.nodes[idx].tag.as_str(),
                "head" | "style" | "script" | "title" | "meta" | "link"
            )
        {
            return 0;
        }
        if self.nodes[idx].tag == "#text" {
            let text = normalize_whitespace(&self.nodes[idx].text);
            return self.layout_text(idx, &text, x, y, available_w, &style, String::new());
        }
        if self.nodes[idx].tag == "br" {
            return line_height(style.font_size);
        }
        if self.nodes[idx].tag == "hr" {
            self.items.push(PaintItem {
                x,
                y: y + 7,
                w: available_w,
                h: 1,
                kind: PaintKind::Box {
                    background: Some(style.border_color),
                    border: style.border_color,
                    border_width: 0,
                    radius: 0,
                },
            });
            return 16;
        }

        let outer_x = x + style.margin.left;
        let outer_y = y + style.margin.top;
        let max_w = (available_w - style.margin.left - style.margin.right).max(1);
        let box_w = resolve_width(style.width, max_w)
            .unwrap_or(max_w)
            .min(max_w)
            .max(1);
        let inner_x = outer_x + style.padding.left + style.border_width;
        let inner_y = outer_y + style.padding.top + style.border_width;
        let inner_w =
            (box_w - style.padding.left - style.padding.right - style.border_width * 2).max(1);

        let paint_index = self.items.len();
        self.items.push(PaintItem {
            x: outer_x,
            y: outer_y,
            w: box_w,
            h: 0,
            kind: PaintKind::Box {
                background: style.background,
                border: style.border_color,
                border_width: style.border_width,
                radius: style.radius,
            },
        });

        let tag = self.nodes[idx].tag.clone();
        let href = self.effective_href(idx);
        let textual = is_textual_tag(&tag);
        let mut content_h = 0;

        if textual {
            let text = self.collect_text(idx);
            content_h =
                self.layout_text(idx, &text, inner_x, inner_y, inner_w, &style, href.clone());
        } else {
            let own_text = normalize_whitespace(&self.nodes[idx].text);
            if !own_text.is_empty() {
                content_h += self.layout_text(
                    idx,
                    &own_text,
                    inner_x,
                    inner_y,
                    inner_w,
                    &style,
                    href.clone(),
                );
            }

            let children = self.nodes[idx].children.clone();
            if style.display == Display::Flex
                && style.flex_direction == FlexDirection::Row
                && !children.is_empty()
            {
                let visible_count = children
                    .iter()
                    .filter(|child| self.is_layout_visible(**child, styles))
                    .count()
                    .max(1) as i32;
                let child_w = ((inner_w - style.gap * (visible_count - 1)) / visible_count).max(1);
                let mut child_x = inner_x;
                let mut max_h = 0;
                for child in children {
                    if !self.is_layout_visible(child, styles) {
                        continue;
                    }
                    let h = self.layout_node(child, child_x, inner_y, child_w, styles);
                    max_h = max_h.max(h);
                    child_x += child_w + style.gap;
                }
                content_h = content_h.max(max_h);
            } else {
                let mut child_y = inner_y + content_h;
                for child in children {
                    let h = self.layout_node(child, inner_x, child_y, inner_w, styles);
                    if h > 0 {
                        child_y += h + style.gap;
                        content_h = child_y - inner_y - style.gap;
                    }
                }
            }
        }

        let natural_h =
            content_h + style.padding.top + style.padding.bottom + style.border_width * 2;
        let box_h = style.height.unwrap_or(natural_h).max(natural_h).max(1);
        self.items[paint_index].h = box_h;

        if !href.is_empty() {
            self.hits.push(HitArea {
                node: idx,
                x: outer_x,
                y: outer_y,
                w: box_w,
                h: box_h,
                href,
            });
        }

        style.margin.top + box_h + style.margin.bottom
    }

    fn is_layout_visible(&self, idx: usize, styles: &[Style]) -> bool {
        styles[idx].display != Display::None
            && (self.nodes[idx].tag != "#text"
                || !normalize_whitespace(&self.nodes[idx].text).is_empty())
    }

    fn layout_text(
        &mut self,
        idx: usize,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        style: &Style,
        href: String,
    ) -> i32 {
        if text.trim().is_empty() {
            return 0;
        }
        let lines = wrap_text(text, width.max(1), style.font_size);
        let line_h = line_height(style.font_size);
        let large = style.font_size >= 22 || style.bold;
        for (line_index, line) in lines.iter().enumerate() {
            let text_w = measure_text(line, style.font_size).min(width);
            let text_x = match style.align {
                TextAlign::Left => x,
                TextAlign::Center => x + (width - text_w) / 2,
                TextAlign::Right => x + width - text_w,
            };
            self.items.push(PaintItem {
                x: text_x,
                y: y + line_index as i32 * line_h,
                w: text_w,
                h: line_h,
                kind: PaintKind::Text {
                    text: line.clone(),
                    color: style.color,
                    large,
                    underline: style.underline,
                },
            });
        }
        let h = lines.len() as i32 * line_h;
        if !href.is_empty() {
            self.hits.push(HitArea {
                node: idx,
                x,
                y,
                w: width,
                h,
                href,
            });
        }
        h
    }

    fn collect_text(&self, idx: usize) -> String {
        let mut output = String::new();
        self.collect_text_into(idx, &mut output);
        normalize_whitespace(&output)
    }

    fn collect_text_into(&self, idx: usize, output: &mut String) {
        let node = &self.nodes[idx];
        if node.tag == "span" && !node.attr("config").is_empty() {
            if let Some(path) = node.attr("config").strip_prefix("os://") {
                let path = path.split('?').next().unwrap_or(path).trim_matches('/');
                if let Some(value) = config::get_config().get(path) {
                    output.push_str(value);
                    return;
                }
            }
        }
        output.push_str(&node.text);
        for child in &node.children {
            if self.nodes[*child].tag == "br" {
                output.push('\n');
            } else {
                self.collect_text_into(*child, output);
            }
        }
    }

    fn effective_href(&self, idx: usize) -> String {
        let href = self.nodes[idx].attr("href");
        if href.starts_with("os://") || href.starts_with("app://") {
            href.to_string()
        } else {
            String::new()
        }
    }
}

