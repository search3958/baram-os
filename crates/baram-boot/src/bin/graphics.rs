#![no_std]
#![no_main]

extern crate alloc;

use baram_core::subsystem::{SubsystemExports, SubsystemContext, KeyEventData, MouseEventData};

#[no_mangle]
pub static BARAM_SUBSYSTEM_EXPORTS: SubsystemExports = SubsystemExports {
    magic: baram_core::subsystem::SUBSYSTEM_MAGIC,
    version: baram_core::subsystem::SUBSYSTEM_VERSION,
    name: b"graphics\0".as_ptr(),
    init: gfx_init,
    handle_key: gfx_handle_key,
    handle_mouse: gfx_handle_mouse,
    tick: gfx_tick,
    render: gfx_render,
    shutdown: gfx_shutdown,
};

struct GraphicsState {
    initialized: bool,
}

static mut STATE: Option<GraphicsState> = None;

extern "C" fn gfx_init(_ctx: *mut SubsystemContext) -> i32 {
    unsafe {
        STATE = Some(GraphicsState {
            initialized: true,
        });
    }
    0
}

extern "C" fn gfx_handle_key(_ctx: *mut SubsystemContext, _event: *const KeyEventData) -> i32 {
    0
}

extern "C" fn gfx_handle_mouse(_ctx: *mut SubsystemContext, _event: *const MouseEventData) -> i32 {
    0
}

extern "C" fn gfx_tick(_ctx: *mut SubsystemContext) -> i32 {
    0
}

extern "C" fn gfx_render(_ctx: *mut SubsystemContext) -> i32 {
    0
}

extern "C" fn gfx_shutdown(_ctx: *mut SubsystemContext) {
    unsafe {
        STATE = None;
    }
}

fn graphics_app(_nano: baram_nano_system::NanoSystem) -> uefi::Status {
    uefi::Status::SUCCESS
}

baram_nano_system::nano_entry!(graphics_app);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
