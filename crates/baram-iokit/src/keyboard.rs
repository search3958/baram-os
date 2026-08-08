use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use uefi::boot;
use uefi::proto::usb::io::{ControlTransfer, UsbIo};
use uefi::proto::console::text::{Input, InputEx, Key};
use uefi_raw::protocol::console::KeyShiftState;
#[cfg(not(target_arch = "aarch64"))]
use uefi_raw::protocol::console::KeyToggleState;
use uefi::system::with_stdin;
use baram_bsd::shift_key::load_shift_key;
use baram_core::KeyEvent;

// USB HID boot keyboard report: modifier(1) + reserved(1) + keys(6)
const BOOT_KEYMAP: [u8; 128] = {
    let mut map = [0u8; 128];
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
    map[0x28] = b'\n'; map[0x29] = 0x1b; map[0x2A] = 0x08; map[0x2B] = b'\t';
    map[0x58] = b'\n'; map[0x2C] = b' '; map[0x2D] = b'-'; map[0x2E] = b'=';
    map[0x2F] = b'['; map[0x30] = b']'; map[0x31] = b'\\'; map[0x33] = b';'; map[0x34] = b'\'';
    map[0x35] = b'`';
    map[0x36] = b','; map[0x37] = b'.'; map[0x38] = b'/';
    map[0x4C] = 0x7f;
    map
};

pub struct Keyboard {
    usb_io: Option<(boot::ScopedProtocol<UsbIo>, u8, Vec<u8>)>,
    input_ex: Option<boot::ScopedProtocol<InputEx>>,
    prev_keys: [u8; 6],
    prev_modifiers: u8,
    pub cur_modifiers: u8,
    pub cur_keys: [u8; 6],
    pub shift_key: u8,
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

    fn probe_usb_kbd(handle: uefi::Handle) -> Option<(u8, u8, u16)> {
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
        let mut keyboard_iface: Option<u8> = None;

        while off + 2 < cfg_buf.len() {
            let b_len = cfg_buf[off] as usize;
            let b_type = cfg_buf[off + 1];
            if b_len < 2 || off + b_len > cfg_buf.len() { break; }

            match b_type {
                4 => {
                    if b_len < 9 {
                        keyboard_iface = None;
                        off += b_len;
                        continue;
                    }
                    let class = cfg_buf[off + 5];
                    let subclass = cfg_buf[off + 6];
                    let protocol = cfg_buf[off + 7];
                    keyboard_iface = (class == 3 && subclass == 1 && protocol == 1)
                        .then_some(cfg_buf[off + 2]);
                }
                5 => {
                    if b_len >= 7 {
                        let ea = cfg_buf[off + 2];
                        let attrs = cfg_buf[off + 3];
                        if let Some(iface) = keyboard_iface {
                            if ea & 0x80 != 0 && attrs & 0x03 == 3 {
                                let mps = u16::from_le_bytes([
                                    cfg_buf[off + 4],
                                    cfg_buf[off + 5],
                                ]);
                                baram_font::log_line_str(&format!(
                                    "  KBD USB IO: iface={} ep=0x{:02x}",
                                    iface,
                                    ea,
                                ));
                                return Some((iface, ea, mps));
                            }
                        }
                    }
                }
                _ => {}
            }
            off += b_len;
        }

        None
    }

    pub fn reset() {
        with_stdin(|input| { let _ = input.reset(false); });
    }

    pub fn open() -> Self {
        Self::open_with_shift_key(load_shift_key())
    }

    /// Open the keyboard with an explicit modifier mapping, without reading
    /// the BaramOS configuration. Nano System uses this before kernel config.
    pub fn open_with_shift_key(shift_key: u8) -> Self {
        // Try direct USB IO for keyboard
        if let Ok(handles) = boot::find_handles::<UsbIo>() {
            for handle in handles {
                if let Some((iface_num, ep, mps)) = Self::probe_usb_kbd(handle) {
                    let params = boot::OpenProtocolParams {
                        handle,
                        agent: boot::image_handle(),
                        controller: None,
                    };
                    if let Ok(usb) = unsafe {
                        boot::open_protocol::<UsbIo>(params, boot::OpenProtocolAttributes::GetProtocol)
                    } {
                        let mut usb_obj = usb;
                        // HID SetProtocol(0) selects the fixed 8-byte boot
                        // report. Do not immediately switch back to report
                        // protocol: many physical keyboards use a different
                        // report layout there.
                        let _ = usb_obj.control_transfer(0x21, 0x0B, 0, iface_num as u16, ControlTransfer::None, 5000);

                        let report_buf = vec![0u8; (mps as usize).max(8)];
                        baram_font::log_line_str("KBD: using USB IO (direct HID boot protocol)");
                        return Keyboard {
                            usb_io: Some((usb_obj, ep, report_buf)),
                            input_ex: None,
                            prev_keys: [0u8; 6],
                            prev_modifiers: 0,
                            cur_modifiers: 0,
                            cur_keys: [0u8; 6],
                            shift_key,
                        };
                    }
                }
            }
        }
        Self::open_firmware_with_shift_key(shift_key)
    }

    /// Open only the keyboard protocols already initialized by UEFI.
    /// This avoids re-probing USB controllers during early platform startup.
    pub fn open_firmware_with_shift_key(shift_key: u8) -> Self {
        #[cfg(not(target_arch = "aarch64"))]
        let input_ex = Self::open_input_ex();
        // AAVMF may block while opening InputEx after pointer protocols have
        // been claimed. `poll` uses the firmware's basic stdin when this is
        // None, which is sufficient for Nano System's keyboard contract.
        #[cfg(target_arch = "aarch64")]
        let input_ex = None;
        baram_font::log_line_str(if input_ex.is_some() {
            "KBD: using UEFI extended input protocol"
        } else {
            "KBD: using UEFI basic input protocol"
        });
        Keyboard { usb_io: None, input_ex, prev_keys: [0u8; 6], prev_modifiers: 0, cur_modifiers: 0, cur_keys: [0u8; 6], shift_key }
    }

    fn open_input_ex() -> Option<boot::ScopedProtocol<InputEx>> {
        let handle = boot::get_handle_for_protocol::<InputEx>().ok()?;
        let params = boot::OpenProtocolParams {
            handle,
            agent: boot::image_handle(),
            controller: None,
        };
        #[cfg(not(target_arch = "aarch64"))]
        let mut input = unsafe {
            boot::open_protocol::<InputEx>(params, boot::OpenProtocolAttributes::GetProtocol).ok()?
        };
        #[cfg(target_arch = "aarch64")]
        let input = unsafe {
            boot::open_protocol::<InputEx>(params, boot::OpenProtocolAttributes::GetProtocol).ok()?
        };
        // Raspberry Pi's UEFI exposes InputEx but can hang indefinitely in
        // SetState(EXPOSED). Direct USB HID still supplies modifier-only
        // reports there, while ordinary InputEx key reads remain available as
        // a fallback.
        #[cfg(not(target_arch = "aarch64"))]
        let _ = input.set_state(KeyToggleState::VALID | KeyToggleState::EXPOSED);
        Some(input)
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
                        let prev_mod = self.prev_modifiers;
                        self.prev_modifiers = self.cur_modifiers;

                        // Find newly pressed keys
                        for &key in &keys {
                            if key == 0 { continue; }
                            if !self.prev_keys.contains(&key) {
                                self.prev_keys = keys;

                                let mut ascii = if (key as usize) < BOOT_KEYMAP.len() {
                                    BOOT_KEYMAP[key as usize]
                                } else {
                                    0
                                };
                                if self.cur_modifiers & 0x22 != 0 {
                                    ascii = shifted_ascii(ascii);
                                }
                                let printable = if ascii != 0 { Some(ascii) } else { None };

                                return Some(KeyEvent { printable, scancode: 0, modifiers: self.cur_modifiers, raw_key: key });
                            }
                        }

                        // Detect modifier-only press (no key slot change)
                        let newly_pressed = self.cur_modifiers & !prev_mod;
                        if newly_pressed != 0 {
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
            // USB returned 0 bytes or error — fall through to UEFI
        }

        if let Some(input) = self.input_ex.as_mut() {
            return match input.read_key() {
                Ok(Some(data)) => {
                    self.cur_modifiers = uefi_modifiers(data.key_state.key_shift_state);
                    let newly_pressed = self.cur_modifiers & !self.prev_modifiers;
                    self.prev_modifiers = self.cur_modifiers;
                    match data.key {
                        Key::Printable(ch) => {
                            let value: u16 = ch.into();
                            if value == 0 {
                                if newly_pressed == 0 {
                                    None
                                } else {
                                    let bit = newly_pressed.trailing_zeros() as u8;
                                    Some(KeyEvent { printable: None, scancode: 0, modifiers: self.cur_modifiers, raw_key: 0x80 | bit })
                                }
                            } else {
                                Some(uefi_printable_event(value, self.cur_modifiers))
                            }
                        }
                        Key::Special(sc) => {
                            let printable = (sc.0 == 0x08).then_some(0x7f);
                            let raw = if sc.0 < 256 { sc.0 as u8 } else { 0 };
                            Some(KeyEvent { printable, scancode: sc.0, modifiers: self.cur_modifiers, raw_key: raw })
                        }
                    }
                }
                Ok(None) => None,
                Err(error) => {
                    baram_font::log_line_str(&format!("KBD: extended read error: {:?}", error));
                    None
                }
            };
        }

        // Fallback: UEFI protocol
        with_stdin(|input| {
            match input.read_key() {
                Ok(Some(Key::Printable(ch))) => {
                    let v: u16 = ch.into();
                    Some(uefi_printable_event(v, 0))
                }
                Ok(Some(Key::Special(sc))) => {
                    let raw = if sc.0 > 0 && sc.0 < 256 { sc.0 as u8 } else { 0 };
                    let printable = (sc.0 == 0x08).then_some(0x7f);
                    Some(KeyEvent { printable, scancode: sc.0, modifiers: 0, raw_key: raw })
                }
                Ok(None) => None,
                Err(e) => {
                    baram_font::log_line_str(&format!("KBD: read_key error: {:?}", e));
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

fn uefi_modifiers(state: Option<KeyShiftState>) -> u8 {
    let Some(state) = state else { return 0; };
    let mut modifiers = 0u8;
    if state.contains(KeyShiftState::LEFT_CONTROL) { modifiers |= 0x01; }
    if state.contains(KeyShiftState::LEFT_SHIFT) { modifiers |= 0x02; }
    if state.contains(KeyShiftState::LEFT_ALT) { modifiers |= 0x04; }
    if state.contains(KeyShiftState::LEFT_LOGO) { modifiers |= 0x08; }
    if state.contains(KeyShiftState::RIGHT_CONTROL) { modifiers |= 0x10; }
    if state.contains(KeyShiftState::RIGHT_SHIFT) { modifiers |= 0x20; }
    if state.contains(KeyShiftState::RIGHT_ALT) { modifiers |= 0x40; }
    if state.contains(KeyShiftState::RIGHT_LOGO) { modifiers |= 0x80; }
    modifiers
}

fn uefi_printable_event(value: u16, modifiers: u8) -> KeyEvent {
    let printable = match value {
        0x0d => Some(b'\n'),
        0x08 => Some(0x08),
        1..=0x7f => Some(value as u8),
        _ => None,
    };
    let raw_key = match value {
        0x0d | 0x0a => 0x28,
        0x08 => 0x2a,
        _ if value < 256 => value as u8,
        _ => 0,
    };
    KeyEvent { printable, scancode: 0, modifiers, raw_key }
}

fn shifted_ascii(value: u8) -> u8 {
    match value {
        b'a'..=b'z' => value - b'a' + b'A',
        b'1' => b'!', b'2' => b'@', b'3' => b'#', b'4' => b'$', b'5' => b'%',
        b'6' => b'^', b'7' => b'&', b'8' => b'*', b'9' => b'(', b'0' => b')',
        b'-' => b'_', b'=' => b'+', b'[' => b'{', b']' => b'}', b'\\' => b'|',
        b';' => b':', b'\'' => b'"', b'`' => b'~', b',' => b'<', b'.' => b'>',
        b'/' => b'?',
        _ => value,
    }
}
