# i386 Bootloader Assembly (Multiboot2 compliant)
.section .multiboot
    # Multiboot2 header
    .align 8
header_start:
    .long 0xe85250d6           # Magic number
    .long 0                    # Architecture (0 = protected mode i386)
    .long header_end - header_start  # Header length
    .long 0x10000000 - (0xe85250d6 + 0 + (header_end - header_start)) # Checksum

    # Framebuffer tag
    .short 5                   # Type (framebuffer)
    .short 0                   # Optional
    .long framebuffer_tag_end - framebuffer_tag_start  # Size
    .long 800                  # Width (preferred)
    .long 600                  # Height (preferred)
    .long 32                   # Depth (bits per pixel)
framebuffer_tag_start:
framebuffer_tag_end:

    # End tag
    .short 0                   # Type
    .short 0                   # Optional
    .long 8                    # Size
header_end:

.section .text
.global _start
.extern kernel_main

.code32
_start:
    # Set up stack
    mov $stack_top, %esp
    
    # Clear EBP for backtrace
    xor %ebp, %ebp
    
    # Push boot_info pointer (null for now)
    push $0
    
    # Call kernel main
    call kernel_main
    
    # Hang if kernel returns
hang:
    hlt
    jmp hang

.section .bss
.align 16
stack_bottom:
    .skip 65536  # 64 KB stack
stack_top:
