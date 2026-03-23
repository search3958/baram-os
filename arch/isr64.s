BITS 64

section .text

extern irq_handler
extern exception_handler

%macro PUSH_GPRS 0
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
%endmacro

%macro POP_GPRS 0
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
%endmacro

irq_common_stub:
    PUSH_GPRS
    mov rdi, rsp
    call irq_handler
    POP_GPRS
    add rsp, 16
    iretq

exception_common_stub:
    PUSH_GPRS
    mov rdi, rsp
    call exception_handler
    POP_GPRS
    add rsp, 16
    iretq

%macro IRQ_HANDLER 1
global irq%1
irq%1:
    cli
    push qword 0
    push qword (32 + %1)
    jmp irq_common_stub
%endmacro

%macro EXCEPTION_HANDLER_NO_ERROR 1
global isr%1
isr%1:
    cli
    push qword 0
    push qword %1
    jmp exception_common_stub
%endmacro

%macro EXCEPTION_HANDLER_WITH_ERROR 1
global isr%1
isr%1:
    cli
    push qword %1
    jmp exception_common_stub
%endmacro

EXCEPTION_HANDLER_NO_ERROR 0
EXCEPTION_HANDLER_NO_ERROR 1
EXCEPTION_HANDLER_NO_ERROR 2
EXCEPTION_HANDLER_NO_ERROR 3
EXCEPTION_HANDLER_NO_ERROR 4
EXCEPTION_HANDLER_NO_ERROR 5
EXCEPTION_HANDLER_NO_ERROR 6
EXCEPTION_HANDLER_NO_ERROR 7
EXCEPTION_HANDLER_WITH_ERROR 8
EXCEPTION_HANDLER_NO_ERROR 9
EXCEPTION_HANDLER_WITH_ERROR 10
EXCEPTION_HANDLER_WITH_ERROR 11
EXCEPTION_HANDLER_WITH_ERROR 12
EXCEPTION_HANDLER_WITH_ERROR 13
EXCEPTION_HANDLER_WITH_ERROR 14
EXCEPTION_HANDLER_NO_ERROR 15
EXCEPTION_HANDLER_NO_ERROR 16
EXCEPTION_HANDLER_WITH_ERROR 17
EXCEPTION_HANDLER_NO_ERROR 18
EXCEPTION_HANDLER_NO_ERROR 19
EXCEPTION_HANDLER_NO_ERROR 20
EXCEPTION_HANDLER_NO_ERROR 21
EXCEPTION_HANDLER_NO_ERROR 22
EXCEPTION_HANDLER_NO_ERROR 23
EXCEPTION_HANDLER_NO_ERROR 24
EXCEPTION_HANDLER_NO_ERROR 25
EXCEPTION_HANDLER_NO_ERROR 26
EXCEPTION_HANDLER_NO_ERROR 27
EXCEPTION_HANDLER_NO_ERROR 28
EXCEPTION_HANDLER_NO_ERROR 29
EXCEPTION_HANDLER_NO_ERROR 30
EXCEPTION_HANDLER_NO_ERROR 31

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
