#![no_std]
#![no_main]

extern crate alloc;

use baram_core::subsystem::{KeyEventData, MouseEventData, SubsystemContext, SubsystemExports};

#[no_mangle]
pub static BARAM_SUBSYSTEM_EXPORTS: SubsystemExports = SubsystemExports {
    magic: baram_core::subsystem::SUBSYSTEM_MAGIC,
    version: baram_core::subsystem::SUBSYSTEM_VERSION,
    name: b"iokit\0".as_ptr(),
    init: iokit_init,
    handle_key: iokit_handle_key,
    handle_mouse: iokit_handle_mouse,
    tick: iokit_tick,
    render: iokit_render,
    shutdown: iokit_shutdown,
};

struct IOKitState {
    initialized: bool,
}

static mut STATE: Option<IOKitState> = None;

extern "C" fn iokit_init(_ctx: *mut SubsystemContext) -> i32 {
    unsafe {
        STATE = Some(IOKitState { initialized: true });
    }
    0
}

extern "C" fn iokit_handle_key(_ctx: *mut SubsystemContext, _event: *const KeyEventData) -> i32 {
    0
}

extern "C" fn iokit_handle_mouse(
    _ctx: *mut SubsystemContext,
    _event: *const MouseEventData,
) -> i32 {
    0
}

extern "C" fn iokit_tick(_ctx: *mut SubsystemContext) -> i32 {
    0
}

extern "C" fn iokit_render(_ctx: *mut SubsystemContext) -> i32 {
    0
}

extern "C" fn iokit_shutdown(_ctx: *mut SubsystemContext) {
    unsafe {
        STATE = None;
    }
}

fn iokit_app(_nano: nano_system::NanoSystem) -> uefi::Status {
    uefi::Status::SUCCESS
}

nano_system::nano_entry!(iokit_app);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
