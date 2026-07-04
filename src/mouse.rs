//! Combined mouse driver: Absolute Pointer (usb-tablet) + Simple Pointer
//! (usb-mouse).
//!
//! We prefer Absolute Pointer because:
//!  1. QEMU's `usb-tablet` device is exposed by AAVMF via this protocol.
//!  2. Absolute coordinates are perfect for a UI cursor (no acceleration,
//!     no relative drift, no edge clamping).
//!  3. `mouse_move` HMP commands actually generate events that the
//!     Absolute Pointer driver picks up (unlike Simple Pointer + usb-mouse,
//!     which often silently drops them).
//!
//! As a fallback we also support Simple Pointer for keyboards-with-
//! trackpad combos on real hardware.
//!
//! Important: we open the protocol ONCE at startup and keep the
//! `ScopedProtocol` alive for the lifetime of the OS.  Re-opening every
//! frame (as the previous version did) was the main reason events were
//! dropped — `open_protocol_exclusive` interferes with the firmware's
//! internal USB polling state.

use crate::absolute_pointer::AbsolutePointer;
use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol};
use uefi::proto::console::pointer::Pointer;

/// One mouse event, normalised to absolute screen coordinates.
#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
pub struct MouseEvent {
    /// Absolute X coordinate (0..=abs_max_x from device mode).
    pub abs_x: u64,
    /// Absolute Y coordinate (0..=abs_max_y from device mode).
    pub abs_y: u64,
    /// Relative X delta (only meaningful for Simple Pointer sources).
    pub rel_dx: i32,
    /// Relative Y delta (only meaningful for Simple Pointer sources).
    pub rel_dy: i32,
    /// True if this event came from an Absolute Pointer device.
    pub is_absolute: bool,
    /// Left mouse button state.
    pub left: bool,
    /// Right mouse button state.
    pub right: bool,
    /// Middle/alt button state (Absolute Pointer exposes a 3rd "active"
    /// button; Simple Pointer only has 2).
    pub middle: bool,
}

/// Owned mouse device — either an Absolute Pointer or a Simple Pointer.
///
/// We hold a `ScopedProtocol` so the protocol stays open for the lifetime
/// of the OS.  We use `GetProtocol` (non-exclusive) instead of
/// `Exclusive` because the latter disconnects the firmware's USB polling
/// driver, which stops events from being delivered.
enum MouseHandle {
    Absolute(ScopedProtocol<AbsolutePointer>, u64, u64),  // ptr, max_x, max_y
    Simple(ScopedProtocol<Pointer>),
}

pub struct Mouse {
    handle: MouseHandle,
}

impl Mouse {
    /// Locate and open the best available mouse device.  Tries Absolute
    /// Pointer first, then Simple Pointer.  Returns `Err` if neither is
    /// present.
    pub fn open() -> Result<Mouse, &'static str> {
        // Try Absolute Pointer first.
        if let Ok(handle) = boot::get_handle_for_protocol::<AbsolutePointer>() {
            // Open with GetProtocol (non-exclusive) so the firmware's
            // USB polling driver keeps running and continues to deliver
            // events via GetState.
            let params = OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            };
            // SAFETY: We keep the ScopedProtocol alive for the lifetime
            // of the OS, so the protocol interface remains valid.  We
            // only call immutable methods (`get_state`, `mode`) plus the
            // documented `reset`.  No conflicting concurrent access
            // because UEFI is single-threaded.
            if let Ok(ptr) = unsafe {
                boot::open_protocol::<AbsolutePointer>(params, OpenProtocolAttributes::GetProtocol)
            } {
                let mode = ptr.mode();
                let max_x = mode.absolute_max_x.max(1);
                let max_y = mode.absolute_max_y.max(1);
                let mut m = Mouse { handle: MouseHandle::Absolute(ptr, max_x, max_y) };
                let _ = m.reset();
                return Ok(m);
            }
        }
        // Fall back to Simple Pointer.
        if let Ok(handle) = boot::get_handle_for_protocol::<Pointer>() {
            let params = OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            };
            // SAFETY: same as above.
            if let Ok(ptr) = unsafe {
                boot::open_protocol::<Pointer>(params, OpenProtocolAttributes::GetProtocol)
            } {
                let mut m = Mouse { handle: MouseHandle::Simple(ptr) };
                let _ = m.reset();
                return Ok(m);
            }
        }
        Err("no mouse device found")
    }

    /// Reset the device (flush queued events).
    pub fn reset(&mut self) -> uefi::Result {
        match &mut self.handle {
            MouseHandle::Absolute(p, _, _) => p.reset(false),
            MouseHandle::Simple(p) => p.reset(false),
        }
    }

    /// Returns true if this mouse is an Absolute Pointer.
    pub fn is_absolute(&self) -> bool {
        matches!(self.handle, MouseHandle::Absolute(..))
    }

    /// Returns the (max_x, max_y) for an absolute pointer, or (0, 0) for
    /// a simple pointer.
    pub fn abs_max(&self) -> (u64, u64) {
        match &self.handle {
            MouseHandle::Absolute(_, mx, my) => (*mx, *my),
            MouseHandle::Simple(_) => (0, 0),
        }
    }

    /// Poll the device.  Drains all queued events and returns the merged
    /// result.  Returns `None` if no events were waiting.
    ///
    /// **Polling strategy**: We call `get_state` (or `read_state`) in a
    /// tight loop.  UEFI returns `NOT_READY` when the queue is empty, so
    /// we break on that.  We *don't* use `check_event` on the
    /// WaitForInput event because on QEMU+AAVMF the event is never
    /// signalled for usb-tablet — only `get_state` returns events.
    pub fn poll(&mut self) -> Option<MouseEvent> {
        match &mut self.handle {
            MouseHandle::Absolute(ptr, max_x, max_y) => {
                let mut acc = MouseEvent::default();
                acc.is_absolute = true;
                let mut got = false;
                // Drain all queued state changes.  We also do one extra
                // `get_state` call after the queue empties, because some
                // firmware coalesces multiple state changes into one
                // final value and only returns it on the next read.
                let mut empty_polls = 0;
                loop {
                    match ptr.get_state() {
                        Ok(Some(state)) => {
                            acc.abs_x = state.current_x;
                            acc.abs_y = state.current_y;
                            if state.active_buttons & 0x1 != 0 { acc.left = true; }
                            if state.active_buttons & 0x2 != 0 { acc.right = true; }
                            got = true;
                            empty_polls = 0;
                        }
                        _ => {
                            empty_polls += 1;
                            if empty_polls >= 1 { break; }
                        }
                    }
                }
                let _ = (max_x, max_y);
                if got { Some(acc) } else { None }
            }
            MouseHandle::Simple(ptr) => {
                let mut acc = MouseEvent::default();
                acc.is_absolute = false;
                let mut got = false;
                loop {
                    match ptr.read_state() {
                        Ok(Some(state)) => {
                            acc.rel_dx += state.relative_movement[0] / 2;
                            acc.rel_dy += state.relative_movement[1] / 2;
                            if state.button[0] { acc.left = true; }
                            if state.button[1] { acc.right = true; }
                            got = true;
                        }
                        _ => break,
                    }
                }
                if got { Some(acc) } else { None }
            }
        }
    }
}
