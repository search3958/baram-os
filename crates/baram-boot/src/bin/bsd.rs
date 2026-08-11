#![no_std]
#![no_main]

extern crate alloc;

use baram_core::subsystem::{KeyEventData, MouseEventData, SubsystemContext, SubsystemExports};

#[no_mangle]
pub static BARAM_SUBSYSTEM_EXPORTS: SubsystemExports = SubsystemExports {
    magic: baram_core::subsystem::SUBSYSTEM_MAGIC,
    version: baram_core::subsystem::SUBSYSTEM_VERSION,
    name: b"bsd\0".as_ptr(),
    init: bsd_init,
    handle_key: bsd_handle_key,
    handle_mouse: bsd_handle_mouse,
    tick: bsd_tick,
    render: bsd_render,
    shutdown: bsd_shutdown,
};

struct BSDState {
    initialized: bool,
}

static mut STATE: Option<BSDState> = None;

extern "C" fn bsd_init(_ctx: *mut SubsystemContext) -> i32 {
    unsafe {
        STATE = Some(BSDState { initialized: true });
    }
    0
}

extern "C" fn bsd_handle_key(_ctx: *mut SubsystemContext, _event: *const KeyEventData) -> i32 {
    0
}

extern "C" fn bsd_handle_mouse(_ctx: *mut SubsystemContext, _event: *const MouseEventData) -> i32 {
    0
}

extern "C" fn bsd_tick(_ctx: *mut SubsystemContext) -> i32 {
    0
}

extern "C" fn bsd_render(_ctx: *mut SubsystemContext) -> i32 {
    0
}

extern "C" fn bsd_shutdown(_ctx: *mut SubsystemContext) {
    unsafe {
        STATE = None;
    }
}

fn bsd_app(_nano: nano_system::NanoSystem) -> uefi::Status {
    uefi::Status::SUCCESS
}

nano_system::nano_entry!(bsd_app);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    nano_system::NanoSystem::panic_report(info)
}
