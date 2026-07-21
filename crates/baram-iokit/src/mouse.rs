use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use uefi::boot;
use uefi::proto::console::pointer::Pointer;
use uefi::proto::usb::io::{ControlTransfer, UsbIo};
use uefi::Handle;

use crate::absolute_pointer::AbsolutePointer;
use crate::pointer_accel::{
    MotionFilter, MotionSettings, TrackpadFilter, TrackpadInterpolator, TrackpadSettings,
};
use crate::usb_hid::{parse_hid_report_descs, parse_input_report, HidReportLayout};

fn load_motion_settings() -> MotionSettings {
    let defaults = MotionSettings::default();
    MotionSettings {
        speed: baram_bsd::config::get_f32("mouse/speed", defaults.speed),
        base_gain: baram_bsd::config::get_f32("mouse/base_gain", defaults.base_gain),
        quadratic_gain: baram_bsd::config::get_f32("mouse/quadratic_gain", defaults.quadratic_gain),
        max_gain: baram_bsd::config::get_f32("mouse/max_gain", defaults.max_gain),
        click_guard_distance: baram_bsd::config::get_f32(
            "mouse/click_guard_distance",
            defaults.click_guard_distance,
        ),
        click_guard_reports: baram_bsd::config::get_usize(
            "mouse/click_guard_reports",
            defaults.click_guard_reports as usize,
        )
        .min(u8::MAX as usize) as u8,
        smoothing: baram_bsd::config::get_f32("mouse/smoothing", defaults.smoothing),
    }
    .sanitized()
}

fn load_trackpad_settings() -> TrackpadSettings {
    let defaults = TrackpadSettings::default();
    TrackpadSettings {
        speed: baram_bsd::config::get_f32("trackpad/speed", defaults.speed),
        base_gain: baram_bsd::config::get_f32("trackpad/base_gain", defaults.base_gain),
        quadratic_gain: baram_bsd::config::get_f32(
            "trackpad/quadratic_gain",
            defaults.quadratic_gain,
        ),
        max_gain: baram_bsd::config::get_f32("trackpad/max_gain", defaults.max_gain),
        click_guard_distance: baram_bsd::config::get_f32(
            "trackpad/click_guard_distance",
            defaults.click_guard_distance,
        ),
        click_guard_reports: baram_bsd::config::get_usize(
            "trackpad/click_guard_reports",
            defaults.click_guard_reports as usize,
        )
        .min(u8::MAX as usize) as u8,
        smoothing: baram_bsd::config::get_f32("trackpad/smoothing", defaults.smoothing),
    }
    .sanitized()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MouseEvent {
    pub abs_x: u64,
    pub abs_y: u64,
    pub abs_max_x: u64,
    pub abs_max_y: u64,
    pub rel_dx: i32,
    pub rel_dy: i32,
    pub trackpad_dx: i32,
    pub trackpad_dy: i32,
    pub is_absolute: bool,
    pub is_trackpad: bool,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
    pub scroll: i32,
}

struct AbsoluteDevice {
    pointer: boot::ScopedProtocol<AbsolutePointer>,
    range_x: u64,
    range_y: u64,
    min_x: u64,
    min_y: u64,
    buttons: u8,
}

struct SimpleDevice {
    pointer: boot::ScopedProtocol<Pointer>,
    buttons: u8,
}

struct UsbPointer {
    io: boot::ScopedProtocol<UsbIo>,
    endpoint: u8,
    interface: u8,
    report_buf: Vec<u8>,
    last_report: Vec<u8>,
    layouts: Vec<HidReportLayout>,
    boot_mouse: bool,
    last_touch: Option<(i32, i32)>,
    buttons: u8,
}

impl UsbPointer {
    fn layout_for_report(&self, report: &[u8]) -> Option<HidReportLayout> {
        self.layouts
            .iter()
            .find(|layout| layout.report_id == 0 || report.first() == Some(&layout.report_id))
            .copied()
    }

    fn poll(&mut self) -> Option<MouseEvent> {
        match self
            .io
            .sync_interrupt_receive(self.endpoint, &mut self.report_buf, 1)
        {
            Ok(length) if length != 0 => {
                let report = self.report_buf[..length].to_vec();
                self.decode_report(&report)
            }
            Ok(_) => None,
            Err(_) => {
                // Some firmware implements UsbIo but not interrupt receive.
                // GET_REPORT provides a useful fallback for those machines.
                let report_id = self.layouts.first().map(|l| l.report_id).unwrap_or(0);
                if self
                    .io
                    .control_transfer(
                        0xa1,
                        0x01,
                        0x0100 | report_id as u16,
                        self.interface as u16,
                        ControlTransfer::DataIn(&mut self.report_buf),
                        1,
                    )
                    .is_err()
                {
                    return None;
                }
                let expected = self
                    .layout_for_report(&self.report_buf)
                    .map(|layout| layout.report_size_bytes as usize)
                    .unwrap_or(if self.boot_mouse { 4 } else { 0 })
                    .min(self.report_buf.len());
                if expected == 0 || self.last_report.as_slice() == &self.report_buf[..expected] {
                    return None;
                }
                let report = self.report_buf[..expected].to_vec();
                self.last_report.clear();
                self.last_report.extend_from_slice(&report);
                self.decode_report(&report)
            }
        }
    }

    fn decode_report(&mut self, report: &[u8]) -> Option<MouseEvent> {
        if self.boot_mouse {
            if report.len() < 3 {
                return None;
            }
            self.buttons = report[0] & 0x07;
            return Some(MouseEvent {
                rel_dx: report[1] as i8 as i32,
                rel_dy: report[2] as i8 as i32,
                scroll: report.get(3).copied().unwrap_or(0) as i8 as i32,
                ..event_with_buttons(self.buttons)
            });
        }

        let layout = self.layout_for_report(report)?;
        let parsed = parse_input_report(&layout, report)?;
        self.buttons = parsed.buttons & 0x07;
        let mut event = event_with_buttons(self.buttons);
        event.scroll = parsed.wheel;

        let is_touchpad = layout.application_usage_page == 0x0d && layout.application_usage == 0x05;
        if layout.is_absolute && is_touchpad {
            // A trackpad reports finger coordinates, but the desktop pointer
            // must move by the delta within a gesture.  A new contact starts a
            // fresh gesture and therefore must not teleport the cursor.
            if layout.has_tip && !parsed.touching {
                self.last_touch = None;
                return Some(event);
            }
            if let Some((last_x, last_y)) = self.last_touch {
                event.is_trackpad = true;
                event.trackpad_dx = parsed.x - last_x;
                event.trackpad_dy = parsed.y - last_y;
            }
            self.last_touch = Some((parsed.x, parsed.y));
        } else if layout.is_absolute {
            event.is_absolute = true;
            event.abs_x = parsed.x.saturating_sub(parsed.x_min) as u64;
            event.abs_y = parsed.y.saturating_sub(parsed.y_min) as u64;
            event.abs_max_x = parsed.x_max.saturating_sub(parsed.x_min).max(1) as u64;
            event.abs_max_y = parsed.y_max.saturating_sub(parsed.y_min).max(1) as u64;
        } else {
            event.rel_dx = parsed.x;
            event.rel_dy = parsed.y;
        }
        Some(event)
    }
}

pub struct Mouse {
    absolute: Vec<AbsoluteDevice>,
    simple: Vec<SimpleDevice>,
    usb: Vec<UsbPointer>,
    // A UEFI USB mouse commonly installs UsbIo and SimplePointer on the same
    // handle.  Remember claimed handles so it is not read twice.
    claimed_handles: Vec<Handle>,
    scanned_usb_handles: Vec<Handle>,
    motion_filter: MotionFilter,
    motion_settings: MotionSettings,
    trackpad_filter: TrackpadFilter,
    trackpad_settings: TrackpadSettings,
    config_revision: usize,
    trackpad_interpolator: TrackpadInterpolator,
    poll_count: u32,
}

impl Mouse {
    fn empty() -> Self {
        Self {
            absolute: Vec::new(),
            simple: Vec::new(),
            usb: Vec::new(),
            claimed_handles: Vec::new(),
            scanned_usb_handles: Vec::new(),
            motion_filter: MotionFilter::default(),
            motion_settings: load_motion_settings(),
            trackpad_filter: TrackpadFilter::default(),
            trackpad_settings: load_trackpad_settings(),
            config_revision: baram_bsd::config::revision(),
            trackpad_interpolator: TrackpadInterpolator::default(),
            poll_count: 0,
        }
    }

    pub fn get_wait_event() -> Option<uefi::Event> {
        // The main loop also has a 1 ms timer, so one pointer event is enough
        // to reduce latency; every device is still polled on each wakeup.
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
            "Mouse: handles USB={} Absolute={} Simple={}",
            usb_count, abs_count, simple_count
        ));

        let mut mouse = Self::empty();
        mouse.scan_uefi();
        mouse.scan_usb();
        if mouse.device_count() == 0 {
            log_line_str("Mouse: no device yet; hotplug scan remains active");
        } else {
            mouse.log_active_devices();
        }
        // Always retain the object so hot-plugged mice can be discovered.
        Ok(mouse)
    }

    fn device_count(&self) -> usize {
        self.absolute.len() + self.simple.len() + self.usb.len()
    }

    fn log_active_devices(&self) {
        log_line_str(&format!(
            "Mouse: active Absolute={} Simple={} USB-HID={}",
            self.absolute.len(),
            self.simple.len(),
            self.usb.len()
        ));
    }

    fn scan_uefi(&mut self) -> bool {
        let before = self.device_count();

        if let Ok(handles) = boot::find_handles::<AbsolutePointer>() {
            for handle in handles {
                if self.claimed_handles.contains(&handle) {
                    continue;
                }
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
                let _ = pointer.reset(false);
                let mode = pointer.mode();
                let min_x = mode.absolute_min_x;
                let min_y = mode.absolute_min_y;
                let range_x = mode.absolute_max_x.saturating_sub(min_x).max(1);
                let range_y = mode.absolute_max_y.saturating_sub(min_y).max(1);
                self.absolute.push(AbsoluteDevice {
                    pointer,
                    range_x,
                    range_y,
                    min_x,
                    min_y,
                    buttons: 0,
                });
                self.claimed_handles.push(handle);
                log_line_str(&format!(
                    "Mouse: opened AbsolutePointer #{} range=({},{})",
                    self.absolute.len(),
                    range_x,
                    range_y
                ));
            }
        }

        // Do not return after finding an AbsolutePointer.  Convertible laptops
        // commonly expose the touchscreen as AbsolutePointer and the trackpad
        // (plus USB mice) as separate SimplePointer handles.
        if let Ok(handles) = boot::find_handles::<Pointer>() {
            for handle in handles {
                if self.claimed_handles.contains(&handle) {
                    continue;
                }
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
                self.simple.push(SimpleDevice {
                    pointer,
                    buttons: 0,
                });
                self.claimed_handles.push(handle);
                log_line_str(&format!(
                    "Mouse: opened SimplePointer #{}",
                    self.simple.len()
                ));
            }
        }
        self.device_count() != before
    }

    fn scan_usb(&mut self) -> bool {
        let before = self.device_count();
        let Ok(handles) = boot::find_handles::<UsbIo>() else {
            return false;
        };
        for handle in handles {
            if self.claimed_handles.contains(&handle) || self.scanned_usb_handles.contains(&handle)
            {
                continue;
            }
            self.scanned_usb_handles.push(handle);
            if let Some(device) = Self::open_usb_handle(handle) {
                self.usb.push(device);
                self.claimed_handles.push(handle);
            }
        }
        self.device_count() != before
    }

    fn open_usb_handle(handle: Handle) -> Option<UsbPointer> {
        let params = boot::OpenProtocolParams {
            handle,
            agent: boot::image_handle(),
            controller: None,
        };
        let mut io = unsafe {
            boot::open_protocol::<UsbIo>(params, boot::OpenProtocolAttributes::GetProtocol).ok()?
        };
        let interface = io.interface_descriptor().ok()?;
        if interface.interface_class != 3 {
            return None;
        }

        let mut endpoint = None;
        let mut max_packet = 0usize;
        for index in 0..interface.num_endpoints {
            if let Ok(descriptor) = io.endpoint_descriptor(index) {
                if descriptor.endpoint_address & 0x80 != 0 && descriptor.attributes & 0x03 == 3 {
                    endpoint = Some(descriptor.endpoint_address);
                    max_packet = descriptor.max_packet_size as usize;
                    break;
                }
            }
        }
        let endpoint = endpoint?;

        let mut descriptor = vec![0u8; 1024];
        let layouts = if io
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
            return None;
        }

        // The UEFI USB bus driver has already selected a configuration.
        // Reissuing SET_CONFIGURATION here would reset live endpoints.
        let _ = io.control_transfer(
            0x21,
            0x0a,
            0,
            interface.interface_number as u16,
            ControlTransfer::None,
            100,
        );
        if boot_mouse {
            let _ = io.control_transfer(
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
            "Mouse: opened USB HID iface={} ep=0x{:02x} reports={}{}",
            interface.interface_number,
            endpoint,
            layouts.len(),
            if boot_mouse { " boot" } else { "" }
        ));
        Some(UsbPointer {
            io,
            endpoint,
            interface: interface.interface_number,
            report_buf: vec![0; report_len],
            last_report: Vec::new(),
            layouts,
            boot_mouse,
            last_touch: None,
            buttons: 0,
        })
    }

    pub fn is_absolute(&self) -> bool {
        !self.absolute.is_empty()
            || self.usb.iter().any(|device| {
                device.layouts.iter().any(|layout| {
                    layout.is_absolute
                        && !(layout.application_usage_page == 0x0d
                            && layout.application_usage == 0x05)
                })
            })
    }

    /// Compatibility fallback.  Each new absolute event also carries its own
    /// range, which is required when several absolute devices differ.
    pub fn abs_max(&self) -> (u64, u64) {
        self.absolute
            .first()
            .map(|device| (device.range_x, device.range_y))
            .unwrap_or((1, 1))
    }

    pub fn poll(&mut self) -> Option<MouseEvent> {
        self.refresh_settings_if_needed();
        self.poll_count = self.poll_count.wrapping_add(1);
        if self.poll_count % 1000 == 0 {
            let changed = self.scan_uefi() | self.scan_usb();
            if changed {
                self.log_active_devices();
            }
        }

        for index in 0..self.usb.len() {
            let event = self.usb[index].poll();
            if let Some(event) = event {
                return Some(self.finish_event(event));
            }
        }

        // HID trackpads usually report at a much lower cadence than USB
        // mice. Spread each transformed delta over four 1 ms timer ticks so
        // cursor rendering stays continuous without delaying USB mouse data.
        if let Some(event) = self.take_pending_trackpad_event() {
            return Some(event);
        }

        for index in 0..self.absolute.len() {
            let event = {
                let device = &mut self.absolute[index];
                let mut latest = None;
                for _ in 0..32 {
                    match device.pointer.get_state() {
                        Ok(Some(state)) => latest = Some(state),
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
                latest.map(|state| {
                    device.buttons = (state.active_buttons & 0x03) as u8;
                    MouseEvent {
                        abs_x: state.current_x.saturating_sub(device.min_x),
                        abs_y: state.current_y.saturating_sub(device.min_y),
                        abs_max_x: device.range_x,
                        abs_max_y: device.range_y,
                        is_absolute: true,
                        ..event_with_buttons(device.buttons)
                    }
                })
            };
            if let Some(event) = event {
                return Some(self.finish_event(event));
            }
        }

        for index in 0..self.simple.len() {
            let event = {
                let device = &mut self.simple[index];
                let mut event = MouseEvent::default();
                let mut received = false;
                loop {
                    match device.pointer.read_state() {
                        Ok(Some(state)) => {
                            event.rel_dx = event.rel_dx.saturating_add(state.relative_movement[0]);
                            event.rel_dy = event.rel_dy.saturating_add(state.relative_movement[1]);
                            // The last state determines whether the button was
                            // released; OR-ing states can leave clicks stuck.
                            device.buttons =
                                (state.button[0] as u8) | ((state.button[1] as u8) << 1);
                            received = true;
                        }
                        _ => break,
                    }
                }
                if received {
                    let buttons = device.buttons;
                    event.left = buttons & 1 != 0;
                    event.right = buttons & 2 != 0;
                    Some(event)
                } else {
                    None
                }
            };
            if let Some(event) = event {
                return Some(self.finish_event(event));
            }
        }
        None
    }

    fn with_global_buttons(&self, mut event: MouseEvent) -> MouseEvent {
        let mut buttons = 0u8;
        for device in &self.absolute {
            buttons |= device.buttons;
        }
        for device in &self.simple {
            buttons |= device.buttons;
        }
        for device in &self.usb {
            buttons |= device.buttons;
        }
        event.left = buttons & 1 != 0;
        event.right = buttons & 2 != 0;
        event.middle = buttons & 4 != 0;
        event
    }

    fn refresh_settings_if_needed(&mut self) {
        let revision = baram_bsd::config::revision();
        if revision != self.config_revision {
            self.motion_settings = load_motion_settings();
            self.trackpad_settings = load_trackpad_settings();
            self.config_revision = revision;
        }
    }

    fn take_pending_trackpad_event(&mut self) -> Option<MouseEvent> {
        let (dx, dy) = self.trackpad_interpolator.take()?;
        Some(self.with_global_buttons(MouseEvent {
            rel_dx: dx,
            rel_dy: dy,
            is_trackpad: true,
            ..MouseEvent::default()
        }))
    }

    fn finish_event(&mut self, event: MouseEvent) -> MouseEvent {
        let mut event = self.with_global_buttons(event);
        let buttons = (event.left as u8) | ((event.right as u8) << 1) | ((event.middle as u8) << 2);
        if event.is_trackpad {
            let (dx, dy) = self.trackpad_filter.apply(
                event.trackpad_dx,
                event.trackpad_dy,
                buttons,
                &self.trackpad_settings,
            );
            self.trackpad_interpolator.enqueue(dx, dy);
            if let Some(portion) = self.take_pending_trackpad_event() {
                event.rel_dx = portion.rel_dx;
                event.rel_dy = portion.rel_dy;
            } else {
                event.rel_dx = 0;
                event.rel_dy = 0;
            }
            // Keep click state synchronized when switching pointer devices.
            let _ = self
                .motion_filter
                .apply(0, 0, buttons, &self.motion_settings);
        } else {
            let (dx, dy) = self.motion_filter.apply(
                if event.is_absolute { 0 } else { event.rel_dx },
                if event.is_absolute { 0 } else { event.rel_dy },
                buttons,
                &self.motion_settings,
            );
            if !event.is_absolute {
                event.rel_dx = dx;
                event.rel_dy = dy;
            }
        }
        event
    }
}

fn event_with_buttons(buttons: u8) -> MouseEvent {
    MouseEvent {
        left: buttons & 1 != 0,
        right: buttons & 2 != 0,
        middle: buttons & 4 != 0,
        ..MouseEvent::default()
    }
}

pub fn apply_mouse_event(
    cx: &mut i32,
    cy: &mut i32,
    event: &MouseEvent,
    screen_w: usize,
    screen_h: usize,
    fallback_abs_max: (u64, u64),
) -> (i32, i32) {
    if screen_w == 0 || screen_h == 0 {
        return (*cx, *cy);
    }
    let abs_max_x = if event.abs_max_x != 0 {
        event.abs_max_x
    } else {
        fallback_abs_max.0
    };
    let abs_max_y = if event.abs_max_y != 0 {
        event.abs_max_y
    } else {
        fallback_abs_max.1
    };
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
