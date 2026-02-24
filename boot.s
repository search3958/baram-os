; boot.s - Multiboot + スタック設定
; NASM syntax

section .multiboot
align 4

; Multiboot1 ヘッダ
MAGIC    equ 0x1BADB002
FLAGS    equ 0x00000007   ; bit0: mem_info, bit1: boot_device, bit2: video mode
CHECKSUM equ -(MAGIC + FLAGS)

    dd MAGIC
    dd FLAGS
    dd CHECKSUM

    ; Multiboot仕様: bit2が立つ場合は以下のフィールドが続く
    ; (a.out形式じゃないので 0 で構わない)
    dd 0    ; header_addr    (ELFなので無視)
    dd 0    ; load_addr      (ELFなので無視)
    dd 0    ; load_end_addr  (ELFなので無視)
    dd 0    ; bss_end_addr   (ELFなので無視)
    dd 0    ; entry_addr     (ELFなので無視)

    ; グラフィクスモード指定
    dd 0        ; mode_type: 0=linear framebuffer
    dd 1280     ; width
    dd 720      ; height
    dd 32       ; depth

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

    ; スタックポインタを「スタック領域の一番上 (高アドレス側)」にセット
    mov esp, stack_top

    ; x87 FPU を初期化（BIOSから不定状態で渡されるのでリセット必須）
    fninit

    ; 引数をスタックに積む (cdecl: 右から左)
    push ebx    ; mbi（第2引数）
    push eax    ; magic（第1引数）

    call kmain

.halt:
    cli
    hlt
    jmp .halt

; ========== GDT ==========
section .data
align 4

gdt_start:
    ; Null記述子
    dd 0x00000000
    dd 0x00000000
    ; コードセグメント (0x08): base=0, limit=4GB, DPL=0, 32bit
    dw 0xFFFF   ; limit[15:0]
    dw 0x0000   ; base[15:0]
    db 0x00     ; base[23:16]
    db 0x9A     ; type: code, execute/read
    db 0xCF     ; flags: 4KB gran, 32bit
    db 0x00     ; base[31:24]
    ; データセグメント (0x10): base=0, limit=4GB, DPL=0
    dw 0xFFFF
    dw 0x0000
    db 0x00
    db 0x92     ; type: data, read/write
    db 0xCF
    db 0x00
gdt_end:

gdt_ptr:
    dw gdt_end - gdt_start - 1
    dd gdt_start

; ========== スタック ==========
section .bss
align 16
stack_bottom:
    resb 16384          ; 16KB スタック (8KBから増量)
stack_top:              ; スタックは高アドレスから低アドレス方向に伸びる