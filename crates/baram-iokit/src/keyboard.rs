#![allow(dead_code)]

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use baram_bsd::shift_key::load_shift_key;
use baram_core::KeyEvent;

#[cfg(feature = "uefi")]
use uefi::boot;
#[cfg(feature = "uefi")]
use uefi::proto::console::text::{Input, InputEx, Key};
#[cfg(feature = "uefi")]
use uefi::proto::usb::io::{ControlTransfer, UsbIo};
#[cfg(feature = "uefi")]
use uefi::system::with_stdin;
#[cfg(feature = "uefi")]
use uefi_raw::protocol::console::KeyShiftState;
#[cfg(all(feature = "uefi", not(target_arch = "aarch64")))]
use uefi_raw::protocol::console::KeyToggleState;

#[cfg(feature = "esp32s3")]
use esp_hal::gpio::Input;
#[cfg(feature = "esp32s3")]
use esp_hal::usb::UsbBus;

const BOOT_KEYMAP: [u8; 128] = {
    let mut map = [0u8; 128];
    map[0x04] = b'a';
    map[0x05] = b'b';
    map[0x06] = b'c';
    map[0x07] = b'd';
    map[0x08] = b'e';
    map[0x09] = b'f';
    map[0x0A] = b'g';
    map[0x0B] = b'h';
    map[0x0C] = b'i';
    map[0x0D] = b'j';
    map[0x0E] = b'k';
    map[0x0F] = b'l';
    map[0x10] = b'm';
    map[0x11] = b'n';
    map[0x12] = b'o';
    map[0x13] = b'p';
    map[0x14] = b'q';
    map[0x15] = b'r';
    map[0x16] = b's';
    map[0x17] = b't';
    map[0x18] = b'u';
    map[0x19] = b'v';
    map[0x1A] = b'w';
    map[0x1B] = b'x';
    map[0x1C] = b'y';
    map[0x1D] = b'z';
    map[0x1E] = b'1';
    map[0x1F] = b'2';
    map[0x20] = b'3';
    map[0x21] = b'4';
    map[0x22] = b'5';
    map[0x23] = b'6';
    map[0x24] = b'7';
    map[0x25] = b'8';
    map[0x26] = b'9';
    map[0x27] = b'0';
    map[0x28] = b'\n';
    map[0x29] = 0x1b;
    map[0x2A] = 0x08;
    map[0x2B] = b'\t';
    map[0x58] = b'\n';
    map[0x2C] = b' ';
    map[0x2D] = b'-';
    map[0x2E] = b'=';
    map[0x2F] = b'[';
    map[0x30] = b']';
    map[0x31] = b'\\';
    map[0x33] = b';';
    map[0x34] = b'\'';
    map[0x35] = b'`';
    map[0x36] = b',';
    map[0x37] = b'.';
    map[0x38] = b'/';
    map[0x4C] = 0x7f;
    map
};

#[cfg(feature = "uefi")]
pub struct Keyboard {
    usb_io: Option<(boot::ScopedProtocol<UsbIo>, u8, Vec<u8>)>,
    input_ex: Option<boot::ScopedProtocol<InputEx>>,
    prev_keys: [u8; 6],
    prev_modifiers: u8,
    pub cur_modifiers: u8,
    pub cur_keys: [u8; 6],
    pub shift_key: u8,
}

#[cfg(feature = "esp32s3")]
pub struct Keyboard {
    prev_keys: [u8; 6],
    prev_modifiers: u8,
    pub cur_modifiers: u8,
    pub cur_keys: [u8; 6],
    pub shift_key: u8,
    _phantom: core::marker::PhantomData<UsbBus>,
}

#[cfg(feature = "uefi")]
impl Keyboard {
    pub fn is_present() -> bool {
        if uefi::boot::get_handle_for_protocol::<Input>().is_ok() {
            return true;
        }
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
        let _ = usb.control_transfer(
            0x80, 6, 0x0100, 0,
            ControlTransfer::DataIn(&mut dev_buf),
            5000,
        );

        let mut cfg_buf = vec![0u8; 512];
        let _ = usb.control_transfer(
            0x80, 6, 0x0200, 0,
            ControlTransfer::DataIn(&mut cfg_buf),
            5000,
        );

        let mut off = 0;
        let mut keyboard_iface: Option<u8> = None;

        while off + 2 < cfg_buf.len() {
            let b_len = cfg_buf[off] as usize;
            let b_type = cfg_buf[off + 1];
            if b_len < 2 || off + b_len > cfg_buf.len() {
                break;
            }
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
                    keyboard_iface =
                        (class == 3 && subclass == 1 && protocol == 1).then_some(cfg_buf[off + 2]);
                }
                5 => {
                    if b_len >= 7 {
                        let ea = cfg_buf[off + 2];
                        let attrs = cfg_buf[off + 3];
                        if let Some(iface) = keyboard_iface {
                            if ea & 0x80 != 0 && attrs & 0x03 == 3 {
                                let mps = u16::from_le_bytes([cfg_buf[off + 4], cfg_buf[off + 5]]);
                                baram_font::log_line_str(&format!(
                                    "  KBD USB IO: iface={} ep=0x{:02x}", iface, ea,
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
        with_stdin(|input| {
            let _ = input.reset(false);
        });
    }

    pub fn open() -> Self {
        Self::open_with_shift_key(load_shift_key())
    }

    pub fn open_with_shift_key(shift_key: u8) -> Self {
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
                        let _ = usb_obj.control_transfer(
                            0x21, 0x0B, 0, iface_num as u16,
                            ControlTransfer::None, 5000,
                        );
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

    pub fn open_firmware_with_shift_key(shift_key: u8) -> Self {
        #[cfg(not(target_arch = "aarch64"))]
        let input_ex = Self::open_input_ex();
        #[cfg(target_arch = "aarch64")]
        let input_ex = None;
        baram_font::log_line_str(if input_ex.is_some() {
            "KBD: using UEFI extended input protocol"
        } else {
            "KBD: using UEFI basic input protocol"
        });
        Keyboard {
            usb_io: None,
            input_ex,
            prev_keys: [0u8; 6],
            prev_modifiers: 0,
            cur_modifiers: 0,
            cur_keys: [0u8; 6],
            shift_key,
        }
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
            boot::open_protocol::<InputEx>(params, boot::OpenProtocolAttributes::GetProtocol)
                .ok()?
        };
        #[cfg(target_arch = "aarch64")]
        let input = unsafe {
            boot::open_protocol::<InputEx>(params, boot::OpenProtocolAttributes::GetProtocol)
                .ok()?
        };
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
            uefi::boot::open_protocol::<Input>(params, uefi::boot::OpenProtocolAttributes::GetProtocol)
                .ok()?
        };
        input.wait_for_key_event().ok()
    }

    pub fn poll(&mut self) -> Option<KeyEvent> {
        if let Some((usb, ep, report_buf)) = &mut self.usb_io {
            if let Ok(n) = usb.sync_interrupt_receive(*ep, report_buf, 10) {
                if n >= 1 {
                    let r = &report_buf[..n];
                    self.cur_modifiers = r[0];
                    if n >= 8 {
                        let keys = [r[2], r[3], r[4], r[5], r[6], r[7]];
                        self.cur_keys = keys;
                        let prev_mod = self.prev_modifiers;
                        self.prev_modifiers = self.cur_modifiers;
                        for &key in &keys {
                            if key == 0 { continue; }
                            if !self.prev_keys.contains(&key) {
                                self.prev_keys = keys;
                                let mut ascii = if (key as usize) < BOOT_KEYMAP.len() {
                                    BOOT_KEYMAP[key as usize]
                                } else { 0 };
                                if self.cur_modifiers & 0x22 != 0 {
                                    ascii = shifted_ascii(ascii);
                                }
                                let printable = if ascii != 0 { Some(ascii) } else { None };
                                return Some(KeyEvent {
                                    printable, scancode: 0,
                                    modifiers: self.cur_modifiers, raw_key: key,
                                });
                            }
                        }
                        let newly_pressed = self.cur_modifiers & !prev_mod;
                        if newly_pressed != 0 {
                            let bit = newly_pressed.trailing_zeros() as u8;
                            return Some(KeyEvent {
                                printable: None, scancode: 0,
                                modifiers: self.cur_modifiers, raw_key: 0x80 | bit,
                            });
                        }
                        self.prev_keys = keys;
                    } else {
                        self.cur_keys = [0u8; 6];
                    }
                    return None;
                }
            }
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
                                if newly_pressed == 0 { None } else {
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
        with_stdin(|input| match input.read_key() {
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
        })
    }
}

#[cfg(feature = "esp32s3")]
impl Keyboard {
    pub fn is_present() -> bool { true }
    pub fn reset() {}
    pub fn open() -> Self { Self::open_with_shift_key(load_shift_key()) }
    pub fn open_with_shift_key(shift_key: u8) -> Self {
        baram_font::log_line_str("KBD: using ESP32-S3 GPIO input");
        Keyboard {
            prev_keys: [0u8; 6],
            prev_modifiers: 0,
            cur_modifiers: 0,
            cur_keys: [0u8; 6],
            shift_key,
            _phantom: core::marker::PhantomData,
        }
    }
    pub fn open_firmware_with_shift_key(shift_key: u8) -> Self {
        Self::open_with_shift_key(shift_key)
    }
    pub fn poll(&mut self) -> Option<KeyEvent> { None }
    pub fn stdin_event() -> Option<()> { None }
}

pub fn is_present() -> bool { Keyboard::is_present() }
pub fn reset() { Keyboard::reset(); }
pub fn open() -> Keyboard { Keyboard::open() }

#[cfg(feature = "uefi")]
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

#[cfg(feature = "uefi")]
fn uefi_printable_event(value: u16, modifiers: u8) -> KeyEvent {
    let printable = match value {
        0x0d => Some(b'\n'), 0x08 => Some(0x08),
        1..=0x7f => Some(value as u8), _ => None,
    };
    let raw_key = match value {
        0x0d | 0x0a => 0x28, 0x08 => 0x2a,
        _ if value < 256 => value as u8, _ => 0,
    };
    KeyEvent { printable, scancode: 0, modifiers, raw_key }
}

fn shifted_ascii(value: u8) -> u8 {
    match value {
        b'a'..=b'z' => value - b'a' + b'A',
        b'1' => b'!', b'2' => b'@', b'3' => b'#',
        b'4' => b'$', b'5' => b'%', b'6' => b'^',
        b'7' => b'&', b'8' => b'*', b'9' => b'(',
        b'0' => b')', b'-' => b'_', b'=' => b'+',
        b'[' => b'{', b']' => b'}', b'\\' => b'|',
        b';' => b':', b'\'' => b'"', b'`' => b'~',
        b',' => b'<', b'.' => b'>', b'/' => b'?',
        _ => value,
    }
}
