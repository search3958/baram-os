/**
 * @file interrupt.c
 * @brief 割り込み制御関数の実装
 * @license MIT
 */

#include "kernel.h"

#ifdef __x86_64__

void cli(void) {
    __asm__ volatile ("cli");
}

void sti(void) {
    __asm__ volatile ("sti");
}

void halt(void) {
    __asm__ volatile ("hlt");
}

#else // ARM64

void cli(void) {
    // ARM64: DAIFレジスタのIビットを設定
    __asm__ volatile ("msr daifset, #2");
}

void sti(void) {
    // ARM64: DAIFレジスタのIビットをクリア
    __asm__ volatile ("msr daifclr, #2");
}

void halt(void) {
    // ARM64: WFI (Wait For Interrupt)
    __asm__ volatile ("wfi");
}

#endif
