#![no_std]
#![no_main]

extern crate alloc;

use uefi::prelude::*;
use baram_core::subsystem::{SubsystemExports, SubsystemContext, KeyEventData, MouseEventData};

#[no_mangle]
pub static BARAM_SUBSYSTEM_EXPORTS: SubsystemExports = SubsystemExports {
    magic: baram_core::subsystem::SUBSYSTEM_MAGIC,
    version: baram_core::subsystem::SUBSYSTEM_VERSION,
    name: b"font\0".as_ptr(),
    init: font_init,
    handle_key: font_handle_key,
    handle_mouse: font_handle_mouse,
    tick: font_tick,
    render: font_render,
    shutdown: font_shutdown,
};

struct FontState {
    initialized: bool,
}

static mut STATE: Option<FontState> = None;

extern "C" fn font_init(_ctx: *mut SubsystemContext) -> i32 {
    unsafe {
        STATE = Some(FontState {
            initialized: true,
        });
    }
    0
}

extern "C" fn font_handle_key(_ctx: *mut SubsystemContext, _event: *const KeyEventData) -> i32 {
    0
}

extern "C" fn font_handle_mouse(_ctx: *mut SubsystemContext, _event: *const MouseEventData) -> i32 {
    0
}

extern "C" fn font_tick(_ctx: *mut SubsystemContext) -> i32 {
    0
}

extern "C" fn font_render(_ctx: *mut SubsystemContext) -> i32 {
    0
}

extern "C" fn font_shutdown(_ctx: *mut SubsystemContext) {
    unsafe {
        STATE = None;
    }
}

#[entry]
fn main() -> uefi::Status {
    uefi::Status::SUCCESS
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
