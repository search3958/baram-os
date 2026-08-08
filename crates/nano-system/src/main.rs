#![no_std]
#![no_main]

use nano_system::NanoSystem;
use uefi::Status;

fn nano_idle(mut nano: NanoSystem) -> Status {
    let mut cursor_x = nano.display.width / 2;
    let mut cursor_y = nano.display.height / 2;
    let Some(timer) = nano.take_timer_event() else {
        NanoSystem::paint_failure_screen();
        return Status::DEVICE_ERROR;
    };
    let mut tick = 0u64;
    let mut yellow_until = 0u64;
    let mut was_yellow = false;
    NanoSystem::draw_pointer_test_frame(cursor_x, cursor_y, false);

    loop {
        let mut events = [unsafe { core::ptr::read(&timer) }];
        let _ = uefi::boot::wait_for_event(&mut events);
        tick = tick.wrapping_add(1);
        let mut redraw = false;

        while nano.poll_keyboard().is_some() {
            yellow_until = tick.saturating_add(200);
            redraw = true;
        }
        while let Some(event) = nano.poll_pointer() {
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
            redraw = true;
        }

        let yellow = tick < yellow_until;
        if yellow != was_yellow {
            redraw = true;
            was_yellow = yellow;
        }
        if redraw {
            NanoSystem::draw_pointer_test_frame(cursor_x, cursor_y, yellow);
        }
    }
}

nano_system::nano_entry!(nano_idle);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    NanoSystem::paint_failure_screen();
    loop {}
}
