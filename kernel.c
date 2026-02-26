#include <math.h>   // sinf, cosf
#include <stdarg.h> // va_list
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>  // FILE
#include <stdlib.h> // malloc, free, realloc
#include <string.h> // memcpy, memset

#include "drivers.h"
#include "fonts.h"
#include "svg_data.h"
#include <stddef.h>

#define NANOSVG_IMPLEMENTATION
#include "nanosvg/nanosvg.h"
#define NANOSVGRAST_IMPLEMENTATION
#include "nanosvg/nanosvgrast.h"

#define SVG_WIDTH NOTE_TEST_SVG_WIDTH
#define SVG_HEIGHT NOTE_TEST_SVG_HEIGHT
#define BASE_BG_COLOR 0xFF8B0000u
#define HOVER_SCALE 1.2f
#define HOVER_EASE 0.1f

typedef struct {
  NSVGshape *shape;
  unsigned char *rgba;
  uint8_t flags;
  int x;
  int y;
  int w;
  int h;
} svg_shape_cache_t;

static NSVGimage *g_svg_image = NULL;
static NSVGrasterizer *g_svg_rast = NULL;
static unsigned char *g_svg_rgba = NULL;
static svg_shape_cache_t *g_svg_cache = NULL;
static int g_svg_shape_count = 0;
static unsigned char *g_svg_hover_buf = NULL;
static size_t g_svg_hover_buf_cap = 0;
static float g_svg_scale = 1.0f;
static float g_svg_tx = 0.0f;
static float g_svg_ty = 0.0f;
static int g_svg_ready = 0;

static volatile uint32_t idle_ticks = 0;
static volatile int cpu_idle = 0;

// FPU有効化
void enable_fpu() {
  unsigned long cr0;
  __asm__ __volatile__("mov %%cr0, %0" : "=r"(cr0));
  cr0 &= ~(1 << 2); // EM ビット解除 (エミュレーション無効)
  cr0 |= (1 << 1);  // MP ビット設定
  __asm__ __volatile__("mov %0, %%cr0" : : "r"(cr0));
  unsigned long cr4;
  __asm__ __volatile__("mov %%cr4, %0" : "=r"(cr4));
  cr4 |= (3 << 9); // OSFXSR と OSXMMEXCPT ビット設定
  __asm__ __volatile__("mov %0, %%cr4" : : "r"(cr4));
  __asm__ __volatile__("finit");
}

// ソフトウェア浮動小数点のヘルパー関数をハードウェアFPU(インラインアセンブリ)で実装
// ターゲット属性が効かない環境でも直接命令を発行することで無限再帰を避ける
float __mulsf3(float a, float b) {
  float r;
  __asm__("flds %1; fmuls %2; fstps %0" : "=m"(r) : "m"(a), "m"(b));
  return r;
}
float __addsf3(float a, float b) {
  float r;
  __asm__("flds %1; fadds %2; fstps %0" : "=m"(r) : "m"(a), "m"(b));
  return r;
}
float __subsf3(float a, float b) {
  float r;
  __asm__("flds %1; fsubs %2; fstps %0" : "=m"(r) : "m"(a), "m"(b));
  return r;
}
float __divsf3(float a, float b) {
  float r;
  __asm__("flds %1; fdivs %2; fstps %0" : "=m"(r) : "m"(a), "m"(b));
  return r;
}
int __gtsf2(float a, float b) { return a > b; }
int __ltsf2(float a, float b) { return a < b; }
int __nesf2(float a, float b) { return a != b; }
int __eqsf2(float a, float b) { return a == b; }
int __gesf2(float a, float b) { return a >= b; }
int __lesf2(float a, float b) { return a <= b; }
float __floatsisf(int i) {
  float r;
  __asm__("fildl %1; fstps %0" : "=m"(r) : "m"(i));
  return r;
}
int __fixsfsi(float f) {
  int r;
  __asm__("flds %1; fistpl %0" : "=m"(r) : "m"(f));
  return r;
}

double __muldf3(double a, double b) {
  double r;
  __asm__("fldl %1; fmull %2; fstpl %0" : "=m"(r) : "m"(a), "m"(b));
  return r;
}
double __adddf3(double a, double b) {
  double r;
  __asm__("fldl %1; faddl %2; fstpl %0" : "=m"(r) : "m"(a), "m"(b));
  return r;
}
double __subdf3(double a, double b) {
  double r;
  __asm__("fldl %1; fsubl %2; fstpl %0" : "=m"(r) : "m"(a), "m"(b));
  return r;
}
double __divdf3(double a, double b) {
  double r;
  __asm__("fldl %1; fdivl %2; fstpl %0" : "=m"(r) : "m"(a), "m"(b));
  return r;
}
int __gtdf2(double a, double b) { return a > b; }
int __ltdf2(double a, double b) { return a < b; }
int __nedf2(double a, double b) { return a != b; }
int __ledf2(double a, double b) { return a <= b; }
double __floatsidf(int i) {
  double r;
  __asm__("fildl %1; fstpl %0" : "=m"(r) : "m"(i));
  return r;
}
double __extendsfdf2(float f) {
  double r;
  __asm__("flds %1; fstpl %0" : "=m"(r) : "m"(f));
  return r;
}
float __truncdfsf2(double d) {
  float r;
  __asm__("fldl %1; fstps %0" : "=m"(r) : "m"(d));
  return r;
}
int __fixdfsi(double d) {
  int r;
  __asm__("fldl %1; fistpl %0" : "=m"(r) : "m"(d));
  return r;
}

// long double 用の libgcc
// ヘルパは使わないので未定義のままでよい（参照もしていない）

// レイヤー用
static uint32_t desktop_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
static uint32_t svg_buf[SVG_WIDTH * SVG_HEIGHT];
static uint32_t svg_base_buf[SVG_WIDTH * SVG_HEIGHT];
static uint32_t blink_buf[50 * 50];
static uint32_t hud_buf[240 * 16];

// 起動中の進捗表示に使うデスクトップレイヤー
static layer_t *g_boot_status_layer = NULL;

// 進捗表示関数のプロトタイプ（C89 の暗黙宣言を避ける）
static void boot_status_update(int stage, int total_stages, const char *label);

// メモリアロケータ
#undef memcpy
#undef memset
#undef strncpy

static char heap[1024 * 1024 * 16];
static uint32_t heap_ptr = 0;
typedef struct {
  void *ptr;
  size_t size;
} alloc_entry_t;
static alloc_entry_t allocs[8192];
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

  if (isnan((double)x))
    return 0.0f;
  if (x > 1e12f || x < -1e12f)
    return 0.0f; // 極端に大きい値は大まかに 0 に（無限ループ防止）

  while (x > pi)
    x -= two_pi;
  while (x < -pi)
    x += two_pi;
  return x;
}

float sinf(float x) {
  x = wrap_pi(x);
  float x2 = x * x;
  return x * (1.0f - x2 / 6.0f + (x2 * x2) / 120.0f - (x2 * x2 * x2) / 5040.0f);
}

float cosf(float x) {
  x = wrap_pi(x);
  float x2 = x * x;
  return 1.0f - x2 / 2.0f + (x2 * x2) / 24.0f - (x2 * x2 * x2) / 720.0f;
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

#undef isnan
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

static void svg_render_full(layer_t *layer) {
  nsvgRasterize(g_svg_rast, g_svg_image, g_svg_tx, g_svg_ty, g_svg_scale,
                g_svg_rgba, layer->width, layer->height, layer->width * 4);

  const uint32_t bg = BASE_BG_COLOR;
  uint8_t bg_r = (bg >> 16) & 0xFF;
  uint8_t bg_g = (bg >> 8) & 0xFF;
  uint8_t bg_b = bg & 0xFF;

  for (int y = 0; y < layer->height; ++y) {
    for (int x = 0; x < layer->width; ++x) {
      size_t idx = (size_t)(y * layer->width + x) * 4;
      uint8_t r = g_svg_rgba[idx + 0];
      uint8_t g = g_svg_rgba[idx + 1];
      uint8_t b = g_svg_rgba[idx + 2];
      uint8_t a = g_svg_rgba[idx + 3];

      uint8_t out_r = (uint8_t)((r * a + bg_r * (255 - a)) / 255);
      uint8_t out_g = (uint8_t)((g * a + bg_g * (255 - a)) / 255);
      uint8_t out_b = (uint8_t)((b * a + bg_b * (255 - a)) / 255);

      layer->buffer[y * layer->width + x] =
          (0xFFu << 24) | ((uint32_t)out_r << 16) | ((uint32_t)out_g << 8) |
          (uint32_t)out_b;
    }
  }
}

static int svg_init(layer_t *layer) {
  if (g_svg_ready)
    return 1;

  boot_status_update(41, 100, "SVG: CLEAR LAYER");
  layer_fill(layer, BASE_BG_COLOR);

  boot_status_update(42, 100, "SVG: COPY SOURCE");
  char *svg_copy = (char *)malloc(note_test_svg_len + 1);
  if (!svg_copy)
    return 0;
  memcpy(svg_copy, note_test_svg, note_test_svg_len);
  svg_copy[note_test_svg_len] = '\0';

  boot_status_update(44, 100, "SVG: PARSE");
  g_svg_image = nsvgParse(svg_copy, "px", 96.0f);
  if (!g_svg_image)
    return 0;

  boot_status_update(46, 100, "SVG: RASTERIZER");
  g_svg_rast = nsvgCreateRasterizer();
  if (!g_svg_rast)
    return 0;

  boot_status_update(48, 100, "SVG: RGBA BUFFER");
  g_svg_rgba =
      (unsigned char *)malloc((size_t)layer->width * (size_t)layer->height * 4);
  if (!g_svg_rgba)
    return 0;

  float scale_x = g_svg_image->width > 0.0f
                      ? (float)layer->width / g_svg_image->width
                      : 1.0f;
  float scale_y = g_svg_image->height > 0.0f
                      ? (float)layer->height / g_svg_image->height
                      : 1.0f;
  g_svg_scale = scale_x < scale_y ? scale_x : scale_y;
  g_svg_tx = 0.0f;
  g_svg_ty = 0.0f;

  boot_status_update(52, 100, "SVG: COUNT SHAPES");
  int count = 0;
  for (NSVGshape *s = g_svg_image->shapes; s; s = s->next)
    count++;

  if (count > 0) {
    boot_status_update(54, 100, "SVG: ALLOC CACHE");
    g_svg_cache =
        (svg_shape_cache_t *)malloc(sizeof(svg_shape_cache_t) * (size_t)count);
    if (g_svg_cache) {
      int i = 0;
      for (NSVGshape *s = g_svg_image->shapes; s; s = s->next) {
        g_svg_cache[i].shape = s;
        g_svg_cache[i].rgba = NULL;
        g_svg_cache[i].flags = s->flags;
        g_svg_cache[i].x = 0;
        g_svg_cache[i].y = 0;
        g_svg_cache[i].w = 0;
        g_svg_cache[i].h = 0;
        i++;
      }
      g_svg_shape_count = count;
    }
  }

  if (g_svg_cache) {
    boot_status_update(56, 100, "SVG: PRE-RENDER");
    for (int i = 0; i < g_svg_shape_count; ++i) {
      g_svg_cache[i].shape->flags = 0;
    }

    for (int i = 0; i < g_svg_shape_count; ++i) {
      svg_shape_cache_t *c = &g_svg_cache[i];
      if ((c->flags & NSVG_FLAGS_VISIBLE) == 0)
        continue;

      float x0f = c->shape->bounds[0] * g_svg_scale + g_svg_tx;
      float y0f = c->shape->bounds[1] * g_svg_scale + g_svg_ty;
      float x1f = c->shape->bounds[2] * g_svg_scale + g_svg_tx;
      float y1f = c->shape->bounds[3] * g_svg_scale + g_svg_ty;

      float padf = c->shape->strokeWidth * g_svg_scale + 3.0f;
      int pad = (int)ceilf(padf);
      if (pad < 2)
        pad = 2;
      int x0 = (int)floorf(x0f) - pad;
      int y0 = (int)floorf(y0f) - pad;
      int x1 = (int)ceilf(x1f) + pad;
      int y1 = (int)ceilf(y1f) + pad;

      if (x0 < 0)
        x0 = 0;
      if (y0 < 0)
        y0 = 0;
      if (x1 > layer->width)
        x1 = layer->width;
      if (y1 > layer->height)
        y1 = layer->height;

      int w = x1 - x0;
      int h = y1 - y0;
      if (w <= 0 || h <= 0)
        continue;

      unsigned char *buf = (unsigned char *)malloc((size_t)w * (size_t)h * 4);
      if (!buf)
        continue;

      c->rgba = buf;
      c->x = x0;
      c->y = y0;
      c->w = w;
      c->h = h;

      c->shape->flags = NSVG_FLAGS_VISIBLE;
      nsvgRasterize(g_svg_rast, g_svg_image, g_svg_tx - (float)x0,
                    g_svg_ty - (float)y0, g_svg_scale, buf, w, h, w * 4);
      c->shape->flags = 0;
    }

    for (int i = 0; i < g_svg_shape_count; ++i) {
      g_svg_cache[i].shape->flags = g_svg_cache[i].flags;
    }
  }

  boot_status_update(60, 100, "SVG: RENDER FULL");
  svg_render_full(layer);
  memcpy(svg_base_buf, layer->buffer,
         sizeof(uint32_t) * layer->width * layer->height);
  g_svg_ready = 1;

  boot_status_update(65, 100, "SVG: DONE");
  return 1;
}

static void svg_update_region(layer_t *layer, int rx, int ry, int rw, int rh,
                              int hover_index, float hover_scale,
                              float hover_offx, float hover_offy) {
  if (!g_svg_ready || rw <= 0 || rh <= 0)
    return;

  int x0 = rx;
  int y0 = ry;
  int x1 = rx + rw;
  int y1 = ry + rh;

  if (x0 < 0)
    x0 = 0;
  if (y0 < 0)
    y0 = 0;
  if (x1 > layer->width)
    x1 = layer->width;
  if (y1 > layer->height)
    y1 = layer->height;

  if (x0 >= x1 || y0 >= y1)
    return;

  for (int y = y0; y < y1; ++y) {
    uint32_t *dst = &layer->buffer[y * layer->width + x0];
    uint32_t *src = &svg_base_buf[y * layer->width + x0];
    for (int x = x0; x < x1; ++x) {
      *dst++ = *src++;
    }
  }

  if (hover_index >= 0) {
    // Draw hovered shape scaled up on top.
    svg_shape_cache_t *c = &g_svg_cache[hover_index];
    unsigned char *src_rgba = NULL;
    int src_x = 0, src_y = 0, src_w = 0, src_h = 0;

    if (c->rgba && c->w > 0 && c->h > 0) {
      src_rgba = c->rgba;
      src_x = c->x;
      src_y = c->y;
      src_w = c->w;
      src_h = c->h;
    } else if (c->shape) {
      float x0f = c->shape->bounds[0] * g_svg_scale + g_svg_tx;
      float y0f = c->shape->bounds[1] * g_svg_scale + g_svg_ty;
      float x1f = c->shape->bounds[2] * g_svg_scale + g_svg_tx;
      float y1f = c->shape->bounds[3] * g_svg_scale + g_svg_ty;

      float padf = c->shape->strokeWidth * g_svg_scale + 3.0f;
      int pad = (int)ceilf(padf);
      if (pad < 2)
        pad = 2;
      int x0 = (int)floorf(x0f) - pad;
      int y0 = (int)floorf(y0f) - pad;
      int x1 = (int)ceilf(x1f) + pad;
      int y1 = (int)ceilf(y1f) + pad;

      if (x0 < 0)
        x0 = 0;
      if (y0 < 0)
        y0 = 0;
      if (x1 > layer->width)
        x1 = layer->width;
      if (y1 > layer->height)
        y1 = layer->height;

      int w = x1 - x0;
      int h = y1 - y0;
      if (w > 0 && h > 0) {
        size_t bytes = (size_t)w * (size_t)h * 4;
        if (bytes > g_svg_hover_buf_cap) {
          g_svg_hover_buf = (unsigned char *)realloc(g_svg_hover_buf, bytes);
          if (g_svg_hover_buf)
            g_svg_hover_buf_cap = bytes;
        }
        if (g_svg_hover_buf && g_svg_hover_buf_cap >= bytes) {
          for (int i = 0; i < g_svg_shape_count; ++i)
            g_svg_cache[i].shape->flags = 0;
          c->shape->flags = NSVG_FLAGS_VISIBLE;

          nsvgRasterize(g_svg_rast, g_svg_image, g_svg_tx - (float)x0,
                        g_svg_ty - (float)y0, g_svg_scale, g_svg_hover_buf, w,
                        h, w * 4);

          for (int i = 0; i < g_svg_shape_count; ++i)
            g_svg_cache[i].shape->flags = g_svg_cache[i].flags;

          src_rgba = g_svg_hover_buf;
          src_x = x0;
          src_y = y0;
          src_w = w;
          src_h = h;
        }
      }
    }

    if (src_rgba && src_w > 0 && src_h > 0) {
      float scale = hover_scale;
      int dst_w = (int)ceilf((float)src_w * scale);
      int dst_h = (int)ceilf((float)src_h * scale);
      float src_center_x = (float)src_x + (float)src_w * 0.5f;
      float src_center_y = (float)src_y + (float)src_h * 0.5f;
      int center_x = (int)(src_center_x + hover_offx);
      int center_y = (int)(src_center_y + hover_offy);
      int dst_x0 = center_x - dst_w / 2;
      int dst_y0 = center_y - dst_h / 2;
      int dst_x1 = dst_x0 + dst_w;
      int dst_y1 = dst_y0 + dst_h;

      if (dst_x0 < x0)
        dst_x0 = x0;
      if (dst_y0 < y0)
        dst_y0 = y0;
      if (dst_x1 > x1)
        dst_x1 = x1;
      if (dst_y1 > y1)
        dst_y1 = y1;

      for (int y = dst_y0; y < dst_y1; ++y) {
        uint32_t *dst = &layer->buffer[y * layer->width];
        for (int x = dst_x0; x < dst_x1; ++x) {
          float sx =
              ((float)x - ((float)center_x - (float)dst_w * 0.5f)) / scale;
          float sy =
              ((float)y - ((float)center_y - (float)dst_h * 0.5f)) / scale;
          int isx = (int)sx;
          int isy = (int)sy;
          if (isx < 0 || isy < 0 || isx >= src_w || isy >= src_h)
            continue;
          size_t idx = (size_t)(isy * src_w + isx) * 4;
          uint8_t sa = src_rgba[idx + 3];
          if (sa == 0)
            continue;

          uint8_t sr = src_rgba[idx + 0];
          uint8_t sg = src_rgba[idx + 1];
          uint8_t sb = src_rgba[idx + 2];

          uint32_t d = dst[x];
          uint8_t dr = (d >> 16) & 0xFF;
          uint8_t dg = (d >> 8) & 0xFF;
          uint8_t db = d & 0xFF;

          uint8_t out_r = (uint8_t)((sr * sa + dr * (255 - sa)) / 255);
          uint8_t out_g = (uint8_t)((sg * sa + dg * (255 - sa)) / 255);
          uint8_t out_b = (uint8_t)((sb * sa + db * (255 - sa)) / 255);

          dst[x] = (0xFFu << 24) | ((uint32_t)out_r << 16) |
                   ((uint32_t)out_g << 8) | (uint32_t)out_b;
        }
      }
    }
  }
}

static int svg_get_shape_rect_scaled(int index, float scale, float offx,
                                     float offy, int *x, int *y, int *w,
                                     int *h) {
  if (!g_svg_cache || index < 0 || index >= g_svg_shape_count)
    return 0;
  svg_shape_cache_t *c = &g_svg_cache[index];
  int src_x = 0;
  int src_y = 0;
  int src_w = 0;
  int src_h = 0;

  if (c->rgba && c->w > 0 && c->h > 0) {
    src_x = c->x;
    src_y = c->y;
    src_w = c->w;
    src_h = c->h;
  } else if (c->shape) {
    float x0f = c->shape->bounds[0] * g_svg_scale + g_svg_tx;
    float y0f = c->shape->bounds[1] * g_svg_scale + g_svg_ty;
    float x1f = c->shape->bounds[2] * g_svg_scale + g_svg_tx;
    float y1f = c->shape->bounds[3] * g_svg_scale + g_svg_ty;

    float padf = c->shape->strokeWidth * g_svg_scale + 3.0f;
    int pad = (int)ceilf(padf);
    if (pad < 2)
      pad = 2;

    int x0 = (int)floorf(x0f) - pad;
    int y0 = (int)floorf(y0f) - pad;
    int x1 = (int)ceilf(x1f) + pad;
    int y1 = (int)ceilf(y1f) + pad;

    if (x0 < 0)
      x0 = 0;
    if (y0 < 0)
      y0 = 0;
    if (x1 > SVG_WIDTH)
      x1 = SVG_WIDTH;
    if (y1 > SVG_HEIGHT)
      y1 = SVG_HEIGHT;

    src_x = x0;
    src_y = y0;
    src_w = x1 - x0;
    src_h = y1 - y0;
  }

  if (src_w <= 0 || src_h <= 0)
    return 0;
  int dst_w = (int)ceilf((float)src_w * scale);
  int dst_h = (int)ceilf((float)src_h * scale);
  int center_x = (int)((float)(src_x + src_w / 2) + offx);
  int center_y = (int)((float)(src_y + src_h / 2) + offy);
  int x0 = center_x - dst_w / 2;
  int y0 = center_y - dst_h / 2;
  *x = x0;
  *y = y0;
  *w = dst_w;
  *h = dst_h;
  return 1;
}

static int svg_get_shape_center(int index, float *cx, float *cy) {
  if (!g_svg_cache || index < 0 || index >= g_svg_shape_count)
    return 0;
  NSVGshape *s = g_svg_cache[index].shape;
  if (!s || (s->flags & NSVG_FLAGS_VISIBLE) == 0)
    return 0;
  float x0 = s->bounds[0];
  float y0 = s->bounds[1];
  float x1 = s->bounds[2];
  float y1 = s->bounds[3];
  *cx = ((x0 + x1) * 0.5f) * g_svg_scale + g_svg_tx;
  *cy = ((y0 + y1) * 0.5f) * g_svg_scale + g_svg_ty;
  return 1;
}

static int svg_pick_shape(layer_t *layer, int screen_x, int screen_y) {
  if (!g_svg_ready)
    return -1;
  if (screen_x < layer->x || screen_y < layer->y ||
      screen_x >= layer->x + layer->width ||
      screen_y >= layer->y + layer->height) {
    return -1;
  }

  float lx = (float)(screen_x - layer->x);
  float ly = (float)(screen_y - layer->y);
  float ix = (lx - g_svg_tx) / g_svg_scale;
  float iy = (ly - g_svg_ty) / g_svg_scale;

  int hit = -1;
  for (int i = 0; i < g_svg_shape_count; ++i) {
    NSVGshape *s = g_svg_cache[i].shape;
    if (!s || (s->flags & NSVG_FLAGS_VISIBLE) == 0)
      continue;
    if (ix >= s->bounds[0] && ix <= s->bounds[2] && iy >= s->bounds[1] &&
        iy <= s->bounds[3]) {
      hit = i;
    }
  }
  return hit;
}

// タイマー設定 (0.1秒点滅用)
volatile uint32_t timer_ticks = 0;
void timer_handler(struct regs *r) {
  timer_ticks++;
  if (cpu_idle)
    idle_ticks++;
}

void timer_phase(int hz) {
  int divisor = 1193180 / hz;
  outb(0x43, 0x36);
  outb(0x40, divisor & 0xFF);
  outb(0x40, (divisor >> 8) & 0xFF);
}

static char *append_uint(char *p, unsigned int v) {
  char tmp[10];
  int n = 0;
  if (v == 0) {
    *p++ = '0';
    return p;
  }
  while (v > 0 && n < (int)sizeof(tmp)) {
    tmp[n++] = (char)('0' + (v % 10));
    v /= 10;
  }
  while (n-- > 0) {
    *p++ = tmp[n];
  }
  return p;
}

// 起動中の進捗をデスクトップ左上に表示（例: STAGE 3: DESKTOP READY (50%)）
static void boot_status_update(int stage, int total_stages, const char *label) {
  layer_t *desktop = g_boot_status_layer;
  if (!desktop || !desktop->buffer)
    return;

  char line[64];
  char *p = line;

  const char *head = "STAGE ";
  while (*head)
    *p++ = *head++;
  p = append_uint(p, (unsigned int)stage);
  *p++ = ':';
  *p++ = ' ';

  while (*label)
    *p++ = *label++;

  *p++ = ' ';
  *p++ = '(';
  unsigned int percent = 0;
  if (total_stages > 0) {
    percent = (unsigned int)(stage * 100 / total_stages);
  }
  p = append_uint(p, percent);
  *p++ = '%';
  *p++ = ')';
  *p = '\0';

  // 背景色で塗りつぶしながら描画して見やすくする
  layer_draw_string(desktop, 4, 4, line, 0xFFFFFFFF, BASE_BG_COLOR);
  screen_mark_static_dirty();
  screen_refresh();
}

static void hud_update(layer_t *hud, unsigned int cpu_percent,
                       unsigned int mem_used_kb, unsigned int mem_total_kb) {
  layer_fill(hud, 0xFF000000);

  char line1[24];
  char line2[32];
  char *p = line1;
  *p++ = 'C';
  *p++ = 'P';
  *p++ = 'U';
  *p++ = ' ';
  p = append_uint(p, cpu_percent);
  *p++ = '%';
  *p = '\0';

  p = line2;
  *p++ = 'M';
  *p++ = 'E';
  *p++ = 'M';
  *p++ = ' ';
  p = append_uint(p, mem_used_kb);
  *p++ = '/';
  p = append_uint(p, mem_total_kb);
  *p++ = 'K';
  *p++ = 'B';
  *p = '\0';

  layer_draw_string(hud, 2, 0, line1, 0xFFFFFFFF, TRANSPARENT_COLOR);
  layer_draw_string(hud, 2, 8, line2, 0xFFFFFFFF, TRANSPARENT_COLOR);
}

// キー入力バッファ
#define KEYBUF_MAX 256
static char keybuf_str[KEYBUF_MAX] = "";

// UTF-8→Unicode変換（簡易）
static uint16_t utf8_next(const char **p) {
  const unsigned char *s = (const unsigned char *)*p;
  uint16_t code = 0;
  if (s[0] < 0x80) {
    code = s[0];
    (*p)++;
  } else if ((s[0] & 0xE0) == 0xC0) {
    code = ((s[0] & 0x1F) << 6) | (s[1] & 0x3F);
    (*p) += 2;
  } else if ((s[0] & 0xF0) == 0xE0) {
    code = ((s[0] & 0x0F) << 12) | ((s[1] & 0x3F) << 6) | (s[2] & 0x3F);
    (*p) += 3;
  } else {
    (*p)++;
  }
  return code;
}

// SVGパスを使ったグリフ描画（ダミー: 枠のみ）
static void layer_draw_glyph(layer_t *layer, int x, int y, uint16_t code,
                             uint32_t color) {
  // fonts.h の font_glyphs[] から code を検索
  extern const Glyph font_glyphs[];
  for (int i = 0; font_glyphs[i].code != 0; ++i) {
    if (font_glyphs[i].code == code) {
      // 文字コードに応じて豆腐の中身を塗りつぶし
      for (int dy = 0; dy < 24; ++dy) {
        for (int dx = 0; dx < 24; ++dx) {
          int px = x + dx, py = y + dy;
          if (px >= 0 && px < layer->width && py >= 0 && py < layer->height) {
            // 文字コード code を利用してそれっぽいパターンを作る
            int pattern = ((code >> (dx / 4)) ^ (code >> (dy / 4))) & 1;
            if (dx == 0 || dx == 23 || dy == 0 || dy == 23 || pattern)
              layer->buffer[py * layer->width + px] = color;
          }
        }
      }
      return;
    }
  }
  // 登録されていなくても豆腐（四角）を描画
  for (int dy = 0; dy < 24; ++dy) {
    for (int dx = 0; dx < 24; ++dx) {
      int px = x + dx, py = y + dy;
      if (px >= 0 && px < layer->width && py >= 0 && py < layer->height) {
        if (dx == 0 || dx == 23 || dy == 0 || dy == 23)
          layer->buffer[py * layer->width + px] = color;
      }
    }
  }
}

// 日本語文字列描画（1文字24x24pxで描画）
static void layer_draw_glyph_string(layer_t *layer, int x, int y,
                                    const char *str, uint32_t color) {
  int cx = x;
  while (*str) {
    uint16_t code = utf8_next(&str);
    if (code < 128) {
      layer_draw_char(layer, cx, y, (char)code, color, 0xFFFFFFFF);
      cx += 8;
    } else {
      layer_draw_glyph(layer, cx, y, code, color);
      cx += 24;
    }
  }
}

void draw_test_and_keys(layer_t *layer) {
  layer_fill(layer, 0xFFFFFFFF); // 白背景
  layer_draw_glyph_string(layer, 20, 20, "テストaaa123漢字", 0xFF000000);
  layer_draw_glyph_string(layer, 20, 60, keybuf_str, 0xFF000000);
}

extern void register_layer(layer_t *layer);
extern void screen_mark_static_dirty();
extern volatile char keybuf[];
extern volatile int keybuf_len;
extern volatile int32_t mouse_x;
extern volatile int32_t mouse_y;
static void fill_framebuffer_red_early(struct multiboot_info *mbi) {
  if (!mbi)
    return;

  uint32_t *fb = (uint32_t *)(uintptr_t)mbi->framebuffer_addr;
  if (!fb)
    return;

  for (uint32_t y = 0; y < mbi->framebuffer_height; ++y) {
    uint32_t *row =
        (uint32_t *)((uint8_t *)(uintptr_t)fb + y * mbi->framebuffer_pitch);
    for (uint32_t x = 0; x < mbi->framebuffer_width; ++x) {
      row[x] = BASE_BG_COLOR;
    }
  }
}
void kmain(uint32_t magic, struct multiboot_info *mbi) {
  (void)magic;

  // SVG描画などの初期化より前に、まず赤画面を出す
  for (int i = 0; i < 30; ++i) { // 約0.3秒間、赤で塗りつぶし続ける
    fill_framebuffer_red_early(mbi);
    for (volatile int j = 0; j < 1000000; ++j) {
      __asm__ __volatile__("nop");
    }
  }

  fill_framebuffer_red_early(mbi); // 最後にもう一度赤で塗る

  enable_fpu();

  set_framebuffer_info((uint32_t *)(uintptr_t)mbi->framebuffer_addr,
                       mbi->framebuffer_width, mbi->framebuffer_height,
                       mbi->framebuffer_pitch);

  // 割り込み・デバイス初期化
  idt_install();
  irq_install();
  irq_install_handler(0, timer_handler);
  timer_phase(100); // 100Hz
  keyboard_install();
  mouse_install();
  enable_interrupts();

  // 1. 背景 (赤)
  layer_t desktop;
  desktop.buffer = desktop_buf;
  desktop.x = 0;
  desktop.y = 0;
  desktop.width = SCREEN_WIDTH;
  desktop.height = SCREEN_HEIGHT;
  desktop.transparent = 0;
  desktop.active = 1;
  desktop.dynamic = 0;
  layer_fill(&desktop, BASE_BG_COLOR);
  register_layer(&desktop);

  // 進捗表示で使うレイヤーを登録
  g_boot_status_layer = &desktop;

  // 進捗表示: デスクトップレイヤー作成完了 (全 6 ステージ中の 3)
  boot_status_update(3, 6, "DESKTOP READY");

  // 2. SVG表示エリア (左上)
  layer_t svg_layer;
  svg_layer.buffer = svg_buf;
  svg_layer.x = 0;
  svg_layer.y = 0;
  svg_layer.width = SVG_WIDTH;
  svg_layer.height = SVG_HEIGHT;
  svg_layer.transparent = 0;
  svg_layer.active = 1;
  svg_layer.dynamic = 1;
  svg_init(&svg_layer);
  register_layer(&svg_layer);

  // 進捗表示: SVG レイヤー初期化完了
  boot_status_update(4, 6, "SVG READY");

  // 3. 点滅インジケータ (右下)
  layer_t blink_layer;
  blink_layer.buffer = blink_buf;
  blink_layer.x = SCREEN_WIDTH - 60;
  blink_layer.y = SCREEN_HEIGHT - 60;
  blink_layer.width = 50;
  blink_layer.height = 50;
  blink_layer.transparent = 0;
  blink_layer.active = 1;
  blink_layer.dynamic = 1;
  layer_fill(&blink_layer, 0xFF0000FF); // 青色
  register_layer(&blink_layer);

  // 4. HUD (左下)
  layer_t hud_layer;
  hud_layer.buffer = hud_buf;
  hud_layer.x = 10;
  hud_layer.y = SCREEN_HEIGHT - 30;
  hud_layer.width = 240;
  hud_layer.height = 16;
  hud_layer.transparent = 0;
  hud_layer.active = 1;
  hud_layer.dynamic = 1;
  register_layer(&hud_layer);

  // 起動直後から HUD にテスト文字列を表示しておく
  draw_test_and_keys(&hud_layer);

  // 進捗表示: HUD レイヤー作成完了
  boot_status_update(5, 6, "HUD READY");

  uint32_t last_blink_tick = 0;
  int blink_state = 0;
  uint32_t last_stat_tick = 0;
  uint32_t last_idle_tick = 0;
  unsigned int cpu_percent = 0;
  uint32_t mem_total_kb = mbi->mem_upper;

  int last_hover = -2;
  int last_mouse_x = -1;
  int last_mouse_y = -1;
  int active_hover = -1;
  float hover_scale = 1.0f;
  float hover_target_scale = 1.0f;
  float hover_offx = 0.0f;
  float hover_offy = 0.0f;
  float hover_target_offx = 0.0f;
  float hover_target_offy = 0.0f;
  uint32_t last_anim_tick = 0;
  int last_draw_x = 0, last_draw_y = 0, last_draw_w = 0, last_draw_h = 0;
  int have_draw = 0;

  screen_refresh(); // 最初の描画

  // 進捗表示: メインループ突入
  boot_status_update(6, 6, "MAIN LOOP");

  while (1) {
    int need_refresh = 0;

    // 0.1秒(10 ticks)ごとに点滅
    if (timer_ticks - last_blink_tick >= 10) {
      blink_state = !blink_state;
      blink_layer.active = blink_state;
      last_blink_tick = timer_ticks;
      need_refresh = 1;
    }

    int mx = mouse_x;
    int my = mouse_y;
    if (mx != last_mouse_x || my != last_mouse_y) {
      need_refresh = 1;
      int hover = svg_pick_shape(&svg_layer, mx, my);
      if (hover != last_hover) {
        last_hover = hover;
        if (hover >= 0) {
          active_hover = hover;
          hover_scale = 1.0f;
          hover_offx = 0.0f;
          hover_offy = 0.0f;
          hover_target_scale = HOVER_SCALE;
        } else {
          hover_target_scale = 1.0f;
        }
      }
      last_mouse_x = mx;
      last_mouse_y = my;
    }

    if (active_hover >= 0) {
      if (last_hover >= 0) {
        hover_target_scale = HOVER_SCALE;
        float cx = 0.0f, cy = 0.0f;
        if (svg_get_shape_center(active_hover, &cx, &cy)) {
          hover_target_offx = ((float)mx - cx) * 0.2f;
          hover_target_offy = ((float)my - cy) * 0.2f;
        } else {
          hover_target_offx = 0.0f;
          hover_target_offy = 0.0f;
        }
      } else {
        hover_target_scale = 1.0f;
        hover_target_offx = 0.0f;
        hover_target_offy = 0.0f;
      }

      if (timer_ticks != last_anim_tick || need_refresh) {
        uint32_t dt = timer_ticks - last_anim_tick;
        if (dt == 0)
          dt = 1;
        last_anim_tick = timer_ticks;
        for (uint32_t i = 0; i < dt; ++i) {
          hover_scale += (hover_target_scale - hover_scale) * HOVER_EASE;
          hover_offx += (hover_target_offx - hover_offx) * HOVER_EASE;
          hover_offy += (hover_target_offy - hover_offy) * HOVER_EASE;
        }

        int x, y, w, h;
        if (svg_get_shape_rect_scaled(active_hover, hover_scale, hover_offx,
                                      hover_offy, &x, &y, &w, &h)) {
          int rx = x, ry = y, rw = w, rh = h;
          if (have_draw) {
            int x0 = (last_draw_x < x) ? last_draw_x : x;
            int y0 = (last_draw_y < y) ? last_draw_y : y;
            int x1 = (last_draw_x + last_draw_w > x + w)
                         ? (last_draw_x + last_draw_w)
                         : (x + w);
            int y1 = (last_draw_y + last_draw_h > y + h)
                         ? (last_draw_y + last_draw_h)
                         : (y + h);
            rx = x0;
            ry = y0;
            rw = x1 - x0;
            rh = y1 - y0;
          }
          svg_update_region(&svg_layer, rx, ry, rw, rh, active_hover,
                            hover_scale, hover_offx, hover_offy);
          screen_mark_static_dirty();
          need_refresh = 1;
          last_draw_x = x;
          last_draw_y = y;
          last_draw_w = w;
          last_draw_h = h;
          have_draw = 1;
        }

        if (last_hover < 0 && (fabsf(hover_scale - 1.0f) < 0.01f) &&
            (fabsf(hover_offx) < 0.5f) && (fabsf(hover_offy) < 0.5f)) {
          if (have_draw) {
            svg_update_region(&svg_layer, last_draw_x, last_draw_y, last_draw_w,
                              last_draw_h, -1, 1.0f, 0.0f, 0.0f);
            screen_mark_static_dirty();
            need_refresh = 1;
          }
          active_hover = -1;
          have_draw = 0;
        }
      }
    }

    // キー入力監視
    if (keybuf_len > 0) {
      int len = keybuf_len;
      if (len > KEYBUF_MAX - 1)
        len = KEYBUF_MAX - 1;
      memcpy(keybuf_str, (const void *)keybuf, len);
      keybuf_str[len] = '\0';
      draw_test_and_keys(&hud_layer);
      need_refresh = 1;
      keybuf_len = 0;
    }

    if (timer_ticks - last_stat_tick >= 100) {
      uint32_t total = timer_ticks - last_stat_tick;
      uint32_t idle = idle_ticks - last_idle_tick;
      if (total > 0) {
        uint32_t idle_pct = (idle * 100u) / total;
        cpu_percent = (idle_pct >= 100u) ? 0u : (100u - idle_pct);
      }
      hud_update(&hud_layer, cpu_percent, (unsigned int)(heap_ptr / 1024),
                 mem_total_kb);
      last_stat_tick = timer_ticks;
      last_idle_tick = idle_ticks;
      need_refresh = 1;
    }

    if (need_refresh) {
      cpu_idle = 0;
      screen_refresh();
    } else {
      cpu_idle = 1;
      __asm__ __volatile__("hlt");
      cpu_idle = 0;
    }
  }
}
