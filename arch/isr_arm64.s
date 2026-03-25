.macro save_context
    sub sp, sp, #256
    stp x0, x1, [sp, #0]
    stp x2, x3, [sp, #16]
    stp x4, x5, [sp, #32]
    stp x6, x7, [sp, #48]
    stp x8, x9, [sp, #64]
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x17, x18, [sp, #144]
    stp x19, x20, [sp, #160]
    stp x21, x22, [sp, #176]
    stp x23, x24, [sp, #192]
    stp x25, x26, [sp, #208]
    stp x27, x28, [sp, #224]
    stp x29, x30, [sp, #240]
.endm

.macro restore_context
    ldp x0, x1, [sp, #0]
    ldp x2, x3, [sp, #16]
    ldp x4, x5, [sp, #32]
    ldp x6, x7, [sp, #48]
    ldp x8, x9, [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x17, x18, [sp, #144]
    ldp x19, x20, [sp, #160]
    ldp x21, x22, [sp, #176]
    ldp x23, x24, [sp, #192]
    ldp x25, x26, [sp, #208]
    ldp x27, x28, [sp, #224]
    ldp x29, x30, [sp, #240]
    add sp, sp, #256
.endm

.align 11
.global vector_table
vector_table:
    // --- Current EL with SP0 ---
    .align 7
    b sync_handler
    .align 7
    b irq_handler_asm
    .align 7
    b fiq_handler
    .align 7
    b serror_handler

    // --- Current EL with SPx ---
    .align 7
    b sync_handler
    .align 7
    b irq_handler_asm
    .align 7
    b fiq_handler
    .align 7
    b serror_handler

    // --- Lower EL using AArch64 ---
    .align 7
    b sync_handler
    .align 7
    b irq_handler_asm
    .align 7
    b fiq_handler
    .align 7
    b serror_handler

    // --- Lower EL using AArch32 ---
    .align 7
    b sync_handler
    .align 7
    b irq_handler_asm
    .align 7
    b fiq_handler
    .align 7
    b serror_handler

sync_handler:
    save_context
    mov x0, sp
    bl exception_handler_c
    restore_context
    eret

irq_handler_asm:
    save_context
    mov x0, sp
    bl irq_handler_c
    restore_context
    eret

fiq_handler:
    b .
serror_handler:
    b .
