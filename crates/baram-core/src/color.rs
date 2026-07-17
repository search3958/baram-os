#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color(0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }
    pub const fn r(self) -> u8 { ((self.0 >> 16) & 0xFF) as u8 }
    pub const fn g(self) -> u8 { ((self.0 >>  8) & 0xFF) as u8 }
    pub const fn b(self) -> u8 { ((self.0 >>  0) & 0xFF) as u8 }

    pub const BLACK:   Color = Color::rgb(0x00, 0x00, 0x00);
    pub const BG:      Color = Color::rgb(0xF0, 0xF0, 0xF0);
    pub const PANEL:   Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const ACCENT:  Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const TEXT:    Color = Color::rgb(0x1A, 0x1A, 0x1A);
    pub const MUTED:   Color = Color::rgb(0x66, 0x66, 0x66);
    pub const TITLE_INACTIVE: Color = Color::rgb(0x88, 0x88, 0x88);
    pub const GOOD:    Color = Color::rgb(0x10, 0x7C, 0x10);
    #[allow(dead_code)]
    pub const WARN:    Color = Color::rgb(0xD8, 0x3B, 0x01);
    pub const CURSOR:  Color = Color::rgb(0x1A, 0x1A, 0x1A);
    pub const BORDER:  Color = Color::rgb(0xD0, 0xD0, 0xD0);
    pub const TASKBAR: Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const WIN_BG:  Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const WIN_INACTIVE: Color = Color::rgb(0xE8, 0xE8, 0xE8);
    pub const CARD_BG: Color = Color::rgb(0xF5, 0xF5, 0xF5);
    pub const SHADOW:  Color = Color::rgb(0xA0, 0xA0, 0xA0);
    pub const TRANSPARENT: Color = Color(0x0000_0000);
}
