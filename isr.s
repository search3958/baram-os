; isr.s - 割り込み・例外サービスルーチン
section .text

extern irq_handler
extern exception_handler

; ========== IRQ 共通スタブ ==========
; isr.s の IRQ スタブでは push byte 0 / push byte N を使う
; → irq_common_stub から戻る際に add esp, 8 でそれを除去する
irq_common_stub:
    pusha           ; eax,ecx,edx,ebx,esp_orig,ebp,esi,edi をプッシュ
    push ds
    push es
    push fs
    push gs

    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    push esp        ; struct regs * を渡す
    call irq_handler
    add esp, 4      ; push esp の分だけ戻す

    pop gs
    pop fs
    pop es
    pop ds
    popa
    add esp, 8      ; IRQスタブが push した int_no + err_code の 8 バイト
    iret

; ========== 例外 共通スタブ ==========
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

    push esp
    call exception_handler
    add esp, 4

    pop gs
    pop fs
    pop es
    pop ds
    popa
    add esp, 8
    iret

; ========== IRQ マクロ ==========
%macro IRQ_HANDLER 1
global irq%1
irq%1:
    cli
    push dword 0            ; err_code (ダミー)
    push dword (32 + %1)   ; int_no
    jmp irq_common_stub
%endmacro

; ========== 例外マクロ ==========
; エラーコードなし
%macro EXCEPTION_NO_ERR 1
global isr%1
isr%1:
    cli
    push dword 0    ; err_code ダミー
    push dword %1   ; int_no
    jmp exception_common_stub
%endmacro

; エラーコードあり (CPUが既にプッシュ済み)
%macro EXCEPTION_WITH_ERR 1
global isr%1
isr%1:
    cli
    push dword %1   ; int_no (err_code はCPUが積んだ)
    jmp exception_common_stub
%endmacro

; ========== 例外 0-31 ==========
EXCEPTION_NO_ERR   0    ; #DE  Divide by zero
EXCEPTION_NO_ERR   1    ; #DB  Debug
EXCEPTION_NO_ERR   2    ;      NMI
EXCEPTION_NO_ERR   3    ; #BP  Breakpoint
EXCEPTION_NO_ERR   4    ; #OF  Overflow
EXCEPTION_NO_ERR   5    ; #BR  Bound range exceeded
EXCEPTION_NO_ERR   6    ; #UD  Invalid opcode
EXCEPTION_NO_ERR   7    ; #NM  Device not available
EXCEPTION_WITH_ERR 8    ; #DF  Double fault
EXCEPTION_NO_ERR   9    ;      Coprocessor segment overrun
EXCEPTION_WITH_ERR 10   ; #TS  Invalid TSS
EXCEPTION_WITH_ERR 11   ; #NP  Segment not present
EXCEPTION_WITH_ERR 12   ; #SS  Stack-segment fault
EXCEPTION_WITH_ERR 13   ; #GP  General protection fault
EXCEPTION_WITH_ERR 14   ; #PF  Page fault
EXCEPTION_NO_ERR   15   ;      Reserved
EXCEPTION_NO_ERR   16   ; #MF  x87 FPU error
EXCEPTION_WITH_ERR 17   ; #AC  Alignment check
EXCEPTION_NO_ERR   18   ; #MC  Machine check
EXCEPTION_NO_ERR   19   ; #XF  SIMD FP exception
EXCEPTION_NO_ERR   20   ;      Virtualization
EXCEPTION_NO_ERR   21   ;      Reserved
EXCEPTION_NO_ERR   22   ;      Reserved
EXCEPTION_NO_ERR   23   ;      Reserved
EXCEPTION_NO_ERR   24   ;      Reserved
EXCEPTION_NO_ERR   25   ;      Reserved
EXCEPTION_NO_ERR   26   ;      Reserved
EXCEPTION_NO_ERR   27   ;      Reserved
EXCEPTION_NO_ERR   28   ;      Reserved
EXCEPTION_NO_ERR   29   ;      Reserved
EXCEPTION_NO_ERR   30   ;      Reserved
EXCEPTION_NO_ERR   31   ;      Reserved

; ========== IRQ 0-15 ==========
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
