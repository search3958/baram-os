#ifndef DRIVERS_H
#define DRIVERS_H

#include <stdint.h>

// --- IO (io.h) ---
/* ポートから1バイト読み込む */
static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    __asm__ __volatile__ ("inb %1, %0" : "=a"(ret) : "dN"(port));
    return ret;
}

/* ポートに1バイト書き込む */
static inline void outb(uint16_t port, uint8_t val) {
    __asm__ __volatile__ ("outb %0, %1" : : "a"(val), "dN"(port));
}

// --- IDT (idt.h) ---
/* IDTエントリの構造体 */
struct idt_entry {
    uint16_t base_lo;    // ハンドラアドレスの下位16ビット
    uint16_t sel;        // コードセグメントセレクタ
    uint8_t  always0;    // 常に0でなければならない
    uint8_t  flags;      // タイプと属性
    uint16_t base_hi;    // ハンドラアドレスの上位16ビット
} __attribute__((packed));

/* IDTポインタの構造体 (LIDT命令で使用) */
struct idt_ptr {
    uint16_t limit;
    uint32_t base;
} __attribute__((packed));

void idt_set_gate(uint8_t num, uint32_t base, uint16_t sel, uint8_t flags);
void idt_install();

// --- IRQ (irq.h) ---
/* 割り込み発生時にスタックに積まれるレジスタの状態 */
struct regs
{
    uint32_t gs, fs, es, ds;
    uint32_t edi, esi, ebp, esp, ebx, edx, ecx, eax;
    uint32_t int_no, err_code;
    uint32_t eip, cs, eflags, useresp, ss;
};

/* IRQハンドラの型定義 */
typedef void (*irq_handler_t)(struct regs *r);

void irq_install_handler(int irq, irq_handler_t handler);
void irq_uninstall_handler(int irq);
void irq_install();
void enable_interrupts();

// --- Mouse (mouse.h) ---
void mouse_install();
extern volatile int32_t mouse_x;
extern volatile int32_t mouse_y;
extern volatile uint32_t mouse_interrupt_counter;

#endif /* DRIVERS_H */
