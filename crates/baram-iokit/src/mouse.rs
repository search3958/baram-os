use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use uefi::boot;
use uefi::proto::console::pointer::Pointer;
use uefi::proto::usb::io::{ControlTransfer, UsbIo};

use crate::absolute_pointer::AbsolutePointer;
use crate::usb_hid::{parse_hid_report_descs, parse_input_report, HidParsedEvent, HidReportLayout};

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
    // Native UEFI pointer protocols are deliberately preferred.  Firmware has
    // already converted PS/2, I2C-HID and vendor-specific trackpads into these
    // protocols, which is much safer than reconfiguring their USB interface.
    abs: Option<(boot::ScopedProtocol<AbsolutePointer>, u64, u64, u64, u64)>,
    simple: Option<boot::ScopedProtocol<Pointer>>,

    // Direct USB HID is the fallback for firmware that exposes no pointer
    // protocol (and is also useful for hot-plugged USB mice).
    usb_io: Option<boot::ScopedProtocol<UsbIo>>,
    ep_addr: u8,
    iface: u8,
    report_buf: Vec<u8>,
    last_report: Vec<u8>,
    hid_layouts: Vec<HidReportLayout>,
    usb_boot_mouse: bool,

    // Absolute HID touchpads need relative finger tracking.  Mapping their
    // surface coordinates straight onto the display makes the cursor jump at
    // the beginning of every gesture.
    last_touch: Option<(i32, i32)>,
    touch_acc_x: f32,
    touch_acc_y: f32,
    poll_count: u32,
    usb_error_count: u32,
}

impl Mouse {
    fn empty() -> Self {
        Self {
            abs: None,
            simple: None,
            usb_io: None,
            ep_addr: 0,
            iface: 0,
            report_buf: vec![0; 8],
            last_report: Vec::new(),
            hid_layouts: Vec::new(),
            usb_boot_mouse: false,
            last_touch: None,
            touch_acc_x: 0.0,
            touch_acc_y: 0.0,
            poll_count: 0,
            usb_error_count: 0,
        }
    }

    pub fn get_wait_event() -> Option<uefi::Event> {
        if let Some(handle) = boot::find_handles::<AbsolutePointer>()
            .ok()
            .and_then(|handles| handles.into_iter().next())
        {
            let params = boot::OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            };
            if let Ok(pointer) = unsafe {
                boot::open_protocol::<AbsolutePointer>(
                    params,
                    boot::OpenProtocolAttributes::GetProtocol,
                )
            } {
                let event = pointer.wait_for_input_event();
                let raw = unsafe { core::mem::transmute_copy::<uefi::Event, usize>(&event) };
                core::mem::forget(pointer);
                return Some(unsafe { core::mem::transmute_copy::<usize, uefi::Event>(&raw) });
            }
        }

        if let Some(handle) = boot::find_handles::<Pointer>()
            .ok()
            .and_then(|handles| handles.into_iter().next())
        {
            let params = boot::OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            };
            if let Ok(pointer) = unsafe {
                boot::open_protocol::<Pointer>(params, boot::OpenProtocolAttributes::GetProtocol)
            } {
                if let Ok(event) = pointer.wait_for_input_event() {
                    let raw = unsafe { core::mem::transmute_copy::<uefi::Event, usize>(&event) };
                    core::mem::forget(pointer);
                    return Some(unsafe { core::mem::transmute_copy::<usize, uefi::Event>(&raw) });
                }
            }
        }
        None
    }

    pub fn open() -> Result<Self, &'static str> {
        let usb_count = boot::find_handles::<UsbIo>().map(|h| h.len()).unwrap_or(0);
        let abs_count = boot::find_handles::<AbsolutePointer>()
            .map(|h| h.len())
            .unwrap_or(0);
        let simple_count = boot::find_handles::<Pointer>()
            .map(|h| h.len())
            .unwrap_or(0);
        log_line_str(&format!(
            "Mouse: devices USB={} Abs={} Simple={}",
            usb_count, abs_count, simple_count
        ));

        let mut mouse = Self::empty();
        if mouse.try_open_uefi() || mouse.try_open_usb() {
            return Ok(mouse);
        }

        // Keep a live object so a mouse connected after boot can be found by
        // the periodic rescan in poll().
        log_line_str("Mouse: no device yet; hotplug scan remains active");
        Ok(mouse)
    }

    fn try_open_uefi(&mut self) -> bool {
        if self.abs.is_some() || self.simple.is_some() {
            return true;
        }

        // AbsolutePointer is preferred for tablets/touchscreens and for QEMU's
        // usb-tablet.  Preserve the mode's non-zero coordinate origin.
        if let Ok(handles) = boot::find_handles::<AbsolutePointer>() {
            for handle in handles {
                let params = boot::OpenProtocolParams {
                    handle,
                    agent: boot::image_handle(),
                    controller: None,
                };
                let Ok(mut pointer) = (unsafe {
                    boot::open_protocol::<AbsolutePointer>(
                        params,
                        boot::OpenProtocolAttributes::GetProtocol,
                    )
                }) else {
                    continue;
                };
                // Extended verification is slow and is not required for normal
                // operation.  A failed reset does not make GetState unusable.
                let _ = pointer.reset(false);
                let mode = pointer.mode();
                let min_x = mode.absolute_min_x;
                let min_y = mode.absolute_min_y;
                let range_x = mode.absolute_max_x.saturating_sub(min_x).max(1);
                let range_y = mode.absolute_max_y.saturating_sub(min_y).max(1);
                log_line_str(&format!(
                    "Mouse: using UEFI AbsolutePointer range=({},{})",
                    range_x, range_y
                ));
                self.abs = Some((pointer, range_x, range_y, min_x, min_y));
                return true;
            }
        }

        // SimplePointer is the normal firmware abstraction for laptop
        // trackpads, PS/2 mice and many USB mice.
        if let Ok(handles) = boot::find_handles::<Pointer>() {
            for handle in handles {
                let params = boot::OpenProtocolParams {
                    handle,
                    agent: boot::image_handle(),
                    controller: None,
                };
                let Ok(mut pointer) = (unsafe {
                    boot::open_protocol::<Pointer>(
                        params,
                        boot::OpenProtocolAttributes::GetProtocol,
                    )
                }) else {
                    continue;
                };
                let _ = pointer.reset(false);
                log_line_str("Mouse: using UEFI SimplePointer");
                self.simple = Some(pointer);
                return true;
            }
        }
        false
    }

    fn try_open_usb(&mut self) -> bool {
        if self.usb_io.is_some() {
            return true;
        }
        let Ok(handles) = boot::find_handles::<UsbIo>() else {
            return false;
        };

        for handle in handles {
            let params = boot::OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            };
            let Ok(mut usb) = (unsafe {
                boot::open_protocol::<UsbIo>(params, boot::OpenProtocolAttributes::GetProtocol)
            }) else {
                continue;
            };

            let Ok(interface) = usb.interface_descriptor() else {
                continue;
            };
            if interface.interface_class != 3 {
                continue;
            }

            let mut endpoint = None;
            let mut max_packet = 0usize;
            for index in 0..interface.num_endpoints {
                if let Ok(descriptor) = usb.endpoint_descriptor(index) {
                    if descriptor.endpoint_address & 0x80 != 0 && descriptor.attributes & 0x03 == 3
                    {
                        endpoint = Some(descriptor.endpoint_address);
                        max_packet = descriptor.max_packet_size as usize;
                        break;
                    }
                }
            }
            let Some(endpoint) = endpoint else { continue };

            // Asking for a larger descriptor is valid: the device terminates
            // the control transfer at its real descriptor length.
            let mut descriptor = vec![0u8; 1024];
            let layouts = if usb
                .control_transfer(
                    0x81,
                    0x06,
                    0x2200,
                    interface.interface_number as u16,
                    ControlTransfer::DataIn(&mut descriptor),
                    500,
                )
                .is_ok()
            {
                parse_hid_report_descs(&descriptor)
            } else {
                Vec::new()
            };

            let boot_mouse = interface.interface_subclass == 1
                && interface.interface_protocol == 2
                && layouts.is_empty();
            if layouts.is_empty() && !boot_mouse {
                continue;
            }

            // Do not issue SET_CONFIGURATION here: the UEFI USB bus driver has
            // already configured the interface.  Reissuing it resets endpoint
            // state on real machines.  Only force boot protocol if descriptor
            // parsing failed and this is explicitly a boot-mouse interface.
            let _ = usb.control_transfer(
                0x21,
                0x0a,
                0,
                interface.interface_number as u16,
                ControlTransfer::None,
                100,
            );
            if boot_mouse {
                let _ = usb.control_transfer(
                    0x21,
                    0x0b,
                    0,
                    interface.interface_number as u16,
                    ControlTransfer::None,
                    100,
                );
            }

            let report_len = layouts
                .iter()
                .map(|layout| layout.report_size_bytes as usize)
                .max()
                .unwrap_or(4)
                .max(max_packet)
                .clamp(4, 1024);
            log_line_str(&format!(
                "Mouse: using USB HID iface={} ep=0x{:02x} reports={}{}",
                interface.interface_number,
                endpoint,
                layouts.len(),
                if boot_mouse { " boot" } else { "" }
            ));
            self.usb_io = Some(usb);
            self.ep_addr = endpoint;
            self.iface = interface.interface_number;
            self.report_buf = vec![0; report_len];
            self.last_report.clear();
            self.hid_layouts = layouts;
            self.usb_boot_mouse = boot_mouse;
            return true;
        }
        false
    }

    pub fn is_absolute(&self) -> bool {
        self.abs.is_some()
            || self.hid_layouts.iter().any(|layout| {
                layout.is_absolute
                    && !(layout.application_usage_page == 0x0d && layout.application_usage == 0x05)
            })
    }

    pub fn abs_max(&self) -> (u64, u64) {
        if let Some((_, range_x, range_y, _, _)) = &self.abs {
            return (*range_x, *range_y);
        }
        self.hid_layouts
            .iter()
            .filter(|layout| layout.is_absolute)
            .map(|layout| {
                (
                    layout.x_max.saturating_sub(layout.x_min).max(1) as u64,
                    layout.y_max.saturating_sub(layout.y_min).max(1) as u64,
                )
            })
            .next()
            .unwrap_or((1, 1))
    }

    pub fn poll(&mut self) -> Option<MouseEvent> {
        self.poll_count = self.poll_count.wrapping_add(1);
        if self.abs.is_none()
            && self.simple.is_none()
            && self.usb_io.is_none()
            && self.poll_count % 1000 == 0
        {
            if self.try_open_uefi() || self.try_open_usb() {
                log_line_str("Mouse: hotplug device recognized");
            }
        }

        if self.usb_io.is_some() {
            if let Some(event) = self.poll_usb() {
                return Some(event);
            }
        }

        if let Some((pointer, _range_x, _range_y, min_x, min_y)) = &mut self.abs {
            let mut latest = None;
            // Drain queued states and use the newest position.
            for _ in 0..32 {
                match pointer.get_state() {
                    Ok(Some(state)) => latest = Some(state),
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
            if let Some(state) = latest {
                return Some(MouseEvent {
                    abs_x: state.current_x.saturating_sub(*min_x),
                    abs_y: state.current_y.saturating_sub(*min_y),
                    is_absolute: true,
                    left: state.active_buttons & 1 != 0,
                    right: state.active_buttons & 2 != 0,
                    ..MouseEvent::default()
                });
            }
        }

        if let Some(pointer) = &mut self.simple {
            let mut event = MouseEvent::default();
            let mut received = false;
            loop {
                match pointer.read_state() {
                    Ok(Some(state)) => {
                        event.rel_dx = event.rel_dx.saturating_add(state.relative_movement[0]);
                        event.rel_dy = event.rel_dy.saturating_add(state.relative_movement[1]);
                        event.left |= state.button[0];
                        event.right |= state.button[1];
                        received = true;
                    }
                    _ => break,
                }
            }
            if received {
                return Some(event);
            }
        }
        None
    }

    fn poll_usb(&mut self) -> Option<MouseEvent> {
        let transfer = {
            let usb = self.usb_io.as_mut()?;
            usb.sync_interrupt_receive(self.ep_addr, &mut self.report_buf, 1)
        };

        match transfer {
            Ok(length) if length != 0 => {
                self.usb_error_count = 0;
                let report = self.report_buf[..length].to_vec();
                self.decode_usb_report(&report)
            }
            Ok(_) => None,
            Err(_) => {
                self.usb_error_count = self.usb_error_count.saturating_add(1);
                // Some firmware implements UsbIo but not synchronous interrupt
                // receive.  GET_REPORT is a useful last-resort path.  Avoid
                // returning the same held-state report on every timer tick.
                let report_id = self.hid_layouts.first().map(|l| l.report_id).unwrap_or(0);
                let request_ok = {
                    let usb = self.usb_io.as_mut()?;
                    usb.control_transfer(
                        0xa1,
                        0x01,
                        0x0100 | report_id as u16,
                        self.iface as u16,
                        ControlTransfer::DataIn(&mut self.report_buf),
                        1,
                    )
                    .is_ok()
                };
                if !request_ok {
                    return None;
                }
                let expected = self
                    .layout_for_report(&self.report_buf)
                    .map(|layout| layout.report_size_bytes as usize)
                    .unwrap_or(if self.usb_boot_mouse { 4 } else { 0 })
                    .min(self.report_buf.len());
                if expected == 0 || self.last_report.as_slice() == &self.report_buf[..expected] {
                    return None;
                }
                let report = self.report_buf[..expected].to_vec();
                self.last_report.clear();
                self.last_report.extend_from_slice(&report);
                self.decode_usb_report(&report)
            }
        }
    }

    fn layout_for_report(&self, report: &[u8]) -> Option<HidReportLayout> {
        self.hid_layouts
            .iter()
            .find(|layout| layout.report_id == 0 || report.first() == Some(&layout.report_id))
            .copied()
    }

    fn decode_usb_report(&mut self, report: &[u8]) -> Option<MouseEvent> {
        if self.usb_boot_mouse {
            if report.len() < 3 {
                return None;
            }
            return Some(MouseEvent {
                rel_dx: report[1] as i8 as i32,
                rel_dy: report[2] as i8 as i32,
                scroll: report.get(3).copied().unwrap_or(0) as i8 as i32,
                left: report[0] & 1 != 0,
                right: report[0] & 2 != 0,
                middle: report[0] & 4 != 0,
                ..MouseEvent::default()
            });
        }

        let layout = self.layout_for_report(report)?;
        let parsed = parse_input_report(&layout, report)?;
        let mut event = buttons_and_wheel(&parsed);

        let is_touchpad = layout.application_usage_page == 0x0d && layout.application_usage == 0x05;
        if layout.is_absolute && is_touchpad {
            if layout.has_tip && !parsed.touching {
                self.last_touch = None;
                self.touch_acc_x = 0.0;
                self.touch_acc_y = 0.0;
                return Some(event);
            }
            if let Some((last_x, last_y)) = self.last_touch {
                // Typical precision touchpads report roughly 3000-6000 units
                // across their surface.  This scale gives useful desktop
                // motion while retaining fractional movement.
                const TRACKPAD_SCALE: f32 = 0.35;
                let dx = (parsed.x - last_x) as f32 * TRACKPAD_SCALE + self.touch_acc_x;
                let dy = (parsed.y - last_y) as f32 * TRACKPAD_SCALE + self.touch_acc_y;
                event.rel_dx = dx as i32;
                event.rel_dy = dy as i32;
                self.touch_acc_x = dx - event.rel_dx as f32;
                self.touch_acc_y = dy - event.rel_dy as f32;
            }
            self.last_touch = Some((parsed.x, parsed.y));
        } else if layout.is_absolute {
            event.is_absolute = true;
            event.abs_x = parsed.x.saturating_sub(parsed.x_min) as u64;
            event.abs_y = parsed.y.saturating_sub(parsed.y_min) as u64;
        } else {
            event.rel_dx = parsed.x;
            event.rel_dy = parsed.y;
        }
        Some(event)
    }
}

fn buttons_and_wheel(parsed: &HidParsedEvent) -> MouseEvent {
    MouseEvent {
        left: parsed.buttons & 1 != 0,
        right: parsed.buttons & 2 != 0,
        middle: parsed.buttons & 4 != 0,
        scroll: parsed.wheel,
        ..MouseEvent::default()
    }
}

pub fn apply_mouse_event(
    cx: &mut i32,
    cy: &mut i32,
    event: &MouseEvent,
    screen_w: usize,
    screen_h: usize,
    abs_max: (u64, u64),
) -> (i32, i32) {
    if screen_w == 0 || screen_h == 0 {
        return (*cx, *cy);
    }
    let (abs_max_x, abs_max_y) = abs_max;
    if event.is_absolute && abs_max_x != 0 && abs_max_y != 0 {
        *cx = ((event.abs_x as u128 * (screen_w - 1) as u128) / abs_max_x as u128)
            .min((screen_w - 1) as u128) as i32;
        *cy = ((event.abs_y as u128 * (screen_h - 1) as u128) / abs_max_y as u128)
            .min((screen_h - 1) as u128) as i32;
    } else {
        *cx = cx
            .saturating_add(event.rel_dx)
            .clamp(0, screen_w as i32 - 1);
        *cy = cy
            .saturating_add(event.rel_dy)
            .clamp(0, screen_h as i32 - 1);
    }
    (*cx, *cy)
}

pub fn log_line_str(s: &str) {
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.output_string(uefi::cstr16!("BaramOS: "));
        let mut buffer = Vec::<u16>::with_capacity(s.len() + 1);
        for &byte in s.as_bytes() {
            if byte >= 0x80 {
                break;
            }
            buffer.push(byte as u16);
        }
        buffer.push(0);
        if let Ok(text) = uefi::CStr16::from_u16_with_nul(&buffer) {
            let _ = stdout.output_string(text);
        }
        let _ = stdout.output_string(uefi::cstr16!("\r\n"));
    });
    crate::debug_log::log(s);
}
