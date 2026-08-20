pub struct WindowManager {
    windows: Vec<Window>,
    next_z: i32,
    next_id: u32,
    pub focused_id: Option<WinId>,
    screen_w: i32,
    screen_h: i32,
    shadow_cache: Vec<(WinId, Option<CachedShadow>)>,
    temp_layer: Option<LayerSystem>,
    order_changed: bool,
    pending_damage: Option<(usize, usize, usize, usize)>,
    interaction_blocked: Option<WinId>,
    file_dialog: Option<NativeFileDialog>,
}


