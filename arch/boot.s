; NASM syntax
section .multiboot
align 4
    dd 0x1BADB002        ; magic
    dd 0x00000007        ; flags (bit 0: mem, bit 1: bootdev, bit 2: video)
    dd -(0x1BADB002 + 0x00000007) ; checksum

    ; Graphics fields
    dd 0, 0, 0, 0, 0     ; header, load, load_end, bss_end, entry
    dd 0                 ; mode_type (0: linear)
    dd 1280              ; width
    dd 720               ; height
    dd 32                ; depth

section .text
global _start
extern kmain

_start:
    cli
    lgdt [gdt_ptr]
    jmp 0x08:.reload_segments

.reload_segments:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, stack_space
    
    push ebx                  ; multiboot_info ptr
    push eax                  ; magic
    
    call kmain
    
.halt:
    hlt
    jmp .halt

section .data
align 4
gdt_start:
    dd 0, 0
    dw 0xFFFF, 0x0000
    db 0x00, 0x9A, 0xCF, 0x00
    dw 0xFFFF, 0x0000
    db 0x00, 0x92, 0xCF, 0x00
gdt_end:

gdt_ptr:
    dw gdt_end - gdt_start - 1
    dd gdt_start

section .bss
align 16
resb 8192
stack_space:
