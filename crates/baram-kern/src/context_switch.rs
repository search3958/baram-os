#![no_std]

use crate::process::CpuContext;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ContextSwitchFrame {
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub x28: u64,
    pub x29: u64,
    pub lr: u64,
    pub sp: u64,
}

pub fn save_context(ctx: &mut CpuContext) {
    unsafe {
        core::arch::asm!(
            "str x0, [{ctx}, #0]",
            "str x1, [{ctx}, #8]",
            "str x2, [{ctx}, #16]",
            "str x3, [{ctx}, #24]",
            "str x4, [{ctx}, #32]",
            "str x5, [{ctx}, #40]",
            "str x6, [{ctx}, #48]",
            "str x7, [{ctx}, #56]",
            "str x8, [{ctx}, #64]",
            "str x9, [{ctx}, #72]",
            "str x10, [{ctx}, #80]",
            "str x11, [{ctx}, #88]",
            "str x12, [{ctx}, #96]",
            "str x13, [{ctx}, #104]",
            "str x14, [{ctx}, #112]",
            "str x15, [{ctx}, #120]",
            "str x16, [{ctx}, #128]",
            "str x17, [{ctx}, #136]",
            "str x18, [{ctx}, #144]",
            "str x19, [{ctx}, #152]",
            "str x20, [{ctx}, #160]",
            "str x21, [{ctx}, #168]",
            "str x22, [{ctx}, #176]",
            "str x23, [{ctx}, #184]",
            "str x24, [{ctx}, #192]",
            "str x25, [{ctx}, #200]",
            "str x26, [{ctx}, #208]",
            "str x27, [{ctx}, #216]",
            "str x28, [{ctx}, #224]",
            "str x29, [{ctx}, #232]",
            "mrs x0, elr_el1",
            "str x0, [{ctx}, #248]",
            "mrs x0, spsr_el1",
            "str x0, [{ctx}, #256]",
            ctx = in(reg) ctx,
            out("x0") _,
        );
    }
}

pub fn restore_context(ctx: &CpuContext) {
    unsafe {
        core::arch::asm!(
            "ldr x0, [{ctx}, #248]",
            "msr elr_el1, x0",
            "ldr x0, [{ctx}, #256]",
            "msr spsr_el1, x0",
            "ldr x19, [{ctx}, #152]",
            "ldr x20, [{ctx}, #160]",
            "ldr x21, [{ctx}, #168]",
            "ldr x22, [{ctx}, #176]",
            "ldr x23, [{ctx}, #184]",
            "ldr x24, [{ctx}, #192]",
            "ldr x25, [{ctx}, #200]",
            "ldr x26, [{ctx}, #208]",
            "ldr x27, [{ctx}, #216]",
            "ldr x28, [{ctx}, #224]",
            "ldr x29, [{ctx}, #232]",
            "ldr x0, [{ctx}, #0]",
            "ldr x1, [{ctx}, #8]",
            "ldr x2, [{ctx}, #16]",
            "ldr x3, [{ctx}, #24]",
            "ldr x4, [{ctx}, #32]",
            "ldr x5, [{ctx}, #40]",
            "ldr x6, [{ctx}, #48]",
            "ldr x7, [{ctx}, #56]",
            "ldr x8, [{ctx}, #64]",
            "ldr x9, [{ctx}, #72]",
            "ldr x10, [{ctx}, #80]",
            "ldr x11, [{ctx}, #88]",
            "ldr x12, [{ctx}, #96]",
            "ldr x13, [{ctx}, #104]",
            "ldr x14, [{ctx}, #112]",
            "ldr x15, [{ctx}, #120]",
            "ldr x16, [{ctx}, #128]",
            "ldr x17, [{ctx}, #136]",
            "ldr x18, [{ctx}, #144]",
            "eret",
            ctx = in(reg) ctx,
        );
    }
}

pub fn switch_context(from: &mut CpuContext, to: &CpuContext) {
    unsafe {
        let frame = ContextSwitchFrame {
            x19: 0, x20: 0, x21: 0, x22: 0, x23: 0, x24: 0, x25: 0, x26: 0, x27: 0, x28: 0, x29: 0, lr: 0, sp: 0,
        };
        let frame_ptr = &frame as *const ContextSwitchFrame;

        core::arch::asm!(
            "stp x19, x20, [{frame}, #0]",
            "stp x21, x22, [{frame}, #16]",
            "stp x23, x24, [{frame}, #32]",
            "stp x25, x26, [{frame}, #48]",
            "stp x27, x28, [{frame}, #64]",
            "stp x29, lr, [{frame}, #80]",
            "mov {tmp}, sp",
            "str {tmp}, [{frame}, #96]",
            "mov sp, {to_sp}",
            "ldp x19, x20, [{to}, #152]",
            "ldp x21, x22, [{to}, #168]",
            "ldp x23, x24, [{to}, #184]",
            "ldp x25, x26, [{to}, #200]",
            "ldp x27, x28, [{to}, #216]",
            "ldp x29, x30, [{to}, #232]",
            frame = in(reg) frame_ptr,
            to = in(reg) to,
            to_sp = in(reg) to.sp,
            tmp = out(reg) _,
        );
    }
}

pub fn enable_interrupts() {
    unsafe {
        core::arch::asm!("msr daifclr, #0xF");
    }
}

pub fn disable_interrupts() {
    unsafe {
        core::arch::asm!("msr daifset, #0xF");
    }
}

pub fn is_interrupt_enabled() -> bool {
    let daif: u64;
    unsafe {
        core::arch::asm!("mrs {daif}, daif", daif = out(reg) daif);
    }
    daif & 0xF == 0
}

pub fn enable_irq() {
    unsafe {
        core::arch::asm!("msr daifclr, #0x2");
    }
}

pub fn disable_irq() {
    unsafe {
        core::arch::asm!("msr daifset, #0x2");
    }
}

pub fn get_timer_freq() -> u64 {
    let freq: u64;
    unsafe {
        core::arch::asm!("mrs {freq}, cntfrq_el0", freq = out(reg) freq);
    }
    freq
}

pub fn get_timer_count() -> u64 {
    let count: u64;
    unsafe {
        core::arch::asm!("mrs {count}, cntpct_el0", count = out(reg) count);
    }
    count
}

pub fn set_timer_compare(compare: u64) {
    unsafe {
        core::arch::asm!("msr cntp_cval_el0, {compare}", compare = in(reg) compare);
    }
}

pub fn enable_timer() {
    unsafe {
        core::arch::asm!("msr cntp_ctl_el0, {val}", val = in(reg) 1u64);
    }
}

pub fn disable_timer() {
    unsafe {
        core::arch::asm!("msr cntp_ctl_el0, {val}", val = in(reg) 0u64);
    }
}
