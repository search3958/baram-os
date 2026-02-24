#include "drivers.h"
#include "font8x8_basic.h"
#include <stddef.h>
#include <stdint.h>

// ==========================================
// グラフィックス・バックバッファ
// ==========================================

static uint32_t *g_vram = NULL;
static uint32_t g_vram_width = 0;
static uint32_t g_vram_height = 0;
static uint32_t g_vram_pitch = 0;

// バックバッファ (1280x720 32bpp = 約3.6MB)
static uint32_t g_backbuffer[SCREEN_WIDTH * SCREEN_HEIGHT];

// レイヤー管理 (将来的に動的化も可能)
#define MAX_LAYERS 8
static layer_t *g_layers[MAX_LAYERS];
static int g_num_layers = 0;

void set_framebuffer_info(uint32_t *fb, uint32_t width, uint32_t height,
                          uint32_t pitch) {
  g_vram = fb;
  g_vram_width = width;
  g_vram_height = height;
  g_vram_pitch = pitch;
}

void register_layer(layer_t *layer) {
  if (g_num_layers < MAX_LAYERS) {
    g_layers[g_num_layers++] = layer;
  }
}

// 全レイヤーをバックバッファに合成し、VRAMに転送
void screen_refresh() {
  if (!g_vram)
    return;

  // 1. バックバッファを毎フレームクリア（active=0レイヤーの残像防止）
  for (uint32_t i = 0; i < SCREEN_WIDTH * SCREEN_HEIGHT; i++)
    g_backbuffer[i] = 0xFF000000;

  for (int i = 0; i < g_num_layers; i++) {
    layer_t *l = g_layers[i];
    if (!l->active || !l->buffer)
      continue;

    for (int y = 0; y < l->height; y++) {
      int screen_y = l->y + y;
      if (screen_y < 0 || screen_y >= (int)g_vram_height)
        continue;

      for (int x = 0; x < l->width; x++) {
        int screen_x = l->x + x;
        if (screen_x < 0 || screen_x >= (int)g_vram_width)
          continue;

        uint32_t color = l->buffer[y * l->width + x];
        if (l->transparent != 0 && color == l->transparent)
          continue;

        g_backbuffer[screen_y * SCREEN_WIDTH + screen_x] = color;
      }
    }
  }

  // 最後にマウスを合成（マウス専用レイヤーがない場合の簡易実装）
  uint32_t white = 0xFFFFFFFF;
  for (int my = 0; my < 15; my++) {
    for (int mx = 0; mx < 10; mx++) {
      int sx = mouse_x + mx;
      int sy = mouse_y + my;
      if (sx >= 0 && sx < (int)g_vram_width && sy >= 0 &&
          sy < (int)g_vram_height) {
        g_backbuffer[sy * SCREEN_WIDTH + sx] = white;
      }
    }
  }

  // バックバッファからVRAMへ転送 (Flip)
  for (uint32_t y = 0; y < g_vram_height; y++) {
    uint32_t *dest = (uint32_t *)((uint8_t *)g_vram + y * g_vram_pitch);
    uint32_t *src = &g_backbuffer[y * SCREEN_WIDTH];
    for (uint32_t x = 0; x < g_vram_width; x++) {
      dest[x] = src[x];
    }
  }
}

void layer_fill(layer_t *layer, uint32_t color) {
  for (int i = 0; i < layer->width * layer->height; i++) {
    layer->buffer[i] = color;
  }
}

void layer_draw_char(layer_t *layer, int x, int y, char c, uint32_t color,
                     uint32_t bg_color) {
  if (c < 0 || c > 127)
    return;
  for (int row = 0; row < 8; row++) {
    uint8_t bits = font8x8_basic[(int)c][row];
    for (int col = 0; col < 8; col++) {
      int px = x + col;
      int py = y + row;
      if (px >= 0 && px < layer->width && py >= 0 && py < layer->height) {
        if (bits & (1 << col)) {
          layer->buffer[py * layer->width + px] = color;
        } else if (bg_color != TRANSPARENT_COLOR) {
          layer->buffer[py * layer->width + px] = bg_color;
        }
      }
    }
  }
}

void layer_draw_string(layer_t *layer, int x, int y, const char *str,
                       uint32_t color, uint32_t bg_color) {
  int cx = x;
  while (*str) {
    layer_draw_char(layer, cx, y, *str, color, bg_color);
    cx += 8;
    str++;
  }
}

// ==========================================
// IDT / IRQ
// ==========================================

struct idt_entry idt[256];
struct idt_ptr idtp;

void idt_set_gate(uint8_t num, uintptr_t base, uint16_t sel, uint8_t flags) {
  idt[num].base_lo = (base & 0xFFFF);
  idt[num].base_hi = (base >> 16) & 0xFFFF;
  idt[num].sel = sel;
  idt[num].always0 = 0;
  idt[num].flags = flags;
}

void idt_install() {
  idtp.limit = (sizeof(struct idt_entry) * 256) - 1;
  idtp.base = (uint32_t)(uintptr_t)&idt;

  for (int i = 0; i < 256; i++) {
    idt[i].base_lo = 0;
    idt[i].base_hi = 0;
    idt[i].sel = 0;
    idt[i].always0 = 0;
    idt[i].flags = 0;
  }

  __asm__ __volatile__("lidt %0" : : "m"(idtp) : "memory");
}

irq_handler_t irq_routines[16] = {0};

void irq_install_handler(int irq, irq_handler_t handler) {
  irq_routines[irq] = handler;
}

void irq_uninstall_handler(int irq) { irq_routines[irq] = 0; }

void pic_remap(void) {
  outb(0x20, 0x11);
  outb(0x21, 0x20);
  outb(0x21, 0x04);
  outb(0x21, 0x01);

  outb(0xA0, 0x11);
  outb(0xA1, 0x28);
  outb(0xA1, 0x02);
  outb(0xA1, 0x01);

  // IRQ0(timer) と IRQ2(slave PIC cascade) を有効化。
  // IRQ1(keyboard) / IRQ12(mouse) は各 install 時に有効化する。
  outb(0x21, 0xFA);
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

  idt_set_gate(32, (uintptr_t)irq0, 0x08, 0x8E);
  idt_set_gate(33, (uintptr_t)irq1, 0x08, 0x8E);
  idt_set_gate(34, (uintptr_t)irq2, 0x08, 0x8E);
  idt_set_gate(35, (uintptr_t)irq3, 0x08, 0x8E);
  idt_set_gate(36, (uintptr_t)irq4, 0x08, 0x8E);
  idt_set_gate(37, (uintptr_t)irq5, 0x08, 0x8E);
  idt_set_gate(38, (uintptr_t)irq6, 0x08, 0x8E);
  idt_set_gate(39, (uintptr_t)irq7, 0x08, 0x8E);
  idt_set_gate(40, (uintptr_t)irq8, 0x08, 0x8E);
  idt_set_gate(41, (uintptr_t)irq9, 0x08, 0x8E);
  idt_set_gate(42, (uintptr_t)irq10, 0x08, 0x8E);
  idt_set_gate(43, (uintptr_t)irq11, 0x08, 0x8E);
  idt_set_gate(44, (uintptr_t)irq12, 0x08, 0x8E);
  idt_set_gate(45, (uintptr_t)irq13, 0x08, 0x8E);
  idt_set_gate(46, (uintptr_t)irq14, 0x08, 0x8E);
  idt_set_gate(47, (uintptr_t)irq15, 0x08, 0x8E);

  idt_set_gate(0, (uintptr_t)isr0, 0x08, 0x8E);
  idt_set_gate(1, (uintptr_t)isr1, 0x08, 0x8E);
  idt_set_gate(2, (uintptr_t)isr2, 0x08, 0x8E);
  idt_set_gate(3, (uintptr_t)isr3, 0x08, 0x8E);
  idt_set_gate(4, (uintptr_t)isr4, 0x08, 0x8E);
  idt_set_gate(5, (uintptr_t)isr5, 0x08, 0x8E);
  idt_set_gate(6, (uintptr_t)isr6, 0x08, 0x8E);
  idt_set_gate(7, (uintptr_t)isr7, 0x08, 0x8E);
  idt_set_gate(8, (uintptr_t)isr8, 0x08, 0x8E);
  idt_set_gate(9, (uintptr_t)isr9, 0x08, 0x8E);
  idt_set_gate(10, (uintptr_t)isr10, 0x08, 0x8E);
  idt_set_gate(11, (uintptr_t)isr11, 0x08, 0x8E);
  idt_set_gate(12, (uintptr_t)isr12, 0x08, 0x8E);
  idt_set_gate(13, (uintptr_t)isr13, 0x08, 0x8E);
  idt_set_gate(14, (uintptr_t)isr14, 0x08, 0x8E);
  idt_set_gate(15, (uintptr_t)isr15, 0x08, 0x8E);
  idt_set_gate(16, (uintptr_t)isr16, 0x08, 0x8E);
  idt_set_gate(17, (uintptr_t)isr17, 0x08, 0x8E);
  idt_set_gate(18, (uintptr_t)isr18, 0x08, 0x8E);
  idt_set_gate(19, (uintptr_t)isr19, 0x08, 0x8E);
  idt_set_gate(20, (uintptr_t)isr20, 0x08, 0x8E);
  idt_set_gate(21, (uintptr_t)isr21, 0x08, 0x8E);
  idt_set_gate(22, (uintptr_t)isr22, 0x08, 0x8E);
  idt_set_gate(23, (uintptr_t)isr23, 0x08, 0x8E);
  idt_set_gate(24, (uintptr_t)isr24, 0x08, 0x8E);
  idt_set_gate(25, (uintptr_t)isr25, 0x08, 0x8E);
  idt_set_gate(26, (uintptr_t)isr26, 0x08, 0x8E);
  idt_set_gate(27, (uintptr_t)isr27, 0x08, 0x8E);
  idt_set_gate(28, (uintptr_t)isr28, 0x08, 0x8E);
  idt_set_gate(29, (uintptr_t)isr29, 0x08, 0x8E);
  idt_set_gate(30, (uintptr_t)isr30, 0x08, 0x8E);
  idt_set_gate(31, (uintptr_t)isr31, 0x08, 0x8E);
}

void enable_interrupts() { __asm__ __volatile__("sti"); }

// ==========================================
// キーボード
// ==========================================
#define KEYBUF_SIZE 256
volatile char keybuf[KEYBUF_SIZE];
volatile int keybuf_len = 0;

static const char scancode_to_ascii[128] = {
    0,   0,   '1', '2', '3', '4', '5', '6', '7', '8', '9',  '0', '-', '=',  0,
    0,   'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p',  '[', ']', '\\', 0,
    'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\'', 0,   'z', 'x',  'c',
    'v', 'b', 'n', 'm', ',', '.', '/', 0,   0,   0,   0,    0,   0,   0,    0,
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,    0,   0,   0,    0,
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,    0,   0,   0,    0,
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,    0,   0,   0,    0,
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0};

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
  uint8_t mask = inb(0x21);
  mask &= ~(1 << 1);
  outb(0x21, mask);
}

// ==========================================
// 割り込み共通
// ==========================================

void irq_handler(struct regs *r) {
  void (*handler)(struct regs *r);
  int irq_num = r->int_no - 32;

  handler = irq_routines[irq_num];
  if (handler) {
    handler(r);
  }

  if (irq_num >= 8) {
    outb(0xA0, 0x20);
  }
  outb(0x20, 0x20);
}

void exception_handler(struct regs *r) {
  // 例外発生時は画面を白くするなどの簡易処理
  for (int i = 0; i < SCREEN_WIDTH * SCREEN_HEIGHT; i++)
    g_backbuffer[i] = 0xFFFFFFFF;
  screen_refresh();
  while (1)
    ;
}

// ==========================================
// マウス
// ==========================================

volatile int32_t mouse_x = 0;
volatile int32_t mouse_y = 0;
static uint8_t mouse_cycle = 0;
static int8_t mouse_packet[3];
volatile uint32_t mouse_interrupt_counter = 0;

void mouse_wait(uint8_t a_type) {
  uint32_t timeout = 100000;
  if (a_type == 0) {
    while (timeout--)
      if ((inb(0x64) & 2) == 0)
        return;
  } else {
    while (timeout--)
      if ((inb(0x64) & 1) == 1)
        return;
  }
}

void mouse_write(uint8_t a_write) {
  mouse_wait(0);
  outb(0x64, 0xD4);
  mouse_wait(0);
  outb(0x60, a_write);
}

uint8_t mouse_read() {
  mouse_wait(1);
  return inb(0x60);
}

static void mouse_handler(struct regs *r) {
  mouse_interrupt_counter++;
  uint8_t data = inb(0x60);
  switch (mouse_cycle) {
  case 0:
    mouse_packet[0] = data;
    mouse_cycle++;
    break;
  case 1:
    mouse_packet[1] = data;
    mouse_cycle++;
    break;
  case 2:
    mouse_packet[2] = data;
    mouse_cycle = 0;
    int dx = mouse_packet[1];
    int dy = mouse_packet[2];
    if (mouse_packet[0] & 0x10)
      dx -= 256;
    if (mouse_packet[0] & 0x20)
      dy -= 256;
    mouse_x += dx;
    mouse_y -= dy;
    if (mouse_x < 0)
      mouse_x = 0;
    if (mouse_y < 0)
      mouse_y = 0;
    if (mouse_x > SCREEN_WIDTH - 1)
      mouse_x = SCREEN_WIDTH - 1;
    if (mouse_y > SCREEN_HEIGHT - 1)
      mouse_y = SCREEN_HEIGHT - 1;
    break;
  }
}

void mouse_install() {
  mouse_wait(0);
  outb(0x64, 0xA8);
  mouse_wait(0);
  outb(0x64, 0x20);
  mouse_wait(1);
  uint8_t status = inb(0x60);
  status |= 2;
  mouse_wait(0);
  outb(0x64, 0x60);
  mouse_wait(0);
  outb(0x60, status);
  mouse_write(0xF6);
  mouse_read();
  mouse_write(0xF4);
  mouse_read();
  irq_install_handler(12, mouse_handler);
  uint8_t mask = inb(0xA1);
  mask &= ~(1 << 4);
  outb(0xA1, mask);
}
