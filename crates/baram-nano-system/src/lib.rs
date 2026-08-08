#![no_std]

//! Standalone platform gate started before every BaramOS executable.
//!
//! This crate intentionally depends only on UEFI. It can build when every
//! BaramOS-specific crate has been removed, and its handoff type forms the
//! boundary for a future binary loader and portable application ABI.

use core::ptr;
use core::time::Duration;
use uefi::boot::{self, ScopedProtocol};
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::proto::console::pointer::Pointer;
use uefi::proto::console::text::Input;
use uefi::proto::unsafe_protocol;
use uefi::{boot::TimerTrigger, Status};
use uefi_raw::protocol::console::AbsolutePointerProtocol;
use uefi_raw::table::{boot::EventType, boot::Tpl, runtime::ResetType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NanoColor(pub u32);

impl NanoColor {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const FAILURE_RED: Self = Self::rgb(0xd8, 0x10, 0x20);

    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(0xff00_0000 | ((red as u32) << 16) | ((green as u32) << 8) | blue as u32)
    }

    const fn red(self) -> u8 {
        ((self.0 >> 16) & 0xff) as u8
    }
    const fn green(self) -> u8 {
        ((self.0 >> 8) & 0xff) as u8
    }
    const fn blue(self) -> u8 {
        (self.0 & 0xff) as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NanoDisplayInfo {
    pub width: usize,
    pub height: usize,
    pub stride: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NanoInputInfo {
    pub keyboard_available: bool,
    pub pointer_available: bool,
    pub absolute_pointer_available: bool,
}

/// Capabilities validated by Nano System and handed to an executable.
pub struct NanoSystem {
    pub display: NanoDisplayInfo,
    pub input: NanoInputInfo,
    pub timer_event: uefi::Event,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartError {
    Display(Status),
    Timer,
}

#[derive(Debug)]
#[repr(transparent)]
#[unsafe_protocol(AbsolutePointerProtocol::GUID)]
struct AbsolutePointer(AbsolutePointerProtocol);

impl NanoSystem {
    /// Initialize the freestanding platform layer without BaramOS-specific
    /// configuration, fonts, filesystems or drivers.
    pub fn start(clear_color: NanoColor) -> Result<Self, StartError> {
        let _ = uefi::helpers::init();
        let _ = boot::set_watchdog_timer(0, 0, None);

        let mut graphics = open_display().map_err(StartError::Display)?;
        choose_working_mode(&mut graphics);
        let display = display_info(&graphics);
        fill_display(&mut graphics, clear_color);

        log_phase(uefi::cstr16!("nano: display ready"));
        let keyboard_available = boot::get_handle_for_protocol::<Input>().is_ok();
        if keyboard_available {
            uefi::system::with_stdin(|input| {
                let _ = input.reset(false);
            });
        }
        let pointer_available = boot::get_handle_for_protocol::<Pointer>().is_ok();
        let absolute_pointer_available = boot::get_handle_for_protocol::<AbsolutePointer>().is_ok();
        log_phase(uefi::cstr16!("nano: input capabilities ready"));

        let Some(timer_event) = create_periodic_timer(Duration::from_millis(1)) else {
            fill_display(&mut graphics, NanoColor::FAILURE_RED);
            return Err(StartError::Timer);
        };
        log_phase(uefi::cstr16!("nano: handoff ready"));

        Ok(Self {
            display,
            input: NanoInputInfo {
                keyboard_available,
                pointer_available,
                absolute_pointer_available,
            },
            timer_event,
        })
    }

    /// Common security/platform gate for every executable entry point.
    pub fn launch(application: fn(NanoSystem) -> Status) -> Status {
        match Self::start(NanoColor::BLACK) {
            Ok(nano) => application(nano),
            Err(error) => {
                Self::paint_failure_screen();
                match error {
                    StartError::Display(status) => status,
                    StartError::Timer => Status::DEVICE_ERROR,
                }
            }
        }
    }

    /// Best-effort full-screen failure indicator. If GOP itself is missing,
    /// no implementation can draw a framebuffer error screen.
    pub fn paint_failure_screen() {
        if let Ok(mut graphics) = open_display() {
            fill_display(&mut graphics, NanoColor::FAILURE_RED);
        }
    }

    pub fn cold_reset() -> ! {
        uefi::runtime::reset(ResetType::COLD, Status::SUCCESS, None)
    }
}

/// Declare a UEFI executable whose entry always passes through Nano System.
#[macro_export]
macro_rules! nano_entry {
    ($application:path) => {
        #[uefi::entry]
        fn main() -> uefi::Status {
            $crate::NanoSystem::launch($application)
        }
    };
}

fn open_display() -> Result<ScopedProtocol<GraphicsOutput>, Status> {
    let handle =
        boot::get_handle_for_protocol::<GraphicsOutput>().map_err(|_| Status::UNSUPPORTED)?;
    boot::open_protocol_exclusive::<GraphicsOutput>(handle).map_err(|_| Status::ACCESS_DENIED)
}

fn choose_working_mode(graphics: &mut GraphicsOutput) {
    const TARGET_W: usize = 1280;
    const TARGET_H: usize = 720;
    let mut best_score = usize::MAX;
    let mut best_mode = None;
    for mode in graphics.modes() {
        let (width, height) = mode.info().resolution();
        let score = width.abs_diff(TARGET_W).saturating_mul(TARGET_H)
            + height.abs_diff(TARGET_H).saturating_mul(TARGET_W);
        if score < best_score {
            best_score = score;
            best_mode = Some(mode);
        }
    }
    if let Some(mode) = best_mode {
        let _ = graphics.set_mode(&mode);
    }
}

fn display_info(graphics: &GraphicsOutput) -> NanoDisplayInfo {
    let mode = graphics.current_mode_info();
    let (width, height) = mode.resolution();
    NanoDisplayInfo {
        width,
        height,
        stride: mode.stride(),
    }
}

fn fill_display(graphics: &mut GraphicsOutput, color: NanoColor) {
    let mode = graphics.current_mode_info();
    let (width, height) = mode.resolution();
    let stride = mode.stride();
    let pixel = match mode.pixel_format() {
        PixelFormat::Rgb => {
            ((color.blue() as u32) << 16) | ((color.green() as u32) << 8) | color.red() as u32
        }
        PixelFormat::Bgr => {
            ((color.red() as u32) << 16) | ((color.green() as u32) << 8) | color.blue() as u32
        }
        _ => color.0,
    };
    let mut framebuffer = graphics.frame_buffer();
    let base = framebuffer.as_mut_ptr() as *mut u32;
    for y in 0..height {
        for x in 0..width {
            unsafe { ptr::write_volatile(base.add(y * stride + x), pixel) };
        }
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
    if boot::set_timer(&event, TimerTrigger::Periodic(period)).is_err() {
        let _ = boot::close_event(event);
        return None;
    }
    Some(event)
}

#[no_mangle]
pub unsafe extern "C" fn wcslen(mut string: *const u16) -> usize {
    let start = string;
    while unsafe { *string } != 0 {
        string = unsafe { string.add(1) };
    }
    unsafe { string.offset_from(start) as usize }
}
