#include <math.h>   // sinf, cosf
#include <stdarg.h> // va_list
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>  // FILE
#include <stdlib.h> // malloc, free, realloc
#include <string.h> // memcpy, memset

#include "drivers.h"
#include "font/fonts.h"
#include "ui/svg_data.h"

#include <stddef.h>

#define NANOSVG_IMPLEMENTATION
#include "nanosvg/nanosvg.h"
#define NANOSVGRAST_IMPLEMENTATION
#include "nanosvg/nanosvgrast.h"
#ifndef BUILD_NUMBER
#include "build_no.h"
#endif
#include "ui/warp_engine.h"
#include "ui/warp1_engine.h"

// stb_truetype
#define STB_TRUETYPE_IMPLEMENTATION
#define STBTT_malloc(x, u) malloc(x)
#define STBTT_free(x, u) free(x)
#define STBTT_assert(x)
// double版数学関数をfloat版にリダイレクト（カーネルにはdouble版がない）
#define STBTT_ifloor(x) ((int)floorf((float)(x)))
#define STBTT_iceil(x) ((int)ceilf((float)(x)))
#define STBTT_sqrt(x) sqrtf((float)(x))
#define STBTT_pow(x, y) ((float)pow((double)(x), (double)(y)))
#define STBTT_fmod(x, y) fmodf((float)(x), (float)(y))
#define STBTT_cos(x) cosf((float)(x))
#define STBTT_acos(x) acosf((float)(x))
#define STBTT_fabs(x) fabsf((float)(x))
#include "font/stb_truetype.h"

#define SVG_WIDTH NOTE_TEST_SVG_WIDTH
#define SVG_HEIGHT NOTE_TEST_SVG_HEIGHT
#define BASE_BG_COLOR 0xFF000000u
#define HOVER_SCALE 1.2f
#define HOVER_EASE 0.1f

// Mouse Hotspot Correction
#define MOUSE_HOTSPOT_X 28
#define MOUSE_HOTSPOT_Y 21


typedef struct {
  NSVGshape *shape;
  unsigned char *rgba;
  uint8_t flags;
  int x;
  int y;
  int w;
  int h;
} svg_shape_cache_t;

// Multiboot module エントリ
typedef struct {
  uint32_t mod_start;
  uint32_t mod_end;
  uint32_t string;
  uint32_t reserved;
} __attribute__((packed)) multiboot_module_t;

typedef enum { OS_MODE_CLASSIC, OS_MODE_WARPDESKTOP } os_mode_t;

// --- グローバル変数 (Classic) ---
static NSVGimage *g_svg_image = NULL;
static NSVGrasterizer *g_svg_rast = NULL;
static unsigned char *g_svg_rgba = NULL;
static unsigned char *g_svg_full_rgba = NULL;
static int g_svg_full_w = 0;
static int g_svg_full_h = 0;
static svg_shape_cache_t *g_svg_cache = NULL;
static int g_svg_shape_count = 0;
static unsigned char *g_svg_hover_buf = NULL;
static size_t g_svg_hover_buf_cap = 0;
static float g_svg_scale = 1.0f;
static float g_svg_tx = 0.0f;
static float g_svg_ty = 0.0f;
static int g_svg_ready = 0;

static float g_scroll_x = 0.0f;
static float g_scroll_y = 0.0f;
static float g_target_scroll_x = 0.0f;
static float g_target_scroll_y = 0.0f;
#define SCROLL_EASE 0.15f

static volatile uint32_t idle_ticks = 0;
static volatile int cpu_idle = 0;

// --- グローバル変数 (Nextgen/Warp) ---
static os_mode_t current_os_mode = OS_MODE_CLASSIC;
static char g_last_svg_parse_status[64] = "None";
static int g_warp_mod_found = 0;
static uint32_t g_mod_count = 0;
static char g_warp_buffer[32768] = "screen(text:\"Warp module not found\")";
static char g_warp1_buffer[32768] = "screen{ id:(main), text:(\"Warp1 module not found\") }";
static int g_warp1_mod_found = 0;
static char g_bootlogo_buffer[65536] = "";
static int g_bootlogo_found = 0;
static char g_wallpaper_buffer[131072] = "";
static int g_wallpaper_found = 0;
static int g_svg_dirty = 1;
static char g_hud_status[64] = "Idle";

static int g_dragging_slider = 0;
static int g_dragging_window = -1;
 

// --- 前方宣言 ---
static uint32_t lerp_color(uint32_t c1, uint32_t c2, float t);
static void apply_conic_gradient(unsigned char *data, int w, int h, int rx,
                                 int ry, int rw, int rh, uint32_t c1,
                                 uint32_t c2);
static void svg_render_full(layer_t *layer);
static void redraw_warp_svg(layer_t *layer);
void layer_draw_ttf(layer_t *layer, int px, int py, const char *str,
                    float font_size, uint32_t color);

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

// i686 では通常 long double は 80bit (xf)
long double __extendsftf2(float f) { return (long double)f; }
float __trunctfsf2(long double d) { return (float)d; }
long double __extenddftf2(double d) { return (long double)d; }
double __trunctfdf2(long double d) { return (double)d; }
long double __multf3(long double a, long double b) { return a * b; }
long double __addtf3(long double a, long double b) { return a + b; }
long double __subtf3(long double a, long double b) { return a - b; }
long double __divtf3(long double a, long double b) { return a / b; }

// レイヤー用
static uint32_t desktop_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
static uint32_t svg_buf[SVG_WIDTH * SVG_HEIGHT];
static uint32_t svg_base_buf[SVG_WIDTH * SVG_HEIGHT];
static uint32_t blink_buf[50 * 50];
#define HUD_W 320
#define HUD_H 64
static uint32_t hud_buf[HUD_W * HUD_H];
// 文字レイヤー (全画面 透過)
#define TEXT_LAYER_W SCREEN_WIDTH
#define TEXT_LAYER_H SCREEN_HEIGHT
static uint32_t text_buf[TEXT_LAYER_W * TEXT_LAYER_H];
static uint32_t nextgen_ui_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
// stbtt フォント
static stbtt_fontinfo g_font;
static int g_font_ready = 0;
static const char *g_font_error = NULL;

// メモリアロケータ (フリーリスト方式)
static char heap[1024 * 1024 * 128]; // 128MB に拡大

typedef struct block_header {
  size_t size; // このブロックのデータサイズ (ヘッダ除く)
  int used;    // 1=使用中, 0=空き
} block_header_t;

#define BLOCK_HDR_SIZE (sizeof(block_header_t))

static int heap_initialized = 0;

static void heap_init(void) {
  block_header_t *first = (block_header_t *)heap;
  first->size = sizeof(heap) - BLOCK_HDR_SIZE;
  first->used = 0;
  heap_initialized = 1;
}

void *malloc(size_t size) {
  if (!heap_initialized)
    heap_init();
  if (size == 0)
    return NULL;
  // 8バイトアライメント
  size = (size + 7) & ~7;

  char *p = heap;
  char *end = heap + sizeof(heap);

  while (p + BLOCK_HDR_SIZE <= end) {
    block_header_t *hdr = (block_header_t *)p;
    if (!hdr->used && hdr->size >= size) {
      // このブロックを分割できるか確認
      size_t remaining = hdr->size - size;
      if (remaining > BLOCK_HDR_SIZE + 8) {
        // 後ろに新しい空きブロックを作る
        block_header_t *next = (block_header_t *)(p + BLOCK_HDR_SIZE + size);
        next->size = remaining - BLOCK_HDR_SIZE;
        next->used = 0;
        hdr->size = size;
      }
      hdr->used = 1;
      return p + BLOCK_HDR_SIZE;
    }
    p += BLOCK_HDR_SIZE + hdr->size;
  }
  return NULL; // 枯渇
}

void free(void *ptr) {
  if (!ptr)
    return;
  block_header_t *hdr = (block_header_t *)((char *)ptr - BLOCK_HDR_SIZE);
  hdr->used = 0;

  // 隣接する空きブロックをマージ (前方向)
  char *p = heap;
  char *end = heap + sizeof(heap);
  while (p + BLOCK_HDR_SIZE <= end) {
    block_header_t *cur = (block_header_t *)p;
    char *next_p = p + BLOCK_HDR_SIZE + cur->size;
    if (!cur->used && next_p + BLOCK_HDR_SIZE <= end) {
      block_header_t *next = (block_header_t *)next_p;
      if (!next->used) {
        // マージ
        cur->size += BLOCK_HDR_SIZE + next->size;
        continue; // 同じ位置から再チェック
      }
    }
    p = next_p;
  }
}

void *realloc(void *ptr, size_t size) {
  if (!ptr)
    return malloc(size);
  if (size == 0) {
    free(ptr);
    return NULL;
  }
  block_header_t *hdr = (block_header_t *)((char *)ptr - BLOCK_HDR_SIZE);
  if (hdr->size >= size)
    return ptr; // 既に十分なサイズ
  void *next = malloc(size);
  if (!next)
    return NULL;
  memcpy(next, ptr, hdr->size < size ? hdr->size : size);
  free(ptr);
  return next;
}

uint32_t get_used_memory(void) {
  if (!heap_initialized)
    return 0;
  uint32_t used = 0;
  char *p = heap;
  char *end = heap + sizeof(heap);
  while (p + BLOCK_HDR_SIZE <= end) {
    block_header_t *hdr = (block_header_t *)p;
    if (hdr->used) {
      used += hdr->size + BLOCK_HDR_SIZE;
    }
    p += BLOCK_HDR_SIZE + hdr->size;
  }
  return used;
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

static int tolower(int c) {
  if (c >= 'A' && c <= 'Z')
    return c + ('a' - 'A');
  return c;
}

int strcasecmp(const char *a, const char *b) {
  while (*a && (tolower((unsigned char)*a) == tolower((unsigned char)*b))) {
    a++;
    b++;
  }
  return tolower((unsigned char)*a) - tolower((unsigned char)*b);
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

int strncasecmp(const char *a, const char *b, size_t n) {
  for (size_t i = 0; i < n; ++i) {
    unsigned char ca = (unsigned char)tolower((unsigned char)a[i]);
    unsigned char cb = (unsigned char)tolower((unsigned char)b[i]);
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

char *strcat(char *dest, const char *src) {
  char *d = dest;
  while (*d)
    d++;
  while ((*d++ = *src++))
    ;
  return dest;
}

char *strncat(char *dest, const char *src, size_t n) {
  char *d = dest;
  while (*d)
    d++;
  size_t i;
  for (i = 0; i < n && src[i]; i++)
    *d++ = src[i];
  *d = '\0';
  return dest;
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

// 2つの色を線形補間
static uint32_t lerp_color(uint32_t c1, uint32_t c2, float t) {
  uint8_t r1 = (c1 >> 16) & 0xFF, g1 = (c1 >> 8) & 0xFF, b1 = c1 & 0xFF,
          a1 = (c1 >> 24) & 0xFF;
  uint8_t r2 = (c2 >> 16) & 0xFF, g2 = (c2 >> 8) & 0xFF, b2 = c2 & 0xFF,
          a2 = (c2 >> 24) & 0xFF;
  uint8_t r = (uint8_t)(r1 + (r2 - r1) * t);
  uint8_t g = (uint8_t)(g1 + (g2 - g1) * t);
  uint8_t b = (uint8_t)(b1 + (b2 - b1) * t);
  uint8_t a = (uint8_t)(a1 + (a2 - a1) * t);
  return ((uint32_t)a << 24) | ((uint32_t)r << 16) | ((uint32_t)g << 8) |
         (uint32_t)b;
}

static void apply_conic_gradient(unsigned char *data, int w, int h, int rx,
                                 int ry, int rw, int rh, uint32_t c1,
                                 uint32_t c2) {
  float cx = (float)rx + (float)rw / 2.0f;
  float cy = (float)ry + (float)rh / 2.0f;
  const float PI = 3.14159265f;

  for (int y = ry; y < ry + rh; y++) {
    if (y < 0 || y >= h)
      continue;
    for (int x = rx; x < rx + rw; x++) {
      if (x < 0 || x >= w)
        continue;

      // 元のアルファ値をマスクとして使用
      uint8_t mask = data[(y * w + x) * 4 + 3];
      if (mask == 0)
        continue;

      float dx = (float)x - cx;
      float dy = (float)y - cy;
      float angle = atan2f(dy, dx);         // -PI to PI
      float t = (angle + PI) / (2.0f * PI); // 0 to 1

      uint32_t color = lerp_color(c1, c2, t);
      uint8_t r = (color >> 16) & 0xFF;
      uint8_t g = (color >> 8) & 0xFF;
      uint8_t b = color & 0xFF;
      uint8_t a = (uint8_t)((color >> 24) & 0xFF);

      // マスク（図形の形）を考慮して書き込み
      size_t idx = (size_t)(y * w + x) * 4;
      data[idx + 0] = r;
      data[idx + 1] = g;
      data[idx + 2] = b;
      data[idx + 3] = (uint8_t)(a * mask / 255);
    }
  }
}

static void svg_render_full(layer_t *layer) {
  if (!g_svg_full_rgba)
    return;

  const uint32_t bg = BASE_BG_COLOR;
  uint8_t bg_r = (bg >> 16) & 0xFF;
  uint8_t bg_g = (bg >> 8) & 0xFF;
  uint8_t bg_b = bg & 0xFF;

  int scroll_x = (int)roundf(g_scroll_x);
  int scroll_y = (int)roundf(g_scroll_y);

  for (int y = 0; y < layer->height; ++y) {
    uint32_t *line_dst = &layer->buffer[y * layer->width];
    int src_y = y - scroll_y;
    if (src_y < 0 || src_y >= g_svg_full_h) {
      for (int x = 0; x < layer->width; x++)
        line_dst[x] = bg;
      continue;
    }
    unsigned char *line_src = &g_svg_full_rgba[src_y * g_svg_full_w * 4];
    for (int x = 0; x < layer->width; ++x) {
      int src_x = x - scroll_x;
      if (src_x < 0 || src_x >= g_svg_full_w) {
        line_dst[x] = bg;
        continue;
      }
      unsigned char *rgba = &line_src[src_x * 4];
      uint8_t a = rgba[3];
      if (a == 0) {
        line_dst[x] = bg;
      } else if (a == 255) {
        line_dst[x] = (0xFFu << 24) | ((uint32_t)rgba[0] << 16) |
                      ((uint32_t)rgba[1] << 8) | (uint32_t)rgba[2];
      } else {
        uint8_t out_r = (uint8_t)((rgba[0] * a + bg_r * (255 - a)) / 255);
        uint8_t out_g = (uint8_t)((rgba[1] * a + bg_g * (255 - a)) / 255);
        uint8_t out_b = (uint8_t)((rgba[2] * a + bg_b * (255 - a)) / 255);
        line_dst[x] = (0xFFu << 24) | ((uint32_t)out_r << 16) |
                      ((uint32_t)out_g << 8) | (uint32_t)out_b;
      }
    }
  }
}

// SVGソースからrgba(r,g,b,a)の色を抽出する
static uint32_t parse_rgba_smart(const char *str, int color_index) {
  if (!str)
    return 0xFFFFFFFF;
  const char *p = str;
  for (int i = 0; i <= color_index; i++) {
    const char *next = strstr(p, "rgba(");
    if (!next)
      return (i == 0) ? 0xFF5CA8FF : 0xFFFFFFFF;
    p = next + 5;
  }
  int r = (int)strtoll(p, (char **)&p, 10);
  while (*p == ',' || *p == ' ')
    p++;
  int g = (int)strtoll(p, (char **)&p, 10);
  while (*p == ',' || *p == ' ')
    p++;
  int b = (int)strtoll(p, (char **)&p, 10);
  return (0xFFu << 24) | ((uint32_t)r << 16) | ((uint32_t)g << 8) | (uint32_t)b;
}

static int svg_init(layer_t *layer, int load_wallpaper) {
  if (g_svg_ready && !load_wallpaper)
    return 1;
  
  if (load_wallpaper) {
    g_svg_ready = 0; // 重走初期化
  }

  layer_fill(layer, 0xFF000000);

  const char* svg_data = NULL;
  if (load_wallpaper && g_wallpaper_found && g_wallpaper_buffer[0] != '\0') {
    svg_data = g_wallpaper_buffer;
  } else if (g_bootlogo_found && g_bootlogo_buffer[0] != '\0') {
    svg_data = g_bootlogo_buffer;
  }

  if (!svg_data)
    return 0;

  if (g_svg_image) nsvgDelete(g_svg_image);
  g_svg_image = nsvgParse((char*)svg_data, "px", 96.0f);
  if (!g_svg_image)
    return 0;

  if (!g_svg_rast)
    g_svg_rast = nsvgCreateRasterizer();
  if (!g_svg_rast)
    return 0;

  g_svg_full_w = layer->width;
  g_svg_full_h = layer->height;

  if (!g_svg_full_rgba) {
    g_svg_full_rgba = (unsigned char *)malloc((size_t)g_svg_full_w *
                                              (size_t)g_svg_full_h * 4);
  }
  if (!g_svg_full_rgba)
    return 0;
  memset(g_svg_full_rgba, 0, (size_t)g_svg_full_w * (size_t)g_svg_full_h * 4);

  float scale = 1.0f;
  float tx = 0.0f, ty = 0.0f;

  if (load_wallpaper && svg_data == g_wallpaper_buffer) {
    // "Center Cover" logic
    float scale_x = (float)g_svg_full_w / g_svg_image->width;
    float scale_y = (float)g_svg_full_h / g_svg_image->height;
    scale = (scale_x > scale_y) ? scale_x : scale_y;
    tx = (g_svg_full_w - g_svg_image->width * scale) / 2.0f;
    ty = (g_svg_full_h - g_svg_image->height * scale) / 2.0f;
  } else {
    // Center logic for logo
    tx = (g_svg_full_w - g_svg_image->width) / 2.0f;
    ty = (g_svg_full_h - g_svg_image->height) / 2.0f;
  }

  nsvgRasterize(g_svg_rast, g_svg_image, tx, ty, scale, g_svg_full_rgba,
                g_svg_full_w, g_svg_full_h, g_svg_full_w * 4);

  // --- 自動グラデーション抽出ロジック (Bootlogo用) ---
  if (!load_wallpaper && svg_data == g_bootlogo_buffer) {
    const char *conic_pos = strstr(g_bootlogo_buffer, "conic-gradient");
    if (conic_pos) {
      uint32_t c1 = parse_rgba_smart(conic_pos, 2);
      uint32_t c2 = parse_rgba_smart(conic_pos, 3);

      for (NSVGshape *s = g_svg_image->shapes; s; s = s->next) {
        if (s->fill.type != NSVG_PAINT_NONE) {
          int rx = (int)(s->bounds[0] * scale + tx);
          int ry = (int)(s->bounds[1] * scale + ty);
          int rw = (int)((s->bounds[2] - s->bounds[0]) * scale);
          int rh = (int)((s->bounds[3] - s->bounds[1]) * scale);
          if (rw > 0 && rh > 0) {
            apply_conic_gradient(g_svg_full_rgba, g_svg_full_w, g_svg_full_h, rx,
                                 ry, rw, rh, c1, c2);
          }
        }
      }
    }
  }

  if (!g_svg_rgba)
    g_svg_rgba = (unsigned char *)malloc((size_t)layer->width *
                                         (size_t)layer->height * 4);
  g_svg_scale = 1.0f;
  g_svg_tx = 0.0f;
  g_svg_ty = 0.0f;
  svg_render_full(layer);
  memcpy(svg_base_buf, layer->buffer,
         sizeof(uint32_t) * layer->width * layer->height);
  g_svg_ready = 1;
  return 1;
}

const char *g_hardcoded_warp1 = 
"screen{\n"
"    id: (main),\n"
"    Header{\n"
"        text: (\"Warp1 Embedded Demo\"),\n"
"        button{\n"
"            --headerText: (\"\"),\n"
"            text: (\"⚠️\" + --headerText),\n"
"            oneClick: (--headerText = \"Please wait\", reset{now})\n"
"        }\n"
"    },\n"
"    card{\n"
"        text: (\"Status\"),\n"
"        text{ text: (\"Warp1 is running from embedded code.\") }\n"
"    },\n"
"    button{\n"
"        text: (\"FAB\"),\n"
"        frame: (width = 100vw - 40, height = 40),\n"
"        position: (bottom = 20, left = 20)\n"
"    }\n"
"}";

static void warp_ui_mod_init(struct multiboot_info *mbi) {
  if (!mbi || !(mbi->flags & 0x8) || mbi->mods_count == 0)
    return;
  g_mod_count = mbi->mods_count;
  multiboot_module_t *mods = (multiboot_module_t *)(uintptr_t)mbi->mods_addr;

  // 1. 文字列マッチングによる識別
  for (uint32_t i = 0; i < mbi->mods_count; i++) {
    const char *s = (const char *)(uintptr_t)mods[i].string;
    if (s) {
      if (strstr(s, "main.warp") || strstr(s, "MAIN.WARP")) {
        uint32_t size = mods[i].mod_end - mods[i].mod_start;
        if (size > 32767) size = 32767;
        memcpy(g_warp_buffer, (void *)(uintptr_t)mods[i].mod_start, size);
        g_warp_buffer[size] = '\0';
        g_warp_mod_found = 1;
      } else if (strstr(s, "new.warp1") || strstr(s, "NEW.WARP1")) {
        uint32_t size = mods[i].mod_end - mods[i].mod_start;
        if (size > 32767) size = 32767;
        memcpy(g_warp1_buffer, (void *)(uintptr_t)mods[i].mod_start, size);
        g_warp1_buffer[size] = '\0';
        g_warp1_mod_found = 1;
      } else if (strstr(s, "bootlogo.svg") || strstr(s, "BOOTLOGO.SVG")) {
        uint32_t size = mods[i].mod_end - mods[i].mod_start;
        if (size > 65535) size = 65535;
        memcpy(g_bootlogo_buffer, (void *)(uintptr_t)mods[i].mod_start, size);
        g_bootlogo_buffer[size] = '\0';
        g_bootlogo_found = 1;
      } else if (strstr(s, "wallpaper_1.svg") || strstr(s, "WALLPAPER_1.SVG")) {
        uint32_t size = mods[i].mod_end - mods[i].mod_start;
        if (size > 131071) size = 131071;
        memcpy(g_wallpaper_buffer, (void *)(uintptr_t)mods[i].mod_start, size);
        g_wallpaper_buffer[size] = '\0';
        g_wallpaper_found = 1;
      }
    }
  }

  // 2. インデックスベースのフォールバック (grub.cfg の定義順)
  // 0: Font, 1: main.warp, 2: new.warp1, 3: bootlogo.svg, 4: wallpaper_1.svg
  if (!g_warp_mod_found && mbi->mods_count >= 2) {
    uint32_t size = mods[1].mod_end - mods[1].mod_start;
    if (size > 32767) size = 32767;
    memcpy(g_warp_buffer, (void *)(uintptr_t)mods[1].mod_start, size);
    g_warp_buffer[size] = '\0'; g_warp_mod_found = 1;
  }
  if (!g_warp1_mod_found && mbi->mods_count >= 3) {
    uint32_t size = mods[2].mod_end - mods[2].mod_start;
    if (size > 32767) size = 32767;
    memcpy(g_warp1_buffer, (void *)(uintptr_t)mods[2].mod_start, size);
    g_warp1_buffer[size] = '\0'; g_warp1_mod_found = 1;
  }
  if (!g_bootlogo_found && mbi->mods_count >= 4) {
    uint32_t size = mods[3].mod_end - mods[3].mod_start;
    if (size > 65535) size = 65535;
    memcpy(g_bootlogo_buffer, (void *)(uintptr_t)mods[3].mod_start, size);
    g_bootlogo_buffer[size] = '\0'; g_bootlogo_found = 1;
  }
  if (!g_wallpaper_found && mbi->mods_count >= 5) {
    uint32_t size = mods[4].mod_end - mods[4].mod_start;
    if (size > 131071) size = 131071;
    memcpy(g_wallpaper_buffer, (void *)(uintptr_t)mods[4].mod_start, size);
    g_wallpaper_buffer[size] = '\0'; g_wallpaper_found = 1;
  }

  // ファイルが見つからない場合の最終手段 (Warp1)
  if (!g_warp1_mod_found) {
    strncpy(g_warp1_buffer, g_hardcoded_warp1, 32767);
    g_warp1_mod_found = 1;
  }
}

typedef struct {
  int x, y, w, h;
  int old_x, old_y, old_w, old_h;
  int is_maximized;
  char title[64];
  warp_context_t *warp_ctx;
  warp1_context_t *warp1_ctx;
  int is_warp1;
  unsigned char *rgba_buffer;
  int buffer_w, buffer_h;
  int is_dirty;
  int is_dragging;
  int is_resizing;
  int resize_w, resize_h; // Frozen dimensions during resize
  float fade_alpha;      // Fade to white: 0.0 (content) to 1.0 (white)
  int is_calculating;    // Calculation state after resize
  float scroll_x, scroll_y;
  float target_scroll_x, target_scroll_y;
  
  // Caching for performance
  uint8_t *shadow_cache;   // Alpha mask for the shadow
  int shadow_cache_w, shadow_cache_h;
  uint32_t *frame_cache;   // Title bar + rounded corners frame
  int frame_cache_w, frame_cache_h;
  uint8_t *window_mask;    // Alpha mask for the entire window shape (squircle)
} window_t;

#define MAX_WINDOWS 8
static window_t g_windows[MAX_WINDOWS];
static int g_window_count = 0;
static int g_active_window_index = -1;

// Forward declaration for blending
static inline uint32_t blend_colors(uint32_t bg, uint32_t fg, uint8_t alpha);

static void window_update_caches(window_t *win) {
  int title_h = 40;
  int shadow_size = 48;
  float win_r = 30.0f; // Adjusted for Squircle shadow approximation
  
  // 1. Update Shadow Cache (Alpha only) - Optimized with symmetry
  int sw = win->w + shadow_size * 2;
  int sh = win->h + title_h + shadow_size * 2;
  if (!win->shadow_cache || win->shadow_cache_w != sw || win->shadow_cache_h != sh) {
    if (win->shadow_cache) free(win->shadow_cache);
    win->shadow_cache = (uint8_t *)malloc((size_t)sw * (size_t)sh);
    win->shadow_cache_w = sw;
    win->shadow_cache_h = sh;
    
    float win_w_f = (float)win->w;
    float win_h_f = (float)(win->h + title_h);
    float half_sw = (float)sw / 2.0f;
    float half_sh = (float)sh / 2.0f;
    
    // Calculate only one quadrant and mirror
    for (int y = 0; y <= sh / 2; y++) {
      for (int x = 0; x <= sw / 2; x++) {
        float qx = fabsf((float)x - half_sw) - (win_w_f / 2.0f - win_r);
        float qy = fabsf((float)y - half_sh) - (win_h_f / 2.0f - win_r);
        float mx = (qx > 0.0f) ? qx : 0.0f;
        float my = (qy > 0.0f) ? qy : 0.0f;
        float inner = (qx > qy) ? qx : qy;
        if (inner > 0.0f) inner = 0.0f;
        float dist = sqrtf(mx*mx + my*my) + inner - win_r;
        
        uint8_t alpha = 0;
        if (dist <= 0.0f) {
          alpha = 64;
        } else if (dist < (float)shadow_size) {
          float d_ratio = dist / (float)shadow_size;
          alpha = (uint8_t)(64.0f * (1.0f - d_ratio) * (1.0f - d_ratio));
        }
        
        win->shadow_cache[y * sw + x] = alpha;
        win->shadow_cache[y * sw + (sw - 1 - x)] = alpha;
        win->shadow_cache[(sh - 1 - y) * sw + x] = alpha;
        win->shadow_cache[(sh - 1 - y) * sw + (sw - 1 - x)] = alpha;
      }
    }
  }
  
  // 2. Update Frame Cache (Title bar)
  int fw = win->w;
  int fh = title_h;
  if (!win->frame_cache || win->frame_cache_w != fw || win->frame_cache_h != fh) {
    if (win->frame_cache) free(win->frame_cache);
    win->frame_cache = (uint32_t *)malloc((size_t)fw * (size_t)fh * 4);
    win->frame_cache_w = fw;
    win->frame_cache_h = fh;
  }
  
  uint32_t theme = (win == &g_windows[g_active_window_index]) ? 0xFFF5F5F5 : 0xFFE0E0E0;
  for (int i = 0; i < fw * fh; i++) win->frame_cache[i] = 0xFF000000 | theme;

  // 3. Update Window Mask Cache (Squircle) - Reverted to SVG for exact Warp shape
  int mw = win->w;
  int mh = win->h + title_h;
  if (!win->window_mask || win->buffer_w != win->w || win->buffer_h != mh) {
    if (win->window_mask) free(win->window_mask);
    win->window_mask = (uint8_t *)malloc((size_t)mw * (size_t)mh);
    
    char svg_buf[4096];
    svg_buf[0] = '\0';
    char w_str[16], h_str[16];
    append_int(w_str, mw);
    append_int(h_str, mh);
    warp_strcat(svg_buf, "<svg width=\""); warp_strcat(svg_buf, w_str);
    warp_strcat(svg_buf, "\" height=\""); warp_strcat(svg_buf, h_str);
    warp_strcat(svg_buf, "\" xmlns=\"http://www.w3.org/2000/svg\">\n");
    emit_squircle_shape_to(svg_buf, sizeof(svg_buf), 0, 0, mw, mh, 32.0f, "black", "");
    warp_strcat(svg_buf, "</svg>");
    
    NSVGimage *mask_img = nsvgParse(svg_buf, "px", 96.0f);
    if (mask_img) {
      if (!g_svg_rast) g_svg_rast = nsvgCreateRasterizer();
      unsigned char *rgba_mask = (unsigned char *)malloc((size_t)mw * (size_t)mh * 4);
      if (rgba_mask) {
        nsvgRasterize(g_svg_rast, mask_img, 0, 0, 1.0f, rgba_mask, mw, mh, mw * 4);
        for (int i = 0; i < mw * mh; i++) {
          win->window_mask[i] = rgba_mask[i * 4 + 3]; // Use alpha channel
        }
        free(rgba_mask);
      }
      nsvgDelete(mask_img);
    }
  }
}

static void window_redraw(window_t *win) {
  if (!win->warp_ctx && !win->warp1_ctx) return;
  
  strncpy(g_hud_status, "EngineUpdate", 63);
  if (win->is_warp1) {
    warp1_context_update(win->warp1_ctx, win->w, win->h);
  } else {
    warp_context_update(win->warp_ctx, win->w, win->h);
  }
  
  strncpy(g_hud_status, "SVGGen", 63);
  const char *svg = win->is_warp1 ? warp1_context_get_svg(win->warp1_ctx) : warp_context_get_svg(win->warp_ctx);
  
  strncpy(g_hud_status, "NSVGParse", 63);
  NSVGimage *img = nsvgParse((char*)svg, "px", 96.0f);
  if (!img) {
    strncpy(g_hud_status, "ParseErr", 63);
    return;
  }

  // Content height is determined by the SVG itself
  int content_h = (int)img->height;
  if (content_h < win->h) content_h = win->h;

  // Re-allocate buffer if width changed or height grew
  if (!win->rgba_buffer || win->buffer_w != win->w || win->buffer_h != content_h) {
    strncpy(g_hud_status, "Alloc", 63);
    if (win->rgba_buffer) free(win->rgba_buffer);
    win->rgba_buffer = (unsigned char *)malloc((size_t)win->w * (size_t)content_h * 4);
    win->buffer_w = win->w;
    win->buffer_h = content_h;
  }
  
  if (win->rgba_buffer) {
    strncpy(g_hud_status, "ClearBG", 63);
    const uint32_t bg = 0xFFFFFFFF;
    for (int i = 0; i < win->w * win->buffer_h; i++) ((uint32_t*)win->rgba_buffer)[i] = bg;
    
    strncpy(g_hud_status, "NSVGRast", 63);
    if (!g_svg_rast) g_svg_rast = nsvgCreateRasterizer();
    nsvgRasterize(g_svg_rast, img, 0, 0, 1.0f, win->rgba_buffer, win->w, win->buffer_h, win->w * 4);
    
    strncpy(g_hud_status, "RBSwap", 63);
    // R/B Swap to system native
    unsigned char *p = win->rgba_buffer;
    for (int i = 0; i < win->w * win->buffer_h; i++) {
      unsigned char r = p[0], b = p[2];
      p[0] = b; p[2] = r; p += 4;
    }
    
    strncpy(g_hud_status, "TxtDraw", 63);
    layer_t temp_layer;
    temp_layer.buffer = (uint32_t*)win->rgba_buffer;
    temp_layer.width = win->w;
    temp_layer.height = win->buffer_h;
    if (win->is_warp1) {
      warp1_context_draw_texts(win->warp1_ctx, &temp_layer, 0, 0);
    } else {
      warp_context_draw_texts(win->warp_ctx, &temp_layer, 0, 0);
    }
  }
  nsvgDelete(img);
  win->is_dirty = 0;
  win->is_calculating = 0; // Layout/Calculation is done, allow fade back
  
  strncpy(g_hud_status, "Idle", 63);
  // Update caches after redraw
  window_update_caches(win);
}

static void add_window(const char *title, int x, int y, int w, int h, int is_warp1) {
  if (g_window_count >= MAX_WINDOWS) return;
  window_t *win = &g_windows[g_window_count++];
  win->x = x; win->y = y; win->w = w; win->h = h;
  win->scroll_x = 0; win->scroll_y = 0;
  win->target_scroll_x = 0; win->target_scroll_y = 0;
  strncpy(win->title, title, 63);
  win->is_warp1 = is_warp1;
  if (is_warp1) {
    win->warp1_ctx = warp1_context_create(g_warp1_buffer);
    win->warp_ctx = NULL;
  } else {
    win->warp_ctx = warp_context_create(g_warp_buffer);
    win->warp1_ctx = NULL;
  }
  win->rgba_buffer = NULL;
  win->shadow_cache = NULL;
  win->frame_cache = NULL;
  win->window_mask = NULL;
  win->is_dirty = 1;
  win->is_maximized = 0;
  win->is_dragging = 0;
  win->is_resizing = 0;
  win->resize_w = w;
  win->resize_h = h;
  win->fade_alpha = 0.0f;
  win->is_calculating = 0;
  g_active_window_index = g_window_count - 1;
  window_update_caches(win);
}

static void close_active_window() {
  if (g_active_window_index < 0) return;
  window_t *win = &g_windows[g_active_window_index];
  if (win->warp_ctx) warp_context_destroy(win->warp_ctx);
  if (win->warp1_ctx) warp1_context_destroy(win->warp1_ctx);
  if (win->rgba_buffer) free(win->rgba_buffer);
  if (win->shadow_cache) free(win->shadow_cache);
  if (win->frame_cache) free(win->frame_cache);
  if (win->window_mask) free(win->window_mask);
  
  for (int i = g_active_window_index; i < g_window_count - 1; i++) {
    g_windows[i] = g_windows[i+1];
  }
  g_window_count--;
  g_active_window_index = g_window_count - 1;
  g_svg_dirty = 1;
}

static void draw_wallpaper(layer_t *layer) {
  // Use existing bootlogo as wallpaper
  if (g_svg_ready) {
    memcpy(layer->buffer, svg_base_buf, sizeof(uint32_t) * layer->width * layer->height);
  } else {
    layer_fill(layer, 0x00000000); // Transparent fallback
  }
}

static void redraw_warp_svg(layer_t *layer) {
  if (!g_svg_dirty) return;
  draw_wallpaper(layer);
  
  for (int i = 0; i < g_window_count; i++) {
    window_t *win = &g_windows[i];
    // IMPORTANT: Only redraw SVG if NOT resizing. Frame caches (mask/titlebar) are updated in move block.
    if (win->is_dirty && !win->is_resizing) window_redraw(win);
    
    if (win->rgba_buffer && win->shadow_cache && win->frame_cache) {
      int title_h = 40;
      int shadow_size = 48;
      
      // 1. Draw Window Shadow from Cache
      if (!win->is_maximized) {
        int sw = win->shadow_cache_w;
        int sh = win->shadow_cache_h;
        int sx_start = win->x - shadow_size;
        int sy_start = win->y - title_h - shadow_size + 8; // Offset Y by 8
        
        int y0 = (sy_start < 0) ? -sy_start : 0;
        int y1 = (sy_start + sh > layer->height) ? layer->height - sy_start : sh;
        int x0 = (sx_start < 0) ? -sx_start : 0;
        int x1 = (sx_start + sw > layer->width) ? layer->width - sx_start : sw;

        for (int dy = y0; dy < y1; dy++) {
          int py = sy_start + dy;
          uint32_t *dst_line = &layer->buffer[py * layer->width];
          uint8_t *src_mask = &win->shadow_cache[dy * sw];
          
          for (int dx = x0; dx < x1; dx++) {
            uint8_t alpha = src_mask[dx];
            if (alpha == 0) continue;
            
            int px = sx_start + dx;
            uint32_t bg = dst_line[px];
            uint32_t a_bg = bg >> 24;
            uint32_t inv_alpha = 255 - alpha;
            // Shadow is black (0,0,0), so we just darken the background
            uint32_t rb = (bg & 0xFF00FFu) * inv_alpha >> 8;
            uint32_t g = ((bg >> 8) & 0xFF) * inv_alpha >> 8;
            uint32_t a_out = alpha + (a_bg * inv_alpha >> 8);
            dst_line[px] = (a_out << 24) | (rb & 0xFF00FFu) | (g << 8);
          }
        }
      }

      // 2. Draw Title Bar from Cache
      uint32_t *src_frame = win->frame_cache;
      int ty0 = (win->y - title_h < 0) ? -(win->y - title_h) : 0;
      int ty1 = (win->y < layer->height) ? title_h : (layer->height - (win->y - title_h));
      int tx0 = (win->x < 0) ? -win->x : 0;
      int tx1 = (win->x + win->w > layer->width) ? (layer->width - win->x) : win->w;

      for (int dy = ty0; dy < ty1; dy++) {
        int py = win->y - title_h + dy;
        uint32_t *dst_line = &layer->buffer[py * layer->width];
        uint32_t *src_line = &src_frame[dy * win->w];
        uint8_t *mask_line = &win->window_mask[dy * win->w];
        for (int dx = tx0; dx < tx1; dx++) {
          int px = win->x + dx;
          uint32_t color = src_line[dx];
          uint8_t alpha = win->is_maximized ? 255 : mask_line[dx];
          if (alpha == 255) dst_line[px] = color;
          else if (alpha > 0) dst_line[px] = blend_colors(dst_line[px], color, alpha);
        }
      }
      
      // 3. Title bar content
      char header_text[128];
      int action_count = 0;
      int has_header = 0;
      if (win->is_warp1) {
        if (win->warp1_ctx) has_header = warp1_context_get_header_info(win->warp1_ctx, header_text, sizeof(header_text), &action_count);
      } else {
        if (win->warp_ctx) has_header = warp_context_get_header_info(win->warp_ctx, header_text, sizeof(header_text), &action_count);
      }
      
      if (has_header) {
        layer_draw_ttf(layer, win->x + 70, win->y - 28, header_text, 16, 0xFF333333);
        
        // Actions on the right (Buttons)
        int ax = win->x + win->w - 12;
        for (int j = 0; j < action_count; j++) {
          char act_text[64];
          if (win->is_warp1) warp1_context_get_header_action_info(win->warp1_ctx, j, act_text, sizeof(act_text));
          else warp_context_get_header_action_info(win->warp_ctx, j, act_text, sizeof(act_text));
          int text_w = strlen(act_text) * 9; 
          int btn_w = text_w + 24;
          int btn_h = 26; 
          ax -= btn_w;
          
          float r = 8.0f; // button radius
          int ay0 = (win->y - 33 < 0) ? -(win->y - 33) : 0;
          int ay1 = (win->y - 33 + btn_h > layer->height) ? (layer->height - (win->y - 33)) : btn_h;
          int ax0 = (ax < 0) ? -ax : 0;
          int ax1 = (ax + btn_w > layer->width) ? (layer->width - ax) : btn_w;

          for (int dy = ay0; dy < ay1; dy++) {
            int py = win->y - 33 + dy; 
            uint32_t *dst_line = &layer->buffer[py * layer->width];
            for (int dx = ax0; dx < ax1; dx++) {
              int px = ax + dx;
              float alpha_f = 1.0f;
              float fdx = (float)dx, fdy = (float)dy;
              float fbw = (float)btn_w, fbh = (float)btn_h;

              if (fdx < r && fdy < r) {
                float dist = sqrtf((fdx-r)*(fdx-r) + (fdy-r)*(fdy-r));
                if (dist > r + 0.5f) alpha_f = 0.0f;
                else if (dist > r - 0.5f) alpha_f = r + 0.5f - dist;
              } else if (fdx > fbw-r-1.0f && fdy < r) {
                float dist = sqrtf((fdx-(fbw-r-1.0f))*(fdx-(fbw-r-1.0f)) + (fdy-r)*(fdy-r));
                if (dist > r + 0.5f) alpha_f = 0.0f;
                else if (dist > r - 0.5f) alpha_f = r + 0.5f - dist;
              } else if (fdx < r && fdy > fbh-r-1.0f) {
                float dist = sqrtf((fdx-r)*(fdx-r) + (fdy-(fbh-r-1.0f))*(fdy-(fbh-r-1.0f)));
                if (dist > r + 0.5f) alpha_f = 0.0f;
                else if (dist > r - 0.5f) alpha_f = r + 0.5f - dist;
              } else if (fdx > fbw-r-1.0f && fdy > fbh-r-1.0f) {
                float dist = sqrtf((fdx-(fbw-r-1.0f))*(fdx-(fbw-r-1.0f)) + (fdy-(fbh-r-1.0f))*(fdy-(fbh-r-1.0f)));
                if (dist > r + 0.5f) alpha_f = 0.0f;
                else if (dist > r - 0.5f) alpha_f = r + 0.5f - dist;
              }

              if (alpha_f > 0.0f) {
                dst_line[px] = blend_colors(dst_line[px], 0xFFFFFFFF, (uint8_t)(alpha_f * 255));
              }
            }
          }
          layer_draw_ttf(layer, ax + 12, win->y - 26, act_text, 14, 0xFF000000);
          ax -= 10;
        }
      } else {
        layer_draw_ttf(layer, win->x + 70, win->y - 28, win->title, 16, 0xFF333333);
      }
      
      // 4. Draw control circles
      float btn_r = 7.0f;
      int btn_y = win->y - 20;
      int i_r = (int)btn_r + 1;
      uint32_t colors[] = {0xFFFF2836, 0xFF2ECC46};
      int centers_x[] = {win->x + 20, win->x + 44};
      for (int k = 0; k < 2; k++) {
        int cy0 = (btn_y - i_r < 0) ? -(btn_y - i_r) : -i_r;
        int cy1 = (btn_y + i_r >= layer->height) ? (layer->height - 1 - btn_y) : i_r;
        int cx0 = (centers_x[k] - i_r < 0) ? -(centers_x[k] - i_r) : -i_r;
        int cx1 = (centers_x[k] + i_r >= layer->width) ? (layer->width - 1 - centers_x[k]) : i_r;

        for (int dy = cy0; dy <= cy1; dy++) {
          int py = btn_y + dy;
          uint32_t *dst_line = &layer->buffer[py * layer->width];
          for (int dx = cx0; dx <= cx1; dx++) {
            float dist = sqrtf((float)(dx*dx + dy*dy));
            float alpha_f = 0.0f;
            if (dist <= btn_r - 0.5f) alpha_f = 1.0f;
            else if (dist <= btn_r + 0.5f) alpha_f = (btn_r + 0.5f - dist);
            if (alpha_f > 0.0f) {
              int px = centers_x[k] + dx;
              dst_line[px] = blend_colors(dst_line[px], colors[k], (uint8_t)(alpha_f * 255));
            }
          }
        }
      }

      // 5. Content with scroll offset and squircle corners
      int sy_int = (int)roundf(win->scroll_y);
      int cy0 = (win->y < 0) ? -win->y : 0;
      int cy1 = (win->y + win->h > layer->height) ? (layer->height - win->y) : win->h;
      int cx0 = (win->x < 0) ? -win->x : 0;
      int cx1 = (win->x + win->w > layer->width) ? (layer->width - win->x) : win->w;

      for (int dy = cy0; dy < cy1; dy++) {
        int py = win->y + dy;
        int src_y = dy - sy_int;
        uint32_t *dst_line = &layer->buffer[py * layer->width];
        
        // Use frozen dimensions for source lookup during resizing
        int frozen_w = win->is_resizing ? win->resize_w : win->w;
        int frozen_h = win->is_resizing ? win->resize_h : win->h;

        if (src_y < 0 || src_y >= win->buffer_h || (win->is_resizing && (dy >= frozen_h || src_y >= frozen_h))) {
          uint8_t *mask_line = &win->window_mask[(dy + title_h) * win->w];
          for (int dx = cx0; dx < cx1; dx++) {
            uint8_t alpha = win->is_maximized ? 255 : mask_line[dx];
            if (alpha == 255) dst_line[win->x + dx] = 0xFFFFFFFF;
            else if (alpha > 0) dst_line[win->x + dx] = blend_colors(dst_line[win->x + dx], 0xFFFFFFFF, alpha);
          }
          continue;
        }

        uint32_t *src_content = (uint32_t*)&win->rgba_buffer[src_y * win->buffer_w * 4];
        uint8_t *mask_line = &win->window_mask[(dy + title_h) * win->w];
        uint8_t fade_alpha_u8 = (uint8_t)(win->fade_alpha * 255);

        for (int dx = cx0; dx < cx1; dx++) {
          int px = win->x + dx;
          uint8_t alpha = win->is_maximized ? 255 : mask_line[dx];
          if (alpha > 0) {
            uint32_t color;
            if (win->is_resizing && dx >= frozen_w) {
              color = 0xFFFFFFFF; 
            } else {
              color = src_content[dx];
              if (fade_alpha_u8 > 0) {
                color = blend_colors(color, 0xFFFFFFFF, fade_alpha_u8);
              }
            }
            if (alpha == 255) dst_line[px] = color;
            else dst_line[px] = blend_colors(dst_line[px], color, alpha);
          }
        }
      }
      
      // 6. Handle
      if (i == g_active_window_index) {
        int handle_s = 12;
        int hy0 = (win->h - handle_s < cy0) ? cy0 : win->h - handle_s;
        int hy1 = (win->h > cy1) ? cy1 : win->h;
        int hx0 = (win->w - handle_s < cx0) ? cx0 : win->w - handle_s;
        int hx1 = (win->w > cx1) ? cx1 : win->w;

        for (int dy = hy0; dy < hy1; dy++) {
          int py = win->y + dy;
          uint32_t *dst_line = &layer->buffer[py * layer->width];
          for (int dx = hx0; dx < hx1; dx++) {
            int px = win->x + dx;
            dst_line[px] = blend_colors(dst_line[px], 0xFFCCCCCC, 255);
          }
        }
      }
    }
  }
  g_svg_dirty = 0;
}

static int svg_init_nextgen(layer_t *layer) {
  svg_init(layer, 1); // Load and render wallpaper
  // Start with one default window
  add_window("Main Warp", 100, 100, 600, 400, 0);
  redraw_warp_svg(layer);
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

          nsvgRasterize(g_svg_rast, g_svg_image,
                        g_svg_tx + g_scroll_x - (float)x0,
                        g_svg_ty + g_scroll_y - (float)y0, g_svg_scale,
                        g_svg_hover_buf, w, h, w * 4);

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
      int center_x = (int)((float)(src_x + src_w / 2) + hover_offx);
      int center_y = (int)((float)(src_y + src_h / 2) + hover_offy);
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
          float sx = (float)(x - (center_x - dst_w / 2)) / scale;
          float sy = (float)(y - (center_y - dst_h / 2)) / scale;
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
    float x0f = c->shape->bounds[0] * g_svg_scale + g_svg_tx + g_scroll_x;
    float y0f = c->shape->bounds[1] * g_svg_scale + g_svg_ty + g_scroll_y;
    float x1f = c->shape->bounds[2] * g_svg_scale + g_svg_tx + g_scroll_x;
    float y1f = c->shape->bounds[3] * g_svg_scale + g_svg_ty + g_scroll_y;

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
  *cx = ((x0 + x1) * 0.5f) * g_svg_scale + g_svg_tx + g_scroll_x;
  *cy = ((y0 + y1) * 0.5f) * g_svg_scale + g_svg_ty + g_scroll_y;
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
  float ix = (lx - g_svg_tx - g_scroll_x) / g_svg_scale;
  float iy = (ly - g_svg_ty - g_scroll_y) / g_svg_scale;

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

// キー入力バッファ
#define KEYBUF_MAX 256
static char keybuf_str[KEYBUF_MAX] = "";

static uint32_t g_mbi_flags = 0;

static void handle_command(const char *cmd) {
  if (strncasecmp(cmd, "dev ", 4) == 0) {
    const char *kv = cmd + 4;
    char key[64], val[64];
    const char *eq = strchr(kv, '=');
    if (eq) {
      int klen = eq - kv;
      if (klen >= 64) klen = 63;
      strncpy(key, kv, klen);
      key[klen] = '\0';
      
      char full_key[128] = "dev ";
      strncat(full_key, key, 127 - 4);
      
      strncpy(val, eq + 1, 63);
      val[63] = '\0';
      
      for (int i = 0; i < g_window_count; i++) {
        if (g_windows[i].warp_ctx) {
          warp_context_set_state(g_windows[i].warp_ctx, full_key, val);
          g_windows[i].is_dirty = 1;
        }
      }
      g_svg_dirty = 1;
    }
  }
}

static void hud_update(layer_t *hud, unsigned int cpu_percent,
                       unsigned int mem_used_kb, unsigned int mem_total_kb) {
  layer_fill(hud, 0xFF000000);

  char line1[64], line2[64], line3[64], line4[64], line5[64];

  // 1行目: "BaramOS Build <AutoNumber>"
  char *p = line1;
  const char *title = "BaramOS Build ";
  while (*title)
    *p++ = *title++;
  p = append_uint(p, BUILD_NUMBER);
  *p = '\0';

  // 2行目: "CPU: xx% MEM: xx/xxKB"
  p = line2;
  *p++ = 'C';
  *p++ = 'P';
  *p++ = 'U';
  *p++ = ':';
  *p++ = ' ';
  p = append_uint(p, cpu_percent);
  *p++ = '%';
  *p++ = ' ';
  *p++ = 'M';
  *p++ = 'E';
  *p++ = 'M';
  *p++ = ':';
  *p++ = ' ';
  p = append_uint(p, mem_used_kb);
  *p++ = '/';
  p = append_uint(p, mem_total_kb);
  *p++ = 'K';
  *p++ = 'B';
  *p = '\0';

  // 3行目: "M:<MODE> SVG:<SVG> Mod:<OK>(<CNT>) F:<FLAGS>"
  p = line3;
  *p++ = 'M';
  *p++ = ':';
  const char *m_name = (current_os_mode == OS_MODE_CLASSIC) ? "CLS" : "WDP";
  while (*m_name)
    *p++ = *m_name++;
  *p++ = ' ';
  *p++ = 'S';
  *p++ = ':';
  const char *s_status = g_last_svg_parse_status;
  while (*s_status)
    *p++ = *s_status++;
  *p++ = ' ';
  *p++ = 'M';
  *p++ = ':';
  if (g_warp_mod_found) {
    *p++ = 'O';
    *p++ = 'K';
  } else {
    *p++ = 'N';
    *p++ = 'G';
  }
  *p++ = '(';
  p = append_uint(p, g_mod_count);
  *p++ = ')';
  *p++ = ' ';
  *p++ = 'F';
  *p++ = ':';
  // Flagsを16進数っぽく表示 (簡易)
  p = append_uint(p, g_mbi_flags);
  *p = '\0';

  // 4行目: "Status: <g_hud_status>"
  p = line4;
  const char *w_label = "Status: ";
  while (*w_label)
    *p++ = *w_label++;
  const char *w_status = g_hud_status;
  while (*w_status)
    *p++ = *w_status++;
  *p = '\0';

  // 5行目: "Input: <keybuf_str>"
  p = line5;
  const char *k_label = "Input: ";
  while (*k_label)
    *p++ = *k_label++;
  const char *k_str = keybuf_str;
  while (*k_str)
    *p++ = *k_str++;
  *p = '\0';

  layer_draw_string(hud, 2, 0, line1, 0xFFFFFFFF, TRANSPARENT_COLOR);
  layer_draw_string(hud, 2, 8, line2, 0xFFFFFFFF, TRANSPARENT_COLOR);
  layer_draw_string(hud, 2, 16, line3, 0xFF00FF00, TRANSPARENT_COLOR);
  layer_draw_string(hud, 2, 24, line4, 0xFFFFFF00, TRANSPARENT_COLOR);
  layer_draw_string(hud, 2, 32, line5, 0xFF00FFFF, TRANSPARENT_COLOR);
}

// フォント初期化 (Multibootモジュールから)
static int font_init(struct multiboot_info *mbi) {
  if (!mbi) {
    g_font_error = "ERR:no mbi";
    return 0;
  }
  if (!(mbi->flags & 0x8)) {
    g_font_error = "ERR:no mods flag";
    return 0;
  }
  if (mbi->mods_count == 0) {
    g_font_error = "ERR:no modules";
    return 0;
  }
  multiboot_module_t *mod = (multiboot_module_t *)(uintptr_t)mbi->mods_addr;
  unsigned char *ttf = (unsigned char *)(uintptr_t)mod->mod_start;
  uint32_t ttf_size = mod->mod_end - mod->mod_start;
  if (ttf_size < 12) {
    g_font_error = "ERR:ttf too small";
    return 0;
  }
  if (!stbtt_InitFont(&g_font, ttf, stbtt_GetFontOffsetForIndex(ttf, 0))) {
    g_font_error = "ERR:stbtt_InitFont";
    return 0;
  }
  g_font_ready = 1;
  return 1;
}

// 2つの色をアルファ値(0-255)で合成するヘルパー
static inline uint32_t blend_colors(uint32_t bg, uint32_t fg, uint8_t alpha) {
  if (alpha == 0) return bg;
  if (alpha == 255) return (fg | 0xFF000000u);

  uint32_t a_bg = (bg >> 24) & 0xFF;
  if (a_bg == 0) return ((uint32_t)alpha << 24) | (fg & 0x00FFFFFFu);

  uint32_t rb_bg = bg & 0xFF00FFu;
  uint32_t g_bg = (bg >> 8) & 0xFF;
  uint32_t rb_fg = fg & 0xFF00FFu;
  uint32_t g_fg = (fg >> 8) & 0xFF;

  uint32_t rb_out = (rb_bg + (((rb_fg - rb_bg) * alpha) >> 8)) & 0xFF00FFu;
  uint32_t g_out = (g_bg + (((g_fg - g_bg) * alpha) >> 8)) & 0xFF;

  return 0xFF000000u | (g_out << 8) | rb_out;
}

// 文字レイヤーを更新: keybuf_str を画面中央にTTFレンダリング
static void text_layer_redraw(layer_t *text_layer, float font_size) {
  // 透明でクリア
  int i;
  for (i = 0; i < TEXT_LAYER_W * TEXT_LAYER_H; i++)
    text_layer->buffer[i] = TRANSPARENT_COLOR;

  if (!g_font_ready || keybuf_str[0] == '\0')
    return;

  // フォントサイズのガード（0・負にならないよう保証）
  if (font_size < 8.0f)
    font_size = 8.0f;
  if (font_size > 300.0f)
    font_size = 300.0f;

  // 文字サイズ
  float scale = stbtt_ScaleForPixelHeight(&g_font, font_size);

  int ascent, descent, line_gap;
  stbtt_GetFontVMetrics(&g_font, &ascent, &descent, &line_gap);
  int baseline = (int)(ascent * scale);

  // 全体の幅を計算してX中央揃え
  const char *p = keybuf_str;
  int total_w = 0;
  while (*p) {
    uint16_t codepoint;
    const unsigned char *s = (const unsigned char *)p;
    if (s[0] < 0x80) {
      codepoint = s[0];
      p++;
    } else if ((s[0] & 0xE0) == 0xC0) {
      codepoint = ((s[0] & 0x1F) << 6) | (s[1] & 0x3F);
      p += 2;
    } else if ((s[0] & 0xF0) == 0xE0) {
      codepoint = ((s[0] & 0x0F) << 12) | ((s[1] & 0x3F) << 6) | (s[2] & 0x3F);
      p += 3;
    } else {
      p++;
      continue;
    }
    int adv, lsb;
    stbtt_GetCodepointHMetrics(&g_font, codepoint, &adv, &lsb);
    total_w += (int)(adv * scale);
  }

  int start_x = (TEXT_LAYER_W - total_w) / 2;
  int start_y = (TEXT_LAYER_H / 2) - baseline;
  if (start_x < 0)
    start_x = 4;

  // 各文字をレンダリング
  int cx = start_x;
  p = keybuf_str;
  while (*p) {
    uint16_t codepoint;
    const unsigned char *s = (const unsigned char *)p;
    if (s[0] < 0x80) {
      codepoint = s[0];
      p++;
    } else if ((s[0] & 0xE0) == 0xC0) {
      codepoint = ((s[0] & 0x1F) << 6) | (s[1] & 0x3F);
      p += 2;
    } else if ((s[0] & 0xF0) == 0xE0) {
      codepoint = ((s[0] & 0x0F) << 12) | ((s[1] & 0x3F) << 6) | (s[2] & 0x3F);
      p += 3;
    } else {
      p++;
      continue;
    }

    int bw, bh, bx, by;
    unsigned char *bitmap = stbtt_GetCodepointBitmap(
        &g_font, 0, scale, (int)codepoint, &bw, &bh, &bx, &by);
    if (bitmap) {
      int dx, dy;
      for (dy = 0; dy < bh; dy++) {
        int py = start_y + baseline + by + dy;
        if (py < 0 || py >= TEXT_LAYER_H)
          continue;
        for (dx = 0; dx < bw; dx++) {
          int px = cx + bx + dx;
          if (px < 0 || px >= TEXT_LAYER_W)
            continue;
          uint8_t alpha = bitmap[dy * bw + dx];
          if (alpha == 0)
            continue;

          size_t idx = (size_t)py * (size_t)TEXT_LAYER_W + (size_t)px;
          uint32_t bg = text_layer->buffer[idx];
          text_layer->buffer[idx] = blend_colors(bg, 0x00000000u, alpha);
        }
      }
      stbtt_FreeBitmap(bitmap, NULL);
    }

    int adv, lsb;
    stbtt_GetCodepointHMetrics(&g_font, codepoint, &adv, &lsb);
    cx += (int)(adv * scale);
  }
}

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

// --- グリフキャッシュ ---
typedef struct {
  uint16_t codepoint;
  float size;
  int bw, bh, bx, by, adv;
  unsigned char *bitmap;
} glyph_cache_t;
#define MAX_GLYPH_CACHE 1024
static glyph_cache_t g_glyph_cache[MAX_GLYPH_CACHE];
static int g_glyph_cache_count = 0;

static glyph_cache_t* get_glyph(uint16_t codepoint, float size) {
  for (int i = 0; i < g_glyph_cache_count; i++) {
    if (g_glyph_cache[i].codepoint == codepoint && g_glyph_cache[i].size == size)
      return &g_glyph_cache[i];
  }
  if (g_glyph_cache_count >= MAX_GLYPH_CACHE) return NULL;

  glyph_cache_t *gc = &g_glyph_cache[g_glyph_cache_count++];
  float scale = stbtt_ScaleForPixelHeight(&g_font, size);
  gc->bitmap = stbtt_GetCodepointBitmap(&g_font, 0, scale, (int)codepoint, &gc->bw, &gc->bh, &gc->bx, &gc->by);
  int adv_tmp, lsb_tmp;
  stbtt_GetCodepointHMetrics(&g_font, codepoint, &adv_tmp, &lsb_tmp);
  gc->adv = (int)(adv_tmp * scale);
  gc->codepoint = codepoint;
  gc->size = size;
  return gc;
}

void layer_draw_ttf(layer_t *layer, int px, int py, const char *str,
                     float font_size, uint32_t color) {
  if (!g_font_ready || !str || !layer || !layer->buffer)
    return;
  float scale = stbtt_ScaleForPixelHeight(&g_font, font_size);
  int ascent, descent, line_gap;
  stbtt_GetFontVMetrics(&g_font, &ascent, &descent, &line_gap);
  int baseline = (int)(ascent * scale);
  int cx = px;
  const char *p = str;
  while (*p) {
    uint16_t cp = utf8_next(&p);
    glyph_cache_t *gc = get_glyph(cp, font_size);
    if (gc && gc->bitmap) {
      for (int dy = 0; dy < gc->bh; dy++) {
        int dpy = py + baseline + gc->by + dy;
        if (dpy < 0 || dpy >= (int)layer->height)
          continue;
        for (int dx = 0; dx < gc->bw; dx++) {
          int dpx = cx + gc->bx + dx;
          if (dpx < 0 || dpx >= (int)layer->width)
            continue;
          uint8_t alpha = gc->bitmap[dy * gc->bw + dx];
          if (alpha == 0)
            continue;
          uint32_t bg = layer->buffer[dpy * layer->width + dpx];
          layer->buffer[dpy * layer->width + dpx] =
              blend_colors(bg, color, alpha);
        }
      }
      cx += gc->adv;
    }
  }
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
      layer_draw_char(layer, cx, y, (char)code, color, TRANSPARENT_COLOR);
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
extern volatile uint8_t mouse_buttons;
extern volatile uint8_t mouse_buttons;
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
      row[x] = 0xFF8B0000;
    }
  }
}

// ボックスブラー (アルファチャンネルのみ)
static void box_blur_alpha(unsigned char *data, int w, int h, int radius) {
  if (radius <= 0)
    return;
  unsigned char *tmp = (unsigned char *)malloc((size_t)w * (size_t)h);
  if (!tmp)
    return;

  for (int pass = 0; pass < 3; pass++) { // 3回繰り返してガウスブラーに近づける
    // 横方向
    for (int y = 0; y < h; y++) {
      for (int x = 0; x < w; x++) {
        int sum = 0;
        int count = 0;
        for (int dx = -radius; dx <= radius; dx++) {
          int nx = x + dx;
          if (nx >= 0 && nx < w) {
            sum += data[(y * w + nx) * 4 + 3];
            count++;
          }
        }
        tmp[y * w + x] = (unsigned char)(sum / count);
      }
    }
    // 縦方向
    for (int x = 0; x < w; x++) {
      for (int y = 0; y < h; y++) {
        int sum = 0;
        int count = 0;
        for (int dy = -radius; dy <= radius; dy++) {
          int ny = y + dy;
          if (ny >= 0 && ny < h) {
            sum += tmp[ny * w + x];
            count++;
          }
        }
        data[(y * w + x) * 4 + 3] = (unsigned char)(sum / count);
      }
    }
  }
  free(tmp);
}

static void cursor_init(void) {
  const char *cursor_svg =
      "<svg width=\"298\" height=\"352\" viewBox=\"0 0 298 352\" fill=\"none\" "
      "xmlns=\"http://www.w3.org/2000/svg\">"
      "<path d=\"M96.7002 68.2928V175.048C96.7002 181.437 96.7002 184.632 "
      "98.0445 186.647C99.2198 188.41 101.046 189.634 103.123 190.052C105.498 "
      "190.529 108.453 189.316 114.363 186.888L189.092 156.196C194.986 153.776 "
      "197.933 152.565 199.286 150.56C200.47 148.807 200.911 146.657 200.513 "
      "144.579C200.058 142.203 197.825 139.931 193.36 135.385L118.631 "
      "59.3223C111.764 52.3325 108.33 48.8375 105.377 48.5867C102.816 48.3692 "
      "100.306 49.3959 98.6308 51.3463C96.7002 53.5946 96.7002 58.494 96.7002 "
      "68.2928Z\" stroke=\"white\" stroke-width=\"27\" "
      "stroke-linecap=\"round\"/>"
      "<path d=\"M175.272 225.571L122.891 99.8572\" stroke=\"white\" "
      "stroke-width=\"53\" stroke-linecap=\"round\"/>"
      "<path d=\"M96.7002 68.2928V175.048C96.7002 181.437 96.7002 184.632 "
      "98.0445 186.647C99.2198 188.41 101.046 189.634 103.123 190.052C105.498 "
      "190.529 108.453 189.316 114.363 186.888L189.092 156.196C194.986 153.776 "
      "197.933 152.565 199.286 150.56C200.47 148.807 200.911 146.657 200.513 "
      "144.579C200.058 142.203 197.825 139.931 193.36 135.385L118.631 "
      "59.3223C111.764 52.3325 108.33 48.8375 105.377 48.5867C102.816 48.3692 "
      "100.306 49.3959 98.6308 51.3463C96.7002 53.5946 96.7002 58.494 96.7002 "
      "68.2928Z\" fill=\"black\"/>"
      "<path d=\"M175.272 225.571L122.891 99.8572\" stroke=\"black\" "
      "stroke-width=\"25\" stroke-linecap=\"round\"/>"
      "</svg>";

  NSVGimage *img = nsvgParse((char *)cursor_svg, "px", 96.0f);
  if (!img)
    return;

  // 縦48px程度にスケール。シャドウのために余白を追加
  int target_h = 48;
  float scale = (float)target_h / img->height;
  int target_w = (int)(img->width * scale);

  int padding = 16;
  int w = target_w + padding * 2;
  int h = target_h + padding * 2;

  uint32_t *buf = (uint32_t *)malloc((size_t)w * (size_t)h * 4);
  if (!buf) {
    nsvgDelete(img);
    return;
  }

  NSVGrasterizer *rast = nsvgCreateRasterizer();
  unsigned char *rgba = (unsigned char *)malloc((size_t)w * (size_t)h * 4);
  unsigned char *shadow_rgba =
      (unsigned char *)malloc((size_t)w * (size_t)h * 4);

  if (rast && rgba && shadow_rgba) {
    // 1. シャドウ用のラスタライズ (オフセット込)
    memset(shadow_rgba, 0, (size_t)w * (size_t)h * 4);
    nsvgRasterize(rast, img, (float)padding + 2.0f, (float)padding + 4.0f,
                  scale, shadow_rgba, w, h, w * 4);
    box_blur_alpha(shadow_rgba, w, h, 4); // ブラー適用

    // 2. 本体をラスタライズ
    memset(rgba, 0, (size_t)w * (size_t)h * 4);
    nsvgRasterize(rast, img, (float)padding, (float)padding, scale, rgba, w, h,
                  w * 4);

    // 3. 合成 (影 -> 本体)
    for (int i = 0; i < w * h; i++) {
      // 影の色 (黒, 透過度はブラー後のアルファ * 0.5)
      uint8_t shadow_a = (uint8_t)(shadow_rgba[i * 4 + 3] * 0.5f);
      uint8_t r = rgba[i * 4 + 0], g = rgba[i * 4 + 1], b = rgba[i * 4 + 2],
              a = rgba[i * 4 + 3];

      if (a == 255) {
        buf[i] = (0xFFu << 24) | ((uint32_t)r << 16) | ((uint32_t)g << 8) |
                 (uint32_t)b;
      } else {
        // アルファブレンドで合成
        uint8_t out_a = a + (shadow_a * (255 - a) / 255);
        if (out_a == 0) {
          buf[i] = 0;
        } else {
          uint8_t out_r = (uint8_t)((r * a + 0 * (out_a - a)) / out_a);
          uint8_t out_g = (uint8_t)((g * a + 0 * (out_a - a)) / out_a);
          uint8_t out_b = (uint8_t)((b * a + 0 * (out_a - a)) / out_a);
          buf[i] = ((uint32_t)out_a << 24) | ((uint32_t)out_r << 16) |
                   ((uint32_t)out_g << 8) | (uint32_t)out_b;
        }
      }
    }
    set_cursor_bitmap(buf, w, h);
  }

  if (rast)
    nsvgDeleteRasterizer(rast);
  if (rgba)
    free(rgba);
  if (shadow_rgba)
    free(shadow_rgba);
  nsvgDelete(img);
}

void kmain(uint32_t magic, struct multiboot_info *mbi) {
  (void)magic;
  if (mbi)
    g_mbi_flags = mbi->flags;

  // SVG描画などの初期化より前に、まず赤画面を出す
  for (int i = 0; i < 3; ++i) { // 約0.3秒間、赤で塗りつぶし続ける
    fill_framebuffer_red_early(mbi);
    for (volatile int j = 0; j < 3; ++j) {
      __asm__ __volatile__("nop");
    }
  }

  fill_framebuffer_red_early(mbi); // 最後にもう一度赤で塗る

  enable_fpu();

  // 割り込み初期化
  idt_install();
  irq_install();
  irq_install_handler(0, timer_handler);
  timer_phase(100); // 100Hz
  keyboard_install();
  mouse_install();
  enable_interrupts();

  // モジュールとフォントの初期化を最優先で行う
  font_init(mbi);
  warp_ui_mod_init(mbi);

  set_framebuffer_info((uint32_t *)(uintptr_t)mbi->framebuffer_addr,
                       mbi->framebuffer_width, mbi->framebuffer_height,
                       mbi->framebuffer_pitch);

  // カーソル初期化
  cursor_init();

  // 1. 背景 (黒)
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

  // 2. SVG表示エリア (ロゴ用)
  layer_t svg_layer;
  svg_layer.buffer = svg_buf;
  svg_layer.x = 0;
  svg_layer.y = 0;
  svg_layer.width = SVG_WIDTH;
  svg_layer.height = SVG_HEIGHT;
  svg_layer.transparent = 0;
  svg_layer.active = 1;
  svg_layer.dynamic = 0;
  svg_init(&svg_layer, 0);
  register_layer(&svg_layer);

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

  // 4. HUD (左下) - CPU / MEM / モード / ステータス
  layer_t hud_layer;
  hud_layer.buffer = hud_buf;
  hud_layer.x = 10;
  hud_layer.y = SCREEN_HEIGHT - (HUD_H + 10);
  hud_layer.width = HUD_W;
  hud_layer.height = HUD_H;
  hud_layer.transparent = 0;
  hud_layer.active = 1;
  hud_layer.dynamic = 1;
  layer_fill(&hud_layer, 0xFF000000);

  // 5. 次世代UI SVGレイヤー
  layer_t nextgen_ui_layer;
  nextgen_ui_layer.buffer = nextgen_ui_buf;
  nextgen_ui_layer.x = 0;
  nextgen_ui_layer.y = 0;
  nextgen_ui_layer.width = SCREEN_WIDTH;
  nextgen_ui_layer.height = SCREEN_HEIGHT;
  nextgen_ui_layer.transparent = TRANSPARENT_COLOR;
  nextgen_ui_layer.active = 0;
  nextgen_ui_layer.dynamic = 1;
  {
    for (int i = 0; i < SCREEN_WIDTH * SCREEN_HEIGHT; i++)
      nextgen_ui_buf[i] = TRANSPARENT_COLOR;
  }
  register_layer(&nextgen_ui_layer);
  register_layer(&hud_layer); // HUDを上に

  // 6. 文字レイヤー
  layer_t text_layer;
  text_layer.buffer = text_buf;
  text_layer.x = 0;
  text_layer.y = 0;
  text_layer.width = TEXT_LAYER_W;
  text_layer.height = TEXT_LAYER_H;
  text_layer.transparent = TRANSPARENT_COLOR;
  text_layer.active = 1;
  text_layer.dynamic = 1;
  {
    for (int i = 0; i < TEXT_LAYER_W * TEXT_LAYER_H; i++)
      text_buf[i] = TRANSPARENT_COLOR;
  }
  // 初回描画を確実に実行
  screen_mark_static_dirty();
  screen_mark_all_dirty();

  uint32_t last_blink_tick = 0;
  int blink_state = 0;
  uint32_t last_stat_tick = 0;
  uint32_t last_idle_tick = 0;
  unsigned int cpu_percent = 0;
  uint32_t mem_total_kb = mbi->mem_upper;

  int last_hover = -2;
  int last_mouse_x = -1;
  int last_mouse_y = -1;
  uint32_t last_anim_tick = 0;
  uint8_t prev_mouse_buttons = 0;

  uint32_t boot_start_tick = timer_ticks;
  int auto_booted = 0;
  int auto_warp1_booted = 0;

  // メインループ (常時60fpsターゲット)
  while (1) {
    if (!auto_booted && current_os_mode == OS_MODE_CLASSIC &&
        (timer_ticks - boot_start_tick > 50)) {
      current_os_mode = OS_MODE_WARPDESKTOP;
      g_scroll_x = g_scroll_y = g_target_scroll_x = g_target_scroll_y = 0.0f;
      layer_fill(&desktop, 0xFFF5F5F5);
      svg_layer.active = 0;
      blink_layer.active = 0;
      nextgen_ui_layer.active = 1;
      text_layer.active = 0;
      keybuf_str[0] = '\0';
      g_svg_dirty = 1;
      svg_init_nextgen(&nextgen_ui_layer);
      screen_mark_static_dirty();
      auto_booted = 1;
    }

    if (auto_booted && !auto_warp1_booted && (timer_ticks - boot_start_tick > 500)) {
      add_window("New Warp 1", 200, 200, 600, 400, 1);
      g_svg_dirty = 1;
      auto_warp1_booted = 1;
    }

    if (current_os_mode == OS_MODE_CLASSIC) {
      // 0.1秒(10 ticks)ごとに点滅
      if (timer_ticks - last_blink_tick >= 10) {
        blink_state = !blink_state;
        blink_layer.active = blink_state;
        last_blink_tick = timer_ticks;
      }

      int mx = mouse_x + MOUSE_HOTSPOT_X;
      int my = mouse_y + MOUSE_HOTSPOT_Y;
      if (mx != last_mouse_x || my != last_mouse_y) {
        int hover = svg_pick_shape(&svg_layer, mx, my);
        if (hover != last_hover) {
          last_hover = hover;
        }
        last_mouse_x = mx;
        last_mouse_y = my;
        if (keybuf_str[0] != '\0') {
          int my_clamp =
              my < 0 ? 0 : (my >= SCREEN_HEIGHT ? SCREEN_HEIGHT - 1 : my);
          float fsize = 16.0f + (float)my_clamp * (200.0f - 16.0f) /
                                    (float)(SCREEN_HEIGHT - 1);
          text_layer_redraw(&text_layer, fsize);
        }
      }

      if (timer_ticks != last_anim_tick) {
        uint32_t dt = timer_ticks - last_anim_tick;
        last_anim_tick = timer_ticks;
        int moved = 0;
        for (uint32_t i = 0; i < dt; ++i) {
          float dy = (g_target_scroll_y - g_scroll_y) * SCROLL_EASE;
          if (fabsf(dy) < 0.05f) {
            if (g_scroll_y != g_target_scroll_y) {
              g_scroll_y = g_target_scroll_y;
              moved = 1;
            }
          } else {
            g_scroll_y += dy;
            moved = 1;
          }
        }
        if (moved) {
          svg_render_full(&svg_layer);
          memcpy(svg_base_buf, svg_layer.buffer,
                 sizeof(uint32_t) * svg_layer.width * svg_layer.height);
          screen_mark_static_dirty();
        }
      }

      if (keybuf_len > 0) {
        for (int i = 0; i < keybuf_len; i++) {
          char c = (char)keybuf[i];
          if (c == KEY_UP)
            g_target_scroll_y += 100.0f;
          else if (c == KEY_DOWN)
            g_target_scroll_y -= 100.0f;
          else if (c == '\n') {
            handle_command(keybuf_str);
            keybuf_str[0] = '\0';
            text_layer_redraw(&text_layer, 32.0f);
          } else if (c == '\b') {

            int len = strlen(keybuf_str);
            if (len > 0)
              keybuf_str[len - 1] = '\0';
          } else {
            int len = strlen(keybuf_str);
            if (len < KEYBUF_MAX - 1) {
              keybuf_str[len] = c;
              keybuf_str[len + 1] = '\0';
            }
          }
        }
        keybuf_len = 0;
        if (current_os_mode == OS_MODE_CLASSIC) {
          int my_now = (int)mouse_y;
          if (my_now < 0)
            my_now = 0;
          else if (my_now >= SCREEN_HEIGHT)
            my_now = SCREEN_HEIGHT - 1;
          float fsize = 16.0f + (float)my_now * (200.0f - 16.0f) /
                                    (float)(SCREEN_HEIGHT - 1);
          text_layer_redraw(&text_layer, fsize);
        }
      }

      if (timer_ticks - last_stat_tick >= 100) {
        uint32_t total = timer_ticks - last_stat_tick;
        uint32_t idle = idle_ticks - last_idle_tick;
        if (total > 0) {
          uint32_t idle_pct = (idle * 100u) / total;
          cpu_percent = (idle_pct >= 100u) ? 0u : (100u - idle_pct);
        }
        hud_update(&hud_layer, cpu_percent,
                   (unsigned int)(get_used_memory() / 1024), mem_total_kb);
        last_stat_tick = timer_ticks;
        last_idle_tick = idle_ticks;
      }

    } else if (current_os_mode == OS_MODE_WARPDESKTOP) {
      // 1. Scroll Handling (Active Window)
      if (mouse_scroll != 0 && g_active_window_index >= 0) {
        g_windows[g_active_window_index].target_scroll_y += (float)mouse_scroll * 60.0f;
        mouse_scroll = 0;
        if (g_windows[g_active_window_index].target_scroll_y > 0.0f)
          g_windows[g_active_window_index].target_scroll_y = 0.0f;
      }

      // 2. Window Animation (Smooth Scroll)
      if (timer_ticks != last_anim_tick) {
        uint32_t dt = timer_ticks - last_anim_tick;
        last_anim_tick = timer_ticks;
        int moved = 0;
        for (int i = 0; i < g_window_count; i++) {
          window_t *win = &g_windows[i];

          // Resize/Calculating Fade (0.5s = ~50 ticks at 100Hz, or use dt)
          float fade_speed = (float)dt * 0.02f; // 50 ticks for full fade
          if (win->is_resizing) {
            if (win->fade_alpha < 1.0f) {
              win->fade_alpha += fade_speed;
              if (win->fade_alpha > 1.0f) win->fade_alpha = 1.0f;
              moved = 1;
              g_svg_dirty = 1; // Mark SVG dirty to force redraw for fade
            }
          } else if (win->is_calculating) {
            // Keep at 1.0 during heavy calculation
            if (win->fade_alpha < 1.0f) {
                win->fade_alpha = 1.0f;
                moved = 1;
                g_svg_dirty = 1;
            }
          } else {
            if (win->fade_alpha > 0.0f) {
              win->fade_alpha -= fade_speed;
              if (win->fade_alpha < 0.0f) win->fade_alpha = 0.0f;
              moved = 1;
              g_svg_dirty = 1; // Mark SVG dirty to force redraw for fade back
            }
          }

          for (uint32_t j = 0; j < dt; j++) {            float dy = (win->target_scroll_y - win->scroll_y) * SCROLL_EASE;
            if (fabsf(dy) < 0.05f) {
              if (win->scroll_y != win->target_scroll_y) {
                win->scroll_y = win->target_scroll_y;
                moved = 1;
              }
            } else {
              win->scroll_y += dy;
              moved = 1;
            }
          }
        }
        if (moved) g_svg_dirty = 1;
      }

      // 3. Mouse Interaction
      uint8_t curr_btns = mouse_buttons;
      static int last_mouse_x = -1, last_mouse_y = -1;
      int mouse_dx = (last_mouse_x == -1) ? 0 : mouse_x - last_mouse_x;
      int mouse_dy = (last_mouse_y == -1) ? 0 : mouse_y - last_mouse_y;
      last_mouse_x = mouse_x; last_mouse_y = mouse_y;

      if (curr_btns != prev_mouse_buttons) {
        if ((curr_btns & 1) && !(prev_mouse_buttons & 1)) {
          // Press
          int hit_index = -1;
          int hx = mouse_x + MOUSE_HOTSPOT_X;
          int hy = mouse_y + MOUSE_HOTSPOT_Y;
          
          for (int i = g_window_count - 1; i >= 0; i--) {
            window_t *win = &g_windows[i];
            
            // 1. Resize Handle (bottom-right)
            if (hx >= win->x + win->w - 16 && hx < win->x + win->w &&
                hy >= win->y + win->h - 16 && hy < win->y + win->h) {
              hit_index = i;
              g_windows[hit_index].is_resizing = 1;
              g_windows[hit_index].resize_w = win->w; // Capture current w
              g_windows[hit_index].resize_h = win->h; // Capture current h
              g_svg_dirty = 1; // Force animation update
              break;
            }
            
            // 2. Title Bar (Move, Close, Maximize)
            if (hx >= win->x && hx < win->x + win->w &&
                hy >= win->y - 40 && hy < win->y) {
              hit_index = i;
              window_t *hwin = &g_windows[hit_index];
              
              // Close check (left, Red button center at 20px)
              if (hx >= hwin->x + 8 && hx < hwin->x + 32) {
                g_active_window_index = hit_index;
                close_active_window();
                hit_index = -2; // Mark as handled
              } 
              // Maximize check (left, Green button center at 44px)
              else if (hx >= hwin->x + 32 && hx < hwin->x + 56) {
                if (hwin->is_maximized) {
                  hwin->x = hwin->old_x; hwin->y = hwin->old_y;
                  hwin->w = hwin->old_w; hwin->h = hwin->old_h;
                  hwin->is_maximized = 0;
                } else {
                  hwin->old_x = hwin->x; hwin->old_y = hwin->y;
                  hwin->old_w = hwin->w; hwin->old_h = hwin->h;
                  hwin->x = 0; hwin->y = 40; 
                  hwin->w = nextgen_ui_layer.width; hwin->h = nextgen_ui_layer.height - 40;
                  hwin->is_maximized = 1;
                }
                hwin->is_dirty = 1;
              } 
              // Header actions check (right side)
              else {
                int handled = 0;
                char header_text[128];
                int action_count = 0;
                int has_header = 0;
                if (hwin->is_warp1) {
                  if (hwin->warp1_ctx) has_header = warp1_context_get_header_info(hwin->warp1_ctx, header_text, sizeof(header_text), &action_count);
                } else {
                  if (hwin->warp_ctx) has_header = warp_context_get_header_info(hwin->warp_ctx, header_text, sizeof(header_text), &action_count);
                }
                
                if (has_header) {
                  int ax = hwin->x + hwin->w - 12;
                  for (int j = 0; j < action_count; j++) {
                    char act_text[64];
                    if (hwin->is_warp1) warp1_context_get_header_action_info(hwin->warp1_ctx, j, act_text, sizeof(act_text));
                    else warp_context_get_header_action_info(hwin->warp_ctx, j, act_text, sizeof(act_text));
                    int text_w = strlen(act_text) * 9;
                    int btn_w = text_w + 24;
                    ax -= btn_w;
                    if (hx >= ax && hx < ax + btn_w) {
                      if (hwin->is_warp1) warp1_context_click_header_action(hwin->warp1_ctx, j);
                      else warp_context_click_header_action(hwin->warp_ctx, j);
                      hwin->is_dirty = 1;
                      handled = 1;
                      hit_index = -2;
                      break;
                    }
                    ax -= 10;
                  }
                }
                if (!handled && !hwin->is_maximized && hx >= hwin->x + 56) {
                  hwin->is_dragging = 1;
                }
              }
              break;
            }
            
            // ===== kmain.c の該当部分を以下に置き換え =====

// マウスボタン押下時の "3. Content" ブロック:
            // 3. Content
            if (hx >= win->x && hx < win->x + win->w &&
                hy >= win->y && hy < win->y + win->h) {
                hit_index = i;
                
                // Slider チェック（ドラッグフラグを立てる）
                int slider_hit = 0;
                if (win->is_warp1 && win->warp1_ctx) {
                    int rx = hx - win->x;
                    int ry = hy - win->y - (int)win->scroll_y;
                    if (warp1_find_slider_at(win->warp1_ctx, rx, ry)) {
                        g_dragging_slider = 1;
                        g_dragging_window = i;
                        slider_hit = 1;
                    }
                }
                
                // 通常のクリック処理
                if (!slider_hit) {
                    if (win->is_warp1) {
                        warp1_context_click(win->warp1_ctx, hx - win->x, hy - win->y - (int)win->scroll_y);
                    } else {
                        warp_context_click(win->warp_ctx, hx - win->x, hy - win->y - (int)win->scroll_y);
                    }
                    win->is_dirty = 1;
                }
                break;
            }
          }
        }
      }

// マウス移動時の Slider ドラッグ処理:
      if (g_active_window_index >= 0 && (mouse_dx != 0 || mouse_dy != 0)) {
    window_t *awin = &g_windows[g_active_window_index];
    int hx = mouse_x + MOUSE_HOTSPOT_X;
    int hy = mouse_y + MOUSE_HOTSPOT_Y;
    
    // Slider ドラッグ中の処理
    if (g_dragging_slider && g_dragging_window == g_active_window_index) {
        if (awin->is_warp1 && awin->warp1_ctx) {
            int rx = hx - awin->x;
            int ry = hy - awin->y - (int)awin->scroll_y;
            int changed = 0;
            warp1_slider_set_value_at(awin->warp1_ctx, rx, ry, &changed);
            if (changed) {
                awin->is_dirty = 1;
                g_svg_dirty = 1;
            }
        }
    }
    
    // 既存の pointcheck 処理
    if (awin->is_warp1) {
        if (awin->warp1_ctx) {
            warp1_context_set_mouse(awin->warp1_ctx, hx - awin->x, hy - awin->y - (int)awin->scroll_y);
            if (warp1_context_is_dirty(awin->warp1_ctx)) {
                awin->is_dirty = 1;
                g_svg_dirty = 1;
            }
        }
    } else {
        if (awin->warp_ctx) {
            warp_context_set_mouse(awin->warp_ctx, hx - awin->x, hy - awin->y - (int)awin->scroll_y);
            if (warp_context_is_dirty(awin->warp_ctx)) {
                awin->is_dirty = 1;
                g_svg_dirty = 1;
            }
        }
    }
    
    // 既存のドラッグ/リサイズ処理
    if (awin->is_dragging) {
        awin->x += mouse_dx; awin->y += mouse_dy;
        g_svg_dirty = 1;
    } else if (awin->is_resizing) {
        awin->w += mouse_dx; awin->h += mouse_dy;
        if (awin->w < 100) awin->w = 100;
        if (awin->h < 64) awin->h = 64;
        window_update_caches(awin); 
        g_svg_dirty = 1;
    }
}


// キーボード入力処理:
      for (int i = 0; i < keybuf_len; i++) {
        char c = (char)keybuf[i];
        
        // フォーカスされた input があるかチェック
        if (g_active_window_index >= 0) {
            window_t *awin = &g_windows[g_active_window_index];
            const char *focused_id = NULL;
            
            if (awin->is_warp1 && awin->warp1_ctx) {
                focused_id = warp1_context_get_state(awin->warp1_ctx, "--focusedInput");
            }
            
            if (focused_id && focused_id[0]) {
                // input にフォーカスがある場合
                char content_key[128];
                content_key[0] = '-'; content_key[1] = '-'; content_key[2] = '\0';
                strcat(content_key, focused_id);
                strcat(content_key, "Content");
                
                if (c == '\b') {
                    // バックスペース
                    const char *current = warp1_context_get_state(awin->warp1_ctx, content_key);
                    if (current) {
                        int len = strlen(current);
                        if (len > 0) {
                            char new_val[512];
                            strncpy(new_val, current, len - 1);
                            new_val[len - 1] = '\0';
                            warp1_context_set_state(awin->warp1_ctx, content_key, new_val);
                            awin->is_dirty = 1;
                            g_svg_dirty = 1;
                        }
                    }
                } else if (c == '\n') {
                    // Enter でフォーカス解除
                    warp1_context_set_state(awin->warp1_ctx, "--focusedInput", "");
                } else if (c >= 32 && c < 127) {
                    // 通常の文字入力
                    const char *current = warp1_context_get_state(awin->warp1_ctx, content_key);
                    if (current) {
                        char new_val[512];
                        strncpy(new_val, current, 511);
                        new_val[511] = '\0';
                        int len = strlen(new_val);
                        if (len < 511) {
                            new_val[len] = c;
                            new_val[len + 1] = '\0';
                        }
                        warp1_context_set_state(awin->warp1_ctx, content_key, new_val);
                        awin->is_dirty = 1;
                        g_svg_dirty = 1;
                    }
                }
                continue; // input に入力したのでグローバル処理をスキップ
            }
        }
        
        // グローバルキーボード処理
        if (c == KEY_UP)
            g_target_scroll_y += 100.0f;
          else if (c == KEY_DOWN)
            g_target_scroll_y -= 100.0f;
          else if (c == '\b') {
            int len = strlen(keybuf_str);
            if (len > 0)
              keybuf_str[len - 1] = '\0';
          } else if (c == '\n') {
            handle_command(keybuf_str);
            keybuf_str[0] = '\0';
            g_svg_dirty = 1;
          } else {
            int len = strlen(keybuf_str);
            if (len < KEYBUF_MAX - 1) {
              keybuf_str[len] = c;
              keybuf_str[len + 1] = '\0';
              g_svg_dirty = 1;
            }
          }
        }
        keybuf_len = 0;
        if (g_target_scroll_y > 0.0f)
          g_target_scroll_y = 0.0f;
      }

      if (g_svg_dirty) redraw_warp_svg(&nextgen_ui_layer);

      if (timer_ticks - last_stat_tick >= 100) {
        uint32_t total = timer_ticks - last_stat_tick;
        uint32_t idle = idle_ticks - last_idle_tick;
        if (total > 0) {
          uint32_t idle_pct = (idle * 100u) / total;
          cpu_percent = (idle_pct >= 100u) ? 0u : (100u - idle_pct);
        }
        hud_update(&hud_layer, cpu_percent,
                   (unsigned int)(get_used_memory() / 1024), mem_total_kb);
        last_stat_tick = timer_ticks;
        last_idle_tick = idle_ticks;
      }
    }

    // 常時再描画
    cpu_idle = 0;
    screen_refresh();
  }
