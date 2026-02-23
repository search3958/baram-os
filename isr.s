; isr.s - 割り込みサービスルーチンのためのアセンブリスタブ
section .text

; CのIRQハンドラをインポート
extern irq_handler

; 共通の割り込み処理スタブ
irq_common_stub:
    pusha
    push ds
    push es
    push fs
    push gs

    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    push esp
    call irq_handler
    pop esp

    pop gs
    pop fs
    pop es
    pop ds
    popa
    add esp, 8 ; エラーコードとint番号をクリーンアップ
    iret

; IRQハンドラスタブを生成するためのマクロ
; %1: IRQ番号 (0-15)
%macro IRQ_HANDLER 1
global irq%1
irq%1:
    cli
    push byte 0      ; エラーコード(ダミー)
    push byte 32 + %1 ; 割り込み番号
    jmp irq_common_stub
%endmacro

; IRQ 0-15 のためのスタブを生成
IRQ_HANDLER 0
IRQ_HANDLER 1
IRQ_HANDLER 2
IRQ_HANDLER 3
IRQ_HANDLER 4
IRQ_HANDLER 5
IRQ_HANDLER 6
IRQ_HANDLER 7
IRQ_HANDLER 8
IRQ_HANDLER 9
IRQ_HANDLER 10
IRQ_HANDLER 11
IRQ_HANDLER 12
IRQ_HANDLER 13
IRQ_HANDLER 14
IRQ_HANDLER 15
