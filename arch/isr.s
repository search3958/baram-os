; isr.s - 割り込みサービスルーチン（修正版：例外情報表示対応）
section .text

; CのIRQハンドラをインポート
extern irq_handler
extern exception_handler

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

    mov eax, esp
    push eax
    call irq_handler
    add esp, 4

    pop gs
    pop fs
    pop es
    pop ds
    popa
    add esp, 8
    iret

; 共通の例外処理スタブ（0-31）
exception_common_stub:
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

    mov eax, esp
    push eax
    call exception_handler
    add esp, 4

    pop gs
    pop fs
    pop es
    pop ds
    popa
    add esp, 8
    iret

; IRQハンドラスタブを生成するためのマクロ
%macro IRQ_HANDLER 1
global irq%1
irq%1:
    cli
    push byte 0
    push byte 32 + %1
    jmp irq_common_stub
%endmacro

; 例外ハンドラスタブを生成するためのマクロ
; エラーコードなしの例外用
%macro EXCEPTION_HANDLER_NO_ERROR 1
global isr%1
isr%1:
    cli
    push byte 0      ; エラーコードがないので 0 をプッシュ
    push byte %1     ; 例外番号
    jmp exception_common_stub
%endmacro

; エラーコードありの例外用
%macro EXCEPTION_HANDLER_WITH_ERROR 1
global isr%1
isr%1:
    cli
    ; エラーコードは既にスタックに積まれている
    push byte %1     ; 例外番号
    jmp exception_common_stub
%endmacro

; 例外 0-31 のためのスタブを生成
EXCEPTION_HANDLER_NO_ERROR 0    ; Divide by zero
EXCEPTION_HANDLER_NO_ERROR 1    ; Debug
EXCEPTION_HANDLER_NO_ERROR 2    ; NMI
EXCEPTION_HANDLER_NO_ERROR 3    ; Breakpoint
EXCEPTION_HANDLER_NO_ERROR 4    ; Overflow
EXCEPTION_HANDLER_NO_ERROR 5    ; Bound range exceeded
EXCEPTION_HANDLER_NO_ERROR 6    ; Invalid opcode
EXCEPTION_HANDLER_NO_ERROR 7    ; Device not available
EXCEPTION_HANDLER_WITH_ERROR 8  ; Double fault
EXCEPTION_HANDLER_NO_ERROR 9    ; Coprocessor segment overrun
EXCEPTION_HANDLER_WITH_ERROR 10 ; Invalid TSS
EXCEPTION_HANDLER_WITH_ERROR 11 ; Segment not present
EXCEPTION_HANDLER_WITH_ERROR 12 ; Stack-segment fault
EXCEPTION_HANDLER_WITH_ERROR 13 ; General protection fault
EXCEPTION_HANDLER_WITH_ERROR 14 ; Page fault
EXCEPTION_HANDLER_NO_ERROR 15   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 16   ; x87 FPU error
EXCEPTION_HANDLER_WITH_ERROR 17 ; Alignment check
EXCEPTION_HANDLER_NO_ERROR 18   ; Machine check
EXCEPTION_HANDLER_NO_ERROR 19   ; SIMD FP exception
EXCEPTION_HANDLER_NO_ERROR 20   ; Virtualization exception
EXCEPTION_HANDLER_NO_ERROR 21   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 22   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 23   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 24   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 25   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 26   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 27   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 28   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 29   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 30   ; Reserved
EXCEPTION_HANDLER_NO_ERROR 31   ; Reserved

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
