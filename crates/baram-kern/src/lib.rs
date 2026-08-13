#![no_std]

extern crate alloc;

#[cfg(target_arch = "aarch64")]
pub mod context_switch;
pub mod dyld;
pub mod elf;
pub mod init;
pub mod ipc;
pub mod loader;
pub mod panic;
pub mod proc_loader;
pub mod process;
pub mod scheduler;
pub mod subsystem;
pub mod syscall;
pub mod vmm;
