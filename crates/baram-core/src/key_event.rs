#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub printable: Option<u8>,
    pub scancode: u16,
    pub modifiers: u8,
    pub raw_key: u8,
}

impl KeyEvent {
    pub fn ctrl_or_cmd(&self) -> bool {
        self.modifiers & 0x11 != 0
    }

    pub fn is_special(&self) -> bool {
        self.scancode != 0
    }

    pub fn label(&self) -> &'static str {
        if let Some(c) = self.printable {
            return match c {
                b' ' => "SPC",
                b'\t' => "TAB",
                b'\r' | b'\n' => "ENT",
                0x1B => "ESC",
                0x7F => "DEL",
                _ => "ASCII",
            };
        }
        "SPEC"
    }
}
