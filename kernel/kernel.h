/**
 * @file kernel.h
 * @brief カーネル共通ヘッダー
 * @license MIT
 */

#ifndef KERNEL_H
#define KERNEL_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

// アーキテクチャ依存の定義
#ifdef __x86_64__
    #define ARCH_NAME "x86_64"
#elif defined(__aarch64__)
    #define ARCH_NAME "arm64"
#else
    #error "Unsupported architecture"
#endif

// メモリ操作関数
void* memset(void* dest, int val, size_t count);
void* memcpy(void* dest, const void* src, size_t count);
int memcmp(const void* ptr1, const void* ptr2, size_t count);

// I/Oポート操作 (x86_64のみ)
#ifdef __x86_64__
static inline void outb(uint16_t port, uint8_t val) {
    __asm__ volatile ("outb %0, %1" : : "a"(val), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    __asm__ volatile ("inb %1, %0" : "=a"(ret) : "Nd"(port));
    return ret;
}
#endif

// 割り込み制御
void cli(void);
void sti(void);
void halt(void);

#endif // KERNEL_H
