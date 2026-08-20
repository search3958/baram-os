/// CSS `cubic-bezier(0, 0, 0, 1)`. Since both x control points are zero,
/// x(s) = s^3; a short binary solve is sufficient for the 1 ms UI clock.
fn decelerate_scroll(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let mut low = 0.0f32;
    let mut high = 1.0f32;
    for _ in 0..10 {
        let s = (low + high) * 0.5;
        if s * s * s < t {
            low = s;
        } else {
            high = s;
        }
    }
    let s = (low + high) * 0.5;
    s * s * (3.0 - 2.0 * s)
}

struct CachedShadow {
    win_x: i32,
    win_y: i32,
    win_w: usize,
    win_h: usize,
    alpha: Vec<u8>,
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
}

const FILE_DIALOG_LIST_Y: i32 = 96;
const FILE_DIALOG_FOOTER_H: i32 = 56;
const FILE_DIALOG_SMALL_CELL_H: i32 = 42;
const FILE_DIALOG_LARGE_CELL_H: i32 = 100;
const FILE_DIALOG_GRID_COLUMNS: usize = 5;

macro_rules! file_icon_bytes {
    ($size:literal, $name:literal) => {
        include_bytes!(concat!(
            "../../../files/data/file/",
            $size,
            "/",
            $name,
            ".png"
        ))
    };
}

fn file_icon_key(entry: &vfs::FileEntry) -> &'static str {
    if entry.is_dir {
        return match entry.name.as_str() {
            "app" => "files-folder-app",
            "data" => "files-folder-data",
            "os" => "files-folder-os",
            "user" => "files-folder-user",
            _ => "files-folder",
        };
    }
    let name = entry.name.as_str();
    if name.ends_with(".ini") {
        "files-file-appini"
    } else if name.ends_with(".w3s")
        || name.ends_with(".w4s")
        || name.ends_with(".warp")
        || name.ends_with(".sh")
    {
        "files-file-appscript"
    } else if name.ends_with(".w3u") || name.ends_with(".w4u") || name.ends_with(".xml") {
        "files-file-appxml"
    } else if name.ends_with(".w3a") || name.ends_with(".w4a") || name.ends_with(".s4a") {
        "files-folder-app"
    } else if name.ends_with(".svg") {
        "files-file-svg"
    } else if name.ends_with(".png")
        || name.ends_with(".jpg")
        || name.ends_with(".jpeg")
        || name.ends_with(".gif")
    {
        "files-file-image"
    } else if name.ends_with(".yaml") || name.ends_with(".yml") || name.ends_with(".md") {
        "files-file-yaml"
    } else if name.ends_with(".bin") || name.ends_with(".o") {
        "files-file-bin"
    } else if name.ends_with(".txt")
        || name.ends_with(".rs")
        || name.ends_with(".c")
        || name.ends_with(".h")
    {
        "files-file-text"
    } else {
        "files-file"
    }
}

fn file_icon(entry: &vfs::FileEntry, large: bool) -> &'static [u8] {
    let key = file_icon_key(entry);
    if large {
        match key {
            "files-folder-app" => file_icon_bytes!(64, "files-folder-app"),
            "files-folder-data" => file_icon_bytes!(64, "files-folder-data"),
            "files-folder-os" => file_icon_bytes!(64, "files-folder-os"),
            "files-folder-user" => file_icon_bytes!(64, "files-folder-user"),
            "files-folder" => file_icon_bytes!(64, "files-folder"),
            "files-file-appini" => file_icon_bytes!(64, "files-file-appini"),
            "files-file-appscript" => file_icon_bytes!(64, "files-file-appscript"),
            "files-file-appxml" => file_icon_bytes!(64, "files-file-appxml"),
            "files-file-bin" => file_icon_bytes!(64, "files-file-bin"),
            "files-file-image" => file_icon_bytes!(64, "files-file-image"),
            "files-file-redflag" => file_icon_bytes!(64, "files-file-redflag"),
            "files-file-svg" => file_icon_bytes!(64, "files-file-svg"),
            "files-file-text-1" => file_icon_bytes!(64, "files-file-text-1"),
            "files-file-text" => file_icon_bytes!(64, "files-file-text"),
            "files-file-warpfile" => file_icon_bytes!(64, "files-file-warpfile"),
            "files-file-yaml" => file_icon_bytes!(64, "files-file-yaml"),
            _ => file_icon_bytes!(64, "files-file"),
        }
    } else {
        match key {
            "files-folder-app" => file_icon_bytes!(24, "files-folder-app"),
            "files-folder-data" => file_icon_bytes!(24, "files-folder-data"),
            "files-folder-os" => file_icon_bytes!(24, "files-folder-os"),
            "files-folder-user" => file_icon_bytes!(24, "files-folder-user"),
            "files-folder" => file_icon_bytes!(24, "files-folder"),
            "files-file-appini" => file_icon_bytes!(24, "files-file-appini"),
            "files-file-appscript" => file_icon_bytes!(24, "files-file-appscript"),
            "files-file-appxml" => file_icon_bytes!(24, "files-file-appxml"),
            "files-file-bin" => file_icon_bytes!(24, "files-file-bin"),
            "files-file-image" => file_icon_bytes!(24, "files-file-image"),
            "files-file-redflag" => file_icon_bytes!(24, "files-file-redflag"),
            "files-file-svg" => file_icon_bytes!(24, "files-file-svg"),
            "files-file-text-1" => file_icon_bytes!(24, "files-file-text-1"),
            "files-file-text" => file_icon_bytes!(24, "files-file-text"),
            "files-file-warpfile" => file_icon_bytes!(24, "files-file-warpfile"),
            "files-file-yaml" => file_icon_bytes!(24, "files-file-yaml"),
            _ => file_icon_bytes!(24, "files-file"),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum NativeFileDialogAction {
    None,
    Changed,
    Cancel,
    Confirm,
}

/// OS-owned, read-only file chooser. It deliberately has no text editor or
/// app script behind it: the OS owns the path, selection, and buttons.
pub struct NativeFileDialog {
    win_id: WinId,
    content_width: i32,
    content_height: i32,
    path: String,
    entries: Vec<vfs::FileEntry>,
    selected: Option<usize>,
    scroll: SmoothScroll,
    large_view: bool,
}

impl NativeFileDialog {
    pub fn new(win_id: WinId, path: &str, window_width: usize, window_height: usize) -> Self {
        let mut dialog = Self {
            win_id,
            content_width: window_width as i32,
            // Native file dialogs are borderless OS surfaces, so their
            // content starts at the top of the window.
            content_height: window_height as i32,
            path: path.into(),
            entries: Vec::new(),
            selected: None,
            scroll: SmoothScroll::new(),
            large_view: false,
        };
        dialog.reload();
        dialog
    }

    pub fn win_id(&self) -> WinId {
        self.win_id
    }

    pub fn selected_path(&self) -> Option<String> {
        let index = self.selected?;
        let entry = self.entries.get(index)?;
        if entry.is_dir {
            return None;
        }
        Some(format!(
            "{}/{}",
            self.path.trim_end_matches('/'),
            entry.name
        ))
    }

    fn reload(&mut self) {
        self.entries = vfs::list_files(&self.path);
        self.selected = None;
        self.scroll.reset();
        self.update_scroll_limit();
    }

    fn display_path(&self) -> String {
        if self.path.trim_end_matches('/') == "files" {
            "files://".into()
        } else {
            format!(
                "files://{}/",
                self.path.trim_start_matches("files/").trim_end_matches('/')
            )
        }
    }

    fn footer_top(&self) -> i32 {
        self.content_height - FILE_DIALOG_FOOTER_H
    }

    fn cell_height(&self) -> i32 {
        if self.large_view {
            FILE_DIALOG_LARGE_CELL_H
        } else {
            FILE_DIALOG_SMALL_CELL_H
        }
    }

    fn columns(&self) -> usize {
        if self.large_view {
            FILE_DIALOG_GRID_COLUMNS
        } else {
            1
        }
    }

    fn visible_rows(&self) -> usize {
        self.footer_top().saturating_sub(FILE_DIALOG_LIST_Y).max(0) as usize
            / self.cell_height() as usize
    }

    fn update_scroll_limit(&mut self) {
        let rows = (self.entries.len() + self.columns() - 1) / self.columns();
        let content_height = rows as i32 * self.cell_height();
        let viewport_height = self.footer_top().saturating_sub(FILE_DIALOG_LIST_Y);
        self.scroll
            .set_max(content_height.saturating_sub(viewport_height));
    }

    pub fn click(&mut self, x: i32, y: i32) -> NativeFileDialogAction {
        if x >= 12 && x < 108 && y >= 46 && y < 82 {
            let path = self.path.trim_end_matches('/');
            self.path = path
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_string())
                .unwrap_or_else(|| "files".into());
            self.reload();
            return NativeFileDialogAction::Changed;
        }

        let view_button_y = 46;
        if y >= view_button_y && y < view_button_y + 36 {
            let large_x = self.content_width - 84;
            let small_x = self.content_width - 156;
            if x >= small_x && x < small_x + 64 {
                if self.large_view {
                    self.large_view = false;
                    self.update_scroll_limit();
                    return NativeFileDialogAction::Changed;
                }
            } else if x >= large_x && x < large_x + 64 {
                if !self.large_view {
                    self.large_view = true;
                    self.update_scroll_limit();
                    return NativeFileDialogAction::Changed;
                }
            }
        }

        let footer_top = self.footer_top();
        if y >= footer_top {
            let button_w = (560 - 32) / 2;
            let second_x = 20 + button_w;
            if x >= 12 && x < 12 + button_w {
                return NativeFileDialogAction::Cancel;
            }
            if x >= second_x && x < second_x + button_w {
                return NativeFileDialogAction::Confirm;
            }
            return NativeFileDialogAction::None;
        }

        let list_bottom = FILE_DIALOG_LIST_Y + self.visible_rows() as i32 * self.cell_height();
        if y < FILE_DIALOG_LIST_Y || y >= list_bottom {
            return NativeFileDialogAction::None;
        }
        let available_width = self.content_width.saturating_sub(24);
        let gap = 8;
        let columns = self.columns();
        let cell_width = (available_width - gap * (columns as i32 - 1)) / columns as i32;
        let local_x = x - 12;
        if local_x < 0 {
            return NativeFileDialogAction::None;
        }
        let column = (local_x / (cell_width + gap)) as usize;
        if column >= columns {
            return NativeFileDialogAction::None;
        }
        let local_y = y - FILE_DIALOG_LIST_Y + self.scroll.position.max(0);
        let row = (local_y / self.cell_height()) as usize;
        let index = row * columns + column;
        let Some(entry) = self.entries.get(index).cloned() else {
            return NativeFileDialogAction::None;
        };
        if entry.is_dir {
            self.path = format!("{}/{}", self.path.trim_end_matches('/'), entry.name);
            self.reload();
        } else {
            self.selected = Some(index);
        }
        NativeFileDialogAction::Changed
    }

    pub fn scroll_by(&mut self, delta: i32) -> bool {
        self.scroll.scroll(delta)
    }

    pub fn tick_scroll(&mut self, now_ns: u64) -> bool {
        self.scroll.tick(now_ns)
    }

    pub fn draw_to_layer(&self, layer: &mut LayerSystem, body_y: i32) {
        let width = layer.width();
        let height = layer.height();
        let body_top = body_y.max(0) as usize;
        if body_top >= height {
            return;
        }
        layer.fill_rect(
            0,
            body_top,
            width,
            height - body_top,
            Color::rgb(250, 250, 252),
        );

        let path_text = native_truncate(&self.display_path(), width.saturating_sub(32));
        layer.put_str(16, body_top + 14, &path_text, Color::TEXT);
        draw_native_button(layer, 12, body_top + 46, 96, 36, "戻る", false);
        let selected = self
            .selected
            .and_then(|index| self.entries.get(index))
            .map(|entry| format!("選択中: {}", entry.name))
            .unwrap_or_else(|| "ファイルを選択してください".into());

        let small_x = width.saturating_sub(156);
        let large_x = width.saturating_sub(84);
        let selected_text = native_truncate(&selected, small_x.saturating_sub(128));
        layer.put_str(120, body_top + 59, &selected_text, Color::MUTED);
        draw_native_button(
            layer,
            small_x,
            body_top + 46,
            64,
            36,
            "小",
            !self.large_view,
        );
        draw_native_button(layer, large_x, body_top + 46, 64, 36, "大", self.large_view);

        let footer_top = self.footer_top().max(0) as usize + body_top;
        let list_y = body_top + FILE_DIALOG_LIST_Y as usize;
        let gap = 8usize;
        let columns = self.columns();
        let cell_width = (width.saturating_sub(24 + gap * (columns - 1))) / columns;
        let offset = self.scroll.position.max(0) as usize;
        let first_row = offset / self.cell_height() as usize;
        let offset_in_row = offset % self.cell_height() as usize;
        let rows_to_draw = self.visible_rows() + 2;
        for row_offset in 0..rows_to_draw {
            let row = first_row + row_offset;
            for column in 0..columns {
                let index = row * columns + column;
                let x = 12 + column * (cell_width + gap);
                let y = list_y + row_offset * self.cell_height() as usize - offset_in_row;
                let cell_h = self.cell_height().saturating_sub(4) as usize;
                let Some(entry) = self.entries.get(index) else {
                    continue;
                };
                let selected = self.selected == Some(index);
                if !self.large_view {
                    let row_bg = if selected {
                        Color::BTN_PRIMARY
                    } else {
                        Color::rgb(242, 242, 245)
                    };
                    layer.fill_rounded_rect(x, y, cell_width, cell_h, 6, row_bg);
                }
                let icon_size = if self.large_view { 64 } else { 24 };
                let icon_x = if self.large_view {
                    x + cell_width.saturating_sub(icon_size) / 2
                } else {
                    x + 10
                };
                let icon_y = if self.large_view {
                    y + 4
                } else {
                    y + (cell_h.saturating_sub(icon_size)) / 2
                };
                draw_native_file_icon(
                    layer,
                    file_icon(entry, self.large_view),
                    icon_x,
                    icon_y,
                    icon_size,
                );
                let text_color = if !self.large_view && selected {
                    Color::BTN_TEXT
                } else {
                    Color::TEXT
                };
                let label = if self.large_view {
                    native_truncate(&entry.name, cell_width.saturating_sub(10))
                } else {
                    native_truncate(&entry.name, cell_width.saturating_sub(50))
                };
                let label_x = if self.large_view {
                    x + cell_width.saturating_sub(native_text_width(&label)) / 2
                } else {
                    x + 42
                };
                let label_y = if self.large_view {
                    y + cell_h.saturating_sub(16)
                } else {
                    y + (cell_h.saturating_sub(16)) / 2
                };
                if self.large_view && selected {
                    let label_width = native_text_width(&label).saturating_add(10);
                    let label_bg_x = x + cell_width.saturating_sub(label_width) / 2;
                    layer.fill_rounded_rect(
                        label_bg_x,
                        label_y.saturating_sub(4),
                        label_width,
                        22,
                        5,
                        Color::BTN_PRIMARY,
                    );
                    layer.put_str(label_bg_x + 5, label_y, &label, Color::BTN_TEXT);
                } else {
                    layer.put_str(label_x, label_y, &label, text_color);
                }
            }
        }

        let button_y = footer_top.saturating_sub(4);
        let button_w = width.saturating_sub(32) / 2;
        draw_native_button(layer, 12, button_y, button_w, 40, "キャンセル", false);
        draw_native_button(
            layer,
            20 + button_w,
            button_y,
            button_w,
            40,
            "アップロード",
            true,
        );
    }
}

fn draw_native_button(
    layer: &mut LayerSystem,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    label: &str,
    primary: bool,
) {
    let bg = if primary {
        Color::rgb(0, 106, 255)
    } else {
        Color::rgb(232, 232, 235)
    };
    let fg = if primary {
        Color::BTN_TEXT
    } else {
        Color::TEXT
    };
    layer.fill_rounded_rect(x, y, width, height, 8, bg);
    layer.put_str(x + 12, y + 12, label, fg);
}

fn native_text_width(text: &str) -> usize {
    text.chars()
        .map(|ch| {
            let glyph = baram_font::ttf_font::glyph(ch);
            if glyph.w > 0 {
                glyph.advance.max(0) as usize
            } else {
                8
            }
        })
        .sum()
}

fn native_truncate(text: &str, max_width: usize) -> String {
    if native_text_width(text) <= max_width {
        return text.into();
    }
    let ellipsis = "...";
    let ellipsis_width = native_text_width(ellipsis);
    if max_width <= ellipsis_width {
        return ellipsis.into();
    }
    let mut result = String::new();
    for ch in text.chars() {
        let next_width = native_text_width(&result)
            .saturating_add(native_text_width(&ch.to_string()))
            .saturating_add(ellipsis_width);
        if next_width > max_width {
            break;
        }
        result.push(ch);
    }
    result.push_str(ellipsis);
    result
}

fn draw_native_file_icon(layer: &mut LayerSystem, bytes: &[u8], x: usize, y: usize, size: usize) {
    let Ok((header, pixels)) = png_decoder::decode(bytes) else {
        return;
    };
    let src_w = header.width as usize;
    let src_h = header.height as usize;
    let width = layer.width();
    let height = layer.height();
    let buffer = layer.buf_mut();
    for py in 0..size {
        let dst_y = y + py;
        if dst_y >= height {
            continue;
        }
        let src_y = py * src_h / size.max(1);
        for px in 0..size {
            let dst_x = x + px;
            if dst_x >= width {
                continue;
            }
            let src_x = px * src_w / size.max(1);
            let [sr, sg, sb, alpha] = pixels[src_y * src_w + src_x];
            if alpha == 0 {
                continue;
            }
            let index = dst_y * width + dst_x;
            let dst = Color(buffer[index]);
            let inverse = 255u32.saturating_sub(alpha as u32);
            let r = (sr as u32 * alpha as u32 + dst.r() as u32 * inverse) / 255;
            let g = (sg as u32 * alpha as u32 + dst.g() as u32 * inverse) / 255;
            let b = (sb as u32 * alpha as u32 + dst.b() as u32 * inverse) / 255;
            buffer[index] = Color::rgb(r as u8, g as u8, b as u8).0;
        }
    }
}


