#![no_std]

//! Minimal hardware-owning layer started before the BaramOS kernel.
//!
//! `NanoSystem` deliberately contains only the facilities required to bring a
//! kernel up: a framebuffer, keyboard, pointing device, timer and reset.  It
//! remains a library for now, so the nano system and kernel share one UEFI
//! image while keeping an ownership boundary suitable for a later binary
//! loader/handoff.

use baram_core::{Color, Screen};
use baram_iokit::keyboard::Keyboard;
use baram_iokit::mouse::Mouse;
use core::time::Duration;
use uefi::{boot, Status};
use uefi_raw::table::{boot::EventType, boot::Tpl, runtime::ResetType};

/// Hardware resources transferred from the nano system to the main kernel.
pub struct NanoSystem {
    pub screen: Screen,
    pub keyboard: Keyboard,
    pub mouse: Option<Mouse>,
    pub timer_event: Option<uefi::Event>,
    pub mouse_wait_event: Option<uefi::Event>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartError {
    Display(Status),
}

impl NanoSystem {
    /// Initialize the minimum platform services and acquire the base drivers.
    ///
    /// The framebuffer is painted before input discovery, so even a machine
    /// that stalls while probing USB has already left the firmware screen.
    pub fn start(clear_color: Color) -> Result<Self, StartError> {
        let _ = uefi::helpers::init();
        let _ = boot::set_watchdog_timer(0, 0, None);

        let mut screen = Screen::take().map_err(StartError::Display)?;
        screen.clear(clear_color);

        // Reset the firmware console input state before claiming input
        // protocols. Mouse::open resets each UEFI pointer protocol it claims.
        log_phase(uefi::cstr16!("nano: reset keyboard"));
        Keyboard::reset();
        log_phase(uefi::cstr16!("nano: open mouse"));
        let mouse = Mouse::open_with_defaults().ok();
        log_phase(uefi::cstr16!("nano: mouse ready"));
        #[cfg(not(target_arch = "aarch64"))]
        let mouse_wait_event = Mouse::get_wait_event();
        // AAVMF can block when the already-open AbsolutePointer protocol is
        // opened a second time just to retrieve its wait event. The 1 ms
        // periodic timer still polls every pointer with equivalent behavior.
        #[cfg(target_arch = "aarch64")]
        let mouse_wait_event = None;
        log_phase(uefi::cstr16!("nano: open keyboard"));
        #[cfg(not(target_arch = "aarch64"))]
        let keyboard = Keyboard::open_with_shift_key(0);
        #[cfg(target_arch = "aarch64")]
        let keyboard = Keyboard::open_firmware_with_shift_key(0);
        log_phase(uefi::cstr16!("nano: keyboard ready"));
        let timer_event = create_periodic_timer(Duration::from_millis(1));
        log_phase(uefi::cstr16!("nano: timer ready"));

        Ok(Self {
            screen,
            keyboard,
            mouse,
            timer_event,
            mouse_wait_event,
        })
    }

    /// Restart the machine through the firmware runtime service.
    pub fn cold_reset() -> ! {
        uefi::runtime::reset(ResetType::COLD, Status::SUCCESS, None)
    }
}

fn log_phase(message: &uefi::CStr16) {
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.output_string(message);
        let _ = stdout.output_string(uefi::cstr16!("\r\n"));
    });
}

fn create_periodic_timer(period: Duration) -> Option<uefi::Event> {
    let event = unsafe { boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None).ok()? };
    if boot::set_timer(&event, boot::TimerTrigger::Periodic(period)).is_err() {
        let _ = boot::close_event(event);
        return None;
    }
    Some(event)
}

// LLVM may lower optimized UTF-16 length scans in uefi-rs to the C runtime
// symbol `wcslen`. UEFI has no libc, so the base layer provides that one
// freestanding runtime primitive for release builds.
#[no_mangle]
pub unsafe extern "C" fn wcslen(mut string: *const u16) -> usize {
    let start = string;
    while unsafe { *string } != 0 {
        string = unsafe { string.add(1) };
    }
    unsafe { string.offset_from(start) as usize }
}
