#![no_std]

//! Standalone platform gate started before every BaramOS executable.
//!
//! This crate intentionally depends only on UEFI. It can build when every
//! BaramOS-specific crate has been removed, and its handoff type forms the
//! boundary for a future binary loader and portable application ABI.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};

use core::ffi::c_void;
use core::mem::ManuallyDrop;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};
use core::time::Duration;
use uefi::boot::{self, ScopedProtocol};
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::proto::console::pointer::Pointer;
use uefi::proto::console::text::{Input, Key};
use uefi::proto::unsafe_protocol;
use uefi::proto::usb::io::{ControlTransfer, UsbIo};
use uefi::{boot::TimerTrigger, Status};
use uefi_raw::protocol::console::AbsolutePointerProtocol;
use uefi_raw::protocol::usb::io::UsbIoProtocol;
use uefi_raw::protocol::usb::UsbTransferStatus;
use uefi_raw::table::{boot::EventType, boot::Tpl, runtime::ResetType};

const MAX_ABSOLUTE_SAMPLES_PER_POLL: usize = 4;
const MAX_SIMPLE_SAMPLES_PER_POLL: usize = 8;

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

/// Live input snapshot owned and updated exclusively by Nano System.
#[derive(Clone, Copy, Debug, Default)]
pub struct NanoInputState {
    pub key_sequence: u64,
    pub pointer_sequence: u64,
    pub last_printable: Option<u8>,
    pub last_scancode: u16,
    pub modifiers: u8,
    pub pointer_dx: i32,
    pub pointer_dy: i32,
    pub pointer_x: u64,
    pub pointer_y: u64,
    pub pointer_max_x: u64,
    pub pointer_max_y: u64,
    pub pointer_is_absolute: bool,
    pub pointer_is_trackpad: bool,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
    pub scroll: i32,
}

/// Capabilities validated by Nano System and handed to an executable.
pub struct NanoSystem {
    pub display: NanoDisplayInfo,
    pub input: NanoInputInfo,
    pub input_state: NanoInputState,
    pub timer_event: Option<uefi::Event>,
    simple_pointers: Vec<BasicSimpleDevice>,
    absolute_pointers: Vec<BasicAbsoluteDevice>,
    usb_pointers: Vec<BasicUsbPointer>,
    shift_key: u8,
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

struct BasicAbsoluteDevice {
    pointer: ScopedProtocol<AbsolutePointer>,
    min_x: u64,
    min_y: u64,
    range_x: u64,
    range_y: u64,
    last_state: Option<(u64, u64, u32)>,
}

struct BasicSimpleDevice {
    pointer: ScopedProtocol<Pointer>,
    buttons: u8,
}

struct BasicUsbPointer {
    io: ScopedProtocol<UsbIo>,
    endpoint: u8,
    report_len: usize,
    state: ManuallyDrop<Box<AsyncUsbPointerState>>,
    async_active: bool,
    buttons: u8,
}

struct AsyncUsbPointerState {
    dx: AtomicI32,
    dy: AtomicI32,
    scroll: AtomicI32,
    buttons: AtomicU8,
    pending: AtomicBool,
}

impl AsyncUsbPointerState {
    fn new() -> Self {
        Self {
            dx: AtomicI32::new(0),
            dy: AtomicI32::new(0),
            scroll: AtomicI32::new(0),
            buttons: AtomicU8::new(0),
            pending: AtomicBool::new(false),
        }
    }

    fn take(&self) -> Option<(i32, i32, i32, u8)> {
        if !self.pending.swap(false, Ordering::AcqRel) {
            return None;
        }
        Some((
            self.dx.swap(0, Ordering::AcqRel),
            self.dy.swap(0, Ordering::AcqRel),
            self.scroll.swap(0, Ordering::AcqRel),
            self.buttons.load(Ordering::Acquire),
        ))
    }
}

impl Drop for BasicUsbPointer {
    fn drop(&mut self) {
        if self.async_active {
            let protocol = (&mut *self.io as *mut UsbIo).cast::<UsbIoProtocol>();
            let status = unsafe {
                ((*protocol).async_interrupt_transfer)(
                    protocol,
                    self.endpoint,
                    false.into(),
                    0,
                    self.report_len,
                    usb_pointer_callback,
                    (&mut **self.state as *mut AsyncUsbPointerState).cast(),
                )
            };
            // A failed cancellation means firmware may still invoke the
            // callback. Leak this tiny state block instead of leaving a
            // dangling callback context.
            if status.is_success() {
                unsafe { ManuallyDrop::drop(&mut self.state) };
            }
        } else {
            unsafe { ManuallyDrop::drop(&mut self.state) };
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NanoBasicPointerEvent {
    pub dx: i32,
    pub dy: i32,
    pub absolute: Option<(u64, u64, u64, u64)>,
}

/// Retains GOP while the standalone Nano diagnostic is active. The kernel
/// path never constructs this type, so no diagnostic rendering survives a
/// handoff to an application.
pub struct NanoPointerTestDisplay {
    graphics: ScopedProtocol<GraphicsOutput>,
    framebuffer: *mut u32,
    width: usize,
    height: usize,
    stride: usize,
    background_pixel: u32,
    white_pixel: u32,
    yellow_pixel: u32,
}

impl NanoPointerTestDisplay {
    pub fn initialize(&mut self, x: usize, y: usize, yellow: bool) {
        fill_display(&mut self.graphics, NanoColor::rgb(0x00, 0x00, 0x44));
        fill_rect(&mut self.graphics, x, y, 16, 16, pointer_test_color(yellow));
    }

    pub fn update(&mut self, old_x: usize, old_y: usize, x: usize, y: usize, yellow: bool) {
        fill_rect_raw(
            self.framebuffer,
            self.width,
            self.height,
            self.stride,
            old_x,
            old_y,
            16,
            16,
            self.background_pixel,
        );
        fill_rect_raw(
            self.framebuffer,
            self.width,
            self.height,
            self.stride,
            x,
            y,
            16,
            16,
            if yellow {
                self.yellow_pixel
            } else {
                self.white_pixel
            },
        );
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NanoKeyEvent {
    pub printable: Option<u8>,
    pub scancode: u16,
    pub modifiers: u8,
    pub raw_key: u8,
}

impl AbsolutePointer {
    fn get_state(&mut self) -> Option<uefi_raw::protocol::console::AbsolutePointerState> {
        let mut state = uefi_raw::protocol::console::AbsolutePointerState::default();
        match unsafe { (self.0.get_state)(&self.0, &mut state) } {
            Status::NOT_READY => None,
            status if status.is_success() => Some(state),
            _ => None,
        }
    }

    fn mode(&self) -> &uefi_raw::protocol::console::AbsolutePointerMode {
        unsafe { &*self.0.mode }
    }
}

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

        // The timer is a compatibility service for the full OS. Input polling
        // never waits for it, and a broken firmware timer must not prevent Nano
        // from starting or make pointer latency depend on timer delivery.
        let timer_event = create_periodic_timer(Duration::from_millis(1));
        log_phase(uefi::cstr16!("nano: handoff ready"));

        let simple_pointers = open_simple_pointers();
        let absolute_pointers = open_absolute_pointers();
        let usb_pointers = open_usb_pointers();

        Ok(Self {
            display,
            input: NanoInputInfo {
                keyboard_available,
                pointer_available,
                absolute_pointer_available,
            },
            input_state: NanoInputState::default(),
            timer_event,
            simple_pointers,
            absolute_pointers,
            usb_pointers,
            shift_key: 0,
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

    pub fn take_timer_event(&mut self) -> Option<uefi::Event> {
        self.timer_event.take()
    }

    pub fn poll_keyboard(&mut self) -> Option<NanoKeyEvent> {
        let key = uefi::system::with_stdin(|input| input.read_key().ok().flatten());
        if let Some(key) = key {
            self.input_state.key_sequence = self.input_state.key_sequence.wrapping_add(1);
            let event = match key {
                Key::Printable(character) => {
                    let value: u16 = character.into();
                    NanoKeyEvent {
                        printable: u8::try_from(value).ok(),
                        ..NanoKeyEvent::default()
                    }
                }
                Key::Special(scancode) => NanoKeyEvent {
                    scancode: scancode.0,
                    raw_key: u8::try_from(scancode.0).unwrap_or(0),
                    ..NanoKeyEvent::default()
                },
            };
            self.input_state.last_printable = event.printable;
            self.input_state.last_scancode = event.scancode;
            self.input_state.modifiers = event.modifiers;
            Some(event)
        } else {
            None
        }
    }

    pub fn poll_pointer(&mut self) -> Option<NanoBasicPointerEvent> {
        for pointer in &mut self.usb_pointers {
            if let Some((dx, dy, scroll, buttons)) = pointer.state.take() {
                if dx == 0 && dy == 0 && scroll == 0 && buttons == pointer.buttons {
                    continue;
                }
                pointer.buttons = buttons;
                self.input_state.pointer_sequence =
                    self.input_state.pointer_sequence.wrapping_add(1);
                self.input_state.pointer_dx = dx;
                self.input_state.pointer_dy = dy;
                self.input_state.scroll = scroll;
                self.input_state.pointer_is_absolute = false;
                self.input_state.pointer_is_trackpad = false;
                self.input_state.left = buttons & 1 != 0;
                self.input_state.right = buttons & 2 != 0;
                self.input_state.middle = buttons & 4 != 0;
                return Some(NanoBasicPointerEvent {
                    dx,
                    dy,
                    absolute: None,
                });
            }
        }
        for device in &mut self.absolute_pointers {
            // A tiny bounded drain discards stale firmware samples without
            // letting an always-ready implementation monopolize the CPU.
            let mut latest = None;
            for _ in 0..MAX_ABSOLUTE_SAMPLES_PER_POLL {
                match device.pointer.get_state() {
                    Some(state) => latest = Some(state),
                    _ => break,
                }
            }
            if let Some(state) = latest {
                let x = state
                    .current_x
                    .saturating_sub(device.min_x)
                    .min(device.range_x);
                let y = state
                    .current_y
                    .saturating_sub(device.min_y)
                    .min(device.range_y);
                let state_key = (x, y, state.active_buttons);
                if device.last_state == Some(state_key) {
                    continue;
                }
                device.last_state = Some(state_key);
                self.input_state.pointer_sequence =
                    self.input_state.pointer_sequence.wrapping_add(1);
                self.input_state.pointer_x = x;
                self.input_state.pointer_y = y;
                self.input_state.pointer_max_x = device.range_x;
                self.input_state.pointer_max_y = device.range_y;
                self.input_state.pointer_is_absolute = true;
                self.input_state.pointer_is_trackpad = true;
                self.input_state.left = state.active_buttons & 1 != 0;
                self.input_state.right = state.active_buttons & 2 != 0;
                return Some(NanoBasicPointerEvent {
                    absolute: Some((x, y, device.range_x, device.range_y)),
                    ..NanoBasicPointerEvent::default()
                });
            }
        }
        for device in &mut self.simple_pointers {
            // Bound the drain so malformed firmware cannot hold Nano inside a
            // protocol call forever. Eight reports still coalesce a short
            // backlog into one cursor update.
            let mut raw_dx = 0i32;
            let mut raw_dy = 0i32;
            let mut scroll = 0i32;
            let mut buttons = 0u8;
            let mut received = false;
            for _ in 0..MAX_SIMPLE_SAMPLES_PER_POLL {
                match device.pointer.read_state() {
                    Ok(Some(state)) => {
                        raw_dx = raw_dx.saturating_add(state.relative_movement[0]);
                        raw_dy = raw_dy.saturating_add(state.relative_movement[1]);
                        scroll = scroll.saturating_add(state.relative_movement[2]);
                        buttons = (state.button[0] as u8) | ((state.button[1] as u8) << 1);
                        received = true;
                    }
                    _ => break,
                }
            }
            if received {
                if raw_dx == 0 && raw_dy == 0 && scroll == 0 && buttons == device.buttons {
                    continue;
                }
                device.buttons = buttons;
                self.input_state.pointer_sequence =
                    self.input_state.pointer_sequence.wrapping_add(1);
                self.input_state.pointer_dx = raw_dx;
                self.input_state.pointer_dy = raw_dy;
                self.input_state.scroll = scroll;
                self.input_state.pointer_is_absolute = false;
                self.input_state.pointer_is_trackpad = false;
                self.input_state.left = buttons & 1 != 0;
                self.input_state.right = buttons & 2 != 0;
                self.input_state.middle = false;
                return Some(NanoBasicPointerEvent {
                    dx: raw_dx,
                    dy: raw_dy,
                    absolute: None,
                });
            }
        }
        None
    }

    pub fn begin_pointer_test() -> Result<NanoPointerTestDisplay, Status> {
        let mut graphics = open_display()?;
        let mode = graphics.current_mode_info();
        let (width, height) = mode.resolution();
        let stride = mode.stride();
        let format = mode.pixel_format();
        let background_pixel = encode_pixel(format, NanoColor::rgb(0x00, 0x00, 0x44));
        let white_pixel = encode_pixel(format, NanoColor::rgb(0xff, 0xff, 0xff));
        let yellow_pixel = encode_pixel(format, NanoColor::rgb(0xff, 0xff, 0x00));
        let framebuffer = graphics.frame_buffer().as_mut_ptr() as *mut u32;
        Ok(NanoPointerTestDisplay {
            graphics,
            framebuffer,
            width,
            height,
            stride,
            background_pixel,
            white_pixel,
            yellow_pixel,
        })
    }

    pub fn pointer_abs_max(&self) -> (u64, u64) {
        self.absolute_pointers
            .first()
            .map(|pointer| (pointer.range_x, pointer.range_y))
            .unwrap_or((1, 1))
    }

    pub fn key_is_held(&self, _code: u8) -> bool {
        false
    }

    pub fn shift_held(&self) -> bool {
        self.input_state.modifiers & 0x22 != 0
    }

    pub fn ctrl_or_cmd_held(&self) -> bool {
        self.input_state.modifiers & 0x11 != 0
    }

    pub fn set_shift_key(&mut self, key: u8) {
        self.shift_key = key;
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

fn open_usb_pointers() -> Vec<BasicUsbPointer> {
    let mut pointers = Vec::new();
    let Ok(handles) = boot::find_handles::<UsbIo>() else {
        return pointers;
    };
    for handle in handles {
        let params = boot::OpenProtocolParams {
            handle,
            agent: boot::image_handle(),
            controller: None,
        };
        let Ok(mut io) = (unsafe {
            boot::open_protocol::<UsbIo>(params, boot::OpenProtocolAttributes::GetProtocol)
        }) else {
            continue;
        };
        let Ok(interface) = io.interface_descriptor() else {
            continue;
        };
        if interface.interface_class != 3
            || interface.interface_subclass != 1
            || interface.interface_protocol != 2
        {
            continue;
        }
        let mut endpoint = None;
        let mut packet_size = 4usize;
        for index in 0..interface.num_endpoints {
            if let Ok(descriptor) = io.endpoint_descriptor(index) {
                if descriptor.endpoint_address & 0x80 != 0 && descriptor.attributes & 0x03 == 3 {
                    endpoint = Some(descriptor.endpoint_address);
                    packet_size = (descriptor.max_packet_size as usize).clamp(4, 64);
                    break;
                }
            }
        }
        let Some(endpoint) = endpoint else {
            continue;
        };
        let _ = io.control_transfer(
            0x21,
            0x0a,
            0,
            interface.interface_number as u16,
            ControlTransfer::None,
            100,
        );
        let _ = io.control_transfer(
            0x21,
            0x0b,
            0,
            interface.interface_number as u16,
            ControlTransfer::None,
            100,
        );
        let mut pointer = BasicUsbPointer {
            io,
            endpoint,
            report_len: packet_size,
            state: ManuallyDrop::new(Box::new(AsyncUsbPointerState::new())),
            async_active: false,
            buttons: 0,
        };
        let protocol = (&mut *pointer.io as *mut UsbIo).cast::<UsbIoProtocol>();
        let status = unsafe {
            ((*protocol).async_interrupt_transfer)(
                protocol,
                endpoint,
                true.into(),
                1,
                packet_size,
                usb_pointer_callback,
                (&mut **pointer.state as *mut AsyncUsbPointerState).cast(),
            )
        };
        if status.is_success() {
            pointer.async_active = true;
            pointers.push(pointer);
        }
    }
    pointers
}

fn open_simple_pointers() -> Vec<BasicSimpleDevice> {
    let mut pointers = Vec::new();
    if let Ok(handles) = boot::find_handles::<Pointer>() {
        for handle in handles {
            if let Ok(pointer) = boot::open_protocol_exclusive::<Pointer>(handle) {
                pointers.push(BasicSimpleDevice {
                    pointer,
                    buttons: 0,
                });
            }
        }
    }
    pointers
}

fn open_absolute_pointers() -> Vec<BasicAbsoluteDevice> {
    let mut pointers = Vec::new();
    if let Ok(handles) = boot::find_handles::<AbsolutePointer>() {
        for handle in handles {
            if let Ok(pointer) = boot::open_protocol_exclusive::<AbsolutePointer>(handle) {
                let mode = pointer.mode();
                pointers.push(BasicAbsoluteDevice {
                    min_x: mode.absolute_min_x,
                    min_y: mode.absolute_min_y,
                    range_x: mode
                        .absolute_max_x
                        .saturating_sub(mode.absolute_min_x)
                        .max(1),
                    range_y: mode
                        .absolute_max_y
                        .saturating_sub(mode.absolute_min_y)
                        .max(1),
                    last_state: None,
                    pointer,
                });
            }
        }
    }
    pointers
}

unsafe extern "efiapi" fn usb_pointer_callback(
    data: *mut c_void,
    data_length: usize,
    context: *mut c_void,
    status: UsbTransferStatus,
) -> Status {
    if data.is_null() || context.is_null() || data_length < 3 || !status.is_empty() {
        return Status::SUCCESS;
    }
    let report = data.cast::<u8>();
    let state = unsafe { &*context.cast::<AsyncUsbPointerState>() };
    let buttons = unsafe { *report };
    let dx = unsafe { *report.add(1) } as i8 as i32;
    let dy = unsafe { *report.add(2) } as i8 as i32;
    let scroll = if data_length > 3 {
        (unsafe { *report.add(3) }) as i8 as i32
    } else {
        0
    };
    state.dx.fetch_add(dx, Ordering::Relaxed);
    state.dy.fetch_add(dy, Ordering::Relaxed);
    state.scroll.fetch_add(scroll, Ordering::Relaxed);
    state.buttons.store(buttons, Ordering::Relaxed);
    state.pending.store(true, Ordering::Release);
    Status::SUCCESS
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

fn pointer_test_color(yellow: bool) -> NanoColor {
    if yellow {
        NanoColor::rgb(0xff, 0xff, 0x00)
    } else {
        NanoColor::rgb(0xff, 0xff, 0xff)
    }
}

fn encode_pixel(format: PixelFormat, color: NanoColor) -> u32 {
    match format {
        PixelFormat::Rgb => {
            ((color.blue() as u32) << 16) | ((color.green() as u32) << 8) | color.red() as u32
        }
        PixelFormat::Bgr => {
            ((color.red() as u32) << 16) | ((color.green() as u32) << 8) | color.blue() as u32
        }
        _ => color.0,
    }
}

fn fill_rect_raw(
    framebuffer: *mut u32,
    screen_width: usize,
    screen_height: usize,
    stride: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    pixel: u32,
) {
    let row = [pixel; 16];
    let copy_width = width.min(16).min(screen_width.saturating_sub(x));
    if copy_width == 0 {
        return;
    }
    for py in y..y.saturating_add(height).min(screen_height) {
        unsafe {
            ptr::copy_nonoverlapping(row.as_ptr(), framebuffer.add(py * stride + x), copy_width)
        };
    }
}

fn fill_rect(
    graphics: &mut GraphicsOutput,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: NanoColor,
) {
    let mode = graphics.current_mode_info();
    let (screen_width, screen_height) = mode.resolution();
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
    for py in y..y.saturating_add(height).min(screen_height) {
        for px in x..x.saturating_add(width).min(screen_width) {
            unsafe { ptr::write_volatile(base.add(py * stride + px), pixel) };
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
