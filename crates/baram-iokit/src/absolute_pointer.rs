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
use uefi::prelude::*;
use uefi::proto::unsafe_protocol;
use uefi::{Event, Status};
use uefi_raw::protocol::console::{
    AbsolutePointerMode, AbsolutePointerProtocol, AbsolutePointerState,
};

#[derive(Debug)]
#[repr(transparent)]
#[unsafe_protocol(AbsolutePointerProtocol::GUID)]
pub struct AbsolutePointer(AbsolutePointerProtocol);

impl AbsolutePointer {
    pub fn reset(&mut self, extended_verification: bool) -> uefi::Result {
        unsafe { (self.0.reset)(&mut self.0, extended_verification.into()) }.to_result()
    }

    pub fn get_state(&mut self) -> uefi::Result<Option<AbsolutePointerState>> {
        let mut state = AbsolutePointerState::default();
        match unsafe { (self.0.get_state)(&self.0, &mut state) } {
            Status::NOT_READY => Ok(None),
            other => other.to_result_with_val(|| Some(state)),
        }
    }

    pub fn wait_for_input_event(&self) -> Event {
        unsafe { Event::from_ptr(self.0.wait_for_input) }
            .expect("AbsolutePointer wait_for_input event was null")
    }

    pub fn mode(&self) -> &AbsolutePointerMode {
        unsafe { &*self.0.mode }
    }
}

pub fn absolute_pointer_present() -> bool {
    boot::get_handle_for_protocol::<AbsolutePointer>().is_ok()
}

pub fn find_absolute_pointer_handles() -> uefi::Result<Vec<uefi::Handle>> {
    boot::find_handles::<AbsolutePointer>()
}
