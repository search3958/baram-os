


use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use uefi::boot;
use uefi::proto::usb::io::{ControlTransfer, UsbIo};
use uefi::proto::console::pointer::Pointer;
use crate::absolute_pointer::AbsolutePointer;

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
    report_buf: Vec<u8>,
    
    abs: Option<(boot::ScopedProtocol<AbsolutePointer>, u64, u64)>,
    simple: Option<boot::ScopedProtocol<Pointer>>,
}

impl Mouse {
    pub fn open() -> Result<Mouse, &'static str> {
        log_line_str("Mouse: searching for devices...");

        // List all protocol handles for debugging
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

        // Try UEFI protocols first (more reliable on real hardware)
        let mut abs = None;
        let mut simple = None;

        if let Some(h) = boot::find_handles::<AbsolutePointer>().ok().and_then(|h| h.into_iter().next()) {
            log_line_str("  Trying AbsolutePointer...");
            if let Ok(mut ptr) = boot::open_protocol_exclusive::<AbsolutePointer>(h) {
                let _ = ptr.reset(false);
                uefi::boot::stall(core::time::Duration::from_millis(50));
                let mx = ptr.mode().absolute_max_x.max(1);
                let my = ptr.mode().absolute_max_y.max(1);
                log_line_str(&format!("  AbsolutePointer: max=({},{})", mx, my));
                abs = Some((ptr, mx, my));
            } else {
                log_line_str("  AbsolutePointer: open failed");
            }
        }
        if let Some(h) = boot::find_handles::<Pointer>().ok().and_then(|h| h.into_iter().next()) {
            log_line_str("  Trying SimplePointer...");
            if let Ok(mut ptr) = boot::open_protocol_exclusive::<Pointer>(h) {
                let _ = ptr.reset(false);
                uefi::boot::stall(core::time::Duration::from_millis(50));
                log_line_str("  SimplePointer: ready");
                simple = Some(ptr);
            } else {
                log_line_str("  SimplePointer: open failed");
            }
        }

        if abs.is_some() || simple.is_some() {
            log_line_str("Mouse: using UEFI protocol");
            return Ok(Mouse { usb_io: None, ep_addr: 0, report_buf: vec![0u8; 8], abs, simple });
        }

        // Fallback: direct USB IO (works on QEMU, may not on real hardware)
        log_line_str("Mouse: UEFI protocols unavailable, trying direct USB IO...");
        if let Some(m) = Self::try_usb_io() {
            log_line_str("Mouse: USB IO (direct HID)");
            return Ok(m);
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

            
            let mut dev_buf = vec![0u8; 18];
            let _ = usb.control_transfer(0x80, 6, 0x0100, 0, ControlTransfer::DataIn(&mut dev_buf), 5000);

            
            let mut cfg_buf = vec![0u8; 512];
            let _ = usb.control_transfer(0x80, 6, 0x0200, 0, ControlTransfer::DataIn(&mut cfg_buf), 5000);

            
            let mut off = 0;
            let mut in_hid = false;
            let mut iface_num: u8 = 0;
            let mut intr_ep: Option<u8> = None;
            let mut intr_mps: u16 = 0;

            while off + 2 < cfg_buf.len() {
                let b_len = cfg_buf[off] as usize;
                let b_type = cfg_buf[off + 1];
                if b_len < 2 || off + b_len > cfg_buf.len() { break; }

                match b_type {
                    4 => {
                        
                        in_hid = cfg_buf[off + 5] == 3; 
                        iface_num = cfg_buf[off + 2];
                    }
                    5 => {
                        
                        if b_len >= 7 && in_hid && intr_ep.is_none() {
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

            let ep = match intr_ep {
                Some(e) => e,
                None => continue,
            };

            log_line_str(&format!("  USB IO: HID iface={} ep=0x{:02x} mps={}", iface_num, ep, intr_mps));

            
            let _ = usb.control_transfer(0x21, 0x0B, 1, iface_num as u16, ControlTransfer::None, 5000);
            
            let _ = usb.control_transfer(0x21, 0x0A, 0, iface_num as u16, ControlTransfer::None, 5000);

            let report_buf = vec![0u8; intr_mps as usize];

            return Some(Mouse {
                usb_io: Some(usb),
                ep_addr: ep,
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
        
        if let Some(usb) = &mut self.usb_io {
            if self.ep_addr != 0 {
                let n = usb.sync_interrupt_receive(self.ep_addr, &mut self.report_buf, 10).ok()?;
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
                }
                
                else if n >= 4 {
                    ev.left = r[0] & 0x01 != 0;
                    ev.right = r[0] & 0x02 != 0;
                    ev.middle = r[0] & 0x04 != 0;
                    ev.rel_dx = r[1] as i8 as i32;
                    ev.rel_dy = r[2] as i8 as i32;
                    ev.scroll = r[3] as i8 as i32;
                    ev.is_absolute = false;
                }
                
                else if n >= 3 {
                    ev.left = r[0] & 0x01 != 0;
                    ev.right = r[0] & 0x02 != 0;
                    ev.middle = r[0] & 0x04 != 0;
                    ev.rel_dx = r[1] as i8 as i32;
                    ev.rel_dy = r[2] as i8 as i32;
                    ev.is_absolute = false;
                }

                return Some(ev);
            }
        }

        
        if let Some((ptr, _mx, _my)) = &mut self.abs {
            let mut acc = MouseEvent::default();
            acc.is_absolute = true;
            let mut got = false;

            // Wait for input event first (required by UEFI spec)
            let mut evt = ptr.wait_for_input_event();
            let _ = boot::wait_for_event(&mut [evt]);

            // Drain all pending states
            loop {
                match ptr.get_state() {
                    Ok(Some(state)) => {
                        acc.abs_x = state.current_x;
                        acc.abs_y = state.current_y;
                        if state.active_buttons & 0x1 != 0 { acc.left = true; }
                        if state.active_buttons & 0x2 != 0 { acc.right = true; }
                        got = true;
                    }
                    _ => break,
                }
            }
            if got { return Some(acc); }
        }

        
        if let Some(ptr) = &mut self.simple {
            let mut acc = MouseEvent::default();
            acc.is_absolute = false;
            let mut got = false;
            loop {
                match ptr.read_state() {
                    Ok(Some(state)) => {
                        acc.rel_dx += state.relative_movement[0];
                        acc.rel_dy += state.relative_movement[1];
                        if state.button[0] { acc.left = true; }
                        if state.button[1] { acc.right = true; }
                        got = true;
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
    crate::debug_log::log(s);
}
