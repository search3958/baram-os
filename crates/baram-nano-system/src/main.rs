#![no_std]
#![no_main]

use baram_nano_system::NanoSystem;
use uefi::Status;

fn nano_idle(nano: NanoSystem) -> Status {
    loop {
        let mut events = [unsafe { core::ptr::read(&nano.timer_event) }];
        let _ = uefi::boot::wait_for_event(&mut events);
    }
}

baram_nano_system::nano_entry!(nano_idle);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    NanoSystem::paint_failure_screen();
    loop {}
}
