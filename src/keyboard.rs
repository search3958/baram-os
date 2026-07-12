use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use uefi::boot;
use uefi::proto::usb::io::{ControlTransfer, UsbIo};
use uefi::proto::console::text::{Input, Key, ScanCode};
use uefi::system::with_stdin;

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
}

impl KeyEvent {
    pub fn is_special(&self) -> bool { self.scancode != 0 }

    pub fn label(&self) -> &'static str {
        if let Some(c) = self.printable {
            return match c {
                b' '  => "SPC",
                b'\t' => "TAB",
                b'\r' | b'\n' => "ENT",
                0x1B  => "ESC",
                0x7F  => "DEL",
                _     => "ASCII",
            };
        }
        let sc = ScanCode(self.scancode);
        if sc == ScanCode::UP         { return "UP"; }
        if sc == ScanCode::DOWN       { return "DOWN"; }
        if sc == ScanCode::LEFT       { return "LEFT"; }
        if sc == ScanCode::RIGHT      { return "RIGHT"; }
        if sc == ScanCode::ESCAPE     { return "ESC"; }
        if sc == ScanCode::DELETE     { return "DEL"; }
        if sc == ScanCode::HOME       { return "HOME"; }
        if sc == ScanCode::END        { return "END"; }
        if sc == ScanCode::INSERT     { return "INS"; }
        if sc == ScanCode::PAGE_UP    { return "PGUP"; }
        if sc == ScanCode::PAGE_DOWN  { return "PGDN"; }
        if sc == ScanCode::FUNCTION_1 { return "F1"; }
        if sc == ScanCode::FUNCTION_2 { return "F2"; }
        if sc == ScanCode::FUNCTION_3 { return "F3"; }
        if sc == ScanCode::FUNCTION_4 { return "F4"; }
        if sc == ScanCode::FUNCTION_5 { return "F5"; }
        if sc == ScanCode::FUNCTION_6 { return "F6"; }
        if sc == ScanCode::FUNCTION_7 { return "F7"; }
        if sc == ScanCode::FUNCTION_8 { return "F8"; }
        if sc == ScanCode::FUNCTION_9 { return "F9"; }
        if sc == ScanCode::FUNCTION_10 { return "F10"; }
        if sc == ScanCode::FUNCTION_11 { return "F11"; }
        if sc == ScanCode::FUNCTION_12 { return "F12"; }
        "???"
    }
}

// USB HID boot keyboard report: modifier(1) + reserved(1) + keys(6)
const BOOT_KEYMAP: [u8; 128] = {
    let mut map = [0u8; 128];
    // USB HID usage codes to ASCII
    map[0x04] = b'a'; map[0x05] = b'b'; map[0x06] = b'c'; map[0x07] = b'd';
    map[0x08] = b'e'; map[0x09] = b'f'; map[0x0A] = b'g'; map[0x0B] = b'h';
    map[0x0C] = b'i'; map[0x0D] = b'j'; map[0x0E] = b'k'; map[0x0F] = b'l';
    map[0x10] = b'm'; map[0x11] = b'n'; map[0x12] = b'o'; map[0x13] = b'p';
    map[0x14] = b'q'; map[0x15] = b'r'; map[0x16] = b's'; map[0x17] = b't';
    map[0x18] = b'u'; map[0x19] = b'v'; map[0x1A] = b'w'; map[0x1B] = b'x';
    map[0x1C] = b'y'; map[0x1D] = b'z';
    map[0x1E] = b'1'; map[0x1F] = b'2'; map[0x20] = b'3'; map[0x21] = b'4';
    map[0x22] = b'5'; map[0x23] = b'6'; map[0x24] = b'7'; map[0x25] = b'8';
    map[0x26] = b'9'; map[0x27] = b'0';
    map[0x28] = b'\n'; map[0x58] = b'\n'; map[0x2C] = b' '; map[0x2D] = b'-'; map[0x2E] = b'=';
    map[0x2F] = b'['; map[0x30] = b']'; map[0x33] = b';'; map[0x34] = b'\'';
    map[0x36] = b','; map[0x37] = b'.'; map[0x38] = b'/';
    map
};

pub struct Keyboard {
    usb_io: Option<(boot::ScopedProtocol<UsbIo>, u8, Vec<u8>)>,
    report_buf: Vec<u8>,
    prev_keys: [u8; 6],
    prev_modifiers: u8,
    pub cur_modifiers: u8,
    pub cur_keys: [u8; 6],
    pub shift_key: u8,
}

const SHIFT_KEY_PATH: &str = "apps/.shift_key";

pub fn load_shift_key() -> u8 {
    let data = crate::vfs::read_file(SHIFT_KEY_PATH);
    if data.len() >= 1 { data[0] } else { 0 }
}

pub fn save_shift_key(code: u8) {
    crate::vfs::write_file(SHIFT_KEY_PATH, &[code]);
}

impl Keyboard {
    pub fn is_present() -> bool {
        // Check UEFI protocol
        if uefi::boot::get_handle_for_protocol::<Input>().is_ok() {
            return true;
        }
        // Check USB IO for keyboard HID
        if let Ok(handles) = boot::find_handles::<UsbIo>() {
            for handle in handles {
                if Self::probe_usb_kbd(handle).is_some() {
                    return true;
                }
            }
        }
        false
    }

    fn probe_usb_kbd(handle: uefi::Handle) -> Option<(u8, u16)> {
        let params = boot::OpenProtocolParams {
            handle,
            agent: boot::image_handle(),
            controller: None,
        };
        let mut usb = unsafe {
            boot::open_protocol::<UsbIo>(params, boot::OpenProtocolAttributes::GetProtocol).ok()?
        };

        let mut dev_buf = vec![0u8; 18];
        let _ = usb.control_transfer(0x80, 6, 0x0100, 0, ControlTransfer::DataIn(&mut dev_buf), 5000);

        let mut cfg_buf = vec![0u8; 512];
        let _ = usb.control_transfer(0x80, 6, 0x0200, 0, ControlTransfer::DataIn(&mut cfg_buf), 5000);

        let mut off = 0;
        let mut current_iface: u8 = 0;
        let mut intr_ep: Option<u8> = None;
        let mut intr_mps: u16 = 0;
        let mut is_keyboard = false;

        while off + 2 < cfg_buf.len() {
            let b_len = cfg_buf[off] as usize;
            let b_type = cfg_buf[off + 1];
            if b_len < 2 || off + b_len > cfg_buf.len() { break; }

            match b_type {
                4 => {
                    current_iface = cfg_buf[off + 2];
                    let class = cfg_buf[off + 5];
                    let subclass = cfg_buf[off + 6];
                    let protocol = cfg_buf[off + 7];
                    intr_ep = None;
                    is_keyboard = false;
                    if class == 3 && subclass == 1 && protocol == 1 {
                        is_keyboard = true;
                    }
                }
                5 => {
                    if b_len >= 7 && intr_ep.is_none() {
                        let ea = cfg_buf[off + 2];
                        let attrs = cfg_buf[off + 3];
                        if ea & 0x80 != 0 && attrs & 0x03 == 3 {
                            intr_ep = Some(ea);
                            intr_mps = u16::from_le_bytes([cfg_buf[off + 4], cfg_buf[off + 5]]);
                        }
                    }
                }
                _ => {}
            }
            off += b_len;
        }

        if is_keyboard {
            if let Some(ep) = intr_ep {
                crate::mouse::log_line_str(&format!("  KBD USB IO: iface={} ep=0x{:02x}", current_iface, ep));
                return Some((ep, intr_mps));
            }
        }
        None
    }

    pub fn reset() {
        with_stdin(|input| { let _ = input.reset(false); });
    }

    pub fn open() -> Self {
        // Try direct USB IO for keyboard
        if let Ok(handles) = boot::find_handles::<UsbIo>() {
            for handle in handles {
                if let Some((ep, mps)) = Self::probe_usb_kbd(handle) {
                    let params = boot::OpenProtocolParams {
                        handle,
                        agent: boot::image_handle(),
                        controller: None,
                    };
                    if let Ok(usb) = unsafe {
                        boot::open_protocol::<UsbIo>(params, boot::OpenProtocolAttributes::GetProtocol)
                    } {
                        let mut usb_obj = usb;
                        // Find interface number from the probe
                        let mut cfg_buf = vec![0u8; 512];
                        let _ = usb_obj.control_transfer(0x80, 6, 0x0200, 0, ControlTransfer::DataIn(&mut cfg_buf), 5000);
                        let mut off = 0;
                        let mut iface_num: u8 = 0;
                        while off + 2 < cfg_buf.len() {
                            let b_len = cfg_buf[off] as usize;
                            let b_type = cfg_buf[off + 1];
                            if b_len < 2 || off + b_len > cfg_buf.len() { break; }
                            if b_type == 4 {
                                let class = cfg_buf[off + 5];
                                let subclass = cfg_buf[off + 6];
                                let protocol = cfg_buf[off + 7];
                                if class == 3 && subclass == 1 && protocol == 1 {
                                    iface_num = cfg_buf[off + 2];
                                }
                            }
                            off += b_len;
                        }

                        let _ = usb_obj.control_transfer(0x21, 0x0B, 0, iface_num as u16, ControlTransfer::None, 5000);
                        let _ = usb_obj.control_transfer(0x21, 0x0B, 1, iface_num as u16, ControlTransfer::None, 5000);

                        let report_buf = vec![0u8; mps as usize];
                        crate::mouse::log_line_str("KBD: using USB IO (direct HID boot protocol)");
                        return Keyboard {
                            usb_io: Some((usb_obj, ep, report_buf)),
                            report_buf: vec![0u8; 8],
                            prev_keys: [0u8; 6],
                            prev_modifiers: 0,
                            cur_modifiers: 0,
                            cur_keys: [0u8; 6],
                            shift_key: load_shift_key(),
                        };
                    }
                }
            }
        }
        crate::mouse::log_line_str("KBD: using UEFI protocol");
        Keyboard { usb_io: None, report_buf: vec![0u8; 8], prev_keys: [0u8; 6], prev_modifiers: 0, cur_modifiers: 0, cur_keys: [0u8; 6], shift_key: load_shift_key() }
    }

    pub fn stdin_event() -> Option<uefi::Event> {
        use uefi::proto::console::text::Input;
        let handle = uefi::boot::get_handle_for_protocol::<Input>().ok()?;
        let params = uefi::boot::OpenProtocolParams {
            handle,
            agent: uefi::boot::image_handle(),
            controller: None,
        };
        let input = unsafe {
            uefi::boot::open_protocol::<Input>(params, uefi::boot::OpenProtocolAttributes::GetProtocol).ok()?
        };
        input.wait_for_key_event().ok()
    }

    pub fn poll(&mut self) -> Option<KeyEvent> {
        // Try USB IO first
        if let Some((usb, ep, report_buf)) = &mut self.usb_io {
            if let Ok(n) = usb.sync_interrupt_receive(*ep, report_buf, 10) {
                if n >= 1 {
                    let r = &report_buf[..n];
                    // Boot keyboard report: modifier(1) reserved(1) keys(6)
                    self.cur_modifiers = r[0];
                    if n >= 8 {
                        let keys = [r[2], r[3], r[4], r[5], r[6], r[7]];
                        self.cur_keys = keys;

                        // Find newly pressed keys
                        for &key in &keys {
                            if key == 0 { continue; }
                            if !self.prev_keys.contains(&key) {
                                self.prev_keys = keys;

                                let ascii = if (key as usize) < BOOT_KEYMAP.len() {
                                    BOOT_KEYMAP[key as usize]
                                } else {
                                    0
                                };
                                let printable = if ascii != 0 { Some(ascii) } else { None };

                                return Some(KeyEvent { printable, scancode: 0, modifiers: self.cur_modifiers, raw_key: key });
                            }
                        }

                        // Detect modifier-only press (no key slot change)
                        let prev_mod = self.prev_modifiers;
                        self.prev_modifiers = self.cur_modifiers;
                        let newly_pressed = self.cur_modifiers & !prev_mod;
                        if newly_pressed != 0 {
                            // Return a synthetic event with raw_key = 0x100 + modifier bit index
                            let bit = newly_pressed.trailing_zeros() as u8;
                            return Some(KeyEvent { printable: None, scancode: 0, modifiers: self.cur_modifiers, raw_key: 0x80 | bit });
                        }

                        self.prev_keys = keys;
                    } else {
                        self.cur_keys = [0u8; 6];
                    }
                    return None;
                }
            }
        }

        // Fallback: UEFI protocol
        with_stdin(|input| {
            match input.read_key() {
                Ok(Some(Key::Printable(ch))) => {
                    let v: u16 = ch.into();
                    let printable = if v < 0x80 { Some(v as u8) } else { None };
                    let raw = if v > 0 && v < 256 { v as u8 } else { 0 };
                    Some(KeyEvent { printable, scancode: 0, modifiers: 0, raw_key: raw })
                }
                Ok(Some(Key::Special(sc))) => {
                    let raw = if sc.0 > 0 && sc.0 < 256 { sc.0 as u8 } else { 0 };
                    Some(KeyEvent { printable: None, scancode: sc.0, modifiers: 0, raw_key: raw })
                }
                Ok(None) => None,
                Err(e) => {
                    crate::mouse::log_line_str(&format!("KBD: read_key error: {:?}", e));
                    None
                }
            }
        })
    }

    pub fn is_held(&self, usb_code: u8) -> bool {
        self.cur_keys.contains(&usb_code)
    }

    pub fn ctrl_or_cmd_held(&self) -> bool {
        self.cur_modifiers & 0x11 != 0
    }

    pub fn shift_held(&self) -> bool {
        if self.cur_modifiers & 0x22 != 0 {
            return true;
        }
        if self.shift_key != 0 {
            if self.shift_key & 0x80 != 0 {
                // Modifier key: check the corresponding bit in cur_modifiers
                let bit = self.shift_key & 0x7F;
                let mask = 1u8 << bit;
                if self.cur_modifiers & mask != 0 {
                    return true;
                }
            } else {
                // Regular key: check cur_keys
                if self.cur_keys.contains(&self.shift_key) {
                    return true;
                }
            }
        }
        false
    }
}
