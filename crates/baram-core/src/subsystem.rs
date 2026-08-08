#![no_std]

use core::ffi::c_void;

pub const SUBSYSTEM_MAGIC: u32 = 0x4241524D;
pub const SUBSYSTEM_VERSION: u32 = 1;

#[repr(C)]
pub struct KeyEventData {
    pub printable: u8,
    pub has_printable: u8,
    pub scancode: u16,
    pub modifiers: u8,
    pub raw_key: u8,
    pub _pad: [u8; 2],
}

#[repr(C)]
pub struct MouseEventData {
    pub abs_x: u64,
    pub abs_y: u64,
    pub rel_dx: i32,
    pub rel_dy: i32,
    pub left: u8,
    pub right: u8,
    pub middle: u8,
    pub scroll: i32,
    pub is_absolute: u8,
    pub _pad: [u8; 3],
}

#[repr(C)]
pub struct FramebufferInfo {
    pub base: *mut u32,
    pub width: u32,
    pub height: u32,
}

unsafe impl Send for FramebufferInfo {}
unsafe impl Sync for FramebufferInfo {}

#[repr(C)]
pub struct SubsystemContext {
    pub magic: u32,
    pub version: u32,
    pub fb: FramebufferInfo,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub screen_w: u32,
    pub screen_h: u32,
    pub tick_count: u64,
    pub fps: u32,
    pub userdata: *mut c_void,
}

unsafe impl Send for SubsystemContext {}
unsafe impl Sync for SubsystemContext {}

#[repr(C)]
pub struct SubsystemExports {
    pub magic: u32,
    pub version: u32,
    pub name: *const u8,
    pub init: extern "C" fn(*mut SubsystemContext) -> i32,
    pub handle_key: extern "C" fn(*mut SubsystemContext, *const KeyEventData) -> i32,
    pub handle_mouse: extern "C" fn(*mut SubsystemContext, *const MouseEventData) -> i32,
    pub tick: extern "C" fn(*mut SubsystemContext) -> i32,
    pub render: extern "C" fn(*mut SubsystemContext) -> i32,
    pub shutdown: extern "C" fn(*mut SubsystemContext),
}

unsafe impl Send for SubsystemExports {}
unsafe impl Sync for SubsystemExports {}

#[macro_export]
macro_rules! subsystem_entry {
    ($name:expr, $init:expr, $handle_key:expr, $handle_mouse:expr, $tick:expr, $render:expr, $shutdown:expr) => {
        #[no_mangle]
        pub static BARAM_SUBSYSTEM_EXPORTS: $crate::subsystem::SubsystemExports = $crate::subsystem::SubsystemExports {
            magic: $crate::subsystem::SUBSYSTEM_MAGIC,
            version: $crate::subsystem::SUBSYSTEM_VERSION,
            name: $name.as_ptr(),
            init: $init,
            handle_key: $handle_key,
            handle_mouse: $handle_mouse,
            tick: $tick,
            render: $render,
            shutdown: $shutdown,
        };

        fn baram_subsystem_app(
            _nano: baram_nano_system::NanoSystem,
        ) -> uefi::Status {
            uefi::Status::SUCCESS
        }

        baram_nano_system::nano_entry!(baram_subsystem_app);
    };
}
