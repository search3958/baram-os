; NASM syntax
section .multiboot
align 4
    dd 0x1BADB002        ; magic
    dd 0x00000005        ; flags (bit 0: mem info, bit 2: video info)
    dd -(0x1BADB002 + 0x00000005) ; checksum

    ; Graphics fields (flags bit 2 が立っているため必須)
    dd 0, 0, 0, 0, 0     ; header, load, load_end, bss_end, entry
    dd 0                 ; mode_type (0: linear)
    dd 320               ; width
    dd 200               ; height
    dd 8                 ; depth

section .text
global _start
extern kmain

_start:
    cli
    mov esp, stack_space
    
    ; 引数をスタックに積む (C言語の引数は右から左)
    push ebx                  ; mbi (第2引数)
    push eax                  ; magic (第1引数)
    
    call kmain
    
.halt:
    hlt
    jmp .halt

section .bss
align 16
resb 8192
stack_space: