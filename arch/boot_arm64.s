.section .text.boot
.global _start

_start:
    // Only primary core should boot
    mrs x0, mpidr_el1
    and x0, x0, #0xFF
    cbz x0, 1f
hang:
    wfe
    b hang

1:
    // --- Disable Interrupts ---
    msr daifset, #2

    // --- Enable FPU/SIMD ---
    // CPACR_EL1.FPEN = 0b11
    mov x0, #(3 << 20)
    msr cpacr_el1, x0
    isb

    // --- Setup Stack ---
    ldr x0, =stack_top
    mov sp, x0

    // --- Clear BSS ---
    ldr x0, =_bss_start
    ldr x1, =_bss_end
    sub x1, x1, x0
    cbz x1, 2f
clear_bss:
    str xzr, [x0], #8
    subs x1, x1, #8
    bgt clear_bss

2:
    // --- Jump to C ---
    // Pass fake multiboot magic/info (not really applicable to direct boot but for kmain signature)
    mov x0, #0
    mov x1, #0
    bl kmain

.section .bss
.align 16
stack_bottom:
    .skip 65536
stack_top:
