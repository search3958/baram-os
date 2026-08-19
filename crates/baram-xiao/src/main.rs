#![no_std]
#![no_main]

#[path = "../../baram-boot/src/kiosk.rs"]
mod kiosk;

nano_system::nano_entry_with_target!(kiosk::run, 320, 180);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    nano_system::NanoSystem::panic_report(info)
}
