#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>  // malloc, free, realloc
#include <string.h>  // memcpy, memset
#include <stdio.h>   // FILE
#include <math.h>    // sinf, cosf
#include <stdarg.h>  // va_list

#include "drivers.h"
#include "svg_data.h"
#include <stddef.h>

#define NANOSVG_IMPLEMENTATION
#include "nanosvg/nanosvg.h"
#define NANOSVGRAST_IMPLEMENTATION
#include "nanosvg/nanosvgrast.h"

#define SVG_WIDTH NOTE_TEST_SVG_WIDTH
#define SVG_HEIGHT NOTE_TEST_SVG_HEIGHT

// メモリアロケータ
static char heap[1024 * 1024 * 4];
static uint32_t heap_ptr = 0;
typedef struct {
  void *ptr;
  size_t size;
} alloc_entry_t;
static alloc_entry_t allocs[1024];
static size_t alloc_count = 0;
void *memcpy(void *dest, const void *src, size_t n);
void *malloc(size_t size) {
  size = (size + 7) & ~7;
  if (heap_ptr + size > sizeof(heap))
    return NULL;
  void *ptr = &heap[heap_ptr];
  heap_ptr += size;
  if (alloc_count < (sizeof(allocs) / sizeof(allocs[0]))) {
    allocs[alloc_count].ptr = ptr;
    allocs[alloc_count].size = size;
    alloc_count++;
  } else {
    return NULL;
  }
  return ptr;
}
void free(void *ptr) {}
void *realloc(void *ptr, size_t size) {
  if (!ptr)
    return malloc(size);
  for (size_t i = 0; i < alloc_count; ++i) {
    if (allocs[i].ptr == ptr) {
      if (size <= allocs[i].size)
        return ptr;
      void *next = malloc(size);
      if (!next)
        return NULL;
      memcpy(next, ptr, allocs[i].size);
      return next;
    }
  }
  return malloc(size);
}
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

size_t strlen(const char *s) {
  size_t n = 0;
  if (!s)
    return 0;
  while (s[n])
    n++;
  return n;
}

int strcmp(const char *a, const char *b) {
  while (*a && (*a == *b)) {
    a++;
    b++;
  }
  return (unsigned char)*a - (unsigned char)*b;
}

int strncmp(const char *a, const char *b, size_t n) {
  for (size_t i = 0; i < n; ++i) {
    unsigned char ca = (unsigned char)a[i];
    unsigned char cb = (unsigned char)b[i];
    if (ca != cb || ca == 0 || cb == 0)
      return (int)ca - (int)cb;
  }
  return 0;
}

char *strncpy(char *dst, const char *src, size_t n) {
  size_t i = 0;
  for (; i < n && src[i]; ++i)
    dst[i] = src[i];
  for (; i < n; ++i)
    dst[i] = '\0';
  return dst;
}

char *strchr(const char *s, int c) {
  for (; *s; ++s) {
    if (*s == (char)c)
      return (char *)s;
  }
  return c == 0 ? (char *)s : NULL;
}

char *strrchr(const char *s, int c) {
  const char *last = NULL;
  for (; *s; ++s) {
    if (*s == (char)c)
      last = s;
  }
  if (c == 0)
    return (char *)s;
  return (char *)last;
}

char *strstr(const char *haystack, const char *needle) {
  if (!*needle)
    return (char *)haystack;
  for (const char *h = haystack; *h; ++h) {
    const char *h2 = h;
    const char *n = needle;
    while (*h2 && *n && (*h2 == *n)) {
      h2++;
      n++;
    }
    if (!*n)
      return (char *)h;
  }
  return NULL;
}

long strtol(const char *nptr, char **endptr, int base) {
  (void)base;
  const char *s = nptr;
  while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r' || *s == '\f' ||
         *s == '\v') {
    s++;
  }
  int sign = 1;
  if (*s == '-') {
    sign = -1;
    s++;
  } else if (*s == '+') {
    s++;
  }

  long val = 0;
  while (*s >= '0' && *s <= '9') {
    val = val * 10 + (*s - '0');
    s++;
  }
  if (endptr)
    *endptr = (char *)s;
  return val * sign;
}

long long strtoll(const char *nptr, char **endptr, int base) {
  (void)base;
  const char *s = nptr;
  while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r' || *s == '\f' ||
         *s == '\v') {
    s++;
  }
  int sign = 1;
  if (*s == '-') {
    sign = -1;
    s++;
  } else if (*s == '+') {
    s++;
  }

  long long val = 0;
  while (*s >= '0' && *s <= '9') {
    val = val * 10 + (*s - '0');
    s++;
  }
  if (endptr)
    *endptr = (char *)s;
  return val * sign;
}

double fabs(double x) { return x < 0.0 ? -x : x; }
float fabsf(float x) { return x < 0.0f ? -x : x; }

double sqrt(double x) {
  if (x <= 0.0)
    return 0.0;
  double r = x;
  for (int i = 0; i < 16; ++i)
    r = 0.5 * (r + x / r);
  return r;
}

float sqrtf(float x) {
  if (x <= 0.0f)
    return 0.0f;
  float r = x;
  for (int i = 0; i < 12; ++i)
    r = 0.5f * (r + x / r);
  return r;
}

double pow(double base, double exp) {
  long e = (long)exp;
  if ((double)e != exp)
    return 0.0;
  if (e == 0)
    return 1.0;
  int neg = 0;
  if (e < 0) {
    neg = 1;
    e = -e;
  }
  double result = 1.0;
  double b = base;
  while (e) {
    if (e & 1)
      result *= b;
    b *= b;
    e >>= 1;
  }
  return neg ? 1.0 / result : result;
}

float floorf(float x) {
  int i = (int)x;
  if ((float)i > x)
    i--;
  return (float)i;
}

float ceilf(float x) {
  int i = (int)x;
  if ((float)i < x)
    i++;
  return (float)i;
}

float roundf(float x) {
  return (x >= 0.0f) ? floorf(x + 0.5f) : ceilf(x - 0.5f);
}

float fmodf(float x, float y) {
  if (y == 0.0f)
    return 0.0f;
  int q = (int)(x / y);
  return x - (float)q * y;
}

static float wrap_pi(float x) {
  const float pi = 3.14159265358979323846f;
  const float two_pi = 6.28318530717958647692f;
  while (x > pi)
    x -= two_pi;
  while (x < -pi)
    x += two_pi;
  return x;
}

float sinf(float x) {
  x = wrap_pi(x);
  float x2 = x * x;
  return x * (1.0f - x2 / 6.0f + (x2 * x2) / 120.0f -
              (x2 * x2 * x2) / 5040.0f);
}

float cosf(float x) {
  x = wrap_pi(x);
  float x2 = x * x;
  return 1.0f - x2 / 2.0f + (x2 * x2) / 24.0f -
         (x2 * x2 * x2) / 720.0f;
}

float tanf(float x) {
  float c = cosf(x);
  if (c == 0.0f)
    return 0.0f;
  return sinf(x) / c;
}

static float atan_approx(float z) {
  const float pi = 3.14159265358979323846f;
  if (z > 1.0f)
    return (pi * 0.5f) - atan_approx(1.0f / z);
  if (z < -1.0f)
    return -(pi * 0.5f) - atan_approx(1.0f / z);
  return z / (1.0f + 0.28f * z * z);
}

float atan2f(float y, float x) {
  const float pi = 3.14159265358979323846f;
  if (x > 0.0f)
    return atan_approx(y / x);
  if (x < 0.0f) {
    if (y >= 0.0f)
      return atan_approx(y / x) + pi;
    return atan_approx(y / x) - pi;
  }
  if (y > 0.0f)
    return pi * 0.5f;
  if (y < 0.0f)
    return -pi * 0.5f;
  return 0.0f;
}

float acosf(float x) {
  const float pi = 3.14159265358979323846f;
  if (x <= -1.0f)
    return pi;
  if (x >= 1.0f)
    return 0.0f;
  return atan2f(sqrtf(1.0f - x * x), x);
}

int isnan(double x) { return x != x; }

static void swap_bytes(unsigned char *a, unsigned char *b, size_t size) {
  while (size--) {
    unsigned char tmp = *a;
    *a++ = *b;
    *b++ = tmp;
  }
}

void qsort(void *base, size_t nmemb, size_t size,
           int (*compar)(const void *, const void *)) {
  unsigned char *arr = (unsigned char *)base;
  for (size_t i = 1; i < nmemb; ++i) {
    size_t j = i;
    while (j > 0) {
      unsigned char *a = arr + (j - 1) * size;
      unsigned char *b = arr + j * size;
      if (compar(a, b) <= 0)
        break;
      swap_bytes(a, b, size);
      --j;
    }
  }
}

FILE *fopen(const char *path, const char *mode) {
  (void)path;
  (void)mode;
  return NULL;
}

int fclose(FILE *stream) {
  (void)stream;
  return 0;
}

size_t fread(void *ptr, size_t size, size_t nmemb, FILE *stream) {
  (void)ptr;
  (void)size;
  (void)nmemb;
  (void)stream;
  return 0;
}

int fseek(FILE *stream, long offset, int whence) {
  (void)stream;
  (void)offset;
  (void)whence;
  return -1;
}

long ftell(FILE *stream) {
  (void)stream;
  return -1;
}

int fprintf(FILE *stream, const char *format, ...) {
  (void)stream;
  (void)format;
  return 0;
}

static int hex_value(char c) {
  if (c >= '0' && c <= '9')
    return c - '0';
  if (c >= 'a' && c <= 'f')
    return 10 + (c - 'a');
  if (c >= 'A' && c <= 'F')
    return 10 + (c - 'A');
  return -1;
}

int sscanf(const char *str, const char *format, ...) {
  va_list args;
  va_start(args, format);

  int matched = 0;
  if (strcmp(format, "#%2x%2x%2x") == 0) {
    if (str && str[0] == '#') {
      int v0 = hex_value(str[1]);
      int v1 = hex_value(str[2]);
      int v2 = hex_value(str[3]);
      int v3 = hex_value(str[4]);
      int v4 = hex_value(str[5]);
      int v5 = hex_value(str[6]);
      if (v0 >= 0 && v1 >= 0 && v2 >= 0 && v3 >= 0 && v4 >= 0 && v5 >= 0) {
        unsigned int *r = va_arg(args, unsigned int *);
        unsigned int *g = va_arg(args, unsigned int *);
        unsigned int *b = va_arg(args, unsigned int *);
        *r = (unsigned int)((v0 << 4) | v1);
        *g = (unsigned int)((v2 << 4) | v3);
        *b = (unsigned int)((v4 << 4) | v5);
        matched = 3;
      }
    }
  } else if (strcmp(format, "#%1x%1x%1x") == 0) {
    if (str && str[0] == '#') {
      int v0 = hex_value(str[1]);
      int v1 = hex_value(str[2]);
      int v2 = hex_value(str[3]);
      if (v0 >= 0 && v1 >= 0 && v2 >= 0) {
        unsigned int *r = va_arg(args, unsigned int *);
        unsigned int *g = va_arg(args, unsigned int *);
        unsigned int *b = va_arg(args, unsigned int *);
        *r = (unsigned int)v0;
        *g = (unsigned int)v1;
        *b = (unsigned int)v2;
        matched = 3;
      }
    }
  } else if (strcmp(format, "rgb(%u, %u, %u)") == 0) {
    if (str && strncmp(str, "rgb(", 4) == 0) {
      const char *p = str + 4;
      unsigned int *r = va_arg(args, unsigned int *);
      unsigned int *g = va_arg(args, unsigned int *);
      unsigned int *b = va_arg(args, unsigned int *);
      *r = (unsigned int)strtoll(p, (char **)&p, 10);
      while (*p == ' ' || *p == ',')
        p++;
      *g = (unsigned int)strtoll(p, (char **)&p, 10);
      while (*p == ' ' || *p == ',')
        p++;
      *b = (unsigned int)strtoll(p, (char **)&p, 10);
      matched = 3;
    }
  }

  va_end(args);
  return matched;
}

static void draw_svg_layer(layer_t *layer) {
  layer_fill(layer, 0xFFFBF8FF);

  char *svg_copy = (char *)malloc(note_test_svg_len + 1);
  if (!svg_copy)
    return;
  memcpy(svg_copy, note_test_svg, note_test_svg_len);
  svg_copy[note_test_svg_len] = '\0';

  NSVGimage *image = nsvgParse(svg_copy, "px", 96.0f);
  if (!image)
    return;

  NSVGrasterizer *rast = nsvgCreateRasterizer();
  if (!rast)
    return;

  unsigned char *rgba =
      (unsigned char *)malloc((size_t)layer->width * (size_t)layer->height * 4);
  if (!rgba)
    return;

  float scale_x =
      image->width > 0.0f ? (float)layer->width / image->width : 1.0f;
  float scale_y =
      image->height > 0.0f ? (float)layer->height / image->height : 1.0f;
  float scale = scale_x < scale_y ? scale_x : scale_y;
  float tx = 0.0f;
  float ty = 0.0f;

  nsvgRasterize(rast, image, tx, ty, scale, rgba, layer->width, layer->height,
                layer->width * 4);

  const uint32_t bg = 0xFFFBF8FF;
  for (int y = 0; y < layer->height; ++y) {
    for (int x = 0; x < layer->width; ++x) {
      size_t idx = (size_t)(y * layer->width + x) * 4;
      uint8_t r = rgba[idx + 0];
      uint8_t g = rgba[idx + 1];
      uint8_t b = rgba[idx + 2];
      uint8_t a = rgba[idx + 3];

      uint8_t bg_r = (bg >> 16) & 0xFF;
      uint8_t bg_g = (bg >> 8) & 0xFF;
      uint8_t bg_b = bg & 0xFF;

      uint8_t out_r = (uint8_t)((r * a + bg_r * (255 - a)) / 255);
      uint8_t out_g = (uint8_t)((g * a + bg_g * (255 - a)) / 255);
      uint8_t out_b = (uint8_t)((b * a + bg_b * (255 - a)) / 255);

      layer->buffer[y * layer->width + x] =
          (0xFFu << 24) | ((uint32_t)out_r << 16) | ((uint32_t)out_g << 8) |
          (uint32_t)out_b;
    }
  }
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
  svg_layer.x = 0;
  svg_layer.y = 0;
  svg_layer.width = SVG_WIDTH;
  svg_layer.height = SVG_HEIGHT;
  svg_layer.transparent = 0;
  svg_layer.active = 1;
  draw_svg_layer(&svg_layer);
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
