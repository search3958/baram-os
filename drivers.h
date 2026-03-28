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
#ifdef __aarch64__
static inline uint8_t mmio_read8(uintptr_t addr) {
  return *(volatile uint8_t *)addr;
}
static inline void mmio_write8(uintptr_t addr, uint8_t val) {
  *(volatile uint8_t *)addr = val;
}
static inline void mmio_write16(uintptr_t addr, uint16_t val) {
  *(volatile uint16_t *)addr = val;
}
static inline uint32_t mmio_read32(uintptr_t addr) {
  return *(volatile uint32_t *)addr;
}
static inline void mmio_write32(uintptr_t addr, uint32_t val) {
  *(volatile uint32_t *)addr = val;
}
// Placeholder for x86 compatibility if needed, but ARM64 uses MMIO
static inline uint8_t inb(uint16_t port) { return 0; }
static inline void outb(uint16_t port, uint8_t val) { }
#else
static inline uint8_t inb(uint16_t port) {
  uint8_t ret;
  __asm__ __volatile__("inb %w1, %b0" : "=a"(ret) : "Nd"(port));
  return ret;
}

static inline void outb(uint16_t port, uint8_t val) {
  __asm__ __volatile__("outb %b0, %w1" : : "a"(val), "Nd"(port));
}
#endif

void outw(uint16_t port, uint16_t val);
uint16_t inw(uint16_t port);

// --- IDT/IRQ ---
#ifdef __aarch64__
struct regs {
  uint64_t x[31];
  uint64_t sp;
  uint64_t pc;
  uint64_t pstate;
};
#elif defined(__x86_64__)
struct idt_entry {
  uint16_t base_lo;
  uint16_t sel;
  uint8_t ist;
  uint8_t flags;
  uint16_t base_mid;
  uint32_t base_hi;
  uint32_t reserved;
} __attribute__((packed));

struct idt_ptr {
  uint16_t limit;
  uint64_t base;
} __attribute__((packed));

struct regs {
  uint64_t r15, r14, r13, r12, r11, r10, r9, r8;
  uint64_t rbp, rdi, rsi, rdx, rcx, rbx, rax;
  uint64_t int_no, err_code;
  uint64_t rip, cs, rflags;
};
#else
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
#endif

typedef void (*irq_handler_t)(struct regs *r);

void idt_set_gate(uint8_t num, uintptr_t base, uint16_t sel, uint8_t flags);
void idt_install();
void irq_install_handler(int irq, irq_handler_t handler);
void irq_uninstall_handler(int irq);
void irq_install();
void enable_interrupts();
void timer_handler(struct regs *r);

// --- Graphics & Layers ---
void set_framebuffer_info(uint32_t *fb, uint32_t width, uint32_t height,
                          uint32_t pitch);
void screen_refresh();
void screen_mark_static_dirty();
// ダーティレクトAPI
void screen_mark_dirty_rect(int x, int y, int w, int h);
void screen_mark_layer_dirty(const layer_t *l);
void screen_mark_all_dirty(void);
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
extern volatile int32_t mouse_scroll;
extern volatile uint32_t mouse_interrupt_counter;

// Arrow key custom ASCII codes
#define KEY_UP    0x11
#define KEY_DOWN  0x12
#define KEY_LEFT  0x13
#define KEY_RIGHT 0x14

// --- Multiboot ---
struct multiboot_mmap_entry {
  uint32_t size;
  uint64_t addr;
  uint64_t len;
  uint32_t type;
} __attribute__((packed));

struct multiboot_info {
  uint32_t flags;
  uint32_t mem_lower;
  uint32_t mem_upper;
  uint32_t boot_device;
  uint32_t cmdline;
  uint32_t mods_count;
  uint32_t mods_addr;
  uint32_t syms[4];
  uint32_t mmap_length;
  uint32_t mmap_addr;
  uint32_t drives_length;
  uint32_t drives_addr;
  uint32_t config_table;
  uint32_t boot_loader_name;
  uint32_t apm_table;
  uint32_t vbe_control_info;
  uint32_t vbe_mode_info;
  uint16_t vbe_mode;
  uint16_t vbe_interface_seg;
  uint16_t vbe_interface_off;
  uint16_t vbe_interface_len;
  uint64_t framebuffer_addr;
  uint32_t framebuffer_pitch;
  uint32_t framebuffer_width;
  uint32_t framebuffer_height;
  uint8_t framebuffer_bpp;
  uint8_t framebuffer_type;
  uint8_t color_info[6];
} __attribute__((packed));

void sys_restart(void);
void set_cursor_bitmap(uint32_t *bitmap, int w, int h);

#endif
