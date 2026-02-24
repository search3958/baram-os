#include "drivers.h"
#include "svg_pixels.h"
#include <stddef.h>

// ThorVGがリンクエラーになるため、一旦スタブ（空関数）を定義してビルドを通します
// 本来は ThorVG のライブラリ (.a や .o) をリンクする必要があります
typedef void *Tvg_Canvas;
typedef void *Tvg_Paint;
typedef int Tvg_Result;
#define TVG_RESULT_SUCCESS 0

// メモリアロケータ
static char heap[1024 * 1024 * 4];
static uint32_t heap_ptr = 0;
void *malloc(size_t size) {
  size = (size + 7) & ~7;
  if (heap_ptr + size > sizeof(heap))
    return NULL;
  void *ptr = &heap[heap_ptr];
  heap_ptr += size;
  return ptr;
}
void free(void *ptr) {}
void *realloc(void *ptr, size_t size) { return malloc(size); }
void *memset(void *s, int c, size_t n) {
  unsigned char *p = s;
  while (n--)
    *p++ = (unsigned char)c;
  return s;
}
void *memcpy(void *dest, const void *src, size_t n) {
  unsigned char *d = dest;
  const unsigned char *s = src;
  while (n--)
    *d++ = *s++;
  return dest;
}

// タイマー設定 (0.1秒点滅用)
volatile uint32_t timer_ticks = 0;
void timer_handler(struct regs *r) { timer_ticks++; }

void timer_phase(int hz) {
  int divisor = 1193180 / hz;
  outb(0x43, 0x36);
  outb(0x40, divisor & 0xFF);
  outb(0x40, (divisor >> 8) & 0xFF);
}

// レイヤー用
static uint32_t desktop_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
static uint32_t svg_buf[SVG_WIDTH * SVG_HEIGHT];
static uint32_t blink_buf[50 * 50];

extern void register_layer(layer_t *layer);
extern volatile char keybuf[];
extern volatile int keybuf_len;

void kmain(uint32_t magic, struct multiboot_info *mbi) {
  set_framebuffer_info((uint32_t *)(uintptr_t)mbi->framebuffer_addr,
                       mbi->framebuffer_width, mbi->framebuffer_height,
                       mbi->framebuffer_pitch);

  // 1. 背景 (赤)
  layer_t desktop;
  desktop.buffer = desktop_buf;
  desktop.x = 0;
  desktop.y = 0;
  desktop.width = SCREEN_WIDTH;
  desktop.height = SCREEN_HEIGHT;
  desktop.transparent = 0;
  desktop.active = 1;
  layer_fill(&desktop, 0xFFFF0000);
  register_layer(&desktop);

  // 2. SVG表示エリア (左上)
  layer_t svg_layer;
  svg_layer.buffer = svg_buf;
  svg_layer.x = 10;
  svg_layer.y = 10;
  svg_layer.width = SVG_WIDTH;
  svg_layer.height = SVG_HEIGHT;
  svg_layer.transparent = 0;
  svg_layer.active = 1;
  memcpy(svg_layer.buffer, svg_pixels, sizeof(svg_pixels));
  register_layer(&svg_layer);

  // 3. 点滅ボックス (左下)
  layer_t blink_layer;
  blink_layer.buffer = blink_buf;
  blink_layer.x = 10;
  blink_layer.y = SCREEN_HEIGHT - 60;
  blink_layer.width = 50;
  blink_layer.height = 50;
  blink_layer.transparent = 0;
  blink_layer.active = 1;
  layer_fill(&blink_layer, 0xFF0000FF); // 青
  register_layer(&blink_layer);

  idt_install();
  irq_install();
  irq_install_handler(0, timer_handler);
  timer_phase(100); // 100Hz = 10ms/tick
  keyboard_install();
  mouse_install();
  enable_interrupts();

  uint32_t last_blink_tick = 0;
  int blink_state = 1;

  while (1) {
    // 0.1秒(10 ticks)ごとに点滅
    if (timer_ticks - last_blink_tick >= 10) {
      blink_state = !blink_state;
      blink_layer.active = blink_state;
      last_blink_tick = timer_ticks;
    }
    screen_refresh();
  }
}
