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
#include "storage.h"
#include "fs.h"

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
#define SCROLL_EASE 0.4f

static volatile uint32_t idle_ticks = 0;
static volatile int cpu_idle = 0;

// --- グローバル変数 (Nextgen/Warp) ---
static os_mode_t current_os_mode = OS_MODE_CLASSIC;
static char g_last_svg_parse_status[64] = "None";

typedef struct {
  char name[64];
  uint32_t start;
  uint32_t size;
} warp_module_t;

#define MAX_WARP_MODULES 512
static warp_module_t g_warp_modules[MAX_WARP_MODULES];
static uint32_t g_warp_module_count = 0;

static int g_warp_mod_found = 0;
static uint32_t g_mod_count = 0;

// Files are now pointers to TAR archive instead of copied buffers
static const char *g_warp_ptr = NULL;
static uint32_t g_warp_size = 0;
static const char *g_terminal_warp_ptr = NULL;
static uint32_t g_terminal_warp_size = 0;
static const char *g_menubar_warp_ptr = NULL;
static uint32_t g_menubar_warp_size = 0;
static const char *g_bootlogo_ptr = NULL;
static uint32_t g_bootlogo_size = 0;
static const char *g_wallpaper_ptr = NULL;
static uint32_t g_wallpaper_size = 0;
static const char *g_os_settings_ptr = NULL;
static uint32_t g_os_settings_size = 0;

static int g_terminal_mod_found = 0;
static int g_menubar_mod_found = 0;
static int g_bootlogo_found = 0;
static int g_wallpaper_found = 0;
static int g_os_settings_found = 0;

// Global pointer to Multiboot info
static struct multiboot_info *mbi_ptr = NULL;

// TAR Parser
typedef struct {
    char name[100];
    char mode[8];
    char uid[8];
    char gid[8];
    char size[12];
    char mtime[12];
    char chksum[8];
    char typeflag;
    char linkname[100];
    char magic[6];
    char version[2];
    char uname[32];
    char gname[32];
    char devmajor[8];
    char devminor[8];
    char prefix[155];
} __attribute__((packed)) tar_header_t;

static uint32_t octal_to_int(const char *s, int len) {
    uint32_t res = 0;
    for (int i = 0; i < len; i++) {
        if (s[i] < '0' || s[i] > '7') break;
        res = res * 8 + (s[i] - '0');
    }
    return res;
}

static const char* tar_find_file(const char *tar_data, size_t tar_size, const char *filename, uint32_t *out_size) {
    const char *p = tar_data;
    const char *end = tar_data + tar_size;
    while (p + 512 <= end) {
        tar_header_t *h = (tar_header_t *)p;
        if (h->name[0] == '\0') break;
        uint32_t size = octal_to_int(h->size, 12);
        if (strcmp(h->name, filename) == 0) {
            if (out_size) *out_size = size;
            return p + 512;
        }
        p += 512 + ((size + 511) & ~511);
    }
    return NULL;
}

// OS Settings (mapped from initrd)
int g_dev_pointer_check = 1;
static int g_dev_event_check = 0;
static int g_dev_show_hud = 1;

static int g_svg_dirty = 1;
static char g_hud_status[64] = "Idle";

// --- 前方宣言 ---
static uint32_t lerp_color(uint32_t c1, uint32_t c2, float t);
static void apply_conic_gradient(unsigned char *data, int w, int h, int rx,
                                 int ry, int rw, int rh, uint32_t c1,
                                 uint32_t c2);
static void svg_render_full(layer_t *layer);
static void redraw_warp_svg(layer_t *layer);
static char *append_uint(char *p, unsigned int v);
void layer_draw_ttf(layer_t *layer, int px, int py, const char *str,
                    float font_size, uint32_t color);

// Window Management
typedef struct window_struct {
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
  int no_decoration;
  int is_menubar;

  // Caching for performance
  uint8_t *shadow_cache;   // Alpha mask for the shadow
  int shadow_cache_w, shadow_cache_h;
  uint32_t *frame_cache;   // Title bar + rounded corners frame
  int frame_cache_w, frame_cache_h;
  uint8_t *window_mask;    // Alpha mask for the entire window shape (squircle)
  
  // SVG caching for performance
  char *last_svg_str;      // Cached SVG string to detect changes
  uint32_t *raster_cache;  // Rasterized SVG cache
  int raster_cache_w, raster_cache_h;
  float render_scale;      // Scale at which the buffer was rendered
  void *dynamic_file_ptr;  // Pointer to on-demand loaded file data from storage

  // Unified caching for inactive windows (RAM optimization)
  uint32_t *unified_buffer; // Baked RGBA image (shadow + frame + content)
  int unified_w, unified_h;
} window_t;

static void window_update_caches(window_t *win);
static void add_window(const char *title, int x, int y, int w, int h, int is_warp1);
static void close_active_window();
static inline uint32_t blend_colors(uint32_t bg, uint32_t fg, uint8_t alpha);
void set_pending_command(const char *cmd);

#define MAX_WINDOWS 8
static window_t g_windows[MAX_WINDOWS];
static int g_window_count = 0;
static int g_active_window_index = -1;

// --- Global State Store (Sync with UI) ---
typedef struct { char key[64]; char val[512]; } global_var_t;
#define MAX_GLOBAL_VARS 128
static global_var_t g_global_vars[MAX_GLOBAL_VARS];
static int g_global_var_count = 0;

void set_w1_global(const char *key, const char *val);

const char *get_w1_global(const char *key) {
  // Return current state of dev flags if requested
  if (strcmp(key, "~~dev/pointerCheck") == 0) return g_dev_pointer_check ? "true" : "false";
  if (strcmp(key, "~~dev/eventCheck") == 0) return g_dev_event_check ? "true" : "false";
  if (strcmp(key, "~~dev/showHUD") == 0) return g_dev_show_hud ? "true" : "false";

  for (int i = 0; i < g_global_var_count; i++) {
    if (strcmp(g_global_vars[i].key, key) == 0) return g_global_vars[i].val;
  }
  return "";
}

// --- Simple JSON Parser for OS Settings ---
static const char* json_skip_ws(const char* p) {
    while (*p && (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r')) p++;
    return p;
}

// --- OS Settings Fallback (C-embedded) ---
static const char* g_default_os_settings = 
"{\n"
"  \"dev\": {\n"
"    \"pointerCheck\": false,\n"
"    \"eventCheck\": false,\n"
"    \"showHUD\": true\n"
"  },\n"
"  \"firstboot\": [\n"
"    \"warp topbar.warp\"\n"
"  ],\n"
"  \"main\": {\n"
"    \"dark\": true\n"
"  }\n"
"}";

static void parse_os_settings() {
  const char* buf = g_os_settings_found ? g_os_settings_ptr : g_default_os_settings;
  
  if (g_os_settings_found) {
    set_w1_global("--warpSystemLog", "SettingsInInitrd.");
  } else {
    set_w1_global("--warpSystemLog", "UsingEmbeddedSettings.");
  }
  
  // Robust check for "dark" key
  const char *dark_key = strstr(buf, "\"dark\"");
  if (dark_key) {
    const char *p = json_skip_ws(dark_key + 6);
    if (*p == ':') {
        p = json_skip_ws(p + 1);
        if (strncmp(p, "true", 4) == 0 || strncmp(p, "\"true\"", 6) == 0) {
            set_w1_global("~~main/dark", "true");
        } else {
            set_w1_global("~~main/dark", "false");
        }
    }
  }

  // Dev flags
  const char *ptr_key = strstr(buf, "\"pointerCheck\"");
  if (ptr_key) {
    const char *p = json_skip_ws(ptr_key + 14);
    if (*p == ':') {
        p = json_skip_ws(p + 1);
        if (strncmp(p, "true", 4) == 0) {
            set_w1_global("~~dev/pointerCheck", "true");
            g_dev_pointer_check = 1;
        } else if (strncmp(p, "false", 5) == 0) {
            set_w1_global("~~dev/pointerCheck", "false");
            g_dev_pointer_check = 0;
        }
    }
  }

  // Wallpaper key
  const char *wp_key = strstr(buf, "\"wallpaper\"");
  if (wp_key) {
    const char *p = json_skip_ws(wp_key + 11);
    if (*p == ':') {
      p = json_skip_ws(p + 1);
      p = json_skip_ws(p);
      if (*p == '\"') {
        p++; // skip '\"'
        const char *start = p;
        while (*p && *p != '\"') p++;
        if (*p == '\"') {
          char wp_name[64];
          int len = p - start;
          if (len > 63) len = 63;
          memcpy(wp_name, start, len);
          wp_name[len] = '\0';
          set_w1_global("~~main/wallpaper", wp_name);
          
          // Try to load this wallpaper from TAR modules (initrd)
          const char *tar_start = NULL;
          size_t tar_size = 0;
          for (uint32_t i = 0; i < g_mod_count; i++) {
            multiboot_module_t *mods = (multiboot_module_t *)(uintptr_t)mbi_ptr->mods_addr;
            const char *mod_str = (const char *)(uintptr_t)mods[i].string;
            if (mod_str && (strstr(mod_str, "initrd") || strstr(mod_str, "tar"))) {
              tar_start = (const char *)(uintptr_t)mods[i].mod_start;
              tar_size = mods[i].mod_end - mods[i].mod_start;
              break;
            }
          }
          
          if (tar_start) {
              uint32_t size = 0;
              const char *file_ptr = tar_find_file(tar_start, tar_size, wp_name, &size);
              if (file_ptr) {
                  g_wallpaper_ptr = file_ptr;
                  g_wallpaper_size = size;
                  g_wallpaper_found = 1;
                  set_w1_global("--warpSystemLog", "WallpaperSetFromInitrd.");
              }
          }
        }
      }
    }
  }
  
  if (strstr(buf, "\"eventCheck\": true")) set_w1_global("~~dev/eventCheck", "true");
  else if (strstr(buf, "\"eventCheck\": false")) set_w1_global("~~dev/eventCheck", "false");
  
  if (strstr(buf, "\"showHUD\": true")) set_w1_global("~~dev/showHUD", "true");
  else if (strstr(buf, "\"showHUD\": false")) set_w1_global("~~dev/showHUD", "false");

  // Firstboot commands
  const char *fb_key = strstr(buf, "\"firstboot\"");
  if (fb_key) {
    const char *p = strstr(fb_key, ":");
    if (p) {
        p++; // skip ':'
        p = json_skip_ws(p);
        if (*p == '[') {
            p++; // skip '['
            while (*p && *p != ']') {
                p = json_skip_ws(p);
                if (*p == ',') { p++; p = json_skip_ws(p); }
                if (*p == '\"') {
                    p++; // skip '\"'
                    const char *start = p;
                    while (*p && *p != '\"') p++;
                    if (*p == '\"') {
                        char cmd[128];
                        int len = p - start;
                        if (len > 127) len = 127;
                        memcpy(cmd, start, len);
                        cmd[len] = '\0';
                        set_pending_command(cmd);
                        p++; // skip '\"'
                    }
                } else if (*p == ']') {
                    break;
                } else {
                    p++;
                }
            }
        }
    }
  }
  
  // Status Log for debug
  const char* dark_val = get_w1_global("~~main/dark");
  char startup_msg[128] = "OSReady Theme:";
  strlcat(startup_msg, dark_val, 127);
  set_w1_global("--warpSystemLog", startup_msg);
  
  // 常にブートを許可
  g_os_settings_found = 1; 
}

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
static uint32_t main_screen_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
static uint32_t *svg_buf = NULL; // 動的確保に変更
static uint32_t svg_base_buf[SVG_WIDTH * SVG_HEIGHT];
static uint32_t blink_buf[50 * 50];
#define HUD_W 320
#define HUD_H_MAX 240
static uint32_t hud_buf[HUD_W * HUD_H_MAX];
static int g_hud_current_h = 64;
// 文字レイヤー (透過処理用)
#define TEXT_LAYER_W SCREEN_WIDTH
#define TEXT_LAYER_H SCREEN_HEIGHT
static uint32_t text_layer_buf[TEXT_LAYER_W * TEXT_LAYER_H];
// stbtt フォント
static stbtt_fontinfo g_font;
static int g_font_ready = 0;
static const char *g_font_error = NULL;

// メモリアロケータ (フリーリスト方式)
extern char _kernel_end[];
static char *heap_ptr = NULL;
static size_t heap_total_size = 0;

typedef struct block_header {
  size_t size; // このブロックのデータサイズ (ヘッダ除く)
  int used;    // 1=使用中, 0=空き
} block_header_t;

#define BLOCK_HDR_SIZE (sizeof(block_header_t))

static int heap_initialized = 0;

static void heap_init(void *start, size_t size) {
  heap_ptr = (char *)start;
  heap_total_size = size;
  block_header_t *first = (block_header_t *)heap_ptr;
  first->size = heap_total_size - BLOCK_HDR_SIZE;
  first->used = 0;
  heap_initialized = 1;
}

void *malloc(size_t size) {
  if (!heap_initialized)
    return NULL;
  if (size == 0)
    return NULL;
  // 8バイトアライメント
  size = (size + 7) & ~7;

  char *p = heap_ptr;
  char *end = heap_ptr + heap_total_size;

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
  char *p = heap_ptr;
  char *end = heap_ptr + heap_total_size;
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
  char *p = heap_ptr;
  char *end = heap_ptr + heap_total_size;
  while (p + BLOCK_HDR_SIZE <= end) {
    block_header_t *hdr = (block_header_t *)p;
    if (hdr->used) {
      used += hdr->size + BLOCK_HDR_SIZE;
    }
    p += BLOCK_HDR_SIZE + hdr->size;
  }
  return used;
}

uint32_t get_static_memory_usage(void) {
    uint32_t size = 0;
    size += sizeof(main_screen_buf);
    size += sizeof(svg_base_buf);
    size += sizeof(blink_buf);
    size += sizeof(hud_buf);
    size += sizeof(text_layer_buf);
    // その他のグローバル配列
    size += sizeof(g_warp_modules);
    size += sizeof(g_windows);
    size += sizeof(g_global_vars);
    return size;
}

uint32_t get_kernel_image_size(void) {
    // 1MB から _kernel_end まで
    return (uint32_t)((uintptr_t)_kernel_end - 0x100000);
}

void *memset(void *s, int c, size_t n) {
  unsigned char *p = (unsigned char *)s;
  while (n--)
    *p++ = (unsigned char)c;
  return s;
}

void *memcpy(void *dest, const void *src, size_t n) {
  unsigned char *d = (unsigned char *)dest;
  const unsigned char *s = (const unsigned char *)src;
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

size_t strlcpy(char *dst, const char *src, size_t siz) {
  size_t len = strlen(src);
  if (siz > 0) {
    size_t n = (len >= siz) ? siz - 1 : len;
    memcpy(dst, src, n);
    dst[n] = '\0';
  }
  return len;
}

size_t strlcat(char *dst, const char *src, size_t siz) {
  size_t dlen = strlen(dst);
  size_t slen = strlen(src);
  if (dlen >= siz) return siz + slen;
  if (slen < siz - dlen) {
    memcpy(dst + dlen, src, slen + 1);
  } else {
    memcpy(dst + dlen, src, siz - dlen - 1);
    dst[siz - 1] = '\0';
  }
  return dlen + slen;
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
  if (load_wallpaper && g_wallpaper_found && g_wallpaper_ptr) {
    svg_data = g_wallpaper_ptr;
  } else if (g_bootlogo_found && g_bootlogo_ptr) {
    svg_data = g_bootlogo_ptr;
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

  if (load_wallpaper && svg_data == g_wallpaper_ptr) {
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
  if (!load_wallpaper && svg_data == g_bootlogo_ptr) {
    const char *conic_pos = strstr(g_bootlogo_ptr, "conic-gradient");
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
  memcpy(svg_base_buf, layer->buffer, sizeof(uint32_t) * layer->width * layer->height);
  g_svg_ready = 1;

  return 1;
}

static void warp_ui_mod_init(struct multiboot_info *mbi) {
  if (!mbi) return;
  mbi_ptr = mbi;
  
  ata_init();
  fs_init();

  // 1. ストレージに個別ファイルがあるか確認（代表として main.warpc）
  uint32_t test_size = 0;
  void *test_ptr = fs_read_file("main.warpc", &test_size);
  
  if (test_ptr) {
      free(test_ptr); // 確認用なので一旦解放
      set_w1_global("--warpSystemLog", "INITRD: [STORAGE-MODE] Loading individual files...");
  } else {
      // 2. なければRAMディスクから「展開インストール」
      const char *ram_tar_ptr = NULL;
      uint32_t ram_tar_size = 0;
      if (mbi->flags & 0x8) {
          multiboot_module_t *mods = (multiboot_module_t *)(uintptr_t)mbi->mods_addr;
          for (uint32_t i = 0; i < mbi->mods_count; i++) {
            const char *s = (const char *)(uintptr_t)mods[i].string;
            if (s && (strstr(s, "initrd") || strstr(s, "tar"))) {
              ram_tar_ptr = (const char *)(uintptr_t)mods[i].mod_start;
              ram_tar_size = mods[i].mod_end - mods[i].mod_start;
              break;
            }
          }
      }

      if (ram_tar_ptr) {
          set_w1_global("--warpSystemLog", "INITRD: [INSTALLING] Extracting TAR to Storage...");
          fs_format();
          // TARを展開して個別に保存
          const char *p = ram_tar_ptr;
          const char *end = ram_tar_ptr + ram_tar_size;
          while (p + 512 <= end) {
              tar_header_t *h = (tar_header_t *)p;
              if (h->name[0] == '\0') break;
              uint32_t f_size = octal_to_int(h->size, 12);
              if (h->typeflag == '0' || h->typeflag == '\0') {
                  fs_write_file(h->name, p + 512, f_size);
              }
              p += 512 + ((f_size + 511) & ~511);
          }
          set_w1_global("--warpSystemLog", "INITRD: [SUCCESS] Extracted to Disk.");
          
          // RAMディスクの参照を即座に消す（これでメモリ計算から除外される）
          multiboot_module_t *mods = (multiboot_module_t *)(uintptr_t)mbi->mods_addr;
          for (uint32_t i = 0; i < mbi->mods_count; i++) {
              const char *s = (const char *)(uintptr_t)mods[i].string;
              if (s && (strstr(s, "initrd") || strstr(s, "tar"))) {
                  mods[i].mod_end = mods[i].mod_start;
              }
          }
      }
  }

  // 3. ストレージ（またはインストール直後のディスク）から必要なファイルだけをロード
  g_warp_ptr = fs_read_file("main.warpc", &g_warp_size);
  g_terminal_warp_ptr = fs_read_file("terminal.warp", &g_terminal_warp_size);
  g_menubar_warp_ptr = fs_read_file("menubar.warp", &g_menubar_warp_size);
  g_bootlogo_ptr = fs_read_file("bootlogo.svg", &g_bootlogo_size);
  g_os_settings_ptr = fs_read_file("os_settings.json", &g_os_settings_size);
  
  // Wallpaper loading
  uint32_t wp_size = 0;
  void *wp_ptr = fs_read_file("wallpaper_1.svg", &wp_size);
  if (wp_ptr) {
      g_wallpaper_ptr = wp_ptr;
      g_wallpaper_size = wp_size;
      g_wallpaper_found = 1;
  }

  if (g_warp_ptr) g_warp_mod_found = 1;
  
  // モジュールリストもストレージから再構築
  g_warp_module_count = 0;
  for (uint32_t i = 0; i < g_sb.num_files && g_warp_module_count < MAX_WARP_MODULES; i++) {
      fs_entry_t *fe = &g_sb.entries[i];
      strncpy(g_warp_modules[g_warp_module_count].name, fe->name, 63);
      // ストレージからのオンデマンド読み込みを簡略化するため、
      // ここではポインタを NULL にし、実行時に読み込む仕組み（既に add_window 等で対応済）にします。
      // もしくは、代表的なものだけをここでロード済みポインタとしてセットします。
      g_warp_modules[g_warp_module_count].start = 0; 
      g_warp_modules[g_warp_module_count].size = fe->size_bytes;
      
      // 既に個別ロードした主要ファイル（main.warpc等）のポインタを反映
      if (strcmp(fe->name, "main.warpc") == 0) g_warp_modules[g_warp_module_count].start = (uint32_t)(uintptr_t)g_warp_ptr;
      else if (strcmp(fe->name, "terminal.warp") == 0) g_warp_modules[g_warp_module_count].start = (uint32_t)(uintptr_t)g_terminal_warp_ptr;
      
      g_warp_module_count++;
  }

  parse_os_settings();
  
  if (g_warp_mod_found) strncpy(g_hud_status, "DiskMode", 63);
}

// --- ターミナル・コマンド処理 ---
#define MAX_PENDING_COMMANDS 8
static char g_pending_commands[MAX_PENDING_COMMANDS][256];
static int g_pending_command_count = 0;

void set_pending_command(const char *cmd) {
  if (!cmd || g_pending_command_count >= MAX_PENDING_COMMANDS) return;
  strncpy(g_pending_commands[g_pending_command_count], cmd, 255);
  g_pending_command_count++;
}

// デバッグ用フラグ
// g_dev_pointer_check is defined earlier as non-static

static void handle_terminal_command(const char *cmd) {
  char trimmed[256];
  strncpy(trimmed, cmd, 255);
  trimmed[255] = '\0';
  
  // Trim trailing whitespace/newlines
  int len = strlen(trimmed);
  while (len > 0 && (trimmed[len-1] == ' ' || trimmed[len-1] == '\n' || trimmed[len-1] == '\r')) {
    trimmed[len-1] = '\0';
    len--;
  }

  const char *file = NULL;
  if (strncmp(trimmed, "warp ", 5) == 0) {
    file = trimmed + 5;
  } else if (strncmp(trimmed, "./", 2) == 0) {
    file = trimmed + 2;
  } else if (strstr(trimmed, ".warp") || strstr(trimmed, ".warpc")) {
    file = trimmed;
  }

  if (file) {
    // Check if the file exists in modules
    int found = 0;
    int mod_idx = -1;
    for (uint32_t i = 0; i < g_warp_module_count; i++) {
      // Try exact match or case-insensitive match
      if (strcasecmp(g_warp_modules[i].name, file) == 0 || strstr(g_warp_modules[i].name, file)) {
        found = 1;
        mod_idx = i;
        break;
      }
    }

    if (found) {
      // Use the canonical name from the module list
      const char *canonical_name = g_warp_modules[mod_idx].name;
      if (strstr(canonical_name, ".warpc")) {
        add_window(canonical_name, 150, 150, 600, 400, 0);
      } else {
        add_window(canonical_name, 200, 200, 600, 400, 1);
      }
    } else if (strstr(file, "terminal") && g_terminal_mod_found) {
      add_window("Terminal", 200, 200, 600, 400, 1);
    } else if (strstr(file, "menubar") && g_menubar_mod_found) {
      add_window("Menubar", 0, 0, 1280, 32, 1);
    } else {
      char err[512] = "Not found. Modules: ";
      for (uint32_t i = 0; i < g_warp_module_count; i++) {
        strlcat(err, g_warp_modules[i].name, 511);
        if (i < g_warp_module_count - 1) strlcat(err, ", ", 511);
      }
      set_w1_global("--warpSystemLog", err);
    }
  } else if (strcmp(trimmed, "ls") == 0 || strcmp(trimmed, "list") == 0) {
    char list_buf[512] = "Mods: ";
    for (uint32_t i = 0; i < g_warp_module_count; i++) {
      if (i > 0) strlcat(list_buf, ", ", 511);
      strlcat(list_buf, g_warp_modules[i].name, 511);
      
      // Data preview (first 4 bytes)
      strlcat(list_buf, "(", 511);
      const char *data = (const char *)(uintptr_t)g_warp_modules[i].start;
      char hex[5] = "....";
      for(int j=0; j<4; j++) {
        unsigned char c = (unsigned char)data[j];
        if (c >= 32 && c <= 126) hex[j] = c;
        else hex[j] = '?';
      }
      strlcat(list_buf, hex, 511);
      strlcat(list_buf, ")", 511);
    }
    set_w1_global("--warpSystemLog", list_buf);
  } else if (strcmp(trimmed, "reboot") == 0) {
    extern void sys_restart(void);
    sys_restart();
  } else if (strcmp(trimmed, "exit") == 0) {
    close_active_window();
  } else if (strcmp(trimmed, "help") == 0) {
    set_w1_global("--warpSystemLog", "Available commands: <file.warp>, ./<file.warp>, warp <file>, reboot, exit, help, ls");
  } else if (strncmp(trimmed, "dev pointerCheck=", 17) == 0) {
    const char *val = trimmed + 17;
    if (strcmp(val, "true") == 0) g_dev_pointer_check = 1;
    else g_dev_pointer_check = 0;
    strncpy(g_hud_status, g_dev_pointer_check ? "PtrCheck:ON" : "PtrCheck:OFF", 63);
  } else if (strncmp(trimmed, "dev dark=", 9) == 0) {
    const char *val = trimmed + 9;
    set_w1_global("~~json/main/dark", val);
    strncpy(g_hud_status, (strcmp(val, "true") == 0) ? "Dark:ON" : "Dark:OFF", 63);
    for (int i = 0; i < g_window_count; i++) {
      window_update_caches(&g_windows[i]);
      g_windows[i].is_dirty = 1;
    }
    g_svg_dirty = 1;
  } else if (strcmp(trimmed, "storage sync") == 0) {
    if (!mbi_ptr) {
        set_w1_global("--warpSystemLog", "Error: No multiboot info");
        return;
    }
    
    // Lazy Init
    ata_init();
    fs_init();
    
    // Find initrd.tar in memory
    const char *tar_data = NULL;
    uint32_t tar_size = 0;
    multiboot_module_t *mods = (multiboot_module_t *)(uintptr_t)mbi_ptr->mods_addr;
    for (uint32_t i = 0; i < mbi_ptr->mods_count; i++) {
        const char *s = (const char *)(uintptr_t)mods[i].string;
        if (s && (strstr(s, "initrd") || strstr(s, "tar"))) {
            tar_data = (const char *)(uintptr_t)mods[i].mod_start;
            tar_size = mods[i].mod_end - mods[i].mod_start;
            break;
        }
    }
    if (tar_data) {
        fs_format();
        fs_write_file("initrd.tar", tar_data, tar_size);
        set_w1_global("--warpSystemLog", "Storage Synced (initrd.tar saved)");
    } else {
        set_w1_global("--warpSystemLog", "Error: initrd.tar not found in RAM");
    }
  } else if (strcmp(trimmed, "storage ls") == 0) {
    ata_init();
    fs_init();
    fs_list_files();
  }
}

static void sync_all_window_themes() {
  static int last_is_dark = -1;
  const char *dark_val = get_w1_global("~~main/dark");
  int is_dark = (strcmp(dark_val, "true") == 0);
  
  if (is_dark != last_is_dark) {
    for (int i = 0; i < g_window_count; i++) {
      window_update_caches(&g_windows[i]);
      g_windows[i].is_dirty = 1;
    }
    last_is_dark = is_dark;
  }
}

void set_w1_global(const char *key, const char *val) {
  // Sync dev flags
  if (strcmp(key, "~~dev/pointerCheck") == 0) {
    g_dev_pointer_check = (strcmp(val, "true") == 0);
  } else if (strcmp(key, "~~dev/eventCheck") == 0) {
    g_dev_event_check = (strcmp(val, "true") == 0);
  } else if (strcmp(key, "~~dev/showHUD") == 0) {
    g_dev_show_hud = (strcmp(val, "true") == 0);
  }

  int is_log = (strcmp(key, "--warpSystemLog") == 0);
  int theme_changed = (strcmp(key, "~~json/main/dark") == 0);

  for (int i = 0; i < g_global_var_count; i++) {
    if (strcmp(g_global_vars[i].key, key) == 0) {
      if (is_log) {
        // Append log with newline
        strlcat(g_global_vars[i].val, "\n", 511);
        strlcat(g_global_vars[i].val, val, 511);
        return;
      }
      if (strcmp(g_global_vars[i].val, val) != 0) {
        strncpy(g_global_vars[i].val, val, 511);
        if (theme_changed) {
          for (int j = 0; j < g_window_count; j++) {
            window_update_caches(&g_windows[j]);
            g_windows[j].is_dirty = 1;
          }
          g_svg_dirty = 1;
        }
      }
      return;
    }
  }
  if (g_global_var_count < MAX_GLOBAL_VARS) {
    strncpy(g_global_vars[g_global_var_count].key, key, 63);
    strncpy(g_global_vars[g_global_var_count].val, val, 511);
    g_global_var_count++;
    if (theme_changed) {
      for (int j = 0; j < g_window_count; j++) {
        window_update_caches(&g_windows[j]);
        g_windows[j].is_dirty = 1;
      }
      g_svg_dirty = 1;
    }
  }
}

static void window_set_all_dirty() {
  for (int i = 0; i < g_window_count; i++) {
    g_windows[i].is_dirty = 1;
  }
  g_svg_dirty = 1;
}

static void window_clear_caches(window_t *win) {
  if (win->shadow_cache) { free(win->shadow_cache); win->shadow_cache = NULL; }
  if (win->frame_cache) { free(win->frame_cache); win->frame_cache = NULL; }
  if (win->window_mask) { free(win->window_mask); win->window_mask = NULL; }
  if (win->rgba_buffer) { free(win->rgba_buffer); win->rgba_buffer = NULL; }
  if (win->raster_cache) { free(win->raster_cache); win->raster_cache = NULL; }
}

static void window_bake(window_t *win) {
  if (!win->shadow_cache || !win->frame_cache || !win->rgba_buffer || !win->window_mask) return;

  float scale = win->render_scale;
  int title_h = win->no_decoration ? 0 : 40;
  int shadow_size = win->no_decoration ? 0 : 48;
  
  int full_sw = win->w + shadow_size * 2;
  int full_sh = win->h + title_h + shadow_size * 2;
  int sw = (int)((float)full_sw * scale);
  int sh = (int)((float)full_sh * scale);
  if (sw < 1) sw = 1; if (sh < 1) sh = 1;

  if (win->unified_buffer) free(win->unified_buffer);
  win->unified_buffer = (uint32_t*)malloc((size_t)sw * (size_t)sh * 4);
  win->unified_w = sw;
  win->unified_h = sh;

  // Clear unified buffer with transparent black
  for (int i = 0; i < sw * sh; i++) win->unified_buffer[i] = 0x00000000;

  // 1. Bake Shadow (offset Y by 8)
  int shadow_off_y = (int)(8.0f * scale);
  for (int y = 0; y < win->shadow_cache_h; y++) {
    int py = y + shadow_off_y;
    if (py >= sh) break;
    for (int x = 0; x < win->shadow_cache_w; x++) {
      uint8_t alpha = win->shadow_cache[y * win->shadow_cache_w + x];
      if (alpha > 0) win->unified_buffer[py * sw + x] = (uint32_t)alpha << 24;
    }
  }

  // 2. Bake Frame (Title Bar)
  int frame_x = (int)((float)shadow_size * scale);
  int frame_y = (int)((float)shadow_size * scale);
  int mw = (int)((float)win->w * scale);
  if (mw < 1 && win->w > 0) mw = 1;

  for (int dy = 0; dy < win->frame_cache_h; dy++) {
    int py = frame_y + dy;
    uint32_t *src_line = &win->frame_cache[dy * win->frame_cache_w];
    uint8_t *mask_line = &win->window_mask[dy * mw];
    for (int dx = 0; dx < win->frame_cache_w; dx++) {
      int px = frame_x + dx;
      uint32_t color = src_line[dx];
      uint8_t alpha = win->is_maximized ? 255 : mask_line[dx];
      win->unified_buffer[py * sw + px] = blend_colors(win->unified_buffer[py * sw + px], color, alpha);
    }
  }

  // 3. Bake Content
  int content_y = frame_y + (int)((float)title_h * scale);
  int mh = (int)((float)(win->h + title_h) * scale);
  for (int dy = 0; dy < win->buffer_h; dy++) {
    int py = content_y + dy;
    if (py >= sh) break;
    uint32_t *src_line = (uint32_t*)&win->rgba_buffer[dy * win->buffer_w * 4];
    int mask_y = (int)((float)title_h * scale) + dy;
    if (mask_y >= mh) mask_y = mh - 1;
    uint8_t *mask_line = &win->window_mask[mask_y * mw];
    for (int dx = 0; dx < win->buffer_w; dx++) {
      int px = frame_x + dx;
      if (px >= sw) break;
      uint32_t color = src_line[dx];
      uint8_t alpha = (win->is_maximized || win->no_decoration) ? 255 : mask_line[dx];
      win->unified_buffer[py * sw + px] = blend_colors(win->unified_buffer[py * sw + px], color, alpha);
    }
  }

  // 4. Cleanup individual caches to save RAM
  window_clear_caches(win);
}

static void window_update_caches(window_t *win) {
  float scale = win->render_scale;
  if (scale <= 0.0f) scale = 1.0f;

  int title_h = win->no_decoration ? 0 : 40;
  int shadow_size = win->no_decoration ? 0 : 48;
  float win_r = 30.0f; // Adjusted for Squircle shadow approximation

  // 1. Update Shadow Cache
  int full_sw = win->w + shadow_size * 2;
  int full_sh = win->h + title_h + shadow_size * 2;
  int sw = (int)((float)full_sw * scale);
  int sh = (int)((float)full_sh * scale);
  if (sw < 1) sw = 1;
  if (sh < 1) sh = 1;

  if (!win->shadow_cache || win->shadow_cache_w != sw || win->shadow_cache_h != sh) {
    if (win->shadow_cache) free(win->shadow_cache);
    win->shadow_cache = (uint8_t *)malloc((size_t)sw * (size_t)sh);
    win->shadow_cache_w = sw;
    win->shadow_cache_h = sh;

    float win_w_f = (float)win->w;
    float win_h_f = (float)(win->h + title_h);
    float half_sw = (float)full_sw / 2.0f;
    float half_sh = (float)full_sh / 2.0f;

    for (int y = 0; y < sh; y++) {
      float fy = (float)y / scale;
      for (int x = 0; x < sw; x++) {
        float fx = (float)x / scale;
        float qx = fabsf(fx - half_sw) - (win_w_f / 2.0f - win_r);
        float qy = fabsf(fy - half_sh) - (win_h_f / 2.0f - win_r);
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
      }
    }
  }

  // 2. Update Frame Cache
  int full_fw = win->w;
  int full_fh = title_h;
  int fw = (int)((float)full_fw * scale);
  int fh = (int)((float)full_fh * scale);
  if (fw < 1 && full_fw > 0) fw = 1;
  if (fh < 1 && full_fh > 0) fh = 1;

  if (!win->frame_cache || win->frame_cache_w != fw || win->frame_cache_h != fh) {
    if (win->frame_cache) free(win->frame_cache);
    win->frame_cache = (uint32_t *)malloc((size_t)fw * (size_t)fh * 4);
    win->frame_cache_w = fw;
    win->frame_cache_h = fh;
  }

  const char *dark_val = get_w1_global("~~main/dark");
  int is_dark = (strcmp(dark_val, "true") == 0);

  uint32_t theme;
  if (is_dark) {
    theme = (win == &g_windows[g_active_window_index]) ? 0xFF1E1E1E : 0xFF333333;
  } else {
    theme = (win == &g_windows[g_active_window_index]) ? 0xFFF5F5F5 : 0xFFE0E0E0;
  }
  for (int i = 0; i < fw * fh; i++) win->frame_cache[i] = theme;

  // Render Title Bar content into frame_cache
  if (fh > 0) {
    layer_t frame_l;
    frame_l.buffer = win->frame_cache;
    frame_l.width = fw;
    frame_l.height = fh;

    char header_text[128];
    int action_count = 0;
    int has_header = 0;
    if (win->is_warp1) {
      if (win->warp1_ctx) has_header = warp1_context_get_header_info(win->warp1_ctx, header_text, sizeof(header_text), &action_count);
    } else {
      if (win->warp_ctx) has_header = warp_context_get_header_info(win->warp_ctx, header_text, sizeof(header_text), &action_count);
    }

    if (has_header) {
      layer_draw_ttf(&frame_l, (int)(70.0f * scale), (int)(12.0f * scale), header_text, 16.0f * scale, is_dark ? 0xFFEEEEEE : 0xFF333333);
      int ax = win->w - 12;
      for (int j = 0; j < action_count; j++) {
        char act_text[64];
        if (win->is_warp1) warp1_context_get_header_action_info(win->warp1_ctx, j, act_text, sizeof(act_text));
        else warp_context_get_header_action_info(win->warp_ctx, j, act_text, sizeof(act_text));
        int text_w = strlen(act_text) * 9; 
        int btn_w = text_w + 24;
        int btn_h = 26; 
        ax -= btn_w;

        // Draw button background
        int bx = (int)((float)ax * scale);
        int by = (int)(7.0f * scale);
        int bw = (int)((float)btn_w * scale);
        int bh = (int)((float)btn_h * scale);
        for (int dy = 0; dy < bh; dy++) {
          for (int dx = 0; dx < bw; dx++) {
             frame_l.buffer[(by + dy) * fw + (bx + dx)] = is_dark ? 0xFF444444 : 0xFFFFFFFF;
          }
        }
        layer_draw_ttf(&frame_l, bx + (int)(12.0f * scale), by + (int)(7.0f * scale), act_text, 14.0f * scale, is_dark ? 0xFFEEEEEE : 0xFF000000);
        ax -= 10;
      }
    } else {
      layer_draw_ttf(&frame_l, (int)(70.0f * scale), (int)(12.0f * scale), win->title, 16.0f * scale, is_dark ? 0xFFEEEEEE : 0xFF333333);
    }

    // Control circles with Anti-Aliasing
    float btn_r = 7.0f;
    int btn_y = 20;
    uint32_t colors[] = {0xFFFF2836, 0xFF2ECC46};
    int centers_x[] = {20, 44};
    for (int k = 0; k < 2; k++) {
      float cx = (float)centers_x[k] * scale;
      float cy = (float)btn_y * scale;
      float cr = btn_r * scale;
      int i_r = (int)cr + 2; 

      for (int dy = -i_r; dy <= i_r; dy++) {
        for (int dx = -i_r; dx <= i_r; dx++) {
          int px = (int)cx + dx;
          int py = (int)cy + dy;
          if (px >= 0 && px < fw && py >= 0 && py < fh) {
            float dist = sqrtf((float)(dx*dx + dy*dy));
            float alpha_f = 0.5f - (dist - cr);
            if (alpha_f < 0.0f) alpha_f = 0.0f;
            else if (alpha_f > 1.0f) alpha_f = 1.0f;

            if (alpha_f > 0.0f) {
              frame_l.buffer[py * fw + px] = blend_colors(frame_l.buffer[py * fw + px], colors[k], (uint8_t)(alpha_f * 255));
            }
          }
        }
      }
    }
  }

  // 3. Update Window Mask Cache
  int full_mw = win->w;
  int full_mh = win->h + title_h;
  int mw = (int)((float)full_mw * scale);
  int mh = (int)((float)full_mh * scale);
  if (mw < 1 && full_mw > 0) mw = 1;
  if (mh < 1 && full_mh > 0) mh = 1;

  // We use buffer_w as a proxy for scaled width and a temporary check for total height
  if (!win->window_mask || win->buffer_w != mw || win->shadow_cache_h != sh) { // sh is a safe proxy for scale change
    if (win->window_mask) free(win->window_mask);
    win->window_mask = (uint8_t *)malloc((size_t)mw * (size_t)mh);

    float rw = (float)full_mw, rh = (float)full_mh;
    float r = 32.0f; // Corner radius
    for (int y = 0; y < mh; y++) {
      float fy = (float)y / scale + 0.5f; 
      for (int x = 0; x < mw; x++) {
        float fx = (float)x / scale + 0.5f;
        float dx = fabsf(fx - rw/2.0f) - (rw/2.0f - r);
        float dy = fabsf(fy - rh/2.0f) - (rh/2.0f - r);

        float dist;
        if (dx > 0 && dy > 0) {
          dist = sqrtf(sqrtf(dx*dx*dx*dx + dy*dy*dy*dy)) - r;
        } else {
          dist = (dx > dy ? dx : dy) - r;
        }

        float alpha_f = 0.5f - dist;
        if (alpha_f < 0.0f) alpha_f = 0.0f;
        else if (alpha_f > 1.0f) alpha_f = 1.0f;
        win->window_mask[y * mw + x] = (uint8_t)(alpha_f * 255.0f);
      }
    }
  }
}

static void window_redraw(window_t *win) {
  if (!win->warp_ctx && !win->warp1_ctx) return;

  int is_active = (win == &g_windows[g_active_window_index]);
  float target_scale = is_active ? 1.0f : 0.5f; // 4 pixels for 1 color (2x2)

  
  // If resolution scale changed, force update
  if (win->render_scale != target_scale) {
    win->is_dirty = 1;
    win->render_scale = target_scale;
    if (target_scale > 0.5f) { // If becoming active
        if (win->unified_buffer) { free(win->unified_buffer); win->unified_buffer = NULL; }
    }
    window_update_caches(win);
  }

  // Check if update is needed (engine_dirty flag)
  int needs_update = 0;
  if (win->is_warp1) {
    needs_update = warp1_context_is_dirty(win->warp1_ctx) || (win->rgba_buffer == NULL);
  } else {
    needs_update = warp_context_is_dirty(win->warp_ctx) || (win->rgba_buffer == NULL);
  }
  
  if (needs_update) {
    strncpy(g_hud_status, "EngineUpdate", 63);
    if (win->is_warp1) {
      warp1_context_update(win->warp1_ctx, win->w, win->h);
      warp1_context_clear_dirty(win->warp1_ctx);
    } else {
      warp_context_update(win->warp_ctx, win->w, win->h);
      warp_context_clear_dirty(win->warp_ctx);
    }
  } else {
    strncpy(g_hud_status, "Cached", 63);
  }

  strncpy(g_hud_status, "SVGGen", 63);
  const char *svg = win->is_warp1 ? warp1_context_get_svg(win->warp1_ctx) : warp_context_get_svg(win->warp_ctx);

  // Check if SVG string changed
  int svg_changed = 1;
  if (win->last_svg_str && strcmp(win->last_svg_str, svg) == 0 && win->rgba_buffer && !win->is_dirty) {
    svg_changed = 0;
  }

  if (svg_changed) {
    strncpy(g_hud_status, "NSVGParse", 63);
    NSVGimage *img = nsvgParse((char*)svg, "px", 96.0f);
    if (!img) {
      strncpy(g_hud_status, "ParseErr", 63);
      return;
    }

    // Content height is determined by the SVG itself
    int content_h = (int)img->height;
    if (content_h < win->h) content_h = win->h;

    int scaled_w = (int)((float)win->w * target_scale);
    int scaled_h = (int)((float)content_h * target_scale);
    if (scaled_w < 1) scaled_w = 1;
    if (scaled_h < 1) scaled_h = 1;

    // Allocate/Resize SVG raster cache
    if (!win->raster_cache || win->raster_cache_w != scaled_w || win->raster_cache_h != scaled_h) {
      if (win->raster_cache) free(win->raster_cache);
      win->raster_cache = (uint32_t *)malloc((size_t)scaled_w * (size_t)scaled_h * 4);
      win->raster_cache_w = scaled_w;
      win->raster_cache_h = scaled_h;
    }

    // Rasterize SVG into cache
    if (win->raster_cache) {
      strncpy(g_hud_status, "ClearCache", 63);
      for (int i = 0; i < scaled_w * scaled_h; i++) win->raster_cache[i] = 0xFFFFFFFF;

      strncpy(g_hud_status, "NSVGRast", 63);
      if (!g_svg_rast) g_svg_rast = nsvgCreateRasterizer();
      nsvgRasterize(g_svg_rast, img, 0, 0, target_scale, (unsigned char*)win->raster_cache, scaled_w, scaled_h, scaled_w * 4);

      strncpy(g_hud_status, "RBSwap", 63);
      unsigned char *p = (unsigned char*)win->raster_cache;
      for (int i = 0; i < scaled_w * scaled_h; i++) {
        unsigned char r = p[0], b = p[2];
        p[0] = b; p[2] = r; p += 4;
      }
    }

    // Update SVG string cache
    if (win->last_svg_str) free(win->last_svg_str);
    size_t svg_len = strlen(svg);
    win->last_svg_str = (char*)malloc(svg_len + 1);
    if (win->last_svg_str) memcpy(win->last_svg_str, svg, svg_len + 1);

    nsvgDelete(img);
  }

  // Prepare RGBA buffer (copy from raster cache and draw text)
  if (win->raster_cache) {
    if (!win->rgba_buffer || win->buffer_w != win->raster_cache_w || win->buffer_h != win->raster_cache_h) {
      if (win->rgba_buffer) free(win->rgba_buffer);
      win->rgba_buffer = (unsigned char *)malloc((size_t)win->raster_cache_w * (size_t)win->raster_cache_h * 4);
      win->buffer_w = win->raster_cache_w;
      win->buffer_h = win->raster_cache_h;
      win->is_dirty = 1; // Force redraw after allocation
    }

    // Only composite text if SVG changed or forced redraw
    if (svg_changed || win->is_dirty) {
      memcpy(win->rgba_buffer, win->raster_cache, (size_t)win->buffer_w * (size_t)win->buffer_h * 4);

      strncpy(g_hud_status, "TxtDraw", 63);
      layer_t temp_layer;
      temp_layer.buffer = (uint32_t*)win->rgba_buffer;
      temp_layer.width = win->buffer_w;
      temp_layer.height = win->buffer_h;
      if (win->is_warp1) {
        warp1_context_draw_texts(win->warp1_ctx, &temp_layer, 0, 0, win->render_scale); 
      } else {
        warp_context_draw_texts(win->warp_ctx, &temp_layer, 0, 0, win->render_scale);
      }
    }
  }

  if (win->render_scale <= 0.5f) {
      window_bake(win);
  }

  win->is_dirty = 0;
  win->is_calculating = 0;
  strncpy(g_hud_status, "Idle", 63);
}

static void add_window(const char *title, int x, int y, int w, int h, int is_warp1) {
  if (g_window_count >= MAX_WINDOWS) return;

  void *dynamic_ptr = NULL;
  const char *buf_to_use = NULL;
  
  if (strstr(title, "Terminal") || strstr(title, "terminal")) {
    if (g_terminal_mod_found) buf_to_use = g_terminal_warp_ptr;
  } else if (strstr(title, "Menubar") || strstr(title, "menubar")) {
    if (g_menubar_mod_found) buf_to_use = g_menubar_warp_ptr;
  }

  if (!buf_to_use) {
    for (uint32_t i = 0; i < g_warp_module_count; i++) {
      if (strcasecmp(g_warp_modules[i].name, title) == 0 || strstr(g_warp_modules[i].name, title)) {
        if (g_warp_modules[i].start != 0) {
            buf_to_use = (const char *)(uintptr_t)g_warp_modules[i].start;
        } else {
            // ストレージからオンデマンド読み込み
            uint32_t sz = 0;
            dynamic_ptr = fs_read_file(g_warp_modules[i].name, &sz);
            buf_to_use = (const char *)dynamic_ptr;
        }
        break;
      }
    }
  }

  if (!buf_to_use) return;

  window_t *win = &g_windows[g_window_count++];
  win->x = x; win->y = y; win->w = w; win->h = h;
  win->scroll_x = 0; win->scroll_y = 0;
  win->target_scroll_x = 0; win->target_scroll_y = 0;
  strncpy(win->title, title, 63);
  win->is_warp1 = is_warp1;
  win->dynamic_file_ptr = dynamic_ptr; // トラック

  if (is_warp1) {
    win->warp1_ctx = warp1_context_create(buf_to_use);
    win->warp_ctx = NULL;
  } else {
    win->warp_ctx = warp_context_create(buf_to_use);
    win->warp1_ctx = NULL;
  }

  win->rgba_buffer = NULL;
  win->buffer_w = 0;
  win->buffer_h = 0;
  win->last_svg_str = NULL;
  win->raster_cache = NULL;
  win->raster_cache_w = 0;
  win->raster_cache_h = 0;
  win->shadow_cache = NULL;
  win->frame_cache = NULL;
  win->window_mask = NULL;
  win->unified_buffer = NULL;
  win->unified_w = 0; win->unified_h = 0;
  win->is_dirty = 1;
  win->is_maximized = 0;
  win->is_dragging = 0;
  win->is_resizing = 0;
  win->resize_w = w;
  win->resize_h = h;
  win->fade_alpha = 0.0f;
  win->is_calculating = 0;
  win->render_scale = 1.0f;
  win->no_decoration = 0;
  win->is_menubar = 0;

  if (strstr(title, "Menubar") || strstr(title, "menubar")) {
    win->is_menubar = 1;
    win->no_decoration = 1;
    win->x = 0; win->y = 0; win->w = SCREEN_WIDTH; win->h = 32;
    win->is_warp1 = 1;
  }

  g_active_window_index = g_window_count - 1;
  window_set_all_dirty();
  window_update_caches(win);
}

static void close_active_window() {
  if (g_active_window_index < 0) return;
  window_t *win = &g_windows[g_active_window_index];
  if (win->warp_ctx) warp_context_destroy(win->warp_ctx);
  if (win->warp1_ctx) warp1_context_destroy(win->warp1_ctx);
  if (win->rgba_buffer) free(win->rgba_buffer);
  if (win->last_svg_str) free(win->last_svg_str);
  if (win->raster_cache) free(win->raster_cache);
  if (win->shadow_cache) free(win->shadow_cache);
  if (win->frame_cache) free(win->frame_cache);
  if (win->window_mask) free(win->window_mask);
  if (win->dynamic_file_ptr) free(win->dynamic_file_ptr);
  if (win->unified_buffer) free(win->unified_buffer);
  
  for (int i = g_active_window_index; i < g_window_count - 1; i++) {
    g_windows[i] = g_windows[i+1];
  }
  g_window_count--;
  g_active_window_index = g_window_count - 1;
  window_set_all_dirty();
}

static void draw_wallpaper(layer_t *layer) {
  if (g_svg_ready) {
    memcpy(layer->buffer, svg_base_buf, sizeof(uint32_t) * layer->width * layer->height);
  } else {
    layer_fill(layer, 0x00000000); // Transparent fallback
  }
}

static void redraw_warp_svg(layer_t *layer) {
  if (!g_svg_dirty) return;
  sync_all_window_themes();
  draw_wallpaper(layer);

  const char *dark_val = get_w1_global("~~main/dark");
  int is_dark = (strcmp(dark_val, "true") == 0);

  for (int i = 0; i < g_window_count; i++) {
    window_t *win = &g_windows[i];
    if (win->is_dirty && !win->is_resizing) window_redraw(win);

    // If we have a unified (baked) buffer, draw it directly and skip individual caches
    if (win->unified_buffer) {
        int title_h = win->no_decoration ? 0 : 40;
        int shadow_size = win->no_decoration ? 0 : 48;
        float scale = win->render_scale;
        
        int sx_start = win->x - shadow_size;
        int sy_start = win->y - title_h - shadow_size + 8;
        
        int full_sw = win->w + shadow_size * 2;
        int full_sh = win->h + title_h + shadow_size * 2;

        int y0 = (sy_start < 0) ? -sy_start : 0;
        int y1 = (sy_start + full_sh > layer->height) ? layer->height - sy_start : full_sh;
        int x0 = (sx_start < 0) ? -sx_start : 0;
        int x1 = (sx_start + full_sw > layer->width) ? layer->width - sx_start : full_sw;

        for (int dy = y0; dy < y1; dy++) {
            int py = sy_start + dy;
            uint32_t *dst_line = &layer->buffer[py * layer->width];
            int scaled_dy = (int)((float)dy * scale);
            if (scaled_dy >= win->unified_h) scaled_dy = win->unified_h - 1;
            uint32_t *src_line = &win->unified_buffer[scaled_dy * win->unified_w];
            
            for (int dx = x0; dx < x1; dx++) {
                int px = sx_start + dx;
                int scaled_dx = (int)((float)dx * scale);
                if (scaled_dx >= win->unified_w) scaled_dx = win->unified_w - 1;
                uint32_t color = src_line[scaled_dx];
                uint8_t alpha = (uint8_t)(color >> 24);
                if (alpha == 0) continue;
                if (alpha == 255) dst_line[px] = color;
                else dst_line[px] = blend_colors(dst_line[px], color, alpha);
            }
        }
        continue; // Skip individual blitting
    }
    
    if (win->rgba_buffer && win->shadow_cache && win->frame_cache) {
      int title_h = win->no_decoration ? 0 : 40;
      int shadow_size = win->no_decoration ? 0 : 48;
      float scale = win->render_scale;
      
      // 1. Draw Window Shadow from Cache
      if (!win->is_maximized && !win->no_decoration) {
        int full_sw = win->w + shadow_size * 2;
        int full_sh = win->h + title_h + shadow_size * 2;
        int sw = win->shadow_cache_w;
        int sx_start = win->x - shadow_size;
        int sy_start = win->y - title_h - shadow_size + 8; // Offset Y by 8
        
        int y0 = (sy_start < 0) ? -sy_start : 0;
        int y1 = (sy_start + full_sh > layer->height) ? layer->height - sy_start : full_sh;
        int x0 = (sx_start < 0) ? -sx_start : 0;
        int x1 = (sx_start + full_sw > layer->width) ? layer->width - sx_start : full_sw;

        for (int dy = y0; dy < y1; dy++) {
          int py = sy_start + dy;
          uint32_t *dst_line = &layer->buffer[py * layer->width];
          int scaled_dy = (int)((float)dy * scale);
          if (scaled_dy >= win->shadow_cache_h) scaled_dy = win->shadow_cache_h - 1;
          uint8_t *src_mask = &win->shadow_cache[scaled_dy * sw];
          for (int dx = x0; dx < x1; dx++) {
            int scaled_dx = (int)((float)dx * scale);
            if (scaled_dx >= sw) scaled_dx = sw - 1;
            uint8_t alpha = src_mask[scaled_dx];
            if (alpha == 0) continue;
            int px = sx_start + dx;
            uint32_t bg = dst_line[px];
            uint32_t a_bg = bg >> 24;
            uint32_t inv_alpha = 255 - alpha;
            uint32_t rb = (bg & 0xFF00FFu) * inv_alpha >> 8;
            uint32_t g = ((bg >> 8) & 0xFF) * inv_alpha >> 8;
            uint32_t a_out = alpha + (a_bg * inv_alpha >> 8);
            dst_line[px] = (a_out << 24) | (rb & 0xFF00FFu) | (g << 8);
          }
        }
      }

      // 2. Draw Title Bar from Cache
      if (!win->no_decoration) {
        uint32_t *src_frame = win->frame_cache;
        int ty0 = (win->y - title_h < 0) ? -(win->y - title_h) : 0;
        int ty1 = (win->y < layer->height) ? title_h : (layer->height - (win->y - title_h));
        int tx0 = (win->x < 0) ? -win->x : 0;
        int tx1 = (win->x + win->w > layer->width) ? (layer->width - win->x) : win->w;

        int mw = (int)((float)win->w * scale);
        if (mw < 1 && win->w > 0) mw = 1;

        for (int dy = ty0; dy < ty1; dy++) {
          int py = win->y - title_h + dy;
          int scaled_dy = (int)((float)dy * scale);
          if (scaled_dy >= win->frame_cache_h) scaled_dy = win->frame_cache_h - 1;
          uint32_t *dst_line = &layer->buffer[py * layer->width];
          uint32_t *src_line = &src_frame[scaled_dy * win->frame_cache_w];
          uint8_t *mask_line = &win->window_mask[scaled_dy * mw];
          for (int dx = tx0; dx < tx1; dx++) {
            int px = win->x + dx;
            int scaled_dx = (int)((float)dx * scale);
            if (scaled_dx >= win->frame_cache_w) scaled_dx = win->frame_cache_w - 1;
            uint32_t color = src_line[scaled_dx];
            uint8_t alpha = win->is_maximized ? 255 : mask_line[scaled_dx];
            dst_line[px] = blend_colors(dst_line[px], color, alpha);
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
        uint32_t *dst_line = &layer->buffer[py * layer->width];
        int frozen_w = win->is_resizing ? win->resize_w : win->w;
        int frozen_h = win->is_resizing ? win->resize_h : win->h;

        int sy_raw = dy - sy_int;
        int scaled_mw = (int)((float)win->w * scale);
        if (scaled_mw < 1 && win->w > 0) scaled_mw = 1;

        if (sy_raw < 0 || sy_raw >= win->h || (win->is_resizing && (dy >= frozen_h || sy_raw >= frozen_h))) {
          int full_mh = win->h + title_h;
          int mh = (int)((float)full_mh * scale);
          int scaled_mask_y = (int)((float)(dy + title_h) * scale);
          if (scaled_mask_y >= mh) scaled_mask_y = mh - 1;
          uint8_t *mask_line = &win->window_mask[scaled_mask_y * scaled_mw];
          for (int dx = cx0; dx < cx1; dx++) {
            int scaled_dx = (int)((float)dx * scale);
            if (scaled_dx >= scaled_mw) scaled_dx = scaled_mw - 1;
            uint8_t alpha = (win->is_maximized || win->no_decoration) ? 255 : mask_line[scaled_dx];
            if (alpha == 255) dst_line[win->x + dx] = 0xFFFFFFFF;
            else if (alpha > 0) dst_line[win->x + dx] = blend_colors(dst_line[win->x + dx], 0xFFFFFFFF, alpha);
          }
          continue;
        }

        // Map screen-space y to buffer-space y
        int src_y = (int)((float)sy_raw * scale);
        if (src_y < 0) src_y = 0;
        if (src_y >= win->buffer_h) src_y = win->buffer_h - 1;

        uint32_t *src_content_line = (uint32_t*)&win->rgba_buffer[src_y * win->buffer_w * 4];
        
        int full_mh = win->h + title_h;
        int mh = (int)((float)full_mh * scale);
        int scaled_mask_y = (int)((float)(dy + title_h) * scale);
        if (scaled_mask_y >= mh) scaled_mask_y = mh - 1;
        uint8_t *mask_line = &win->window_mask[scaled_mask_y * scaled_mw];
        
        uint8_t fade_alpha_u8 = (uint8_t)(win->fade_alpha * 255);
        for (int dx = cx0; dx < cx1; dx++) {
          int px = win->x + dx;
          int scaled_dx = (int)((float)dx * scale);
          if (scaled_dx >= scaled_mw) scaled_dx = scaled_mw - 1;
          uint8_t alpha = (win->is_maximized || win->no_decoration) ? 255 : mask_line[scaled_dx];
          
          // Map screen-space x to buffer-space x
          int src_x = scaled_dx;
          if (src_x >= win->buffer_w) src_x = win->buffer_w - 1;

          uint32_t color = (win->is_resizing && dx >= frozen_w) ? 0xFFFFFFFF : src_content_line[src_x];
          if (fade_alpha_u8 > 0) color = blend_colors(color, 0xFFFFFFFF, fade_alpha_u8);
          dst_line[px] = blend_colors(dst_line[px], color, alpha);
        }
      }
      
      // 6. Handle
      if (i == g_active_window_index && !win->no_decoration) {
        int handle_s = 12;
        int hy0 = (win->h - handle_s < cy0) ? cy0 : win->h - handle_s;
        int hy1 = (win->h > cy1) ? cy1 : win->h;
        int hx0 = (win->w - handle_s < cx0) ? cx0 : win->w - handle_s;
        int hx1 = (win->w > cx1) ? cx1 : win->w;
        for (int dy = hy0; dy < hy1; dy++) {
          int py = win->y + dy;
          uint32_t *dst_line = &layer->buffer[py * layer->width];
          for (int dx = hx0; dx < hx1; dx++) dst_line[win->x + dx] = blend_colors(dst_line[win->x + dx], 0xFFCCCCCC, 255);
        }
      }
    }
  }
  g_svg_dirty = 0;
}

static int svg_init_nextgen(layer_t *layer) {
  svg_init(layer, 1); // Load and render wallpaper
  
  // Execute startup commands
  handle_terminal_command("warp terminal.warp");
  handle_terminal_command("warp menubar.warp");
  
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
  handle_terminal_command(cmd);
}

static void hud_update(layer_t *hud, unsigned int cpu_percent,
                       unsigned int mem_total_kb) {
  if (!g_dev_show_hud) {
    hud->active = 0;
    return;
  }
  hud->active = 1;
  layer_fill(hud, 0xFF000000);

  char line1[64], line2[64], line3[64], line4[64], line5[64];

  // 1行目: Build Info
  char *p = line1;
  const char *title = "BaramOS Build ";
  while (*title) *p++ = *title++;
  p = append_uint(p, BUILD_NUMBER);
  *p = '\0';

  // 2行目: CPU/MEM (Total Used / Total Available)
  uint32_t kernel_size = get_kernel_image_size();
  uint32_t static_size = get_static_memory_usage();
  uint32_t heap_used = get_used_memory();
  uint32_t modules_size = 0;
  for (uint32_t i = 0; i < g_warp_module_count; i++) modules_size += g_warp_modules[i].size;

  uint32_t total_used_kb = (kernel_size + static_size + heap_used + modules_size) / 1024;
  
  p = line2;
  *p++ = 'C'; *p++ = 'P'; *p++ = 'U'; *p++ = ':'; *p++ = ' ';
  p = append_uint(p, cpu_percent);
  *p++ = '%'; *p++ = ' '; *p++ = 'R'; *p++ = 'A'; *p++ = 'M'; *p++ = ':'; *p++ = ' ';
  p = append_uint(p, total_used_kb);
  *p++ = '/'; p = append_uint(p, mem_total_kb);
  *p++ = 'K'; *p++ = 'B'; *p = '\0';

  // 3行目: Memory Breakdown (K:Kernel, S:Static, H:Heap, M:Mods)
  p = line3;
  *p++ = 'K'; *p++ = ':'; p = append_uint(p, kernel_size / 1024);
  *p++ = ' '; *p++ = 'S'; *p++ = ':'; p = append_uint(p, static_size / 1024);
  *p++ = ' '; *p++ = 'H'; *p++ = ':'; p = append_uint(p, heap_used / 1024);
  *p++ = ' '; *p++ = 'M'; *p++ = ':'; p = append_uint(p, modules_size / 1024);
  *p = '\0';

  // 4行目: Engine Status
  p = line4;
  *p++ = 'M'; *p++ = ':';
  const char *m_name = (current_os_mode == OS_MODE_CLASSIC) ? "CLS" : "WDP";
  while (*m_name) *p++ = *m_name++;
  *p++ = ' '; *p++ = 'S'; *p++ = ':';
  const char *s_status = g_last_svg_parse_status;
  while (*s_status) *p++ = *s_status++;
  *p = '\0';

  // 5行目: Status & Storage
  p = line5;
  const char *w_label = "S: ";
  while (*w_label) *p++ = *w_label++;
  const char *w_status = g_hud_status;
  while (*w_status) *p++ = *w_status++;
  
  *p++ = ' '; *p++ = 'D'; *p++ = ':';
  uint32_t disk_used = 0, disk_total = 0;
  fs_get_usage(&disk_used, &disk_total);
  p = append_uint(p, disk_used / 1024);
  *p++ = '/';
  p = append_uint(p, disk_total / 1024);
  *p++ = 'K';
  *p = '\0';

  // 6行目: Mouse
  char line6[64];
  p = line6;
  *p++ = 'M'; *p++ = ':';
  p = append_uint(p, (unsigned int)mouse_x);
  *p++ = ',';
  p = append_uint(p, (unsigned int)mouse_y);
  *p = '\0';

  // 5行目以降: System Log (Warp)
  const char *sys_log = get_w1_global("--warpSystemLog");
  char log_lines[20][64];
  for(int i=0; i<20; i++) log_lines[i][0] = '\0';
  
  int log_count = 0;
  if (sys_log && sys_log[0]) {
    const char *ls = sys_log;
    // Skip old logs if too many (keep last 10)
    int total_newlines = 0;
    while (*ls) { if (*ls == '\n') total_newlines++; ls++; }
    ls = sys_log;
    if (total_newlines > 10) {
      int skip = total_newlines - 10;
      while (skip > 0 && *ls) { if (*ls == '\n') skip--; ls++; }
    }
    
    while (*ls && log_count < 10) {
      char *ld = log_lines[log_count];
      int count = 0;
      while (*ls && *ls != '\n' && count < 63) {
        *ld++ = *ls++;
        count++;
      }
      *ld = '\0';
      if (*ls == '\n') ls++;
      log_count++;
    }
  }

  // 高さの計算 (基本6行(48px) + ログ行数 * 8px + 余白)
  int new_h = 48 + log_count * 8 + 8;
  if (new_h > HUD_H_MAX) new_h = HUD_H_MAX;
  if (new_h < 64) new_h = 64;
  
  // HUDの位置を調整（下から上に伸びる）
  hud->height = new_h;
  hud->y = SCREEN_HEIGHT - (new_h + 10);
  g_hud_current_h = new_h;

  layer_draw_string(hud, 2, 0, line1, 0xFFFFFFFF, TRANSPARENT_COLOR);
  layer_draw_string(hud, 2, 8, line2, 0xFFFFFFFF, TRANSPARENT_COLOR);
  layer_draw_string(hud, 2, 16, line3, 0xFF00FF00, TRANSPARENT_COLOR);
  layer_draw_string(hud, 2, 24, line4, 0xFFFFFF00, TRANSPARENT_COLOR);
  layer_draw_string(hud, 2, 32, line5, 0xFFFFFFFF, TRANSPARENT_COLOR);
  layer_draw_string(hud, 2, 40, line6, 0xFFFFFFFF, TRANSPARENT_COLOR);
  
  for (int i = 0; i < log_count; i++) {
    layer_draw_string(hud, 2, 48 + i * 8, log_lines[i], 0xFF00FFFF, TRANSPARENT_COLOR);
  }
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

  uint32_t inv_alpha = 255 - alpha;
  uint32_t rb_bg = bg & 0xFF00FFu;
  uint32_t g_bg = (bg >> 8) & 0xFF;
  uint32_t rb_fg = fg & 0xFF00FFu;
  uint32_t g_fg = (fg >> 8) & 0xFF;

  uint32_t rb_out = ((rb_fg * alpha) + (rb_bg * inv_alpha)) >> 8;
  uint32_t g_out = ((g_fg * alpha) + (g_bg * inv_alpha)) >> 8;

  return 0xFF000000u | (rb_out & 0xFF00FFu) | ((g_out & 0xFF) << 8);
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
  mbi_ptr = mbi;
  if (mbi)
    g_mbi_flags = mbi->flags;

  // 動的ヒープの初期化
  uintptr_t heap_start = (uintptr_t)_kernel_end;
  if (mbi->flags & 0x8 && mbi->mods_count > 0) {
      multiboot_module_t *mods = (multiboot_module_t *)(uintptr_t)mbi->mods_addr;
      for (uint32_t i = 0; i < mbi->mods_count; i++) {
          if (mods[i].mod_end > heap_start) {
              heap_start = mods[i].mod_end;
          }
      }
  }
  // 4KBアライメント
  heap_start = (heap_start + 4095) & ~4095;
  uint32_t mem_total_kb = mbi->mem_upper;
  uintptr_t heap_end = 0x100000 + mem_total_kb * 1024;
  if (heap_end > heap_start) {
      heap_init((void*)heap_start, heap_end - heap_start);
  }

  // SVG描画などの初期化より前に、まず赤画面を出す
  for (int i = 0; i < 30; ++i) { // 約0.3秒間、赤で塗りつぶし続ける
    fill_framebuffer_red_early(mbi);
    for (volatile int j = 0; j < 1000000; ++j) {
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
  desktop.buffer = main_screen_buf;
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
  svg_buf = (uint32_t *)malloc(SVG_WIDTH * SVG_HEIGHT * 4);
  layer_t svg_layer;
  svg_layer.buffer = svg_buf;
  svg_layer.x = 0;
  svg_layer.y = 0;
  svg_layer.width = SVG_WIDTH;
  svg_layer.height = SVG_HEIGHT;
  svg_layer.transparent = 0;
  svg_layer.active = 1;
  svg_layer.dynamic = 0;
  if (svg_buf) svg_init(&svg_layer, 0);
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
  hud_layer.y = SCREEN_HEIGHT - (g_hud_current_h + 10);
  hud_layer.width = HUD_W;
  hud_layer.height = g_hud_current_h;
  hud_layer.transparent = 0;
  hud_layer.active = g_dev_show_hud;
  hud_layer.dynamic = 1;
  layer_fill(&hud_layer, 0xFF000000);

  // 5. 次世代UI SVGレイヤー
  layer_t nextgen_ui_layer;
  nextgen_ui_layer.buffer = main_screen_buf;
  nextgen_ui_layer.x = 0;
  nextgen_ui_layer.y = 0;
  nextgen_ui_layer.width = SCREEN_WIDTH;
  nextgen_ui_layer.height = SCREEN_HEIGHT;
  nextgen_ui_layer.transparent = TRANSPARENT_COLOR;
  nextgen_ui_layer.active = 0;
  nextgen_ui_layer.dynamic = 1;
  {
    for (int i = 0; i < SCREEN_WIDTH * SCREEN_HEIGHT; i++)
      main_screen_buf[i] = TRANSPARENT_COLOR;
  }
  register_layer(&nextgen_ui_layer);
  register_layer(&hud_layer); // HUDを上に

  // 6. 文字レイヤー
  layer_t text_layer;
  text_layer.buffer = text_layer_buf;
  text_layer.x = 0;
  text_layer.y = 0;
  text_layer.width = TEXT_LAYER_W;
  text_layer.height = TEXT_LAYER_H;
  text_layer.transparent = TRANSPARENT_COLOR;
  text_layer.active = 1;
  text_layer.dynamic = 1;
  {
    for (int i = 0; i < TEXT_LAYER_W * TEXT_LAYER_H; i++)
      text_layer_buf[i] = TRANSPARENT_COLOR;
  }
  // 初回描画を確実に実行
  screen_mark_static_dirty();
  screen_mark_all_dirty();

  uint32_t last_blink_tick = 0;
  int blink_state = 0;
  uint32_t last_stat_tick = 0;
  uint32_t last_idle_tick = idle_ticks;
  unsigned int cpu_percent = 0;

  int last_hover = -2;
  int last_mouse_x = -1;
  int last_mouse_y = -1;
  uint8_t prev_mouse_buttons = 0;

  uint32_t boot_start_tick = timer_ticks;
  int auto_booted = 0;

  // メインループ (常時60fpsターゲット)
  while (1) {
    if (auto_booted && g_pending_command_count > 0) {
      // Process one command at a time from the head
      char log[256] = "ExecFirstboot: ";
      strlcat(log, g_pending_commands[0], 255);
      set_w1_global("--warpSystemLog", log);
      
      handle_terminal_command(g_pending_commands[0]);
      // Shift queue
      for (int i = 0; i < g_pending_command_count - 1; i++) {
        strncpy(g_pending_commands[i], g_pending_commands[i+1], 255);
      }
      g_pending_command_count--;
      g_svg_dirty = 1;
    }

    if (!auto_booted && g_os_settings_found && current_os_mode == OS_MODE_CLASSIC &&
        (timer_ticks - boot_start_tick > 50)) {
      
      current_os_mode = OS_MODE_WARPDESKTOP;
      g_scroll_x = g_scroll_y = g_target_scroll_x = g_target_scroll_y = 0.0f;
      
      // テーマに合わせて背景色を決定
      const char *dark_val = get_w1_global("~~main/dark");
      uint32_t bg_color = (strcmp(dark_val, "true") == 0) ? 0xFF121212 : 0xFFF5F5F5;
      for (int i = 0; i < SCREEN_WIDTH * SCREEN_HEIGHT; i++) main_screen_buf[i] = bg_color;
      
      extern void screen_mark_static_dirty(void);
      screen_mark_static_dirty();
      
      svg_layer.active = 0;
      if (svg_buf) {
          free(svg_buf);
          svg_buf = NULL;
          svg_layer.buffer = NULL;
      }
      blink_layer.active = 0;
      nextgen_ui_layer.active = 1;
      text_layer.active = 0;
      keybuf_str[0] = '\0';
      g_svg_dirty = 1;
      svg_init_nextgen(&nextgen_ui_layer);
      screen_mark_static_dirty();
      auto_booted = 1;
      set_w1_global("--warpSystemLog", "DesktopReady.");
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

      // Classic モード：トラックパッドのスクロール量に直接追従（1px 単位）
      if (mouse_scroll != 0) {
        g_target_scroll_y += (float)mouse_scroll * 30.0f;
        mouse_scroll = 0;
        // 上限：0, 下限：コンテンツ高さ - 画面高さ
        int content_h = g_svg_full_h;
        int min_scroll = SCREEN_HEIGHT - content_h;
        if (min_scroll > 0) min_scroll = 0; // コンテンツが画面より小さい場合はスクロール不可
        if (g_target_scroll_y > 0.0f) g_target_scroll_y = 0.0f;
        if (g_target_scroll_y < (float)min_scroll) g_target_scroll_y = (float)min_scroll;
      }

      // スクロールアニメーション（1px 単位で補間）
      if (g_target_scroll_y != g_scroll_y) {
        float dy = (g_target_scroll_y - g_scroll_y) * SCROLL_EASE;
        if (fabsf(dy) < 1.0f) {
          g_scroll_y = g_target_scroll_y;
        } else {
          g_scroll_y += dy;
        }
        svg_render_full(&svg_layer);
        memcpy(svg_base_buf, svg_layer.buffer,
               sizeof(uint32_t) * svg_layer.width * svg_layer.height);
        screen_mark_static_dirty();
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
        hud_update(&hud_layer, cpu_percent, mem_total_kb);
        last_stat_tick = timer_ticks;
        last_idle_tick = idle_ticks;
      }

    } else if (current_os_mode == OS_MODE_WARPDESKTOP) {
      // 1. Scroll Handling (Active Window) - トラックパッドの量に直接追従（1px 単位）
      if (mouse_scroll != 0 && g_active_window_index >= 0) {
        window_t *win = &g_windows[g_active_window_index];
        float scroll_amount = (float)mouse_scroll * 30.0f;
        mouse_scroll = 0;

        // コンテキストから現在のターゲットスクロールを取得
        float current_target = win->is_warp1
            ? warp1_context_get_target_scroll_y(win->warp1_ctx)
            : warp_context_get_target_scroll_y(win->warp_ctx);

        // 新しいターゲットスクロールを計算
        float new_target = current_target + scroll_amount;

        // 上限・下限制限
        int content_h = win->is_warp1
            ? warp1_context_get_content_height(win->warp1_ctx)
            : warp_context_get_content_height(win->warp_ctx);
        int min_scroll = win->h - content_h;
        if (min_scroll > 0) min_scroll = 0;
        if (new_target > 0.0f) new_target = 0.0f;
        if (new_target < (float)min_scroll) new_target = (float)min_scroll;

        // コンテキストに設定
        if (win->is_warp1) {
          warp1_context_set_target_scroll_y(win->warp1_ctx, new_target);
        } else {
          warp_context_set_target_scroll_y(win->warp_ctx, new_target);
        }
        win->target_scroll_y = new_target;
      }

      // 2. Window Animation (Smooth Scroll)
      int moved = 0;
      for (int i = 0; i < g_window_count; i++) {
        window_t *win = &g_windows[i];

        // コンテキストからスクロール状態を同期
        if (win->warp1_ctx) {
          win->target_scroll_y = warp1_context_get_target_scroll_y(win->warp1_ctx);
        } else if (win->warp_ctx) {
          win->target_scroll_y = warp_context_get_target_scroll_y(win->warp_ctx);
        }

        // Resize/Calculating Fade
        float fade_speed = 0.02f;
        if (win->is_resizing) {
          if (win->fade_alpha < 1.0f) {
            win->fade_alpha += fade_speed;
            if (win->fade_alpha > 1.0f) win->fade_alpha = 1.0f;
            moved = 1;
            g_svg_dirty = 1;
          }
        } else if (win->is_calculating) {
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
            g_svg_dirty = 1;
          }
        }

        // スクロールアニメーション（1px 単位で補間）
        if (win->target_scroll_y != win->scroll_y) {
          float dy = (win->target_scroll_y - win->scroll_y) * SCROLL_EASE;
          if (fabsf(dy) < 1.0f) {
            win->scroll_y = win->target_scroll_y;
          } else {
            win->scroll_y += dy;
          }
          moved = 1;
        }
      }
      if (moved) g_svg_dirty = 1;

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
            if (!win->is_menubar && hx >= win->x + win->w - 16 && hx < win->x + win->w &&
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
            
            // 3. Content
            if (hx >= win->x && hx < win->x + win->w &&
                hy >= win->y && hy < win->y + win->h) {
              hit_index = i;
              if (win->is_warp1) {
                warp1_context_click(win->warp1_ctx, hx - win->x, hy - win->y - (int)win->scroll_y);
              } else {
                warp_context_click(win->warp_ctx, hx - win->x, hy - win->y - (int)win->scroll_y);
              }
              win->is_dirty = 1;
              break;
            }
          }
          
          if (hit_index >= 0) {
            // Bring hit window to front
            if (hit_index != g_window_count - 1) {
              window_t tmp = g_windows[hit_index];
              for (int j = hit_index; j < g_window_count - 1; j++) g_windows[j] = g_windows[j+1];
              g_windows[g_window_count - 1] = tmp;
              g_active_window_index = g_window_count - 1;
              window_set_all_dirty();
            } else {
              g_active_window_index = g_window_count - 1;
              // No need to set all dirty if already active, but keep g_svg_dirty
              g_svg_dirty = 1;
            }
          } else if (hit_index == -1) {
            // Wallpaper click: do nothing
          }
        } else if (!(curr_btns & 1) && (prev_mouse_buttons & 1)) {
          // Release
          for (int i = 0; i < g_window_count; i++) {
            if (g_windows[i].is_resizing) {
              g_windows[i].is_calculating = 1;
              g_windows[i].is_dirty = 1; // Trigger final layout
            }
            g_windows[i].is_dragging = 0;
            g_windows[i].is_resizing = 0;
          }
        }
        prev_mouse_buttons = curr_btns;
      }

      // Drag/Resize movement or pointcheck mouse update
      if (g_active_window_index >= 0 && (mouse_dx != 0 || mouse_dy != 0)) {
        window_t *awin = &g_windows[g_active_window_index];
        int hx = mouse_x + MOUSE_HOTSPOT_X;
        int hy = mouse_y + MOUSE_HOTSPOT_Y;
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
        if (awin->is_dragging) {
          awin->x += mouse_dx; awin->y += mouse_dy;
          g_svg_dirty = 1;
        } else if (awin->is_resizing) {
          awin->w += mouse_dx; awin->h += mouse_dy;
          if (awin->w < 100) awin->w = 100;
          if (awin->h < 64) awin->h = 64;
          // Don't set is_dirty here to keep SVG content frozen
          window_update_caches(awin); 
          g_svg_dirty = 1;
        }
      }

      if (keybuf_len > 0) {
        for (int i = 0; i < keybuf_len; i++) {
          char c = (char)keybuf[i];

          // Warp1 ウィンドウへのキー入力転送
          if (g_active_window_index >= 0) {
            window_t *awin = &g_windows[g_active_window_index];
            if (awin->is_warp1 && awin->warp1_ctx) {
              // TODO: ウィンドウがキー入力を受けるべき状態か判定が必要かもしれない
              warp1_context_key_input(awin->warp1_ctx, c);
              awin->is_dirty = 1; // 追記: ウィンドウ自体の再描画が必要
              g_svg_dirty = 1;
              continue; // Warp1 が処理した場合は以後の処理（コマンドライン等）をスキップ
            }
          }

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
        hud_update(&hud_layer, cpu_percent, mem_total_kb);
        last_stat_tick = timer_ticks;
        last_idle_tick = idle_ticks;
      }
    }

    // 常時再描画
    cpu_idle = 0;
    screen_refresh();
  }
}
