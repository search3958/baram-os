#include "drivers.h"
#include <stdint.h>
#include <stddef.h>

// ==========================================
// グローバル変数：クラッシュ情報
// ==========================================

static uint32_t* g_framebuffer = NULL;
static uint32_t g_screen_width = 0;
static uint32_t g_screen_height = 0;
static uint32_t g_screen_pitch = 0;

void set_framebuffer_info(uint32_t* fb, uint32_t width, uint32_t height, uint32_t pitch) {
    g_framebuffer = fb;
    g_screen_width = width;
    g_screen_height = height;
    g_screen_pitch = pitch;
}

#define ARGB(a, r, g, b) (((uint32_t)(a) << 24) | ((uint32_t)(r) << 16) | ((uint32_t)(g) << 8) | (uint32_t)(b))

void fill_screen(uint32_t color) {
    if (!g_framebuffer) return;
    for (uint32_t y = 0; y < g_screen_height; y++) {
        uint32_t* row = (uint32_t*)((uint8_t*)g_framebuffer + y * g_screen_pitch);
        for (uint32_t x = 0; x < g_screen_width; x++) {
            row[x] = color;
        }
    }
}

void delay_ms(uint32_t ms) {
    volatile uint32_t count = 0;
    for (uint32_t i = 0; i < ms * 10000; i++) {
        count++;
    }
}

// ==========================================
// IDT (idt.c)
// ==========================================

struct idt_entry idt[256];
struct idt_ptr idtp;

void idt_set_gate(uint8_t num, uint32_t base, uint16_t sel, uint8_t flags) {
    idt[num].base_lo = (base & 0xFFFF);
    idt[num].base_hi = (base >> 16) & 0xFFFF;
    idt[num].sel = sel;
    idt[num].always0 = 0;
    idt[num].flags = flags;
}

void idt_install() {
    idtp.limit = (sizeof(struct idt_entry) * 256) - 1;
    idtp.base = (uint32_t)&idt;

    for (int i = 0; i < 256; i++) {
        idt[i].base_lo = 0;
        idt[i].base_hi = 0;
        idt[i].sel = 0;
        idt[i].always0 = 0;
        idt[i].flags = 0;
    }

    __asm__ __volatile__ (
        "lidt %0"
        :
        : "m"(idtp)
        : "memory"
    );
}

// ==========================================
// IRQ (irq.c)
// ==========================================

irq_handler_t irq_routines[16] = {0};

void irq_install_handler(int irq, irq_handler_t handler) {
    irq_routines[irq] = handler;
}

void irq_uninstall_handler(int irq) {
    irq_routines[irq] = 0;
}

void pic_remap(void) {
    outb(0x20, 0x11);
    outb(0x21, 0x20);
    outb(0x21, 0x04);
    outb(0x21, 0x01);

    outb(0xA0, 0x11);
    outb(0xA1, 0x28);
    outb(0xA1, 0x02);
    outb(0xA1, 0x01);

    outb(0x21, 0xFF);
    outb(0xA1, 0xFF);
}

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

extern void isr0();
extern void isr1();
extern void isr2();
extern void isr3();
extern void isr4();
extern void isr5();
extern void isr6();
extern void isr7();
extern void isr8();
extern void isr9();
extern void isr10();
extern void isr11();
extern void isr12();
extern void isr13();
extern void isr14();
extern void isr15();
extern void isr16();
extern void isr17();
extern void isr18();
extern void isr19();
extern void isr20();
extern void isr21();
extern void isr22();
extern void isr23();
extern void isr24();
extern void isr25();
extern void isr26();
extern void isr27();
extern void isr28();
extern void isr29();
extern void isr30();
extern void isr31();

void irq_install() {
    pic_remap();

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
    
    idt_set_gate(0, (uint32_t)isr0, 0x08, 0x8E);
    idt_set_gate(1, (uint32_t)isr1, 0x08, 0x8E);
    idt_set_gate(2, (uint32_t)isr2, 0x08, 0x8E);
    idt_set_gate(3, (uint32_t)isr3, 0x08, 0x8E);
    idt_set_gate(4, (uint32_t)isr4, 0x08, 0x8E);
    idt_set_gate(5, (uint32_t)isr5, 0x08, 0x8E);
    idt_set_gate(6, (uint32_t)isr6, 0x08, 0x8E);
    idt_set_gate(7, (uint32_t)isr7, 0x08, 0x8E);
    idt_set_gate(8, (uint32_t)isr8, 0x08, 0x8E);
    idt_set_gate(9, (uint32_t)isr9, 0x08, 0x8E);
    idt_set_gate(10, (uint32_t)isr10, 0x08, 0x8E);
    idt_set_gate(11, (uint32_t)isr11, 0x08, 0x8E);
    idt_set_gate(12, (uint32_t)isr12, 0x08, 0x8E);
    idt_set_gate(13, (uint32_t)isr13, 0x08, 0x8E);
    idt_set_gate(14, (uint32_t)isr14, 0x08, 0x8E);
    idt_set_gate(15, (uint32_t)isr15, 0x08, 0x8E);
    idt_set_gate(16, (uint32_t)isr16, 0x08, 0x8E);
    idt_set_gate(17, (uint32_t)isr17, 0x08, 0x8E);
    idt_set_gate(18, (uint32_t)isr18, 0x08, 0x8E);
    idt_set_gate(19, (uint32_t)isr19, 0x08, 0x8E);
    idt_set_gate(20, (uint32_t)isr20, 0x08, 0x8E);
    idt_set_gate(21, (uint32_t)isr21, 0x08, 0x8E);
    idt_set_gate(22, (uint32_t)isr22, 0x08, 0x8E);
    idt_set_gate(23, (uint32_t)isr23, 0x08, 0x8E);
    idt_set_gate(24, (uint32_t)isr24, 0x08, 0x8E);
    idt_set_gate(25, (uint32_t)isr25, 0x08, 0x8E);
    idt_set_gate(26, (uint32_t)isr26, 0x08, 0x8E);
    idt_set_gate(27, (uint32_t)isr27, 0x08, 0x8E);
    idt_set_gate(28, (uint32_t)isr28, 0x08, 0x8E);
    idt_set_gate(29, (uint32_t)isr29, 0x08, 0x8E);
    idt_set_gate(30, (uint32_t)isr30, 0x08, 0x8E);
    idt_set_gate(31, (uint32_t)isr31, 0x08, 0x8E);
}

// ==========================================
// キーボード入力デモ用
// ==========================================
#define KEYBUF_SIZE 256
volatile char keybuf[KEYBUF_SIZE];
volatile int keybuf_len = 0;

// US配列スキャンコード→ASCII (一部のみ)
static const char scancode_to_ascii[128] = {
    0, 0, '1','2','3','4','5','6','7','8','9','0','-','=',0,0,
    'q','w','e','r','t','y','u','i','o','p','[',']','\\',0,'a','s',
    'd','f','g','h','j','k','l',';','\'',0,'z','x','c','v','b','n',
    'm',',','.','/',0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
};

static void keyboard_handler(struct regs *r) {
    uint8_t scancode = inb(0x60);
    if (scancode < 128) {
        char c = scancode_to_ascii[scancode];
        if (c && keybuf_len < KEYBUF_SIZE) {
            keybuf[keybuf_len++] = c;
        }
    }
}

void keyboard_install() {
    irq_install_handler(1, keyboard_handler);
}

// ==========================================
// IRQ (irq.c)
// ==========================================

static void default_irq_handler(struct regs *r) {
}

void irq_handler(struct regs *r) {
    void (*handler)(struct regs *r);
    int irq_num = r->int_no - 32;

    handler = irq_routines[irq_num];
    if (handler) {
        handler(r);
    } else {
        default_irq_handler(r);
    }

    if (irq_num >= 8) {
        outb(0xA0, 0x20);
    }
    outb(0x20, 0x20);
}

void exception_handler(struct regs *r) {
    uint32_t exception_num = r->int_no;
    
    uint32_t exception_colors[] = {
        ARGB(255, 255, 0, 0),
        ARGB(255, 255, 127, 0),
        ARGB(255, 255, 255, 0),
        ARGB(255, 127, 255, 0),
        ARGB(255, 0, 255, 0),
        ARGB(255, 0, 255, 127),
        ARGB(255, 0, 255, 255),
        ARGB(255, 0, 127, 255),
        ARGB(255, 0, 0, 255),
        ARGB(255, 127, 0, 255),
        ARGB(255, 255, 0, 255),
        ARGB(255, 255, 0, 127),
        ARGB(255, 200, 0, 100),
        ARGB(255, 128, 128, 0),
        ARGB(255, 128, 0, 128),
        ARGB(255, 64, 64, 64),
    };
    
    uint32_t color;
    if (exception_num < 16) {
        color = exception_colors[exception_num];
    } else {
        color = ARGB(255, 64, 64, 64);
    }
    
    fill_screen(color);
    delay_ms(500);
}

volatile int32_t mouse_x = 0;
volatile int32_t mouse_y = 0;

static uint8_t mouse_cycle = 0;
static int8_t  mouse_packet[3];

volatile uint32_t mouse_interrupt_counter = 0;

void mouse_wait(uint8_t a_type) {
}

void mouse_write(uint8_t a_write) {
}

uint8_t mouse_read() {
    return 0;
}

static void mouse_handler(struct regs *r) {
}

void mouse_install() {
}