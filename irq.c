#include "irq.h"
#include "io.h"
#include "idt.h" // idt_set_gate を使用するため

/* 割り込みハンドラのテーブル */
irq_handler_t irq_routines[16] = {0};

/* 特定のIRQにハンドラをインストール */
void irq_install_handler(int irq, irq_handler_t handler) {
    irq_routines[irq] = handler;
}

/* 特定のIRQからハンドラをアンインストール */
void irq_uninstall_handler(int irq) {
    irq_routines[irq] = 0;
}

/* PIC (Programmable Interrupt Controller) をリマップする */
void pic_remap(void) {
    // マスターPIC
    outb(0x20, 0x11); // 初期化開始
    outb(0x21, 0x20); // IRQ0-7 を IDT 32-39 にマッピング
    outb(0x21, 0x04); // スレーブPICがIRQ2に接続されていることを通知
    outb(0x21, 0x01); // 8086/88 (i86) モード

    // スレーブPIC
    outb(0xA0, 0x11); // 初期化開始
    outb(0xA1, 0x28); // IRQ8-15 を IDT 40-47 にマッピング
    outb(0xA1, 0x02); // マスターPICにIRQ2経由で接続されていることを通知
    outb(0xA1, 0x01); // 8086/88 (i86) モード

    // 全てのIRQをマスク（無効化）
    outb(0x21, 0xFF);
    outb(0xA1, 0xFF);
}

/*
* IRQ0-15に対応するアセンブリハンドラをIDTに登録する
* これらは `isr.s` (または `boot.s`) で定義される
*/
extern void irq0();
extern void irq1();
extern void irq2();
extern void irq3();
extern void irq4();
extern void irq5();
extern void irq6();
extern void irq7();
extern void irq8();
extern void irq9();
extern void irq10();
extern void irq11();
extern void irq12();
extern void irq13();
extern void irq14();
extern void irq15();

/* IRQハンドリングシステムを初期化するメイン関数 */
void irq_install() {
    pic_remap(); // PICをリマップ

    // IDTにIRQハンドラを登録
    idt_set_gate(32, (uint32_t)irq0, 0x08, 0x8E);
    idt_set_gate(33, (uint32_t)irq1, 0x08, 0x8E);
    idt_set_gate(34, (uint32_t)irq2, 0x08, 0x8E);
    idt_set_gate(35, (uint32_t)irq3, 0x08, 0x8E);
    idt_set_gate(36, (uint32_t)irq4, 0x08, 0x8E);
    idt_set_gate(37, (uint32_t)irq5, 0x08, 0x8E);
    idt_set_gate(38, (uint32_t)irq6, 0x08, 0x8E);
    idt_set_gate(39, (uint32_t)irq7, 0x08, 0x8E);
    idt_set_gate(40, (uint32_t)irq8, 0x08, 0x8E);
    idt_set_gate(41, (uint32_t)irq9, 0x08, 0x8E);
    idt_set_gate(42, (uint32_t)irq10, 0x08, 0x8E);
    idt_set_gate(43, (uint32_t)irq11, 0x08, 0x8E);
    idt_set_gate(44, (uint32_t)irq12, 0x08, 0x8E);
    idt_set_gate(45, (uint32_t)irq13, 0x08, 0x8E);
    idt_set_gate(46, (uint32_t)irq14, 0x08, 0x8E);
    idt_set_gate(47, (uint32_t)irq15, 0x08, 0x8E);

    // 全ての割り込みを有効にする
    __asm__ __volatile__ ("sti");
}

/*
* 全てのIRQ割り込みがこのCハンドラにルーティングされる
* ここで、どのIRQが発生したかを特定し、対応するハンドラを呼び出す
*/
void irq_handler(struct regs *r) {
    void (*handler)(struct regs *r);
    int irq_num = r->int_no - 32;

    /* 対応するハンドラが登録されていれば実行 */
    handler = irq_routines[irq_num];
    if (handler) {
        handler(r);
    }

    /* EOI (End of Interrupt) 信号をPICに送信 */
    if (irq_num >= 8) {
        outb(0xA0, 0x20); // スレーブPICにもEOI
    }
    outb(0x20, 0x20); // マスターPICにEOI
}
