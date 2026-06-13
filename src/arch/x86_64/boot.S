# x86_64 Bootloader Assembly (Multiboot2 compliant)

# Multiboot2 header must be in the first 32KB of the kernel image
# Place it BEFORE any code using .text.startup section
.section ".multiboot_header", "aw"
    # Align to 8 bytes as required by Multiboot2 spec
    .align 8
    
    # Magic number (0xe85250d6) - little endian: d6 50 52 e8
    .long 0xe85250d6
    
    # Architecture (0 = protected mode i386, Multiboot2 requires 0 for both 32/64 bit)
    .long 0
    
    # Header length (will be calculated)
header_start:
    .long header_end - header_start
    
    # Checksum (must equal -(magic + arch + header_length))
checksum:
    .long -(0xe85250d6 + 0 + (header_end - header_start))

    # Framebuffer tag - request graphics mode (size = 20 bytes)
framebuffer_tag_start:
    .short 5                   # Type (framebuffer)
    .short 0                   # Optional flag (0 = any graphics mode is OK)
    .long framebuffer_tag_end - framebuffer_tag_start  # Size
    .long 800                  # Width (preferred)
    .long 600                  # Height (preferred)
    .long 32                   # Depth (bits per pixel)
framebuffer_tag_end:

    # End tag (type=0, flags=0, size=8) - MUST be present!
    .short 0                   # Type (end tag)
    .short 0                   # Flags
    .long 8                    # Size (always 8 for end tag)

header_end:

.section .note.GNU-stack,"",@progbits

.section .text
.global _start
.extern kernel_main
.extern multiboot_info_ptr

.code64
_start:
    # Set up stack
    mov $stack_top, %rsp
    
    # Clear RBP for backtrace
    xor %rbp, %rbp
    
    # Save multiboot info pointer (passed by GRUB in EAX)
    # GRUB passes multiboot_info_t* in EAX when jumping to entry point
    mov %eax, %edi  # First argument to kernel_main (boot_info_t*)
    
    # Call kernel main with multiboot info
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
