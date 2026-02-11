; NASM syntax
section .multiboot
    dd 0x1BADB002        ; マジックナンバー
    dd 0x00              ; フラグ
    dd -(0x1BADB002 + 0x00) ; チェックサム

section .text
global _start
extern kmain            ; C言語の関数を呼ぶ

_start:
    cli                  ; 割り込み禁止
    mov esp, stack_space ; スタックポインタの設定
    call kmain           ; C言語のメイン関数へ
    hlt                  ; 停止

section .bss
resb 8192                ; 8KBのスタック領域
stack_space: