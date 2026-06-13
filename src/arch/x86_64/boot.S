# x86_64 Bootloader Assembly (Multiboot2 compliant)

# Multiboot2 header must be in the first 32KB of the kernel image
# Place it BEFORE any code using .text.startup section
.section ".multiboot_header", "a"
    # Align to 8 bytes as required by Multiboot2 spec
    .align 8
    
    # Magic number (0xe85250d6)
    .long 0xe85250d6
    
    # Architecture (0 = protected mode i386)
    .long 0
    
    # Header length (will be calculated)
header_length:
    .long header_end - header_start
    
    # Checksum
checksum:
    .long 0x10000000 - (0xe85250d6 + 0 + (header_end - header_start))

header_start:
    # Framebuffer tag
framebuffer_tag_start:
    .short 5                   # Type (framebuffer)
    .short 0                   # Optional flag
    .long framebuffer_tag_end - framebuffer_tag_start  # Size
    .long 800                  # Width (preferred)
    .long 600                  # Height (preferred)
    .long 32                   # Depth (bits per pixel)
framebuffer_tag_end:

    # End tag (type=0, flags=0, size=8)
    .short 0                   # Type
    .short 0                   # Flags
    .long 8                    # Size
header_end:

.section .note.GNU-stack,"",@progbits

.section .text
.global _start
.extern kernel_main

.code64
_start:
    # Set up stack
    mov $stack_top, %rsp
    
    # Clear RBP for backtrace
    xor %rbp, %rbp
    
    # Create boot_info_t on stack
    # We'll pass a null pointer for now - in real scenario bootloader provides this
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
