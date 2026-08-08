#![no_std]
#![no_main]

use nano_system::NanoSystem;
use uefi::Status;

fn nano_idle(mut nano: NanoSystem) -> Status {
    const MAX_KEY_EVENTS_PER_POLL: usize = 16;

    if let Some(timer) = nano.take_timer_event() {
        let _ = uefi::boot::close_event(timer);
    }
    let mut cursor_x = nano.display.width / 2;
    let mut cursor_y = nano.display.height / 2;
    let mut was_yellow = false;
    let mut yellow = false;
    let Ok(mut display) = NanoSystem::begin_pointer_test() else {
        NanoSystem::paint_failure_screen();
        return Status::UNSUPPORTED;
    };
    display.initialize(cursor_x, cursor_y, false);
    let mut drawn_x = cursor_x;
    let mut drawn_y = cursor_y;

    loop {
        // Pointer acquisition is always first: a keyboard backlog must not
        // sit in front of cursor motion on the latency-critical path.
        let pointer_event = nano.poll_pointer();
        if let Some(event) = pointer_event {
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
            if cursor_x != old_x || cursor_y != old_y || yellow != was_yellow {
                display.update(drawn_x, drawn_y, cursor_x, cursor_y, yellow);
                drawn_x = cursor_x;
                drawn_y = cursor_y;
                was_yellow = yellow;
                yellow = false;
            }
        }

        // Non-pointer work runs only after a pending cursor update has already
        // reached the framebuffer.
        for _ in 0..MAX_KEY_EVENTS_PER_POLL {
            if nano.poll_keyboard().is_none() {
                break;
            }
            yellow = true;
        }
        if yellow != was_yellow {
            display.update(drawn_x, drawn_y, cursor_x, cursor_y, yellow);
            was_yellow = yellow;
            yellow = false;
        }
    }
}

nano_system::nano_entry!(nano_idle);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    NanoSystem::paint_failure_screen();
    loop {}
}
