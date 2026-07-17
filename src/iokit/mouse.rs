


use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use uefi::boot;
use uefi::proto::usb::io::{ControlTransfer, UsbIo};
use uefi::proto::console::pointer::Pointer;
use crate::iokit::absolute_pointer::AbsolutePointer;

#[derive(Clone, Copy, Debug, Default)]
pub struct MouseEvent {
    pub abs_x: u64,
    pub abs_y: u64,
    pub rel_dx: i32,
    pub rel_dy: i32,
    pub is_absolute: bool,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
    pub scroll: i32,
}

pub struct Mouse {
    usb_io: Option<boot::ScopedProtocol<UsbIo>>,
    ep_addr: u8,
    iface: u8,
    report_buf: Vec<u8>,

    abs: Option<(boot::ScopedProtocol<AbsolutePointer>, u64, u64)>,
    simple: Option<boot::ScopedProtocol<Pointer>>,
}

impl Mouse {
    pub fn get_wait_event() -> Option<uefi::Event> {
        // Try AbsolutePointer
        if let Some(h) = boot::find_handles::<AbsolutePointer>().ok().and_then(|h| h.into_iter().next()) {
            let params = boot::OpenProtocolParams {
                handle: h,
                agent: boot::image_handle(),
                controller: None,
            };
            if let Ok(ptr) = unsafe {
                boot::open_protocol::<AbsolutePointer>(params, boot::OpenProtocolAttributes::GetProtocol)
            } {
                let evt = ptr.wait_for_input_event();
                // Event wraps a raw pointer, we can copy the raw pointer
                let raw = unsafe { core::mem::transmute_copy::<uefi::Event, usize>(&evt) };
                core::mem::forget(ptr); // prevent drop so the event stays valid
                return Some(unsafe { core::mem::transmute_copy::<usize, uefi::Event>(&raw) });
            }
        }
        None
    }
    pub fn open() -> Result<Mouse, &'static str> {
        log_line_str("Mouse: searching for devices...");

        if let Ok(handles) = boot::find_handles::<UsbIo>() {
            log_line_str(&format!("  UsbIo handles: {}", handles.len()));
        } else {
            log_line_str("  UsbIo handles: none");
        }
        if let Ok(handles) = boot::find_handles::<AbsolutePointer>() {
            log_line_str(&format!("  AbsolutePointer handles: {}", handles.len()));
        } else {
            log_line_str("  AbsolutePointer handles: none");
        }
        if let Ok(handles) = boot::find_handles::<Pointer>() {
            log_line_str(&format!("  Pointer handles: {}", handles.len()));
        } else {
            log_line_str("  Pointer handles: none");
        }

        log_line_str("Mouse: trying USB IO first...");
        if let Some(m) = Self::try_usb_io() {
            log_line_str("Mouse: USB IO initialized successfully!");
            return Ok(m);
        }

        // Fallback: Try UEFI protocols
        log_line_str("Mouse: trying UEFI protocols...");
        let mut abs = None;
        let mut simple = None;

        if let Some(h) = boot::find_handles::<AbsolutePointer>().ok().and_then(|h| h.into_iter().next()) {
            log_line_str("  Trying AbsolutePointer...");
            let params = boot::OpenProtocolParams {
                handle: h,
                agent: boot::image_handle(),
                controller: None,
            };
            if let Ok(mut ptr) = unsafe {
                boot::open_protocol::<AbsolutePointer>(params, boot::OpenProtocolAttributes::GetProtocol)
            } {
                let _ = ptr.reset(false);
                let mx = ptr.mode().absolute_max_x.max(1);
                let my = ptr.mode().absolute_max_y.max(1);
                log_line_str(&format!("  AbsolutePointer: max=({},{})", mx, my));
                abs = Some((ptr, mx, my));
            }
        }
        if abs.is_none() {
            if let Some(h) = boot::find_handles::<Pointer>().ok().and_then(|h| h.into_iter().next()) {
                log_line_str("  Trying SimplePointer...");
                let params = boot::OpenProtocolParams {
                    handle: h,
                    agent: boot::image_handle(),
                    controller: None,
                };
                if let Ok(mut ptr) = unsafe {
                    boot::open_protocol::<Pointer>(params, boot::OpenProtocolAttributes::GetProtocol)
                } {
                    let _ = ptr.reset(false);
                    log_line_str("  SimplePointer: ready");
                    simple = Some(ptr);
                }
            }
        }

        if abs.is_some() || simple.is_some() {
            log_line_str("Mouse: using UEFI protocol");
            return Ok(Mouse { usb_io: None, ep_addr: 0, iface: 0, report_buf: vec![0u8; 8], abs, simple });
        }

        log_line_str("Mouse: no device found via any protocol");
        Err("no mouse device found")
    }

    fn try_usb_io() -> Option<Mouse> {
        let handles = boot::find_handles::<UsbIo>().ok()?;
        log_line_str(&format!("  Found {} USB IO handles", handles.len()));

        for handle in handles {
            let params = boot::OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            };
            let mut usb = unsafe {
                boot::open_protocol::<UsbIo>(params, boot::OpenProtocolAttributes::GetProtocol).ok()?
            };

            // GET_DESCRIPTOR (device)
            let mut dev_buf = vec![0u8; 18];
            let _ = usb.control_transfer(0x80, 6, 0x0100, 0, ControlTransfer::DataIn(&mut dev_buf), 5000);

            let vid = u16::from_le_bytes([dev_buf[8], dev_buf[9]]);
            let pid = u16::from_le_bytes([dev_buf[10], dev_buf[11]]);
            log_line_str(&format!("  USB IO: vid=0x{:04x} pid=0x{:04x}", vid, pid));

            // GET_DESCRIPTOR (configuration)
            let mut cfg_buf = vec![0u8; 512];
            let _ = usb.control_transfer(0x80, 6, 0x0200, 0, ControlTransfer::DataIn(&mut cfg_buf), 5000);

            // Parse configuration descriptor to find HID interfaces
            let mut off = 0;
            let mut current_iface: u8 = 0;
            let mut intr_ep: Option<u8> = None;
            let mut intr_mps: u16 = 0;
            let mut is_mouse = false;

            while off + 2 < cfg_buf.len() {
                let b_len = cfg_buf[off] as usize;
                let b_type = cfg_buf[off + 1];
                if b_len < 2 || off + b_len > cfg_buf.len() { break; }

                match b_type {
                    4 => {
                        // Interface descriptor
                        current_iface = cfg_buf[off + 2];
                        let class = cfg_buf[off + 5];
                        let subclass = cfg_buf[off + 6];
                        intr_ep = None;
                        is_mouse = false;

                        // HID class = 3
                        if class == 3 && subclass == 1 {
                            // Boot interface subclass — could be mouse or keyboard
                            // Check protocol: 2=mouse, 1=keyboard
                            let protocol = cfg_buf[off + 7];
                            if protocol == 2 {
                                is_mouse = true;
                                log_line_str(&format!("    iface {} protocol=Mouse", current_iface));
                            } else if protocol == 1 {
                                log_line_str(&format!("    iface {} protocol=Keyboard", current_iface));
                            }
                        }
                    }
                    5 => {
                        // Endpoint descriptor
                        if b_len >= 7 && intr_ep.is_none() {
                            let ea = cfg_buf[off + 2];
                            let attrs = cfg_buf[off + 3];
                            if ea & 0x80 != 0 && attrs & 0x03 == 3 {
                                intr_ep = Some(ea);
                                intr_mps = u16::from_le_bytes([cfg_buf[off + 4], cfg_buf[off + 5]]);
                            }
                        }
                    }
                    0x24 => {
                        // HID descriptor (inside interface)
                        if b_len >= 6 && !is_mouse {
                            // Check HID report descriptor for usage
                            let desc_type = cfg_buf[off + 3];
                            if desc_type == 0x22 {
                                // HID Report Descriptor follows
                                // We'll check it after the configuration descriptor
                            }
                        }
                    }
                    _ => {}
                }
                off += b_len;
            }

            let ep = match intr_ep {
                Some(e) => e,
                None => {
                    log_line_str("    no interrupt IN endpoint found");
                    continue;
                }
            };

            log_line_str(&format!("    HID ep=0x{:02x} mps={} mouse={}", ep, intr_mps, is_mouse));

            // Only accept mouse HID devices
            if !is_mouse {
                log_line_str("    skipping (not mouse)");
                continue;
            }

            // Get bConfigurationValue from the configuration descriptor (offset 5 of the first 9-byte desc)
            let config_value = cfg_buf[5];
            log_line_str(&format!("    SET_CONFIGURATION config={}", config_value));

            // 1. SET_CONFIGURATION — required before any endpoint transfers
            let _ = usb.control_transfer(0x00, 0x09, config_value as u16, 0, ControlTransfer::None, 5000);

            // 2. SET_IDLE (bRequest=0x0A) — required for mouse to start sending reports
            let _ = usb.control_transfer(0x21, 0x0A, 0, current_iface as u16, ControlTransfer::None, 5000);

            // 3. SET_PROTOCOL (bRequest=0x0B, wValue=0 for boot protocol)
            let _ = usb.control_transfer(0x21, 0x0B, 0, current_iface as u16, ControlTransfer::None, 5000);

            log_line_str("    USB HID configured");

            let report_buf = vec![0u8; intr_mps as usize];

            log_line_str("Mouse: using USB IO (direct HID boot protocol)");

            return Some(Mouse {
                usb_io: Some(usb),
                ep_addr: ep,
                iface: current_iface,
                report_buf,
                abs: None,
                simple: None,
            });
        }
        None
    }

    pub fn is_absolute(&self) -> bool {
        self.usb_io.is_some() || self.abs.is_some()
    }

    pub fn abs_max(&self) -> (u64, u64) {
        match &self.abs {
            Some((_, mx, my)) => (*mx, *my),
            None => (32767, 32767), 
        }
    }

    pub fn poll(&mut self) -> Option<MouseEvent> {
        // Try USB IO first
        if let Some(usb) = &mut self.usb_io {
            if self.ep_addr != 0 {
                match usb.sync_interrupt_receive(self.ep_addr, &mut self.report_buf, 1) {
                    Ok(n) => {
                        if n == 0 { return None; }
                        let r = &self.report_buf[..n];
                        let mut ev = MouseEvent::default();

                        if n >= 5 {
                            ev.left = r[0] & 0x01 != 0;
                            ev.right = r[0] & 0x02 != 0;
                            ev.middle = r[0] & 0x04 != 0;
                            ev.abs_x = u16::from_le_bytes([r[1], r[2]]) as u64;
                            ev.abs_y = u16::from_le_bytes([r[3], r[4]]) as u64;
                            ev.is_absolute = true;
                        } else if n >= 4 {
                            ev.left = r[0] & 0x01 != 0;
                            ev.right = r[0] & 0x02 != 0;
                            ev.middle = r[0] & 0x04 != 0;
                            ev.rel_dx = r[1] as i8 as i32;
                            ev.rel_dy = r[2] as i8 as i32;
                            ev.scroll = r[3] as i8 as i32;
                            ev.is_absolute = false;
                        } else if n >= 3 {
                            ev.left = r[0] & 0x01 != 0;
                            ev.right = r[0] & 0x02 != 0;
                            ev.middle = r[0] & 0x04 != 0;
                            ev.rel_dx = r[1] as i8 as i32;
                            ev.rel_dy = r[2] as i8 as i32;
                            ev.is_absolute = false;
                        }

                        return Some(ev);
                    }
                    Err(e) => {
                        static mut ERR_COUNT: u32 = 0;
                        unsafe {
                            ERR_COUNT += 1;
                            if ERR_COUNT <= 3 {
                                log_line_str(&format!("Mouse USB IO: sync_interrupt error {:?} (count={})", e, ERR_COUNT));
                            }
                        }
                        // Fallback to GET_REPORT if interrupt transfer fails
                        if usb.control_transfer(0xA1, 0x01, 0x0100, self.iface as u16, ControlTransfer::DataIn(&mut self.report_buf), 1).is_ok() {
                            let n = self.report_buf.len();
                            if n > 0 {
                                let r = &self.report_buf[..n];
                                static mut LAST_REPORT: [u8; 8] = [0; 8];
                                let mut changed = false;
                                for i in 0..n.min(8) {
                                    if unsafe { LAST_REPORT[i] } != r[i] {
                                        changed = true;
                                        break;
                                    }
                                }
                                if changed {
                                    for i in 0..n.min(8) {
                                        unsafe { LAST_REPORT[i] = r[i]; }
                                    }
                                    let mut ev = MouseEvent::default();
                                    if n >= 5 {
                                        ev.left = r[0] & 0x01 != 0;
                                        ev.right = r[0] & 0x02 != 0;
                                        ev.middle = r[0] & 0x04 != 0;
                                        ev.abs_x = u16::from_le_bytes([r[1], r[2]]) as u64;
                                        ev.abs_y = u16::from_le_bytes([r[3], r[4]]) as u64;
                                        ev.is_absolute = true;
                                        return Some(ev);
                                    } else if n >= 4 {
                                        ev.left = r[0] & 0x01 != 0;
                                        ev.right = r[0] & 0x02 != 0;
                                        ev.middle = r[0] & 0x04 != 0;
                                        ev.rel_dx = r[1] as i8 as i32;
                                        ev.rel_dy = r[2] as i8 as i32;
                                        ev.scroll = r[3] as i8 as i32;
                                        ev.is_absolute = false;
                                        return Some(ev);
                                    } else if n >= 3 {
                                        ev.left = r[0] & 0x01 != 0;
                                        ev.right = r[0] & 0x02 != 0;
                                        ev.middle = r[0] & 0x04 != 0;
                                        ev.rel_dx = r[1] as i8 as i32;
                                        ev.rel_dy = r[2] as i8 as i32;
                                        ev.is_absolute = false;
                                        return Some(ev);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some((ptr, _mx, _my)) = &mut self.abs {
            let mut acc = MouseEvent::default();
            acc.is_absolute = true;
            let mut got = false;
            static mut ABS_LOGGED: bool = false;

            for _ in 0..20 {
                match ptr.get_state() {
                    Ok(Some(state)) => {
                        acc.abs_x = state.current_x;
                        acc.abs_y = state.current_y;
                        if state.active_buttons & 0x1 != 0 { acc.left = true; }
                        if state.active_buttons & 0x2 != 0 { acc.right = true; }
                        got = true;
                        unsafe {
                            if !ABS_LOGGED {
                                ABS_LOGGED = true;
                                log_line_str(&format!("AbsPtr: GOT DATA x={} y={} btn={}", state.current_x, state.current_y, state.active_buttons));
                            }
                        }
                    }
                    Ok(None) => { /* NOT_READY */ }
                    Err(e) => {
                        log_line_str(&format!("AbsPtr: get_state error: {:?}", e));
                        break;
                    }
                }
            }
            if got { return Some(acc); }
        }

        
        if let Some(ptr) = &mut self.simple {
            let mut acc = MouseEvent::default();
            acc.is_absolute = false;
            let mut got = false;
            static mut SIMPLE_LOGGED: bool = false;
            loop {
                match ptr.read_state() {
                    Ok(Some(state)) => {
                        acc.rel_dx += state.relative_movement[0];
                        acc.rel_dy += state.relative_movement[1];
                        if state.button[0] { acc.left = true; }
                        if state.button[1] { acc.right = true; }
                        got = true;
                        unsafe {
                            if !SIMPLE_LOGGED {
                                SIMPLE_LOGGED = true;
                                log_line_str(&format!("SimplePtr: GOT DATA dx={} dy={} btn={}{}", state.relative_movement[0], state.relative_movement[1], state.button[0] as u8, state.button[1] as u8));
                            }
                        }
                    }
                    _ => break,
                }
            }
            if got { return Some(acc); }
        }

        None
    }
}

pub fn apply_mouse_event(cx: &mut i32, cy: &mut i32, ev: &MouseEvent,
                     screen_w: usize, screen_h: usize,
                     abs_max: (u64, u64)) -> (i32, i32) {
    let (abs_max_x, abs_max_y) = abs_max;
    if ev.is_absolute && abs_max_x > 0 && abs_max_y > 0 {
        let new_x = ((ev.abs_x as u128 * screen_w as u128) / abs_max_x as u128) as i32;
        let new_y = ((ev.abs_y as u128 * screen_h as u128) / abs_max_y as u128) as i32;
        *cx = new_x.max(0).min(screen_w as i32 - 1);
        *cy = new_y.max(0).min(screen_h as i32 - 1);
    } else {
        *cx = (*cx + ev.rel_dx).clamp(0, screen_w as i32 - 1);
        *cy = (*cy + ev.rel_dy).clamp(0, screen_h as i32 - 1);
    }
    (*cx, *cy)
}

pub fn log_line_str(s: &str) {
    // Print to UEFI console
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.output_string(uefi::cstr16!("BaramOS: "));
        let mut buf = Vec::<u16>::with_capacity(s.len() + 1);
        for &b in s.as_bytes() {
            if b >= 0x80 { break; }
            buf.push(b as u16);
        }
        buf.push(0);
        if let Ok(cs) = uefi::CStr16::from_u16_with_nul(&buf) {
            let _ = stdout.output_string(cs);
        }
        let _ = stdout.output_string(uefi::cstr16!("\r\n"));
    });
    // Also draw on screen
    crate::iokit::debug_log::log(s);
}
