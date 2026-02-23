#ifndef IRQ_H
#define IRQ_H

#include <stdint.h>

/* 割り込み発生時にスタックに積まれるレジスタの状態 */
struct regs
{
    uint32_t ds;
    uint32_t edi, esi, ebp, esp, ebx, edx, ecx, eax;
    uint32_t int_no, err_code;
    uint32_t eip, cs, eflags, useresp, ss;
};


/* IRQハンドラの型定義 */
// 割り込みが発生した際に呼び出される関数のポインタ
typedef void (*irq_handler_t)(struct regs *r);

/* 特定のIRQラインにハンドラをインストールする */
void irq_install_handler(int irq, irq_handler_t handler);

/* 特定のIRQラインからハンドラをアンインストールする */
void irq_uninstall_handler(int irq);

/* PICをリマップし、IRQハンドリングシステムを初期化する */
void irq_install();

#endif /* IRQ_H */
