use crate::config;

pub fn is_setup_done() -> bool {
    config::get_config()
        .get("system/done")
        .map_or(false, |s| s == "1")
}

pub fn mark_setup_done() {
    config::update_and_save(|settings| settings.set("system/done", "1"));
}

#[derive(Clone, Copy, PartialEq)]
pub enum SetupScreen {
    Welcome,
    Timezone,
    Keyboard,
    KeyboardDetected,
    Done,
}

pub struct SetupWizard {
    pub screen: SetupScreen,
    pub key_detected: bool,
    pub detected_raw_key: u8,
    pub skipped: bool,
    dirty: bool,
}

impl SetupWizard {
    pub fn new() -> Self {
        Self {
            screen: SetupScreen::Welcome,
            key_detected: false,
            detected_raw_key: 0,
            skipped: false,
            dirty: true,
        }
    }

    pub fn warp3_screen(&self) -> &'static str {
        match self.screen {
            SetupScreen::Welcome => "welcome",
            SetupScreen::Timezone => "timezone",
            SetupScreen::Keyboard => "keyboard",
            SetupScreen::KeyboardDetected => "keyboardDetected",
            SetupScreen::Done => "done",
        }
    }

    pub fn take_dirty(&mut self) -> bool {
        core::mem::replace(&mut self.dirty, false)
    }

    pub fn on_command(&mut self, command: &str) {
        match command {
            "setup://continue" if self.screen == SetupScreen::Welcome => {
                self.screen = SetupScreen::Timezone;
                self.dirty = true;
            }
            command if self.screen == SetupScreen::Timezone => {
                if let Some(offset) = command.strip_prefix("setup://timezone?") {
                    if is_valid_timezone_offset(offset) {
                        let saved = config::update_and_save(|cfg| {
                            cfg.set("system/timezone", offset);
                        });
                        if saved {
                            self.screen = SetupScreen::Keyboard;
                            self.dirty = true;
                        }
                    }
                }
            }
            "setup://finish" if self.key_detected => self.finish(false),
            "setup://skip" => self.finish(true),
            _ => {}
        }
    }

    pub fn on_key(&mut self, ev: &baram_core::KeyEvent) {
        let is_esc = ev.raw_key == 0x29 || ev.scancode == 0x17;
        if is_esc && self.screen != SetupScreen::Done {
            self.finish(true);
            return;
        }

        let is_enter = ev.printable == Some(b'\n')
            || ev.raw_key == 0x28
            || ev.raw_key == 0x58
            || ev.scancode == 0x1C;
        let is_tab = ev.raw_key == 0x2B || ev.scancode == 0x2C;
        let is_shift = ev.raw_key == 0xE1 || ev.raw_key == 0xE5;

        match self.screen {
            SetupScreen::Welcome if is_enter => {
                self.screen = SetupScreen::Timezone;
                self.dirty = true;
            }
            SetupScreen::Timezone if is_enter => {
                self.screen = SetupScreen::Keyboard;
                self.dirty = true;
            }
            SetupScreen::Keyboard if ev.raw_key != 0 && !is_enter && !is_tab && !is_shift => {
                self.detected_raw_key = ev.raw_key;
                self.key_detected = true;
                self.screen = SetupScreen::KeyboardDetected;
                self.dirty = true;
            }
            SetupScreen::KeyboardDetected if is_enter => self.finish(false),
            _ => {}
        }
    }

    fn finish(&mut self, skipped: bool) {
        let shift_key = if skipped {
            0
        } else {
            self.detected_raw_key
        };
        let saved = config::update_and_save(|cfg| {
            cfg.set("keyboard/shift_key", &alloc::format!("{shift_key}"));
            cfg.set("system/done", "1");
        });
        if !saved {
            // Keep the wizard active so the user can retry instead of showing
            // a completed setup that was never persisted.
            self.dirty = true;
            return;
        }
        self.skipped = skipped;
        self.screen = SetupScreen::Done;
        self.dirty = true;
    }
}

fn is_valid_timezone_offset(offset: &str) -> bool {
    let bytes = offset.as_bytes();
    if bytes.len() != 6 || !matches!(bytes[0], b'+' | b'-') || bytes[3] != b':' {
        return false;
    }
    let parse_two = |digits: &[u8]| -> Option<i32> {
        if digits.iter().all(u8::is_ascii_digit) {
            Some(((digits[0] - b'0') * 10 + (digits[1] - b'0')) as i32)
        } else {
            None
        }
    };
    let (Some(hours), Some(minutes)) = (parse_two(&bytes[1..3]), parse_two(&bytes[4..6])) else {
        return false;
    };
    (hours <= 14 && minutes < 60) && (hours != 14 || minutes == 0)
}
