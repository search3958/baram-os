#ifndef IDT_H
#define IDT_H

#include <stdint.h>

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

/* IDTの特定のエントリ（ゲート）を設定する関数 */
void idt_set_gate(uint8_t num, uint32_t base, uint16_t sel, uint8_t flags);

/* IDTを初期化し、CPUにロードするメイン関数 */
void idt_install();

#endif /* IDT_H */
