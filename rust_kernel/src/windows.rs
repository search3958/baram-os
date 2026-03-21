pub fn should_bake_inactive(
    is_active: i32,
    buffer_h: i32,
    window_h: i32,
    scroll_y: f32,
    target_scroll_y: f32,
) -> i32 {
    if is_active != 0 {
        return 0;
    }
    if buffer_h > window_h {
        return 0;
    }
    if scroll_y != 0.0 || target_scroll_y != 0.0 {
        return 0;
    }
    1
}

pub fn compute_content_src_y(dy: i32, scroll_y: f32, scale: f32, buffer_h: i32) -> i32 {
    let src_y = ((dy as f32 - scroll_y) * scale) as i32;
    if src_y < 0 || src_y >= buffer_h {
        -1
    } else {
        src_y
    }
}
