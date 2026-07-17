pub use baram_font::layer_ext::LayerFontExt;

pub trait LayerWindowExt {
    fn put_char(&mut self, x: usize, y: usize, ch: char, fg: baram_core::Color);
    fn put_str(&mut self, x: usize, y: usize, s: &str, fg: baram_core::Color);
    fn put_str_hud(&mut self, x: usize, y: usize, s: &str, fg: baram_core::Color);
}
