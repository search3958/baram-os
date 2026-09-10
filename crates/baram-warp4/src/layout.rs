use super::*;

impl Warp4Engine {
    pub(crate) fn refresh_visibility(&mut self) {
        for n in &mut self.nodes {
            n.hidden = n.attr("visibility") == "gone";
        }
    }

    pub(crate) fn own_height(&self, idx: usize, forced_h: Option<i32>) -> Option<i32> {
        forced_h.or_else(|| {
            let raw = self.nodes[idx].attr("layout_height");
            if is_match_parent(raw) {
                Some(self.height.max(1))
            } else if raw != "wrap_content" && !raw.is_empty() {
                Some(parse_dim(raw, 0))
            } else {
                None
            }
        })
    }

    pub(crate) fn resolved_height(
        &self,
        idx: usize,
        available: i32,
        intrinsic_available: i32,
    ) -> i32 {
        dimension(
            self.nodes[idx].attr("layout_height"),
            available,
            self.intrinsic_h(idx, intrinsic_available),
        )
    }

    pub(crate) fn layout(
        &mut self,
        idx: usize,
        x: i32,
        y: i32,
        available_w: i32,
        forced_h: Option<i32>,
        parent_orientation: &str,
    ) -> i32 {
        let margin = edges(&self.nodes[idx], "layout_margin");
        let pad = edges(&self.nodes[idx], "padding");
        let width_attr = self.nodes[idx].attr("layout_width");
        let weighted_width = parent_orientation == "LinearLayout"
            && parse_i32(self.nodes[idx].attr("layout_weight")) > 0
            && is_zero_dimension(width_attr);
        let w = if weighted_width {
            available_w
        } else {
            dimension(width_attr, available_w, self.intrinsic_w(idx, available_w))
        }
        .max(1);
        let tag = self.nodes[idx].tag.clone();
        let own_h = self.own_height(idx, forced_h);
        self.nodes[idx].x = x + margin.left;
        self.nodes[idx].y = y + margin.top;
        self.nodes[idx].w = (w - margin.left - margin.right).max(1);
        self.nodes[idx].content_w = (self.nodes[idx].w - pad.left - pad.right).max(1);
        if tag == "LinearLayout" || tag == "RadioGroup" {
            let horizontal = self.nodes[idx].attr("orientation") == "horizontal";
            let inner_w = (self.nodes[idx].w - pad.left - pad.right).max(1);
            let inner_x = self.nodes[idx].x + pad.left;
            let inner_y = self.nodes[idx].y + pad.top;
            let children = self.nodes[idx].children.clone();
            // Match Warp3's native row/section spacing when an XML layout
            // does not specify a gap explicitly.
            let layout_gap = parse_dim(
                self.nodes[idx].attr("layout_gap"),
                if is_xiao() { ui_px(4) } else { ui_px(8) },
            )
            .max(0);
            let visible_children = children
                .iter()
                .filter(|child| self.nodes[**child].visible())
                .count();
            let mut fixed = 0;
            let mut weights = 0i32;
            for &child in &children {
                if !self.nodes[child].visible() {
                    continue;
                }
                let e = edges(&self.nodes[child], "layout_margin");
                let weight = parse_i32(self.nodes[child].attr("layout_weight"));
                if weight > 0 {
                    weights += weight;
                } else if horizontal {
                    fixed += self.intrinsic_w(child, inner_w) + e.left + e.right;
                } else {
                    fixed += self.intrinsic_h(child, inner_w) + e.top + e.bottom;
                }
            }
            fixed += layout_gap * visible_children.saturating_sub(1) as i32;
            let inner_h = own_h.unwrap_or_else(|| {
                if horizontal {
                    self.intrinsic_h(idx, available_w)
                } else {
                    fixed + pad.top + pad.bottom
                }
            });
            self.nodes[idx].h = inner_h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let free = (if horizontal {
                inner_w
            } else {
                inner_h - pad.top - pad.bottom
            })
            .saturating_sub(fixed)
            .max(0);
            let mut cursor = if horizontal { inner_x } else { inner_y };
            for child in children {
                if !self.nodes[child].visible() {
                    continue;
                }
                let e = edges(&self.nodes[child], "layout_margin");
                let weight = parse_i32(self.nodes[child].attr("layout_weight"));
                let allocated = if horizontal {
                    if weight > 0 {
                        free * weight / weights.max(1)
                    } else {
                        self.intrinsic_w(child, inner_w)
                    }
                } else if weight > 0 {
                    free * weight / weights.max(1)
                } else {
                    self.intrinsic_h(child, inner_w)
                };
                if horizontal {
                    let child_h = if is_match_parent(self.nodes[child].attr("layout_height")) {
                        Some(self.nodes[idx].content_h)
                    } else {
                        None
                    };
                    self.layout(child, cursor, inner_y, allocated.max(1), child_h, &tag);
                    let child_h = self.nodes[child].h;
                    let cross = cross_offset(
                        self.nodes[child].attr("layout_gravity"),
                        self.nodes[idx].h - pad.top - pad.bottom,
                        child_h + e.top + e.bottom,
                        true,
                    );
                    self.nodes[child].y = inner_y + cross + e.top;
                    cursor += self.nodes[child].w + e.left + e.right + layout_gap;
                } else {
                    self.layout(
                        child,
                        inner_x,
                        cursor,
                        inner_w,
                        Some(allocated.max(1)),
                        &tag,
                    );
                    let cross = cross_offset(
                        self.nodes[child].attr("layout_gravity"),
                        inner_w,
                        self.nodes[child].w + e.left + e.right,
                        false,
                    );
                    self.nodes[child].x = inner_x + cross + e.left;
                    cursor += self.nodes[child].h + e.top + e.bottom + layout_gap;
                }
            }
        } else if tag == "ScrollView" || tag == "HorizontalScrollView" {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            let inner_w = (self.nodes[idx].w - pad.left - pad.right).max(1);
            let inner_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let child = self.nodes[idx]
                .children
                .iter()
                .copied()
                .find(|c| self.nodes[*c].visible());
            let mut content_w = inner_w;
            let mut content_h = inner_h;
            if let Some(child) = child {
                let fill = self.nodes[idx].attr("fillViewport") == "true";
                let forced = if fill && tag == "ScrollView" {
                    Some(inner_h)
                } else {
                    None
                };
                self.layout(
                    child,
                    self.nodes[idx].x + pad.left,
                    self.nodes[idx].y + pad.top,
                    if tag == "HorizontalScrollView" {
                        self.intrinsic_w(child, inner_w).max(inner_w)
                    } else {
                        inner_w
                    },
                    forced,
                    &tag,
                );
                content_w = if tag == "HorizontalScrollView" {
                    (self.subtree_right(child) - self.nodes[idx].x + pad.right).max(inner_w)
                } else {
                    inner_w
                };
                content_h = if tag == "ScrollView" {
                    (self.subtree_bottom(child) - self.nodes[idx].y + pad.bottom).max(inner_h)
                } else {
                    inner_h
                };
            }
            self.nodes[idx].content_w = content_w;
            self.nodes[idx].content_h = content_h;
        } else if tag == "RelativeLayout" {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            self.layout_relative(idx, pad);
        } else if tag == "FrameLayout"
            || tag == "ViewFlipper"
            || tag == "ViewAnimator"
            || tag == "ViewSwitcher"
            || tag == "TextSwitcher"
        {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let children = self.nodes[idx].children.clone();
            let active = if tag == "ViewFlipper"
                || tag == "ViewAnimator"
                || tag == "ViewSwitcher"
                || tag == "TextSwitcher"
            {
                parse_i32(self.nodes[idx].attr("displayedChild")).max(0) as usize
            } else {
                usize::MAX
            };
            for (pos, child) in children.into_iter().enumerate() {
                if !self.nodes[child].visible() || pos != active && active != usize::MAX {
                    continue;
                }
                let child_w = dimension(
                    self.nodes[child].attr("layout_width"),
                    self.nodes[idx].content_w,
                    self.intrinsic_w(child, self.nodes[idx].content_w),
                );
                let child_h = self.resolved_height(
                    child,
                    self.nodes[idx].content_h,
                    self.nodes[idx].content_w,
                );
                self.layout(
                    child,
                    self.nodes[idx].x + pad.left,
                    self.nodes[idx].y + pad.top,
                    child_w.max(1),
                    Some(child_h.max(1)),
                    &tag,
                );
                let e = edges(&self.nodes[child], "layout_margin");
                let (dx, dy) = gravity_offset(
                    self.nodes[child].attr("layout_gravity"),
                    self.nodes[idx].content_w,
                    self.nodes[idx].content_h,
                    self.nodes[child].w + e.left + e.right,
                    self.nodes[child].h + e.top + e.bottom,
                );
                self.nodes[child].x = self.nodes[idx].x + pad.left + dx + e.left;
                self.nodes[child].y = self.nodes[idx].y + pad.top + dy + e.top;
            }
        } else if tag == "AbsoluteLayout" {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let children = self.nodes[idx].children.clone();
            for child in children {
                if !self.nodes[child].visible() {
                    continue;
                }
                let child_w = dimension(
                    self.nodes[child].attr("layout_width"),
                    self.nodes[idx].content_w,
                    self.intrinsic_w(child, self.nodes[idx].content_w),
                );
                self.layout(
                    child,
                    self.nodes[idx].x + pad.left + parse_dim(self.nodes[child].attr("layout_x"), 0),
                    self.nodes[idx].y + pad.top + parse_dim(self.nodes[child].attr("layout_y"), 0),
                    child_w.max(1),
                    is_match_parent(self.nodes[child].attr("layout_height"))
                        .then_some(self.nodes[idx].content_h),
                    &tag,
                );
            }
        } else if tag == "GridLayout" {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            self.layout_grid(idx, pad);
        } else if tag == "TableLayout" || tag == "TableRow" {
            let horizontal = tag == "TableRow";
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let mut cursor = if horizontal {
                self.nodes[idx].x + pad.left
            } else {
                self.nodes[idx].y + pad.top
            };
            let children = self.nodes[idx].children.clone();
            let table_stretch = self.nodes[idx]
                .parent
                .filter(|parent| self.nodes[*parent].is("TableLayout"))
                .map(|parent| self.nodes[parent].attr("stretchColumns").to_string())
                .unwrap_or_default();
            let stretch_all = table_stretch.contains('*');
            let visible_count = children
                .iter()
                .filter(|c| self.nodes[**c].visible())
                .count()
                .max(1) as i32;
            let mut fixed_width = 0;
            let mut stretch_count = 0;
            for (column, child) in children.iter().enumerate() {
                if !horizontal || !self.nodes[*child].visible() {
                    continue;
                }
                let stretched = stretch_all
                    || table_stretch
                        .split(',')
                        .any(|value| value.trim().parse::<usize>().ok() == Some(column));
                let e = edges(&self.nodes[*child], "layout_margin");
                if stretched {
                    stretch_count += 1;
                } else {
                    fixed_width +=
                        self.intrinsic_w(*child, self.nodes[idx].content_w) + e.left + e.right;
                }
            }
            let stretch_width = (self.nodes[idx].content_w.saturating_sub(fixed_width)
                / stretch_count.max(1))
            .max(1);
            for (column, child) in children.into_iter().enumerate() {
                if !self.nodes[child].visible() {
                    continue;
                }
                let e = edges(&self.nodes[child], "layout_margin");
                if !horizontal
                    && self.nodes[idx].attr("stretchColumns").contains('*')
                    && self.nodes[child].is("TableRow")
                    && self.nodes[child].attr("stretchColumns").is_empty()
                {
                    set_attr(&mut self.nodes[child], "stretchColumns", "*");
                }
                let stretched = horizontal
                    && (stretch_all
                        || table_stretch
                            .split(',')
                            .any(|value| value.trim().parse::<usize>().ok() == Some(column)));
                let allocated = if horizontal && stretched {
                    if stretch_all {
                        (self.nodes[idx].content_w / visible_count).max(1)
                    } else {
                        stretch_width
                    }
                } else if horizontal {
                    self.intrinsic_w(child, self.nodes[idx].content_w)
                } else {
                    self.nodes[idx].content_w
                };
                if horizontal {
                    self.layout(
                        child,
                        cursor,
                        self.nodes[idx].y + pad.top,
                        allocated,
                        None,
                        &tag,
                    );
                    if stretched {
                        self.nodes[child].w = (allocated - e.left - e.right).max(1);
                        self.nodes[child].content_w = (self.nodes[child].w
                            - edges(&self.nodes[child], "padding").left
                            - edges(&self.nodes[child], "padding").right)
                            .max(1);
                    }
                    cursor += allocated + e.left + e.right;
                } else {
                    self.layout(
                        child,
                        self.nodes[idx].x + pad.left,
                        cursor,
                        allocated,
                        None,
                        &tag,
                    );
                    cursor += self.nodes[child].h + e.top + e.bottom;
                }
            }
        } else {
            let h = own_h.unwrap_or_else(|| self.intrinsic_h(idx, available_w));
            self.nodes[idx].h = h.max(1);
            self.nodes[idx].content_h = (self.nodes[idx].h - pad.top - pad.bottom).max(1);
            let children = self.nodes[idx].children.clone();
            let mut cy = self.nodes[idx].y + pad.top;
            for child in children {
                if self.nodes[child].visible() {
                    let ch = self.layout(
                        child,
                        self.nodes[idx].x + pad.left,
                        cy,
                        (self.nodes[idx].w - pad.left - pad.right).max(1),
                        is_match_parent(self.nodes[child].attr("layout_height"))
                            .then_some(self.nodes[idx].content_h),
                        &tag,
                    );
                    cy += ch;
                }
            }
        }
        self.nodes[idx].h + margin.top + margin.bottom
    }

    pub(crate) fn layout_relative(&mut self, idx: usize, pad: Edges) {
        let parent_x = self.nodes[idx].x + pad.left;
        let parent_y = self.nodes[idx].y + pad.top;
        let parent_w = self.nodes[idx].content_w;
        let parent_h = self.nodes[idx].content_h;
        let children = self.nodes[idx].children.clone();
        for child in &children {
            if !self.nodes[*child].visible() {
                continue;
            }
            let cw = dimension(
                self.nodes[*child].attr("layout_width"),
                parent_w,
                self.intrinsic_w(*child, parent_w),
            );
            let ch = self.resolved_height(*child, parent_h, parent_w);
            self.layout(
                *child,
                parent_x,
                parent_y,
                cw.max(1),
                Some(ch.max(1)),
                "RelativeLayout",
            );
        }
        for &child in &children {
            if !self.nodes[child].visible() {
                continue;
            }
            let n = self.nodes[child].clone();
            let e = edges(&n, "layout_margin");
            let mut x = e.left;
            let mut y = e.top;
            if truth(n.attr("layout_alignParentRight")) || truth(n.attr("layout_alignParentEnd")) {
                x = parent_w - n.w - e.right;
            }
            if truth(n.attr("layout_centerHorizontal")) || truth(n.attr("layout_centerInParent")) {
                x = (parent_w - n.w) / 2;
            }
            if truth(n.attr("layout_alignParentBottom")) {
                y = parent_h - n.h - e.bottom;
            }
            if truth(n.attr("layout_centerVertical")) || truth(n.attr("layout_centerInParent")) {
                y = (parent_h - n.h) / 2;
            }
            let sibling = |key: &str, nodes: &Vec<Node>, children: &Vec<usize>| -> Option<Node> {
                let id = nodes[child].attr(key);
                if id.is_empty() {
                    return None;
                }
                let id = id.trim_start_matches("@+id/").trim_start_matches("@id/");
                children
                    .iter()
                    .find_map(|other| (nodes[*other].id() == id).then(|| nodes[*other].clone()))
            };
            if let Some(ref q) = sibling("layout_below", &self.nodes, &children) {
                y = q.y - parent_y + q.h + e.top;
            }
            if let Some(ref q) = sibling("layout_above", &self.nodes, &children) {
                y = q.y - parent_y - n.h - e.bottom;
            }
            if let Some(ref q) = sibling("layout_toRightOf", &self.nodes, &children)
                .or_else(|| sibling("layout_toEndOf", &self.nodes, &children))
            {
                x = q.x - parent_x + q.w + e.left;
            }
            if let Some(ref q) = sibling("layout_toLeftOf", &self.nodes, &children)
                .or_else(|| sibling("layout_toStartOf", &self.nodes, &children))
            {
                x = q.x - parent_x - n.w - e.right;
            }
            if let Some(ref q) = sibling("layout_alignLeft", &self.nodes, &children)
                .or_else(|| sibling("layout_alignStart", &self.nodes, &children))
            {
                x = q.x - parent_x + e.left;
            }
            if let Some(ref q) = sibling("layout_alignRight", &self.nodes, &children)
                .or_else(|| sibling("layout_alignEnd", &self.nodes, &children))
            {
                x = q.x - parent_x + q.w - n.w - e.right;
            }
            if let Some(ref q) = sibling("layout_alignTop", &self.nodes, &children) {
                y = q.y - parent_y + e.top;
            }
            if let Some(ref q) = sibling("layout_alignBottom", &self.nodes, &children) {
                y = q.y - parent_y + q.h - n.h - e.bottom;
            }
            self.nodes[child].x = (parent_x + x).max(parent_x);
            self.nodes[child].y = (parent_y + y).max(parent_y);
        }
    }

    pub(crate) fn layout_grid(&mut self, idx: usize, pad: Edges) {
        let columns = parse_i32(self.nodes[idx].attr("columnCount")).max(1);
        let gap = if is_xiao() { ui_px(4) } else { ui_px(8) };
        let cell_w = ((self.nodes[idx].content_w - gap * (columns - 1)) / columns).max(1);
        let mut row_y = self.nodes[idx].y + pad.top;
        let mut row_h = 0;
        let mut column = 0;
        for child in self.nodes[idx].children.clone() {
            if !self.nodes[child].visible() {
                continue;
            }
            let explicit_col = self.nodes[child].attr("layout_column");
            if !explicit_col.is_empty() {
                column = parse_i32(explicit_col).max(0);
            }
            if column >= columns {
                row_y += row_h + gap;
                row_h = 0;
                column = 0;
            }
            let span = parse_i32(self.nodes[child].attr("layout_columnSpan"))
                .max(1)
                .min(columns - column);
            let allocated = cell_w * span + gap * (span - 1);
            self.layout(
                child,
                self.nodes[idx].x + pad.left + column * (cell_w + gap),
                row_y,
                allocated,
                None,
                "GridLayout",
            );
            row_h = row_h.max(self.nodes[child].h);
            column += span;
            if column >= columns {
                row_y += row_h + gap;
                row_h = 0;
                column = 0;
            }
        }
    }

    pub(crate) fn subtree_bottom(&self, idx: usize) -> i32 {
        self.nodes[idx]
            .children
            .iter()
            .fold(self.nodes[idx].y + self.nodes[idx].h, |bottom, child| {
                bottom.max(self.subtree_bottom(*child))
            })
    }
    pub(crate) fn subtree_right(&self, idx: usize) -> i32 {
        self.nodes[idx]
            .children
            .iter()
            .fold(self.nodes[idx].x + self.nodes[idx].w, |right, child| {
                right.max(self.subtree_right(*child))
            })
    }

    pub(crate) fn intrinsic_w(&self, idx: usize, available: i32) -> i32 {
        let n = &self.nodes[idx];
        let raw_width = n.attr("layout_width");
        if !raw_width.is_empty()
            && !is_match_parent(raw_width)
            && raw_width != "wrap_content"
            && !is_zero_dimension(raw_width)
        {
            return parse_dim(raw_width, available).max(0);
        }
        if n.is("Space") {
            return parse_dim(n.attr("layout_width"), 0).max(0);
        }
        if !n.attr("text").is_empty() {
            let pad = edges(n, "padding");
            let control_width = if n.is("Switch") {
                ui_px(55)
            } else if interactive(n) {
                ui_px(32)
            } else {
                0
            };
            return measure_size(n.attr("text"), text_size(n))
                + pad.left
                + pad.right
                + control_width;
        }
        if is_button_like(n) {
            return (measure_size(
                if n.is("ToggleButton") {
                    n.attr("textOn")
                } else {
                    n.attr("text")
                },
                text_size(n),
            ) + ui_px(32))
            .max(ui_px(64))
            .min(available.max(ui_px(64)));
        }
        if n.is("RatingBar") {
            let stars = parse_i32(n.attr("numStars")).clamp(1, 10);
            return stars * ui_px(22) + (stars - 1).max(0) * ui_px(1);
        }
        if n.is("Switch") {
            return if n.attr("text").is_empty() {
                ui_px(44)
            } else {
                ui_px(55) + measure_size(n.attr("text"), text_size(n))
            };
        }
        if n.is("EditText") || n.is("AutoCompleteTextView") || n.is("MultiAutoCompleteTextView") {
            return available.min(ui_px(240)).max(ui_px(80));
        }
        if (n.is("LinearLayout") || n.is("RadioGroup")) && n.attr("orientation") == "horizontal" {
            let pad = edges(n, "padding");
            return (pad.left
                + pad.right
                + n.children
                    .iter()
                    .filter(|c| self.nodes[**c].visible())
                    .map(|c| self.intrinsic_w(*c, available))
                    .sum::<i32>())
            .min(available);
        }
        if n.is("RelativeLayout")
            || n.is("FrameLayout")
            || n.is("AbsoluteLayout")
            || n.is("GridLayout")
        {
            return available;
        }
        available
    }
    pub(crate) fn intrinsic_h(&self, idx: usize, available: i32) -> i32 {
        let n = &self.nodes[idx];
        let raw_height = n.attr("layout_height");
        if !raw_height.is_empty()
            && !is_match_parent(raw_height)
            && raw_height != "wrap_content"
            && !is_zero_dimension(raw_height)
        {
            return parse_dim(raw_height, available).max(0);
        }
        if n.is("Space") {
            return parse_dim(n.attr("layout_height"), 0).max(0);
        }
        if is_button_like(n) {
            return ui_px(48);
        }
        if n.is("EditText") || n.is("AutoCompleteTextView") {
            return ui_px(38);
        }
        if n.is("MultiAutoCompleteTextView") {
            return ui_px(58);
        }
        if n.is("Switch") || n.is("CheckBox") || n.is("RadioButton") {
            return ui_px(44);
        }
        if n.is("SeekBar") {
            return ui_px(30);
        }
        if n.is("RatingBar") {
            return ui_px(32);
        }
        if n.is("Spinner") || n.is("SearchView") || n.is("DatePicker") || n.is("TimePicker") {
            return ui_px(38);
        }
        if n.is("ProgressBar") {
            return if n.attr("style").contains("progressBarStyleHorizontal") {
                ui_px(6)
            } else {
                ui_px(28)
            };
        }
        if n.is("TextView") {
            let pad = edges(n, "padding");
            let size = text_size(n);
            let line = (size * 1.25) as i32;
            let chars_per_line = (available.max(1)
                / (size.max(ui_size(8.0)) as i32 / 2).max(ui_px(4)))
            .max(1) as usize;
            let lines = n
                .attr("text")
                .split('\n')
                .map(|s| (s.chars().count().max(1) + chars_per_line - 1) / chars_per_line)
                .sum::<usize>()
                .max(1);
            return pad.top + pad.bottom + line * lines as i32;
        }
        let pad = edges(n, "padding");
        let child_h = if (n.is("LinearLayout") || n.is("RadioGroup") || n.is("TableRow"))
            && (n.attr("orientation") == "horizontal" || n.is("TableRow"))
        {
            n.children
                .iter()
                .filter(|c| self.nodes[**c].visible())
                .map(|c| self.intrinsic_h(*c, available))
                .max()
                .unwrap_or(0)
        } else if n.is("GridLayout") {
            let columns = parse_i32(n.attr("columnCount")).max(1) as usize;
            let mut row_h = 0;
            let mut rows = 0usize;
            let mut column = 0usize;
            for child in n.children.iter().filter(|c| self.nodes[**c].visible()) {
                let span = parse_i32(self.nodes[*child].attr("layout_columnSpan")).max(1) as usize;
                row_h = row_h.max(self.intrinsic_h(*child, available));
                column += span;
                if column >= columns {
                    rows += 1;
                    column = 0;
                    row_h = 0;
                }
            }
            if column > 0 {
                rows += 1;
            }
            let pad = edges(n, "padding");
            return (pad.top
                + pad.bottom
                + rows as i32 * ui_px(48)
                + rows.saturating_sub(1) as i32 * ui_px(8))
            .max(1);
        } else if n.is("FrameLayout")
            || n.is("RelativeLayout")
            || n.is("AbsoluteLayout")
            || n.is("GridLayout")
        {
            n.children
                .iter()
                .filter(|c| self.nodes[**c].visible())
                .map(|c| self.intrinsic_h(*c, available))
                .max()
                .unwrap_or(0)
        } else {
            n.children
                .iter()
                .filter(|c| self.nodes[**c].visible())
                .map(|c| {
                    self.intrinsic_h(*c, available)
                        + edges(&self.nodes[*c], "layout_margin").top
                        + edges(&self.nodes[*c], "layout_margin").bottom
                })
                .sum()
        };
        (pad.top + pad.bottom + child_h).max(1)
    }
}
