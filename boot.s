section .multiboot
align 4
    dd 0x1BADB002
    dd 0x00010005
    dd -(0x1BADB002 + 0x00010005)
    dd 0, 0, 0, 0, 0
    dd 0
    dd 854
    dd 480
    dd 32

section .text
global _start
extern kmain

_start:
    cli
    mov esp, stack_top
    push ebx
    push eax
    call kmain
.halt:
    hlt
    jmp .halt

section .bss
align 16
resb 65536 ; スタックを64KBに増強
stack_top: