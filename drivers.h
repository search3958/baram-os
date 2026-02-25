#ifndef DRIVERS_H
#define DRIVERS_H

#include <stddef.h>
#include <stdint.h>

#define SCREEN_WIDTH 1280
#define SCREEN_HEIGHT 720
#define TRANSPARENT_COLOR 0x00000000

// --- Layer Structure ---
typedef struct {
  uint32_t *buffer;
  int x, y;
  int width, height;
  uint32_t transparent; // 透明色（0の場合は透明なし）
  int active;
  int dynamic; // 1: 毎フレーム更新対象
} layer_t;

// --- IO (io.h) ---
static inline uint8_t inb(uint16_t port) {
  uint8_t ret;
  __asm__ __volatile__("inb %w1, %b0" : "=a"(ret) : "Nd"(port));
  return ret;
}

static inline void outb(uint16_t port, uint8_t val) {
  __asm__ __volatile__("outb %b0, %w1" : : "a"(val), "Nd"(port));
}

// --- IDT/IRQ ---
struct idt_entry {
  uint16_t base_lo;
  uint16_t sel;
  uint8_t always0;
  uint8_t flags;
  uint16_t base_hi;
} __attribute__((packed));

struct idt_ptr {
  uint16_t limit;
  uint32_t base;
} __attribute__((packed));

struct regs {
  uint32_t gs, fs, es, ds;
  uint32_t edi, esi, ebp, esp, ebx, edx, ecx, eax;
  uint32_t int_no, err_code;
  uint32_t eip, cs, eflags, useresp, ss;
};

typedef void (*irq_handler_t)(struct regs *r);

void idt_set_gate(uint8_t num, uintptr_t base, uint16_t sel, uint8_t flags);
void idt_install();
void irq_install_handler(int irq, irq_handler_t handler);
void irq_uninstall_handler(int irq);
void irq_install();
void enable_interrupts();

// --- Graphics & Layers ---
void set_framebuffer_info(uint32_t *fb, uint32_t width, uint32_t height,
                          uint32_t pitch);
void screen_refresh(); // バッファを合成してVRAMに反映
void screen_mark_static_dirty(); // 静的レイヤーの再合成要求
void layer_fill(layer_t *layer, uint32_t color);
void layer_draw_char(layer_t *layer, int x, int y, char c, uint32_t color,
                     uint32_t bg_color);
void layer_draw_string(layer_t *layer, int x, int y, const char *str,
                       uint32_t color, uint32_t bg_color);

// --- Mouse ---
void mouse_install();
void keyboard_install();
extern volatile int32_t mouse_x;
extern volatile int32_t mouse_y;
extern volatile uint32_t mouse_interrupt_counter;

// --- Multiboot ---
struct multiboot_info {
  uint32_t flags;
  uint32_t mem_lower;
  uint32_t mem_upper;
  uint32_t boot_device;
  uint32_t cmdline;
  uint32_t mods_count;
  uint32_t mods_addr;
  uint32_t dummy[4];
  uint32_t mmap_length;
  uint32_t mmap_addr;
  uint32_t dummy2[9];
  uint64_t framebuffer_addr;
  uint32_t framebuffer_pitch;
  uint32_t framebuffer_width;
  uint32_t framebuffer_height;
  uint8_t framebuffer_bpp;
  uint8_t framebuffer_type;
  uint8_t color_info[6];
};

#endif
