use baram_core::{Color, LayerSystem};

/// Shared blink cadence for every editable text surface in BaramOS.
pub const BLINK_PERIOD_NS: u64 = 500_000_000;

pub fn visible(now_ns: u64) -> bool {
    (now_ns / BLINK_PERIOD_NS) % 2 == 0
}

pub fn draw(layer: &mut LayerSystem, x: i32, y: i32, height: usize, color: Color) {
    if x < 0 || y < 0 || height == 0 {
        return;
    }
    layer.fill_rect(x as usize, y as usize, 1, height, color);
}
