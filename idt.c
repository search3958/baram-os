#include "idt.h"

/* IDT本体 (256エントリ) */
struct idt_entry idt[256];
struct idt_ptr idtp;

/* CPUにIDTをロードするための外部アセンブリ関数 */
extern void idt_load(struct idt_ptr* idtp);

/* IDTの特定のエントリ（ゲート）を設定する */
void idt_set_gate(uint8_t num, uint32_t base, uint16_t sel, uint8_t flags) {
    idt[num].base_lo = (base & 0xFFFF);
    idt[num].base_hi = (base >> 16) & 0xFFFF;
    idt[num].sel = sel;
    idt[num].always0 = 0;
    idt[num].flags = flags;
}

/* IDTを初期化し、CPUにロードする */
void idt_install() {
    /* IDTポインタを設定 */
    idtp.limit = (sizeof(struct idt_entry) * 256) - 1;
    idtp.base = (uint32_t)&idt;

    /* IDTをゼロクリア */
    for (int i = 0; i < 256; i++) {
        idt[i].base_lo = 0;
        idt[i].base_hi = 0;
        idt[i].sel = 0;
        idt[i].always0 = 0;
        idt[i].flags = 0;
    }

    /* IDTをロード */
    idt_load(&idtp);
}
