//! UEFI Absolute Pointer Protocol wrapper.
//!
//! The Absolute Pointer Protocol is exposed by absolute pointing devices
//! such as USB tablets and touchscreens.  Unlike the Simple Pointer
//! Protocol (which returns relative deltas), Absolute Pointer returns
//! the current (x, y, z) position directly — perfect for a UI cursor.
//!
//! In QEMU, the `usb-tablet` device is exposed by AAVMF via this
//! protocol, so mouse movement works reliably without the relative-
//! tracking quirks that plague `usb-mouse` + Simple Pointer.
//!
//! `uefi-rs` 0.38 doesn't ship a safe wrapper for this protocol, so we
//! use the raw `uefi_raw::protocol::console::AbsolutePointerProtocol`
//! struct directly and wrap it in a small safe API.

use alloc::vec::Vec;
use uefi::boot;
use uefi::prelude::*;  // brings StatusExt into scope
use uefi::proto::unsafe_protocol;
use uefi_raw::protocol::console::{
    AbsolutePointerMode, AbsolutePointerProtocol, AbsolutePointerState,
};
use uefi::{Event, Status};

/// Safe wrapper around the raw Absolute Pointer Protocol.
///
/// `#[unsafe_protocol(GUID)]` generates the `Identify` impl that lets
/// `boot::get_handle_for_protocol` / `boot::open_protocol_exclusive`
/// find devices exposing this protocol.
#[derive(Debug)]
#[repr(transparent)]
#[unsafe_protocol(AbsolutePointerProtocol::GUID)]
pub struct AbsolutePointer(AbsolutePointerProtocol);

impl AbsolutePointer {
    /// Reset the device.
    pub fn reset(&mut self, extended_verification: bool) -> uefi::Result {
        unsafe { (self.0.reset)(&mut self.0, extended_verification.into()) }.to_result()
    }

    /// Read the current absolute pointer state.  Returns `Ok(None)` when
    /// no state change has occurred since the last call (UEFI `NOT_READY`).
    pub fn get_state(&mut self) -> uefi::Result<Option<AbsolutePointerState>> {
        let mut state = AbsolutePointerState::default();
        match unsafe { (self.0.get_state)(&self.0, &mut state) } {
            Status::NOT_READY => Ok(None),
            other => other.to_result_with_val(|| Some(state)),
        }
    }

    /// Returns the WaitForInput event.  Use with `boot::check_event` to
    /// poll for new input without blocking.
    pub fn wait_for_input_event(&self) -> Event {
        // SAFETY: `wait_for_input` is a valid event handle produced by
        // the firmware when the protocol was installed.  We clone the
        // `Event` (which is just a NonNull wrapper) — the underlying
        // event is owned by the protocol and remains valid as long as
        // the protocol is open.
        unsafe { Event::from_ptr(self.0.wait_for_input) }
            .expect("AbsolutePointer wait_for_input event was null")
    }

    /// Mode information (min/max X/Y/Z, attributes).
    pub fn mode(&self) -> &AbsolutePointerMode {
        // SAFETY: `mode` is a valid pointer for the lifetime of the protocol.
        unsafe { &*self.0.mode }
    }
}

/// Helper: are any Absolute Pointer devices present?
pub fn absolute_pointer_present() -> bool {
    boot::get_handle_for_protocol::<AbsolutePointer>().is_ok()
}

/// Find every handle that exposes Absolute Pointer.
pub fn find_absolute_pointer_handles() -> uefi::Result<Vec<uefi::Handle>> {
    boot::find_handles::<AbsolutePointer>()
}
