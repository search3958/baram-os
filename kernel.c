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
#define BASE_BG_COLOR 0xFF8B0000u

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
static int g_svg_cache_complete = 1;
static float g_svg_scale = 1.0f;
static float g_svg_tx = 0.0f;
static float g_svg_ty = 0.0f;
static int g_svg_ready = 0;

static volatile uint32_t idle_ticks = 0;
static volatile int cpu_idle = 0;

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

  layer_fill(layer, BASE_BG_COLOR);

  char *svg_copy = (char *)malloc(note_test_svg_len + 1);
  if (!svg_copy)
    return 0;
  memcpy(svg_copy, note_test_svg, note_test_svg_len);
  svg_copy[note_test_svg_len] = '\0';

  g_svg_image = nsvgParse(svg_copy, "px", 96.0f);
  if (!g_svg_image)
    return 0;

  g_svg_rast = nsvgCreateRasterizer();
  if (!g_svg_rast)
    return 0;

  g_svg_rgba = (unsigned char *)malloc(
      (size_t)layer->width * (size_t)layer->height * 4);
  if (!g_svg_rgba)
    return 0;

  float scale_x =
      g_svg_image->width > 0.0f ? (float)layer->width / g_svg_image->width
                                : 1.0f;
  float scale_y =
      g_svg_image->height > 0.0f ? (float)layer->height / g_svg_image->height
                                 : 1.0f;
  g_svg_scale = scale_x < scale_y ? scale_x : scale_y;
  g_svg_tx = 0.0f;
  g_svg_ty = 0.0f;

  int count = 0;
  for (NSVGshape *s = g_svg_image->shapes; s; s = s->next)
    count++;

  if (count > 0) {
    g_svg_cache =
        (svg_shape_cache_t *)malloc(sizeof(svg_shape_cache_t) * (size_t)count);
    g_svg_cache_complete = (g_svg_cache != NULL);
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

      const int pad = 2;
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
      if (w <= 0 || h <= 0) {
        g_svg_cache_complete = 0;
        continue;
      }

      unsigned char *buf =
          (unsigned char *)malloc((size_t)w * (size_t)h * 4);
      if (!buf) {
        g_svg_cache_complete = 0;
        continue;
      }

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

  svg_render_full(layer);
  g_svg_ready = 1;
  return 1;
}

static void svg_update_region(layer_t *layer, int rx, int ry, int rw, int rh,
                              int hover_index) {
  if (!g_svg_ready || rw <= 0 || rh <= 0)
    return;
  if (!g_svg_cache_complete) {
    svg_render_full(layer);
    return;
  }

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
    uint32_t *row = &layer->buffer[y * layer->width];
    for (int x = x0; x < x1; ++x) {
      row[x] = BASE_BG_COLOR;
    }
  }

  for (int i = 0; i < g_svg_shape_count; ++i) {
    svg_shape_cache_t *c = &g_svg_cache[i];
    if (!c->rgba)
      continue;

    int sx0 = c->x;
    int sy0 = c->y;
    int sx1 = c->x + c->w;
    int sy1 = c->y + c->h;

    int ix0 = (x0 > sx0) ? x0 : sx0;
    int iy0 = (y0 > sy0) ? y0 : sy0;
    int ix1 = (x1 < sx1) ? x1 : sx1;
    int iy1 = (y1 < sy1) ? y1 : sy1;

    if (ix0 >= ix1 || iy0 >= iy1)
      continue;

    for (int y = iy0; y < iy1; ++y) {
      int sy = y - c->y;
      int src_row = sy * c->w;
      uint32_t *dst = &layer->buffer[y * layer->width];
      for (int x = ix0; x < ix1; ++x) {
        int sx = x - c->x;
        size_t idx = (size_t)(src_row + sx) * 4;
        uint8_t sa = c->rgba[idx + 3];
        if (sa == 0)
          continue;
        if (i == hover_index)
          sa = (uint8_t)(sa / 2);

        uint8_t sr = c->rgba[idx + 0];
        uint8_t sg = c->rgba[idx + 1];
        uint8_t sb = c->rgba[idx + 2];

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

static int svg_get_shape_rect(int index, int *x, int *y, int *w, int *h) {
  if (!g_svg_cache || index < 0 || index >= g_svg_shape_count)
    return 0;
  svg_shape_cache_t *c = &g_svg_cache[index];
  if (!c->rgba || c->w <= 0 || c->h <= 0)
    return 0;
  *x = c->x;
  *y = c->y;
  *w = c->w;
  *h = c->h;
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

// レイヤー用
static uint32_t desktop_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
static uint32_t svg_buf[SVG_WIDTH * SVG_HEIGHT];
static uint32_t blink_buf[50 * 50];
static uint32_t hud_buf[240 * 16];

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

extern void register_layer(layer_t *layer);
extern void screen_mark_static_dirty();
extern volatile char keybuf[];
extern volatile int keybuf_len;
extern volatile int32_t mouse_x;
extern volatile int32_t mouse_y;

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
  desktop.dynamic = 0;
  layer_fill(&desktop, BASE_BG_COLOR);
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
  svg_layer.dynamic = 0;
  if (svg_init(&svg_layer)) {
    screen_mark_static_dirty();
  }
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
  blink_layer.dynamic = 1;
  layer_fill(&blink_layer, 0xFF0000FF); // 青
  register_layer(&blink_layer);

  // 4. HUD (右上: CPU/MEM)
  layer_t hud_layer;
  hud_layer.buffer = hud_buf;
  hud_layer.width = 240;
  hud_layer.height = 16;
  hud_layer.x = SCREEN_WIDTH - hud_layer.width - 4;
  hud_layer.y = 4;
  hud_layer.transparent = 0;
  hud_layer.active = 1;
  hud_layer.dynamic = 1;
  hud_update(&hud_layer, 0, (unsigned int)(heap_ptr / 1024),
             (unsigned int)(sizeof(heap) / 1024));
  register_layer(&hud_layer);

  idt_install();
  irq_install();
  irq_install_handler(0, timer_handler);
  timer_phase(100); // 100Hz = 10ms/tick
  keyboard_install();
  mouse_install();
  enable_interrupts();

  uint32_t last_blink_tick = 0;
  int blink_state = 1;
  uint32_t last_stat_tick = 0;
  uint32_t last_idle_tick = 0;
  unsigned int cpu_percent = 0;
  unsigned int mem_total_kb = (unsigned int)(sizeof(heap) / 1024);

  int last_hover = -2;
  int last_mouse_x = -1;
  int last_mouse_y = -1;
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
        int rx = 0, ry = 0, rw = 0, rh = 0;
        int have = 0;
        int x, y, w, h;
        if (svg_get_shape_rect(last_hover, &x, &y, &w, &h)) {
          rx = x;
          ry = y;
          rw = w;
          rh = h;
          have = 1;
        }
        if (svg_get_shape_rect(hover, &x, &y, &w, &h)) {
          if (!have) {
            rx = x;
            ry = y;
            rw = w;
            rh = h;
            have = 1;
          } else {
            int x0 = rx < x ? rx : x;
            int y0 = ry < y ? ry : y;
            int x1 = (rx + rw) > (x + w) ? (rx + rw) : (x + w);
            int y1 = (ry + rh) > (y + h) ? (ry + rh) : (y + h);
            rx = x0;
            ry = y0;
            rw = x1 - x0;
            rh = y1 - y0;
          }
        }
        if (have) {
          svg_update_region(&svg_layer, rx, ry, rw, rh, hover);
          screen_mark_static_dirty();
          need_refresh = 1;
        }
        last_hover = hover;
      }
      last_mouse_x = mx;
      last_mouse_y = my;
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
