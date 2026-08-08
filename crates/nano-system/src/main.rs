#![no_std]
#![no_main]

use nano_system::NanoSystem;
use uefi::Status;

fn nano_idle(mut nano: NanoSystem) -> Status {
    const MAX_POINTER_EVENTS_PER_TICK: usize = 8;
    const MAX_KEY_EVENTS_PER_TICK: usize = 16;

    let mut cursor_x = nano.display.width / 2;
    let mut cursor_y = nano.display.height / 2;
    let Some(timer) = nano.take_timer_event() else {
        NanoSystem::paint_failure_screen();
        return Status::DEVICE_ERROR;
    };
    let mut tick = 0u64;
    let mut yellow_until = 0u64;
    let mut was_yellow = false;
    let mut pending_redraw = false;
    let mut next_frame_tick = 0u64;
    let Ok(mut display) = NanoSystem::begin_pointer_test() else {
        NanoSystem::paint_failure_screen();
        return Status::UNSUPPORTED;
    };
    display.initialize(cursor_x, cursor_y, false);
    let mut drawn_x = cursor_x;
    let mut drawn_y = cursor_y;

    loop {
        let mut events = [unsafe { core::ptr::read(&timer) }];
        let _ = uefi::boot::wait_for_event(&mut events);
        tick = tick.wrapping_add(1);

        for _ in 0..MAX_KEY_EVENTS_PER_TICK {
            if nano.poll_keyboard().is_none() {
                break;
            }
            yellow_until = tick.saturating_add(200);
            pending_redraw = true;
        }
        for _ in 0..MAX_POINTER_EVENTS_PER_TICK {
            let Some(event) = nano.poll_pointer() else {
                break;
            };
            let old_x = cursor_x;
            let old_y = cursor_y;
            if let Some((x, y, max_x, max_y)) = event.absolute {
                cursor_x = (x.saturating_mul(nano.display.width.saturating_sub(16) as u64) / max_x)
                    .min(nano.display.width.saturating_sub(16) as u64)
                    as usize;
                cursor_y = (y.saturating_mul(nano.display.height.saturating_sub(16) as u64) / max_y)
                    .min(nano.display.height.saturating_sub(16) as u64)
                    as usize;
            } else {
                cursor_x = (cursor_x as i64 + event.dx as i64)
                    .clamp(0, nano.display.width.saturating_sub(16) as i64)
                    as usize;
                cursor_y = (cursor_y as i64 + event.dy as i64)
                    .clamp(0, nano.display.height.saturating_sub(16) as i64)
                    as usize;
            }
            pending_redraw |= cursor_x != old_x || cursor_y != old_y;
        }

        let yellow = tick < yellow_until;
        if yellow != was_yellow {
            pending_redraw = true;
            was_yellow = yellow;
        }
        if pending_redraw && tick >= next_frame_tick {
            display.update(drawn_x, drawn_y, cursor_x, cursor_y, yellow);
            drawn_x = cursor_x;
            drawn_y = cursor_y;
            pending_redraw = false;
            next_frame_tick = tick.saturating_add(16);
        }
    }
}

nano_system::nano_entry!(nano_idle);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    NanoSystem::paint_failure_screen();
    loop {}
}
