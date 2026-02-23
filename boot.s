; NASM syntax
section .multiboot
align 4
    dd 0x1BADB002        ; magic
    dd 0x00000005        ; flags (bit 0: mem info, bit 2: video info)
    dd -(0x1BADB002 + 0x00000005) ; checksum

    ; Graphics fields (flags bit 2 が立っているため必須)
    dd 0, 0, 0, 0, 0     ; header, load, load_end, bss_end, entry
    dd 0                 ; mode_type (0: linear)
    dd 1280              ; width (720p)
    dd 720               ; height (720p)
    dd 32                ; depth (32-bit ARGB)

section .text
global _start
extern kmain

_start:
    cli
    lgdt [gdt_ptr]            ; GDTをロード
    jmp 0x08:.reload_segments ; コードセグメントを反映

.reload_segments:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, stack_space
    
    ; 引数をスタックに積む (C言語の引数は右から左)
    push ebx                  ; mbi (第2引数)
    push eax                  ; magic (第1引数)
    
    call kmain
    
.halt:
    hlt
    jmp .halt

; GDTの定義
section .data
align 4
gdt_start:
    ; Null記述子
    dd 0, 0
    ; コードセグメント (0x08): base=0, limit=0xffffffff, type=0x9A, flags=0xCF
    dw 0xFFFF, 0x0000
    db 0x00, 0x9A, 0xCF, 0x00
    ; データセグメント (0x10): base=0, limit=0xffffffff, type=0x92, flags=0xCF
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