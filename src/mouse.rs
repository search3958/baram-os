// Mouse driver — USB IO Protocol async interrupt transfer.
// Bypasses firmware's broken mouse driver entirely.

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
}

pub struct Mouse {
    usb_io: Option<boot::ScopedProtocol<UsbIo>>,
    ep_addr: u8,
    report_buf: Vec<u8>,
    // Fallback
    abs: Option<(boot::ScopedProtocol<AbsolutePointer>, u64, u64)>,
    simple: Option<boot::ScopedProtocol<Pointer>>,
}

impl Mouse {
    pub fn open() -> Result<Mouse, &'static str> {
        // Try USB IO Protocol first (direct USB access)
        if let Some(m) = Self::try_usb_io() {
            log_line_str("Mouse: USB IO (direct HID)");
            return Ok(m);
        }

        // Fallback: UEFI protocols
        let mut abs = None;
        let mut simple = None;

        if let Some(h) = boot::find_handles::<AbsolutePointer>().ok().and_then(|h| h.into_iter().next()) {
            if let Ok(mut ptr) = boot::open_protocol_exclusive::<AbsolutePointer>(h) {
                let _ = ptr.reset(false);
                uefi::boot::stall(core::time::Duration::from_millis(50));
                let mx = ptr.mode().absolute_max_x.max(1);
                let my = ptr.mode().absolute_max_y.max(1);
                log_line_str(&format!("  Absolute Pointer: max=({},{})", mx, my));
                abs = Some((ptr, mx, my));
            }
        }
        if let Some(h) = boot::find_handles::<Pointer>().ok().and_then(|h| h.into_iter().next()) {
            if let Ok(mut ptr) = boot::open_protocol_exclusive::<Pointer>(h) {
                let _ = ptr.reset(false);
                uefi::boot::stall(core::time::Duration::from_millis(50));
                log_line_str("  Simple Pointer: ready");
                simple = Some(ptr);
            }
        }

        if abs.is_some() || simple.is_some() {
            Ok(Mouse { usb_io: None, ep_addr: 0, report_buf: vec![0u8; 8], abs, simple })
        } else {
            Err("no mouse device found")
        }
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

            // Get device descriptor
            let mut dev_buf = vec![0u8; 18];
            let _ = usb.control_transfer(0x80, 6, 0x0100, 0, ControlTransfer::DataIn(&mut dev_buf), 5000);

            // Get configuration descriptor
            let mut cfg_buf = vec![0u8; 512];
            let _ = usb.control_transfer(0x80, 6, 0x0200, 0, ControlTransfer::DataIn(&mut cfg_buf), 5000);

            // Find HID interface with interrupt IN endpoint
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
                        // Interface descriptor
                        in_hid = cfg_buf[off + 5] == 3; // HID class
                        iface_num = cfg_buf[off + 2];
                    }
                    5 => {
                        // Endpoint descriptor
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

            // SET_PROTOCOL = Report (1)
            let _ = usb.control_transfer(0x21, 0x0B, 1, iface_num as u16, ControlTransfer::None, 5000);
            // SET_IDLE
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
            None => (32767, 32767), // QEMU usb-tablet の標準解像度
        }
    }

    pub fn poll(&mut self) -> Option<MouseEvent> {
        // Try USB IO interrupt transfer
        if let Some(usb) = &mut self.usb_io {
            if self.ep_addr != 0 {
                let n = usb.sync_interrupt_receive(self.ep_addr, &mut self.report_buf, 10).ok()?;
                if n == 0 { return None; }
                let r = &self.report_buf[..n];

                let mut ev = MouseEvent::default();
                
                // 6バイトの絶対座標（タブレット）レポートとして処理
                if n >= 5 {
                    ev.left = r[0] & 0x01 != 0;
                    ev.right = r[0] & 0x02 != 0;
                    ev.middle = r[0] & 0x04 != 0;
                    
                    // X, Y を 16bit の絶対座標として結合
                    ev.abs_x = u16::from_le_bytes([r[1], r[2]]) as u64;
                    ev.abs_y = u16::from_le_bytes([r[3], r[4]]) as u64;
                    ev.is_absolute = true;
                } 
                // 念のため3バイトの相対座標レポートへのフォールバックも残す
                else if n >= 3 {
                    ev.left = r[0] & 0x01 != 0;
                    ev.right = r[0] & 0x02 != 0;
                    ev.middle = r[0] & 0x04 != 0;
                    ev.rel_dx = r[1] as i8 as i32;
                    ev.rel_dy = r[2] as i8 as i32;
                    ev.is_absolute = false;
                }

                // Debug: log raw bytes
                let mut raw = alloc::string::String::new();
                for &b in r { raw.push_str(&alloc::format!("{:02x} ", b)); }
                log_line_str(&alloc::format!("RAW[{}]: {}", n, raw));
                return Some(ev);
            }
        }

        // Fallback: Absolute Pointer
        if let Some((ptr, _mx, _my)) = &mut self.abs {
            let mut acc = MouseEvent::default();
            acc.is_absolute = true;
            let mut got = false;
            let mut empty = 0;
            loop {
                match ptr.get_state() {
                    Ok(Some(state)) => {
                        acc.abs_x = state.current_x;
                        acc.abs_y = state.current_y;
                        if state.active_buttons & 0x1 != 0 { acc.left = true; }
                        if state.active_buttons & 0x2 != 0 { acc.right = true; }
                        got = true; empty = 0;
                    }
                    _ => { empty += 1; if empty >= 5 { break; } }
                }
            }
            if got { return Some(acc); }
        }

        // Fallback: Simple Pointer
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

pub fn log_line_str(s: &str) {
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
}
