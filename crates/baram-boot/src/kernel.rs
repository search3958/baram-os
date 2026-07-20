#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use uefi::prelude::*;
use uefi::runtime;

use baram_core::{Color, Screen, LayerSystem};
use baram_core::subsystem::{SubsystemContext, KeyEventData, MouseEventData, FramebufferInfo};
use baram_kern::subsystem::SubsystemManager;
use baram_kern::scheduler::Scheduler;
use baram_kern::vmm::VirtualMemoryManager;
use baram_kern::loader;
use baram_iokit::keyboard::Keyboard;
use baram_iokit::mouse::Mouse;
use baram_bsd::config;

#[entry]
fn kernel_main() -> Status {
    log("BaramOS: starting kernel...");
    let _ = uefi::helpers::init();
    log("BaramOS: UEFI helpers initialized");

    let _ = uefi::boot::set_watchdog_timer(0, 0, None);
    log("BaramOS: watchdog timer disabled");

    config::init_config();
    log("BaramOS: config loaded");

    baram_font::ttf_font::init();
    baram_font::ttf_font_hud::init();
    log("BaramOS: fonts initialized");

    let mut screen = match Screen::take() {
        Ok(s) => {
            log("BaramOS: screen initialized");
            unsafe { baram_font::log::init_screen(&s) };
            s
        },
        Err(_s) => {
            log("BaramOS: screen init failed");
            return Status::UNSUPPORTED
        },
    };

    unsafe { baram_kern::panic::init_from_screen(&screen) };

    log("BaramOS: opening mouse...");
    let mut mouse_opt: Option<Mouse> = match Mouse::open() {
        Ok(m) => Some(m),
        Err(_) => None,
    };
    let mouse_wait = Mouse::get_wait_event();
    log("BaramOS: opening keyboard...");
    let mut keyboard = Keyboard::open();

    let timer_event = unsafe {
        match uefi::boot::create_event(
            uefi_raw::table::boot::EventType::TIMER,
            uefi_raw::table::boot::Tpl::APPLICATION,
            None,
            None,
        ) {
            Ok(evt) => {
                let _ = uefi::boot::set_timer(&evt, uefi::boot::TimerTrigger::Periodic(core::time::Duration::from_millis(1)));
                log("BaramOS: timer event created (1ms periodic)");
                Some(evt)
            }
            Err(_) => {
                log("BaramOS: failed to create timer event");
                None
            }
        }
    };

    let mut cursor_x: i32 = (screen.width() / 2) as i32;
    let mut cursor_y: i32 = (screen.height() / 2) as i32;

    let mut layer = LayerSystem::new(screen.width(), screen.height());
    let mut layer_buf: Vec<u32> = vec![0u32; screen.width() * screen.height()];

    log("BaramOS: loading subsystems...");

    let mut sub_mgr = SubsystemManager::new();
    let mut scheduler = Scheduler::new();

    let subsystem_paths = [
        "EFI/BOOT/bin/windowserver.efi",
        "EFI/BOOT/bin/font.efi",
        "EFI/BOOT/bin/graphics.efi",
        "EFI/BOOT/bin/iokit.efi",
        "EFI/BOOT/bin/bsd.efi",
    ];

    let mut loaded_indices = Vec::new();

    for path in &subsystem_paths {
        log(&alloc::format!("BaramOS: loading {}", path));
        match baram_bsd::vfs::read_file(path) {
            data if !data.is_empty() => {
                match sub_mgr.load_subsystem(&data) {
                    Ok(idx) => {
                        let result = sub_mgr.init_subsystem(
                            idx,
                            &mut layer_buf,
                            screen.width() as u32,
                            screen.height() as u32,
                        );
                        if result == 0 {
                            log(&alloc::format!("BaramOS: subsystem {} initialized", path));
                            let proc_id = scheduler.create_process(path, baram_kern::process::ProcessPriority::Normal);
                            loaded_indices.push(idx);
                        } else {
                            log(&alloc::format!("BaramOS: subsystem {} init failed: {}", path, result));
                        }
                    },
                    Err(e) => {
                        log(&alloc::format!("BaramOS: failed to load {}: {:?}", path, e));
                    },
                }
            },
            _ => {
                log(&alloc::format!("BaramOS: {} not found, skipping", path));
            },
        }
    }

    log(&alloc::format!("BaramOS: {} subsystems loaded", loaded_indices.len()));

    layer_buf.copy_from_slice(layer.buf_ref());
    layer.flush(&mut screen);

    let mut mouse_down = false;
    let mut frames: u32 = 0;
    let mut fps: u32 = 0;
    let mut frames_since_tick: u32 = 0;
    let mut start_time = runtime::get_time().unwrap_or_else(|_| runtime::Time::invalid());
    let mut prev_cursor_x = cursor_x;
    let mut prev_cursor_y = cursor_y;

    log("BaramOS: entering main loop");

    loop {
        if let Some(ref timer) = timer_event {
            let mut events: [uefi::Event; 2] = [unsafe { core::ptr::read(timer) }, unsafe { core::ptr::read(timer) }];
            let mut n = 1;
            if let Some(mevt) = &mouse_wait {
                events[1] = unsafe { core::ptr::read(mevt) };
                n = 2;
            }
            let _ = uefi::boot::wait_for_event(&mut events[..n]);
        }

        while let Some(ev) = keyboard.poll() {
            let key_event = KeyEventData {
                printable: ev.printable.unwrap_or(0),
                has_printable: if ev.printable.is_some() { 1 } else { 0 },
                scancode: ev.scancode,
                modifiers: ev.modifiers,
                raw_key: ev.raw_key,
                _pad: [0; 2],
            };

            for &idx in &loaded_indices {
                if scheduler.get_process(0)
                    .map(|p| p.state == baram_kern::process::ProcessState::Running)
                    .unwrap_or(false)
                {
                    sub_mgr.handle_key(idx, &key_event);
                }
            }
        }

        if let Some(mouse) = mouse_opt.as_mut() {
            while let Some(ev) = mouse.poll() {
                let (cx, cy) = baram_iokit::mouse::apply_mouse_event(
                    &mut cursor_x, &mut cursor_y, &ev,
                    screen.width(), screen.height(), mouse.abs_max(),
                );

                let mouse_event = MouseEventData {
                    abs_x: ev.abs_x,
                    abs_y: ev.abs_y,
                    rel_dx: ev.rel_dx,
                    rel_dy: ev.rel_dy,
                    left: ev.left as u8,
                    right: ev.right as u8,
                    middle: ev.middle as u8,
                    scroll: ev.scroll,
                    is_absolute: ev.is_absolute as u8,
                    _pad: [0; 3],
                };

                for &idx in &loaded_indices {
                    if scheduler.get_process(0)
                        .map(|p| p.state == baram_kern::process::ProcessState::Running)
                        .unwrap_or(false)
                    {
                        if let Some(ctx) = sub_mgr.get_context_mut(idx) {
                            ctx.mouse_x = cursor_x;
                            ctx.mouse_y = cursor_y;
                        }
                        sub_mgr.handle_mouse(idx, &mouse_event);
                    }
                }

                if ev.left && !mouse_down {
                    mouse_down = true;
                } else if !ev.left && mouse_down {
                    mouse_down = false;
                }
            }
        }

        frames = frames.wrapping_add(1);
        frames_since_tick = frames_since_tick.wrapping_add(1);
        if let Ok(now) = runtime::get_time() {
            let elapsed_ns = time_diff_ns(&start_time, &now);
            if elapsed_ns >= 1_000_000_000 {
                fps = frames_since_tick;
                frames_since_tick = 0;
                start_time = now;
            }
        }

        for &idx in &loaded_indices {
            if scheduler.get_process(0)
                .map(|p| p.state == baram_kern::process::ProcessState::Running)
                .unwrap_or(false)
            {
                if let Some(ctx) = sub_mgr.get_context_mut(idx) {
                    ctx.fps = fps;
                }
                sub_mgr.tick(idx);
                sub_mgr.render(idx);
            }
        }

        layer.buf_mut()[..screen.width() * screen.height()]
            .copy_from_slice(&layer_buf);

        cursor_x = cursor_x.max(0).min(screen.width() as i32 - 1);
        cursor_y = cursor_y.max(0).min(screen.height() as i32 - 1);

        if cursor_x != prev_cursor_x || cursor_y != prev_cursor_y {
            layer.flush(&mut screen);
            prev_cursor_x = cursor_x;
            prev_cursor_y = cursor_y;
        } else if frames_since_tick % 16 == 0 {
            layer.flush(&mut screen);
        }
    }
}

fn log(s: &str) {
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.output_string(uefi::cstr16!("BaramOS: "));
        let mut buf = Vec::<u16>::with_capacity(s.len() + 1);
        for &b in s.as_bytes() {
            if b >= 0x80 { break; }
            buf.push(b as u16);
        }
        buf.push(0);
        if let Ok(cs) = uefi::CStr16::from_u16_with_nul(&buf) {
            let _ = stdout.output_string(cs);
        }
        let _ = stdout.output_string(uefi::cstr16!("\r\n"));
    });
}

fn time_diff_ns(a: &runtime::Time, b: &runtime::Time) -> u64 {
    let a_ns = a.nanosecond() as u64
        + a.second() as u64 * 1_000_000_000
        + a.minute() as u64 * 60_000_000_000
        + a.hour() as u64 * 3_600_000_000_000;
    let b_ns = b.nanosecond() as u64
        + b.second() as u64 * 1_000_000_000
        + b.minute() as u64 * 60_000_000_000
        + b.hour() as u64 * 3_600_000_000_000;
    b_ns.saturating_sub(a_ns)
}
