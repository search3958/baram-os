# ARM64 Bootloader Assembly (AArch64)
.section .text
.global _start

_start:
    # Disable MMU and caches (simplified for EL1)
    mrs x0, sctlr_el1
    bic x0, x0, #1              // Clear M bit (MMU)
    bic x0, x0, #4              // Clear C bit (Data cache)
    bic x0, x0, #0x1000         // Clear I bit (Instruction cache)
    msr sctlr_el1, x0
    
    # Set up stack
    ldr x0, =stack_top
    mov sp, x0
    
    # Clear frame pointer
    mov x29, #0
    
    # Call kernel main with null boot_info
    mov x0, #0
    bl kernel_main
    
    # Hang
hang:
    wfi
    b hang

.section .bss
.align 4
stack_bottom:
    .skip 65536
stack_top:
