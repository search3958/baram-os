


use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use uefi::boot;
use uefi::proto::usb::io::{ControlTransfer, UsbIo};
use uefi::proto::console::pointer::Pointer;
use crate::absolute_pointer::AbsolutePointer;
use crate::usb_hid::{HidReportLayout, parse_hid_report_desc, parse_input_report};

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

    // Generic HID report layout (used for trackpads / touch panels / non-boot mice)
    hid_layout: Option<HidReportLayout>,

    abs: Option<(boot::ScopedProtocol<AbsolutePointer>, u64, u64, u64, u64)>,
    simple: Option<(boot::ScopedProtocol<Pointer>, u64, u64)>,

    // SimplePointer auto-calibration: pixels-per-count derived from device
    // resolution (counts/mm). 0 means "unknown", use a sensible default.
    simple_scale: f32,

    // Last absolute position reported by the device (None until first sample)
    last_abs_x: Option<u64>,
    last_abs_y: Option<u64>,

    // Sub-pixel accumulator for relative (SimplePointer) movement.
    simple_acc_x: f32,
    simple_acc_y: f32,

    // Auto-calibrated sensitivity (pixels per reported count) for the
    // SimplePointer, derived from its resolution (counts/mm).
    calibrated: bool,
}

impl Mouse {
    pub fn get_wait_event() -> Option<uefi::Event> {
        // Try AbsolutePointer first
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
                let raw = unsafe { core::mem::transmute_copy::<uefi::Event, usize>(&evt) };
                core::mem::forget(ptr);
                return Some(unsafe { core::mem::transmute_copy::<usize, uefi::Event>(&raw) });
            }
        }

        // Fallback: try SimplePointer
        if let Some(h) = boot::find_handles::<Pointer>().ok().and_then(|h| h.into_iter().next()) {
            let params = boot::OpenProtocolParams {
                handle: h,
                agent: boot::image_handle(),
                controller: None,
            };
            if let Ok(ptr) = unsafe {
                boot::open_protocol::<Pointer>(params, boot::OpenProtocolAttributes::GetProtocol)
            } {
                if let Ok(evt) = ptr.wait_for_input_event() {
                    let raw = unsafe { core::mem::transmute_copy::<uefi::Event, usize>(&evt) };
                    core::mem::forget(ptr);
                    return Some(unsafe { core::mem::transmute_copy::<usize, uefi::Event>(&raw) });
                }
            }
        }

        None
    }
    pub fn open() -> Result<Mouse, &'static str> {
        let usb_count = boot::find_handles::<UsbIo>().map(|h| h.len()).unwrap_or(0);
        let abs_count = boot::find_handles::<AbsolutePointer>().map(|h| h.len()).unwrap_or(0);
        let simple_count = boot::find_handles::<Pointer>().map(|h| h.len()).unwrap_or(0);
        log_line_str(&format!(
            "Mouse: devices USB={} Abs={} Simple={}",
            usb_count, abs_count, simple_count
        ));

        // Try USB HID first; fall back to UEFI pointer protocols.
        let mut mouse = Mouse {
            usb_io: None, ep_addr: 0, iface: 0, report_buf: vec![0u8; 8],
            hid_layout: None, abs: None, simple: None,
            simple_scale: 0.0, simple_acc_x: 0.0, simple_acc_y: 0.0, calibrated: false,
            last_abs_x: None, last_abs_y: None,
        };
        if mouse.try_open() {
            log_line_str("Mouse: initialized");
            return Ok(mouse);
        }

        log_line_str("Mouse: no device found via any protocol");
        Err("no mouse device found")
    }

    /// Open any available input device.  Safe to call repeatedly (e.g. on
    /// hotplug): only fills in the slots that are still empty.
    /// Returns true if at least one device is now active.
    pub fn try_open(&mut self) -> bool {
        if self.usb_io.is_none() && self.abs.is_none() && self.simple.is_none() {
            if let Some(m) = Self::try_usb_io() {
                *self = m;
                return true;
            }
        }
        if self.abs.is_none() {
            if let Some(h) = boot::find_handles::<AbsolutePointer>().ok().and_then(|h| h.into_iter().next()) {
                let params = boot::OpenProtocolParams { handle: h, agent: boot::image_handle(), controller: None };
                if let Ok(mut ptr) = unsafe { boot::open_protocol::<AbsolutePointer>(params, boot::OpenProtocolAttributes::GetProtocol) } {
                    let _ = ptr.reset(true);
                    let mode = ptr.mode();
                    let min_x = mode.absolute_min_x.max(0);
                    let min_y = mode.absolute_min_y.max(0);
                    let mx = (mode.absolute_max_x.max(min_x + 1) - min_x).max(1);
                    let my = (mode.absolute_max_y.max(min_y + 1) - min_y).max(1);
                    self.abs = Some((ptr, mx, my, min_x, min_y));
                }
            }
        }
        if self.simple.is_none() {
            if let Some(h) = boot::find_handles::<Pointer>().ok().and_then(|h| h.into_iter().next()) {
                let params = boot::OpenProtocolParams { handle: h, agent: boot::image_handle(), controller: None };
                if let Ok(mut ptr) = unsafe { boot::open_protocol::<Pointer>(params, boot::OpenProtocolAttributes::GetProtocol) } {
                    let _ = ptr.reset(true);
                    let mode = ptr.mode();
                    let res_x = mode.resolution[0].max(1);
                    let res_y = mode.resolution[1].max(1);
                    self.simple = Some((ptr, res_x, res_y));
                }
            }
        }
        self.usb_io.is_some() || self.abs.is_some() || self.simple.is_some()
    }

    /// Re-scan for devices that were plugged in after boot.
    pub fn rescan(&mut self) -> bool {
        if self.usb_io.is_some() || self.abs.is_some() || self.simple.is_some() {
            return false;
        }
        let had = false;
        let now = self.try_open();
        if now && !had {
            log_line_str("Mouse: hotplug device recognized");
        }
        now
    }

    fn try_usb_io() -> Option<Mouse> {
        let handles = boot::find_handles::<UsbIo>().ok()?;

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

            // GET_DESCRIPTOR (configuration)
            let mut cfg_buf = vec![0u8; 512];
            let _ = usb.control_transfer(0x80, 6, 0x0200, 0, ControlTransfer::DataIn(&mut cfg_buf), 5000);

            // Parse configuration descriptor to find HID interfaces that report X/Y
            let mut off = 0;
            let mut current_iface: u8 = 0;
            let mut intr_ep: Option<u8> = None;
            let mut intr_mps: u16 = 0;
            let mut is_pointing = false;
            let mut hid_desc_len: usize = 0;

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
                        is_pointing = false;

                        // HID class = 3, boot subclass = 1
                        if class == 3 && subclass == 1 {
                            let protocol = cfg_buf[off + 7];
                            if protocol == 2 {
                                // Boot mouse
                                is_pointing = true;
                                log_line_str(&format!("    iface {} protocol=Mouse(boot)", current_iface));
                            } else {
                                // Non-boot HID (trackpad / touch panel / etc.)
                                // Decide based on the report descriptor below.
                                log_line_str(&format!("    iface {} protocol={} (HID)", current_iface, protocol));
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
                    0x21 => {
                        // HID descriptor (class-specific, type 0x21)
                        if b_len >= 6 {
                            // Descriptor list starts after wCountryCode (offset 6)
                            let mut d = off + 6;
                            while d + 2 < cfg_buf.len() && d + 2 < off + b_len {
                                let desc_type = cfg_buf[d];
                                let desc_len = cfg_buf[d + 1] as usize;
                                if desc_type == 0x22 {
                                    // HID Report Descriptor
                                    hid_desc_len = (cfg_buf[d + 2] as usize)
                                        | ((cfg_buf[d + 3] as usize) << 8);
                                }
                                if desc_len < 3 { break; }
                                d += desc_len;
                            }
                        }
                    }
                    _ => {}
                }
                off += b_len;
            }

            let ep = match intr_ep {
                Some(e) => e,
                None => continue,
            };

            // Get bConfigurationValue from the configuration descriptor (offset 5 of the first 9-byte desc)
            let config_value = cfg_buf[5];

            // 1. SET_CONFIGURATION — required before any endpoint transfers
            let _ = usb.control_transfer(0x00, 0x09, config_value as u16, 0, ControlTransfer::None, 5000);

            // Fetch the HID report descriptor to determine the exact report layout.
            // This lets us support trackpads, touch panels, and other HID pointing
            // devices that are NOT boot-protocol mice.
            let mut hid_layout: Option<HidReportLayout> = None;
            if hid_desc_len > 0 {
                let mut rep_buf = vec![0u8; hid_desc_len.min(1024)];
                if usb.control_transfer(
                    0x81, 0x06, 0x2200, current_iface as u16,
                    ControlTransfer::DataIn(&mut rep_buf), 5000,
                ).is_ok()
                {
                    let layout = parse_hid_report_desc(&rep_buf[..hid_desc_len.min(rep_buf.len())]);
                    // Accept any HID device that reports an absolute or relative X/Y.
                    if layout.x_size_bits > 0 && layout.y_size_bits > 0 {
                        is_pointing = true;
                        hid_layout = Some(layout);
                    }
                }
            }

            if !is_pointing {
                continue;
            }

            // 2. SET_IDLE (bRequest=0x0A) — required for the device to start sending reports
            let _ = usb.control_transfer(0x21, 0x0A, 0, current_iface as u16, ControlTransfer::None, 5000);

            // 3. SET_PROTOCOL (bRequest=0x0B, wValue=0 for boot protocol)
            let _ = usb.control_transfer(0x21, 0x0B, 0, current_iface as u16, ControlTransfer::None, 5000);

            let report_buf = vec![0u8; intr_mps as usize];

            log_line_str("Mouse: using USB HID");

            return Some(Mouse {
                usb_io: Some(usb),
                ep_addr: ep,
                iface: current_iface,
                report_buf,
                hid_layout,
                abs: None,
                simple: None,
                simple_scale: 0.0,
                simple_acc_x: 0.0,
                simple_acc_y: 0.0,
                calibrated: false,
                last_abs_x: None,
                last_abs_y: None,
            });
        }
        None
    }

    pub fn is_absolute(&self) -> bool {
        self.hid_layout.as_ref().map(|l| l.is_absolute).unwrap_or(false)
            || self.abs.is_some()
    }

    pub fn abs_max(&self) -> (u64, u64) {
        if let Some(l) = &self.hid_layout {
            if l.is_absolute && l.x_max > 0 && l.y_max > 0 {
                return (l.x_max as u64, l.y_max as u64);
            }
        }
        match &self.abs {
            Some((_, mx, my, _, _)) => (*mx, *my),
            None => (32767, 32767),
        }
    }

    pub fn poll(&mut self) -> Option<MouseEvent> {
        // Periodically re-scan for hotplugged devices (~1 Hz at 1 kHz poll).
        static mut RESCAN_CTR: u32 = 0;
        unsafe {
            RESCAN_CTR = RESCAN_CTR.wrapping_add(1);
            if RESCAN_CTR % 1000 == 0 {
                self.rescan();
            }
        }
        // Try USB IO first
        if let Some(usb) = &mut self.usb_io {
            if self.ep_addr != 0 {
                match usb.sync_interrupt_receive(self.ep_addr, &mut self.report_buf, 1) {
                    Ok(n) => {
                        if n == 0 { return None; }
                        let r = &self.report_buf[..n];

                        // If we have a parsed HID layout (trackpad / touch panel / generic
                        // HID pointing device), decode the report according to it.
                        if let Some(layout) = &self.hid_layout {
                            if let Some(ev) = decode_hid_report(layout, r) {
                                return Some(ev);
                            }
                            return None;
                        }

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
                                    if let Some(layout) = &self.hid_layout {
                                        if let Some(ev) = decode_hid_report(layout, r) {
                                            return Some(ev);
                                        }
                                        return None;
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

        // Merge Absolute Pointer and Simple Pointer input into one event.
        // We prefer an absolute position when available; otherwise we fall
        // back to the relative deltas reported by the simple pointer.  Both
        // protocols may be active at once on real hardware (a trackpad often
        // shows up as one or the other, sometimes both).
        let mut acc = MouseEvent::default();
        let mut got_abs = false;
        let mut got_rel = false;

        if let Some((ptr, mx, my, min_x, min_y)) = &mut self.abs {
            let mut last_x = self.last_abs_x.unwrap_or(0);
            let mut last_y = self.last_abs_y.unwrap_or(0);

            for _ in 0..20 {
                match ptr.get_state() {
                    Ok(Some(state)) => {
                        // Normalize into 0..range using the device's min origin.
                        let nx = state.current_x.saturating_sub(*min_x);
                        let ny = state.current_y.saturating_sub(*min_y);
                        acc.abs_x = nx;
                        acc.abs_y = ny;
                        last_x = nx;
                        last_y = ny;
                        if state.active_buttons & 0x1 != 0 { acc.left = true; }
                        if state.active_buttons & 0x2 != 0 { acc.right = true; }
                        got_abs = true;
                    }
                    Ok(None) => { /* NOT_READY */ }
                    Err(e) => {
                        log_line_str(&format!("AbsPtr: get_state error: {:?}", e));
                        break;
                    }
                }
            }
            if got_abs {
                self.last_abs_x = Some(last_x);
                self.last_abs_y = Some(last_y);
                acc.is_absolute = true;
            } else {
                // Re-emit the last known absolute position so the cursor stays
                // responsive even when the firmware only updates on contact.
                if self.last_abs_x.is_some() {
                    acc.abs_x = last_x;
                    acc.abs_y = last_y;
                    acc.is_absolute = true;
                    got_abs = true;
                }
            }
        }

        if let Some((ptr, res_x, res_y)) = &mut self.simple {
            let mut raw_dx: i32 = 0;
            let mut raw_dy: i32 = 0;
            loop {
                match ptr.read_state() {
                    Ok(Some(state)) => {
                        raw_dx += state.relative_movement[0];
                        raw_dy += state.relative_movement[1];
                        if state.button[0] { acc.left = true; }
                        if state.button[1] { acc.right = true; }
                        got_rel = true;
                    }
                    _ => break,
                }
            }
            // Auto-calibrate pixels-per-count from the device resolution
            // (counts/mm). SENSITIVITY scales the whole thing down so a
            // small physical move does not jump across the screen.
            if !self.calibrated {
                let rx = (*res_x).max(1) as f32;
                const SENSITIVITY: f32 = 0.03;
                self.simple_scale = (3.78 / rx) * SENSITIVITY;
                self.calibrated = true;
            }
            // Accumulate fractional pixels so sub-pixel deltas still move.
            let dx = raw_dx as f32 * self.simple_scale + self.simple_acc_x;
            let dy = raw_dy as f32 * self.simple_scale + self.simple_acc_y;
            acc.rel_dx += dx as i32;
            acc.rel_dy += dy as i32;
            self.simple_acc_x = dx - acc.rel_dx as f32;
            self.simple_acc_y = dy - acc.rel_dy as f32;
        }

        // Merge both devices into a single relative event so that, on machines
        // where the trackpad is exposed as BOTH AbsolutePointer and
        // SimplePointer, both inputs are received.  Absolute movement is
        // converted into a relative delta against the last reported position.
        if got_abs {
            let (nx, ny) = (acc.abs_x, acc.abs_y);
            if let (Some(px), Some(py)) = (self.last_abs_x, self.last_abs_y) {
                acc.rel_dx += (nx as i64 - px as i64) as i32;
                acc.rel_dy += (ny as i64 - py as i64) as i32;
            }
            self.last_abs_x = Some(nx);
            self.last_abs_y = Some(ny);
        }
        if got_rel || got_abs {
            acc.is_absolute = false;
            return Some(acc);
        }

        None
    }
}

fn decode_hid_report(layout: &HidReportLayout, report: &[u8]) -> Option<MouseEvent> {
    let parsed = parse_input_report(layout, report);
    if layout.x_size_bits == 0 || layout.y_size_bits == 0 {
        return None;
    }

    let mut ev = MouseEvent::default();
    ev.left = parsed.buttons & 0x01 != 0;
    ev.right = parsed.buttons & 0x02 != 0;
    ev.middle = parsed.buttons & 0x04 != 0;

    // Trackpads / touch panels often report absolute coordinates over a
    // device-specific range.  Some devices, however, report relative
    // deltas through the same layout.  Respect the descriptor flag.
    if layout.is_absolute && parsed.x_max > 0 && parsed.y_max > 0 {
        ev.is_absolute = true;
        ev.abs_x = parsed.x as u64;
        ev.abs_y = parsed.y as u64;
    } else {
        // Treat the decoded fields as relative deltas (sign-extended).
        ev.is_absolute = false;
        ev.rel_dx = sign_extend(parsed.x, layout.x_size_bits);
        ev.rel_dy = sign_extend(parsed.y, layout.y_size_bits);
    }

    Some(ev)
}

fn sign_extend(value: i32, bits: u8) -> i32 {
    if bits == 0 {
        return 0;
    }
    let shift = 32 - bits as i32;
    (value << shift) >> shift
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
    crate::debug_log::log(s);
}
