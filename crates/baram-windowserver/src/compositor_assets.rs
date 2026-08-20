pub const APPS_SVG: &str = include_str!("../../../files/data/ui/apps.svg");

pub struct IconBitmap {
    pub pixels: Vec<[u8; 4]>,
    pub w: usize,
    pub h: usize,
}

pub fn decode_icon(bytes: &[u8], size: usize) -> Option<IconBitmap> {
    let (header, pixels) = png_decoder::decode(bytes).ok()?;
    let src_w = header.width as usize;
    let src_h = header.height as usize;
    let mut buf = alloc::vec![[0u8; 4]; size * size];
    for y in 0..size {
        let sy = y * src_h / size;
        for x in 0..size {
            let sx = x * src_w / size;
            buf[y * size + x] = pixels[sy * src_w + sx];
        }
    }
    Some(IconBitmap {
        pixels: buf,
        w: size,
        h: size,
    })
}

pub struct AppEntry {
    pub name: alloc::string::String,
    pub app_type: alloc::string::String,
    pub title: alloc::string::String,
    pub icon: alloc::string::String,
    pub tags: Vec<alloc::string::String>,
}

pub fn parse_index_yaml(yaml: &str) -> (Vec<alloc::string::String>, Vec<AppEntry>) {
    let mut autostart = Vec::new();
    let mut apps = Vec::new();
    let mut in_autostart = false;
    let mut in_apps = false;
    let mut current_name = alloc::string::String::new();
    let mut current_type = alloc::string::String::from("warp-2");
    let mut current_title = alloc::string::String::new();
    let mut current_icon = alloc::string::String::new();
    let mut current_tags: Vec<alloc::string::String> = Vec::new();
    let mut in_tags = false;
    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if trimmed == "autostart:" {
            in_autostart = true;
            in_apps = false;
            continue;
        }
        if trimmed == "apps:" {
            in_apps = true;
            in_autostart = false;
            if !current_name.is_empty() {
                let title = if current_title.is_empty() {
                    current_name.clone()
                } else {
                    current_title.clone()
                };
                apps.push(AppEntry {
                    name: current_name.clone(),
                    app_type: current_type.clone(),
                    title,
                    icon: current_icon.clone(),
                    tags: current_tags.clone(),
                });
                current_name.clear();
                current_type = alloc::string::String::from("warp-2");
                current_title.clear();
                current_icon.clear();
                current_tags.clear();
            }
            continue;
        }
        if in_autostart {
            if trimmed.starts_with("- ") {
                let name = alloc::string::String::from(trimmed[2..].trim());
                if !name.is_empty() {
                    autostart.push(name);
                }
            } else if !trimmed.starts_with(' ') && !trimmed.starts_with('\t') {
                in_autostart = false;
            }
        }
        if in_apps {
            if !line.starts_with(' ') && !line.starts_with('\t') {
                if !current_name.is_empty() {
                    let title = if current_title.is_empty() {
                        current_name.clone()
                    } else {
                        current_title.clone()
                    };
                    apps.push(AppEntry {
                        name: current_name.clone(),
                        app_type: current_type.clone(),
                        title,
                        icon: current_icon.clone(),
                        tags: current_tags.clone(),
                    });
                    current_name.clear();
                    current_type = alloc::string::String::from("warp-2");
                    current_title.clear();
                    current_icon.clear();
                    current_tags.clear();
                }
                in_apps = false;
                continue;
            }
            if trimmed.ends_with(':')
                && !trimmed.contains("icon")
                && !trimmed.contains("type")
                && !trimmed.contains("title")
                && !trimmed.starts_with("tag")
            {
                if !current_name.is_empty() {
                    let title = if current_title.is_empty() {
                        current_name.clone()
                    } else {
                        current_title.clone()
                    };
                    apps.push(AppEntry {
                        name: current_name.clone(),
                        app_type: current_type.clone(),
                        title,
                        icon: current_icon.clone(),
                        tags: current_tags.clone(),
                    });
                }
                current_name = alloc::string::String::from(trimmed.trim_end_matches(':'));
                current_type = alloc::string::String::from("warp-2");
                current_title.clear();
                current_icon.clear();
                current_tags.clear();
                in_tags = false;
            } else if let Some(v) = trimmed.strip_prefix("type:") {
                current_type = alloc::string::String::from(v.trim().trim_matches('"'));
            } else if let Some(v) = trimmed.strip_prefix("title:") {
                current_title = alloc::string::String::from(v.trim().trim_matches('"'));
            } else if let Some(v) = trimmed.strip_prefix("icon:") {
                let val = v.trim().trim_matches('"');
                if val.is_empty() || val == "null" {
                    current_icon = alloc::string::String::from("noname.png");
                } else {
                    current_icon = alloc::string::String::from(val);
                }
                in_tags = false;
            } else if let Some(v) = trimmed.strip_prefix("tag:") {
                current_tags.clear();
                let val = v.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    current_tags.push(alloc::string::String::from(val));
                }
                in_tags = true;
            } else if in_tags && trimmed.starts_with("- ") {
                let val = trimmed[2..].trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    current_tags.push(alloc::string::String::from(val));
                }
            }
        }
    }
    if in_apps && !current_name.is_empty() {
        let title = if current_title.is_empty() {
            current_name.clone()
        } else {
            current_title
        };
        apps.push(AppEntry {
            name: current_name,
            app_type: current_type,
            title,
            icon: current_icon,
            tags: current_tags,
        });
    }
    (autostart, apps)
}

pub const WALLPAPER_baram_PNG: &[u8] = include_bytes!("../../../files/data/wallpaper/baram.png");
pub const WALLPAPER_HANUL_PNG: &[u8] = include_bytes!("../../../files/data/wallpaper/hanul.png");
pub const WALLPAPER_REFLECT_PNG: &[u8] =
    include_bytes!("../../../files/data/wallpaper/reflect.png");
pub const WALLPAPERS: &[&[u8]] = &[
    WALLPAPER_baram_PNG,
    WALLPAPER_HANUL_PNG,
    WALLPAPER_REFLECT_PNG,
];

pub fn decode_wallpaper(bytes: &[u8], screen_w: usize, screen_h: usize) -> Option<Vec<u32>> {
    let (header, pixels) = png_decoder::decode(bytes).ok()?;
    let img_w = header.width as usize;
    let img_h = header.height as usize;
    let mut buf = alloc::vec![0u32; screen_w * screen_h];
    let scale = if screen_w * img_h > screen_h * img_w {
        screen_w as f64 / img_w as f64
    } else {
        screen_h as f64 / img_h as f64
    };
    let src_w = (screen_w as f64 / scale) as usize;
    let src_h = (screen_h as f64 / scale) as usize;
    let src_x = (img_w.saturating_sub(src_w)) / 2;
    let src_y = (img_h.saturating_sub(src_h)) / 2;
    for y in 0..screen_h {
        let sy = (y * src_h / screen_h).min(src_h - 1) + src_y;
        let src_row = sy * img_w;
        let dst_row = y * screen_w;
        for x in 0..screen_w {
            let sx = (x * src_w / screen_w).min(src_w - 1) + src_x;
            let px = pixels[src_row + sx];
            buf[dst_row + x] = Color::rgb(px[0], px[1], px[2]).0;
        }
    }
    Some(buf)
}

pub fn make_solid_wallpaper(color: u32, screen_w: usize, screen_h: usize) -> Vec<u32> {
    alloc::vec![color; screen_w * screen_h]
}

static mut TB_BTN_CACHE: [Option<(usize, Vec<u32>)>; 4] = [None, None, None, None];

fn get_or_render_tb_btn(size: usize, ca: u32) -> &'static [u32] {
    let slot_idx = match ca {
        255 => 0,
        100 => 1,
        128 => 2,
        _ => 3,
    };
    unsafe {
        if let Some((cached_size, ref pixels)) = TB_BTN_CACHE[slot_idx] {
            if cached_size == size {
                return pixels;
            }
        }
        let mut pixels = alloc::vec![0u32; size * size];
        let r_f = size as f32 / 2.0;
        for py in 0..size {
            for px in 0..size {
                let dx = px as f32 + 0.5 - r_f;
                let dy = py as f32 + 0.5 - r_f;
                let dist_sq = dx * dx + dy * dy;
                let alpha = if dist_sq < (r_f - 1.0) * (r_f - 1.0) {
                    1.0f32
                } else if dist_sq > (r_f + 0.5) * (r_f + 0.5) {
                    0.0
                } else {
                    let dist = libm::sqrtf(dist_sq);
                    (r_f + 0.5 - dist).clamp(0.0, 1.0)
                };
                if alpha <= 0.0 {
                    continue;
                }
                let a = (alpha * ca as f32) as u32;
                pixels[py * size + px] = (a << 24) | 0x00FF_FFFF;
            }
        }
        TB_BTN_CACHE[slot_idx] = Some((size, pixels));
        TB_BTN_CACHE[slot_idx].as_ref().unwrap().1.as_slice()
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}

#[inline]
fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// CSS-style cubic-bezier(.38, .33, .23, 1.0), used by the launcher opening
/// transition. Solve the x component for the curve parameter, then evaluate y.
fn ease_launcher_open(t: f32) -> f32 {
    let x = t.clamp(0.0, 1.0);
    let mut low = 0.0f32;
    let mut high = 1.0f32;
    for _ in 0..14 {
        let u = (low + high) * 0.5;
        let inv = 1.0 - u;
        let curve_x = 3.0 * inv * inv * u * 0.38 + 3.0 * inv * u * u * 0.23 + u * u * u;
        if curve_x < x {
            low = u;
        } else {
            high = u;
        }
    }
    let u = (low + high) * 0.5;
    let inv = 1.0 - u;
    3.0 * inv * inv * u * 0.33 + 3.0 * inv * u * u + u * u * u
}


