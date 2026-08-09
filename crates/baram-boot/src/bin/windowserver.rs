#![no_std]
#![no_main]

extern crate alloc;

use baram_core::subsystem::{KeyEventData, MouseEventData, SubsystemContext, SubsystemExports};
use baram_core::Color;
use baram_core::LayerSystem;

#[no_mangle]
pub static BARAM_SUBSYSTEM_EXPORTS: SubsystemExports = SubsystemExports {
    magic: baram_core::subsystem::SUBSYSTEM_MAGIC,
    version: baram_core::subsystem::SUBSYSTEM_VERSION,
    name: b"windowserver\0".as_ptr(),
    init: ws_init,
    handle_key: ws_handle_key,
    handle_mouse: ws_handle_mouse,
    tick: ws_tick,
    render: ws_render,
    shutdown: ws_shutdown,
};

struct WindowServerState {
    layer: Option<LayerSystem>,
    width: u32,
    height: u32,
    mouse_x: i32,
    mouse_y: i32,
    initialized: bool,
}

static mut STATE: Option<WindowServerState> = None;

extern "C" fn ws_init(ctx: *mut SubsystemContext) -> i32 {
    let ctx = unsafe { &*ctx };
    let w = ctx.screen_w as usize;
    let h = ctx.screen_h as usize;

    if w == 0 || h == 0 {
        return -1;
    }

    unsafe {
        STATE = Some(WindowServerState {
            layer: Some(LayerSystem::new(w, h)),
            width: ctx.screen_w,
            height: ctx.screen_h,
            mouse_x: 0,
            mouse_y: 0,
            initialized: false,
        });
    }

    0
}

extern "C" fn ws_handle_key(ctx: *mut SubsystemContext, event: *const KeyEventData) -> i32 {
    let _event = unsafe { &*event };
    let _ctx = unsafe { &*ctx };
    0
}

extern "C" fn ws_handle_mouse(ctx: *mut SubsystemContext, event: *const MouseEventData) -> i32 {
    let _event = unsafe { &*event };
    let ctx = unsafe { &mut *ctx };

    unsafe {
        if let Some(state) = &mut STATE {
            state.mouse_x = ctx.mouse_x;
            state.mouse_y = ctx.mouse_y;
        }
    }

    0
}

extern "C" fn ws_tick(_ctx: *mut SubsystemContext) -> i32 {
    0
}

extern "C" fn ws_render(ctx: *mut SubsystemContext) -> i32 {
    let ctx = unsafe { &mut *ctx };

    unsafe {
        if let Some(state) = &mut STATE {
            if let Some(layer) = &mut state.layer {
                let w = state.width as usize;
                let h = state.height as usize;

                if !state.initialized {
                    layer.fill_rect(0, 0, w, h, Color::BG);

                    if w > 100 && h > 100 {
                        layer.fill_rounded_rect(50, 50, w - 100, h - 100, 12, Color::WIN_BG);
                        layer.rounded_rect_outline(
                            50,
                            50,
                            w - 100,
                            h - 100,
                            12,
                            Color::BORDER,
                            Color::WIN_BG,
                        );

                        let title_h = 32;
                        layer.fill_rect(50, 50, w - 100, title_h, Color::PANEL);
                        layer.rounded_rect_outline(
                            50,
                            50,
                            w - 100,
                            h - 100,
                            12,
                            Color::BORDER,
                            Color::WIN_BG,
                        );
                        layer.fill_rect(51, 50 + title_h - 1, w - 102, 1, Color::BORDER);
                    }
                    state.initialized = true;
                }

                if ctx.fb.base != core::ptr::null_mut() {
                    let fb_slice = unsafe {
                        core::slice::from_raw_parts_mut(
                            ctx.fb.base,
                            (ctx.fb.width * ctx.fb.height) as usize,
                        )
                    };
                    if let Some((x0, y0, x1, y1)) = layer.take_dirty() {
                        let x0 = x0.min(w);
                        let x1 = x1.min(w);
                        let y0 = y0.min(h);
                        let y1 = y1.min(h);
                        for y in y0..y1 {
                            let start = y * w + x0;
                            let end = y * w + x1;
                            fb_slice[start..end].copy_from_slice(&layer.buf_ref()[start..end]);
                        }
                    }
                }
            }
        }
    }

    0
}

extern "C" fn ws_shutdown(_ctx: *mut SubsystemContext) {
    unsafe {
        STATE = None;
    }
}

fn windowserver_app(_nano: nano_system::NanoSystem) -> uefi::Status {
    uefi::Status::SUCCESS
}

nano_system::nano_entry!(windowserver_app);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
