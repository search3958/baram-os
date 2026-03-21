// Kernel shim - provides C implementations for functions called from Rust
#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include "drivers.h"
#include "ui/warp_engine.h"
#include "ui/warp1_engine.h"
#include "font/fonts.h"
#include "storage.h"
#include "fs.h"

// Memory allocator (must come before stb_truetype)
extern char _kernel_end[];
static char *heap_ptr = NULL;
static size_t heap_size = 0;
static int heap_init_done = 0;

typedef struct block_header {
    size_t size;
    int used;
} block_header_t;

#define BLOCK_HDR_SIZE (sizeof(block_header_t))

void heap_init(void *start, size_t size) {
    if (heap_init_done) return;
    heap_ptr = (char *)start;
    heap_size = size;
    block_header_t *first = (block_header_t *)heap_ptr;
    first->size = heap_size - BLOCK_HDR_SIZE;
    first->used = 0;
    heap_init_done = 1;
}

void *malloc(size_t size) {
    if (!heap_init_done) return NULL;
    if (size == 0) return NULL;
    size = (size + 7) & ~7;
    
    char *p = heap_ptr;
    char *end = heap_ptr + heap_size;
    
    while (p + BLOCK_HDR_SIZE <= end) {
        block_header_t *hdr = (block_header_t *)p;
        if (!hdr->used && hdr->size >= size) {
            size_t remaining = hdr->size - size;
            if (remaining > BLOCK_HDR_SIZE + 8) {
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
    return NULL;
}

void free(void *ptr) {
    if (!ptr) return;
    block_header_t *hdr = (block_header_t *)((char *)ptr - BLOCK_HDR_SIZE);
    hdr->used = 0;
    
    char *p = heap_ptr;
    char *end = heap_ptr + heap_size;
    while (p + BLOCK_HDR_SIZE <= end) {
        block_header_t *cur = (block_header_t *)p;
        char *next_p = p + BLOCK_HDR_SIZE + cur->size;
        if (!cur->used && next_p + BLOCK_HDR_SIZE <= end) {
            block_header_t *next = (block_header_t *)next_p;
            if (!next->used) {
                cur->size += BLOCK_HDR_SIZE + next->size;
                continue;
            }
        }
        p = next_p;
    }
}

void *realloc(void *ptr, size_t size) {
    if (!ptr) return malloc(size);
    if (size == 0) { free(ptr); return NULL; }
    block_header_t *hdr = (block_header_t *)((char *)ptr - BLOCK_HDR_SIZE);
    if (hdr->size >= size) return ptr;
    void *next = malloc(size);
    if (!next) return NULL;
    memcpy(next, ptr, hdr->size < size ? hdr->size : size);
    free(ptr);
    return next;
}

// Simple math functions for stb_truetype (must come before stb_truetype)
static float sqrtf_local(float x);
static float powf_local(float x, float y);
static float cosf_local(float x);
static float sinf_local(float x);
static float acosf_local(float x);
static float fabsf_local(float x);
static float fmodf_local(float x, float y);
static float atan2f_local(float y, float x);
static float atan_approx_local(float z);

static float sqrtf_local(float x) {
    if (x <= 0.0f) return 0.0f;
    float r = x;
    for (int i = 0; i < 10; i++) r = 0.5f * (r + x / r);
    return r;
}

static float powf_local(float x, float y) {
    if (y == 0.5f) return sqrtf_local(x);
    if (y == 1.0f) return x;
    if (y == 2.0f) return x * x;
    return x;
}

static float cosf_local(float x) {
    float x2 = x * x;
    return 1.0f - x2/2.0f + x2*x2/24.0f - x2*x2*x2/720.0f;
}

static float sinf_local(float x) {
    float x2 = x * x;
    return x * (1.0f - x2/6.0f + x2*x2/120.0f);
}

static float acosf_local(float x) {
    return sqrtf_local(1.0f - x * x);
}

static float fabsf_local(float x) {
    return x < 0.0f ? -x : x;
}

static float fmodf_local(float x, float y) {
    if (y == 0.0f) return 0.0f;
    int q = (int)(x / y);
    return x - (float)q * y;
}

// stb_truetype implementation
#define STB_TRUETYPE_IMPLEMENTATION
#define STBTT_malloc(x, u) malloc(x)
#define STBTT_free(x, u) free(x)
#define STBTT_assert(x)
#define STBTT_ifloor(x) ((int)(x))
#define STBTT_iceil(x) ((int)((x) + 1))
#define STBTT_sqrt(x) sqrtf_local(x)
#define STBTT_pow(x, y) powf_local(x, y)
#define STBTT_fmod(x, y) fmodf_local(x, y)
#define STBTT_cos(x) cosf_local(x)
#define STBTT_acos(x) acosf_local(x)
#define STBTT_fabs(x) fabsf_local(x)
#include "font/stb_truetype.h"

// Multiboot module entry
typedef struct {
    uint32_t mod_start;
    uint32_t mod_end;
    uint32_t string;
    uint32_t reserved;
} __attribute__((packed)) multiboot_module_t;

// Memory functions
void *memcpy(void *dest, const void *src, size_t n) {
    unsigned char *d = (unsigned char *)dest;
    const unsigned char *s = (const unsigned char *)src;
    while (n--) *d++ = *s++;
    return dest;
}

void *memset(void *s, int c, size_t n) {
    unsigned char *p = (unsigned char *)s;
    while (n--) *p++ = (unsigned char)c;
    return s;
}

// String functions
size_t strlen(const char *s) {
    size_t n = 0;
    if (!s) return 0;
    while (s[n]) n++;
    return n;
}

int strcmp(const char *a, const char *b) {
    while (*a && (*a == *b)) { a++; b++; }
    return (unsigned char)*a - (unsigned char)*b;
}

int strncmp(const char *a, const char *b, size_t n) {
    for (size_t i = 0; i < n; ++i) {
        unsigned char ca = (unsigned char)a[i];
        unsigned char cb = (unsigned char)b[i];
        if (ca != cb || ca == 0 || cb == 0) return (int)ca - (int)cb;
    }
    return 0;
}

int strcasecmp(const char *a, const char *b) {
    while (*a && (*b)) {
        unsigned char ca = (unsigned char)*a;
        unsigned char cb = (unsigned char)*b;
        if (ca >= 'A' && ca <= 'Z') ca += 'a' - 'A';
        if (cb >= 'A' && cb <= 'Z') cb += 'a' - 'A';
        if (ca != cb) break;
        a++; b++;
    }
    return (unsigned char)*a - (unsigned char)*b;
}

char *strncpy(char *dst, const char *src, size_t n) {
    size_t i = 0;
    for (; i < n && src[i]; ++i) dst[i] = src[i];
    for (; i < n; ++i) dst[i] = '\0';
    return dst;
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

int memcmp(const void *a, const void *b, size_t n) {
    const unsigned char *pa = (const unsigned char *)a;
    const unsigned char *pb = (const unsigned char *)b;
    for (size_t i = 0; i < n; ++i) {
        if (pa[i] != pb[i]) return (int)pa[i] - (int)pb[i];
    }
    return 0;
}

int bcmp(const void *a, const void *b, size_t n) {
    return memcmp(a, b, n);
}

// Font functions
static stbtt_fontinfo g_font;
static int g_font_ready = 0;

int font_init(struct multiboot_info *mbi) {
    if (!mbi) return 0;
    if (!(mbi->flags & 0x8)) return 0;
    if (mbi->mods_count == 0) return 0;
    
    multiboot_module_t *mod = (multiboot_module_t *)(uintptr_t)mbi->mods_addr;
    unsigned char *ttf = (unsigned char *)(uintptr_t)mod->mod_start;
    uint32_t ttf_size = mod->mod_end - mod->mod_start;
    
    if (!stbtt_InitFont(&g_font, ttf, stbtt_GetFontOffsetForIndex(ttf, 0))) {
        return 0;
    }
    g_font_ready = 1;
    return 1;
}

void layer_draw_ttf(layer_t *layer, int px, int py, const char *str, float font_size, uint32_t color) {
    if (!g_font_ready || !str || !layer || !layer->buffer) return;
    
    float scale = stbtt_ScaleForPixelHeight(&g_font, font_size);
    int ascent, descent, line_gap;
    stbtt_GetFontVMetrics(&g_font, &ascent, &descent, &line_gap);
    int baseline = (int)(ascent * scale);
    
    int cx = px;
    const char *p = str;
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
        unsigned char *bitmap = stbtt_GetCodepointBitmap(&g_font, 0, scale, (int)codepoint, &bw, &bh, &bx, &by);
        if (bitmap) {
            for (int dy = 0; dy < bh; dy++) {
                int dpy = py + baseline + by + dy;
                if (dpy < 0 || dpy >= (int)layer->height) continue;
                for (int dx = 0; dx < bw; dx++) {
                    int dpx = cx + bx + dx;
                    if (dpx < 0 || dpx >= (int)layer->width) continue;
                    uint8_t alpha = bitmap[dy * bw + dx];
                    if (alpha == 0) continue;
                    
                    uint32_t bg = layer->buffer[dpy * layer->width + dpx];
                    uint8_t bg_r = (bg >> 16) & 0xFF;
                    uint8_t bg_g = (bg >> 8) & 0xFF;
                    uint8_t bg_b = bg & 0xFF;
                    uint8_t fg_r = (color >> 16) & 0xFF;
                    uint8_t fg_g = (color >> 8) & 0xFF;
                    uint8_t fg_b = color & 0xFF;
                    
                    uint8_t out_r = (uint8_t)((fg_r * alpha + bg_r * (255 - alpha)) / 255);
                    uint8_t out_g = (uint8_t)((fg_g * alpha + bg_g * (255 - alpha)) / 255);
                    uint8_t out_b = (uint8_t)((fg_b * alpha + bg_b * (255 - alpha)) / 255);
                    
                    layer->buffer[dpy * layer->width + dpx] = (0xFFu << 24) | ((uint32_t)out_r << 16) | ((uint32_t)out_g << 8) | (uint32_t)out_b;
                }
            }
            stbtt_FreeBitmap(bitmap, NULL);
        }
        
        int adv, lsb;
        stbtt_GetCodepointHMetrics(&g_font, codepoint, &adv, &lsb);
        cx += (int)(adv * scale);
    }
}

// SVG functions (using nanosvg)
#define NANOSVG_IMPLEMENTATION
#include "nanosvg/nanosvg.h"
#define NANOSVGRAST_IMPLEMENTATION
#include "nanosvg/nanosvgrast.h"

void *kernel_svg_parse(const char *svg) {
    if (!svg) return NULL;
    return nsvgParse((char *)svg, "px", 96.0f);
}

void kernel_svg_delete(void *image) {
    if (image) nsvgDelete((NSVGimage *)image);
}

int kernel_svg_height(void *image) {
    if (!image) return 0;
    return (int)((NSVGimage *)image)->height;
}

void kernel_svg_rasterize(void *image, unsigned char *dst, int w, int h, float scale) {
    if (!image || !dst || w <= 0 || h <= 0) return;
    NSVGrasterizer *rast = nsvgCreateRasterizer();
    if (rast) {
        nsvgRasterize(rast, (NSVGimage *)image, 0, 0, scale, dst, w, h, w * 4);
        nsvgDeleteRasterizer(rast);
    }
}

// State accessors (forward to Rust)
extern int rt_get_dev_pointer_check(void);
extern int rt_get_dev_event_check(void);
extern int rt_get_dev_show_hud(void);
extern float rt_get_scroll_y(void);
extern void rt_set_scroll_y(float y);
extern float rt_get_target_scroll_y(void);
extern void rt_set_target_scroll_y(float y);
extern uint32_t rt_get_timer_ticks(void);
extern void rt_set_timer_ticks(uint32_t ticks);
extern int rt_get_cpu_idle(void);
extern void rt_set_cpu_idle(int idle);
extern uint32_t rt_get_idle_ticks(void);
extern void rt_set_idle_ticks(uint32_t ticks);
extern int rt_get_svg_dirty(void);
extern void rt_set_svg_dirty(int dirty);

const char *get_w1_global(const char *key);
void set_w1_global(const char *key, const char *val);
void set_pending_command(const char *cmd);

// Kernel command functions (forward to Rust)
extern void rust_kernel_handle_terminal_command(const char *cmd);
extern void rust_kernel_parse_os_settings(const char *buf);

void kernel_load_wallpaper_from_settings(const char *wp_name);
void kernel_mark_os_settings_ready(void);
void kernel_add_window_for_command(const char *title, int x, int y, int w, int h, int is_warp1);
void kernel_close_active_window_for_command(void);
const char *kernel_find_warp_module(const char *name, int *out_is_warp1);
void kernel_list_warp_modules(char *out_buf, int out_buf_len);
void kernel_storage_sync_command(void);
void kernel_storage_ls_command(void);

// Main iteration functions
void kernel_main_iteration_classic(void);
void kernel_main_iteration_desktop(void);
void kernel_finish_iteration(void);
int kernel_process_pending_commands(void);
int kernel_try_autoboot_to_desktop(void);
int kernel_get_current_os_mode(void);

// SVG init functions
int svg_init(layer_t *layer, int mode);
void svg_init_nextgen(layer_t *layer);

// Screen functions
void screen_refresh(void);
void screen_mark_static_dirty(void);
void screen_mark_all_dirty(void);

// Layer functions
void layer_fill(layer_t *layer, uint32_t color);

// Cursor
void cursor_init(void);

// IRQ
void idt_install(void);
void irq_install(void);
void timer_phase(int hz);
void keyboard_install(void);
void mouse_install(void);
void enable_interrupts(void);

// System
void sys_restart(void);

// Warp UI init
void warp_ui_mod_init(struct multiboot_info *mbi);

// Set framebuffer info
void set_framebuffer_info(uint32_t *fb, uint32_t width, uint32_t height, uint32_t pitch);

// Register layer
void register_layer(layer_t *layer);

// Global state (shared with Rust) - exported for drivers.c
int g_dev_pointer_check = 1;
int g_dev_event_check = 0;
int g_dev_show_hud = 1;

// Simple global variable store
#define MAX_GLOBAL_VARS 128
static struct { char key[64]; char val[512]; } g_global_vars[MAX_GLOBAL_VARS];
static int g_global_var_count = 0;

const char *get_w1_global(const char *key) {
    if (!key) return "";
    
    // Dev flags
    if (strcmp(key, "~~dev/pointerCheck") == 0) return g_dev_pointer_check ? "true" : "false";
    if (strcmp(key, "~~dev/eventCheck") == 0) return g_dev_event_check ? "true" : "false";
    if (strcmp(key, "~~dev/showHUD") == 0) return g_dev_show_hud ? "true" : "false";
    
    // Search global vars
    for (int i = 0; i < g_global_var_count; i++) {
        if (strcmp(g_global_vars[i].key, key) == 0) {
            return g_global_vars[i].val;
        }
    }
    return "";
}

void set_w1_global(const char *key, const char *val) {
    if (!key || !val) return;
    
    // Update dev flags
    if (strcmp(key, "~~dev/pointerCheck") == 0) {
        g_dev_pointer_check = (strcmp(val, "true") == 0);
        return;
    }
    if (strcmp(key, "~~dev/eventCheck") == 0) {
        g_dev_event_check = (strcmp(val, "true") == 0);
        return;
    }
    if (strcmp(key, "~~dev/showHUD") == 0) {
        g_dev_show_hud = (strcmp(val, "true") == 0);
        return;
    }
    
    // Handle log appending
    const char *effective_key = key;
    int is_log = (strcmp(key, "--warpSystemLog") == 0);
    if (strcmp(key, "~~json/main/dark") == 0) effective_key = "~~main/dark";
    
    // Search existing
    for (int i = 0; i < g_global_var_count; i++) {
        if (strcmp(g_global_vars[i].key, effective_key) == 0) {
            if (is_log) {
                // Append with newline
                size_t len = strlen(g_global_vars[i].val);
                if (len > 0 && len < 511) {
                    g_global_vars[i].val[len] = '\n';
                    strlcat(g_global_vars[i].val, val, 512);
                } else {
                    strlcpy(g_global_vars[i].val, val, 512);
                }
            } else {
                strlcpy(g_global_vars[i].val, val, 512);
            }
            return;
        }
    }
    
    // Add new
    if (g_global_var_count < MAX_GLOBAL_VARS) {
        int idx = g_global_var_count++;
        strlcpy(g_global_vars[idx].key, effective_key, 64);
        if (is_log) {
            g_global_vars[idx].val[0] = '\0';
            strlcat(g_global_vars[idx].val, val, 512);
        } else {
            strlcpy(g_global_vars[idx].val, val, 512);
        }
    }
}

#define MAX_PENDING_COMMANDS 8
static char g_pending_commands[MAX_PENDING_COMMANDS][256];
static int g_pending_command_count = 0;

void set_pending_command(const char *cmd) {
    if (!cmd || g_pending_command_count >= MAX_PENDING_COMMANDS) return;
    strlcpy(g_pending_commands[g_pending_command_count], cmd, 256);
    g_pending_command_count++;
}

int kernel_process_pending_commands(void) {
    // This is called from Rust - for now just return 0
    return 0;
}

int kernel_try_autoboot_to_desktop(void) {
    // This is called from Rust - for now just return 0
    return 0;
}

int kernel_get_current_os_mode(void) {
    // Return 0 for classic mode
    return 0;
}

void kernel_main_iteration_classic(void) {
    // Classic mode iteration - minimal implementation
}

void kernel_main_iteration_desktop(void) {
    // Desktop mode iteration - minimal implementation
}

void kernel_finish_iteration(void) {
    rt_set_cpu_idle(0);
    screen_refresh();
}

void kernel_load_wallpaper_from_settings(const char *wp_name) {
    // Stub - wallpaper loading handled elsewhere
    (void)wp_name;
}

void kernel_mark_os_settings_ready(void) {
    set_w1_global("--warpSystemLog", "OS settings ready");
}

void kernel_add_window_for_command(const char *title, int x, int y, int w, int h, int is_warp1) {
    // Stub - window management handled in Rust
    (void)title; (void)x; (void)y; (void)w; (void)h; (void)is_warp1;
}

void kernel_close_active_window_for_command(void) {
    // Stub
}

const char *kernel_find_warp_module(const char *name, int *out_is_warp1) {
    // Stub - module lookup handled elsewhere
    (void)name;
    if (out_is_warp1) *out_is_warp1 = 0;
    return NULL;
}

void kernel_list_warp_modules(char *out_buf, int out_buf_len) {
    if (!out_buf || out_buf_len <= 0) return;
    strlcpy(out_buf, "Mods: (none)", out_buf_len);
}

void kernel_storage_sync_command(void) {
    set_w1_global("--warpSystemLog", "Storage sync not implemented");
}

void kernel_storage_ls_command(void) {
    fs_list_files();
}

// Additional string functions needed by nanosvg
char *strchr(const char *s, int c) {
    for (; *s; ++s) {
        if (*s == (char)c) return (char *)s;
    }
    return c == 0 ? (char *)s : NULL;
}

char *strstr(const char *haystack, const char *needle) {
    if (!*needle) return (char *)haystack;
    for (const char *h = haystack; *h; ++h) {
        const char *h2 = h;
        const char *n = needle;
        while (*h2 && *n && (*h2 == *n)) { h2++; n++; }
        if (!*n) return (char *)h;
    }
    return NULL;
}

long strtol(const char *nptr, char **endptr, int base) {
    (void)base;
    const char *s = nptr;
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    int sign = 1;
    if (*s == '-') { sign = -1; s++; }
    else if (*s == '+') { s++; }
    long val = 0;
    while (*s >= '0' && *s <= '9') { val = val * 10 + (*s - '0'); s++; }
    if (endptr) *endptr = (char *)s;
    return val * sign;
}

long long strtoll(const char *nptr, char **endptr, int base) {
    (void)base;
    const char *s = nptr;
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    int sign = 1;
    if (*s == '-') { sign = -1; s++; }
    else if (*s == '+') { s++; }
    long long val = 0;
    while (*s >= '0' && *s <= '9') { val = val * 10 + (*s - '0'); s++; }
    if (endptr) *endptr = (char *)s;
    return val * sign;
}

// Math functions needed by nanosvg - provide standard names
double pow(double base, double exp) {
    long e = (long)exp;
    if ((double)e != exp) return 1.0;
    if (e == 0) return 1.0;
    int neg = 0;
    if (e < 0) { neg = 1; e = -e; }
    double result = 1.0;
    double b = base;
    while (e) {
        if (e & 1) result *= b;
        b *= b;
        e >>= 1;
    }
    return neg ? 1.0 / result : result;
}

// Standard math function wrappers for nanosvg
float sqrtf(float x) { return sqrtf_local(x); }
float cosf(float x) { return cosf_local(x); }
float sinf(float x) { return sinf_local(x); }
double sqrt(double x) { return (double)sqrtf_local((float)x); }

float acosf(float x) {
    const float pi = 3.14159265358979323846f;
    if (x <= -1.0f) return pi;
    if (x >= 1.0f) return 0.0f;
    return atan2f_local(sqrtf_local(1.0f - x * x), x);
}

static float atan2f_local(float y, float x) {
    const float pi = 3.14159265358979323846f;
    if (x > 0.0f) return atan_approx_local(y / x);
    if (x < 0.0f) {
        if (y >= 0.0f) return atan_approx_local(y / x) + pi;
        return atan_approx_local(y / x) - pi;
    }
    if (y > 0.0f) return pi * 0.5f;
    if (y < 0.0f) return -pi * 0.5f;
    return 0.0f;
}

static float atan_approx_local(float z) {
    const float pi = 3.14159265358979323846f;
    if (z > 1.0f) return (pi * 0.5f) - atan_approx_local(1.0f / z);
    if (z < -1.0f) return -(pi * 0.5f) - atan_approx_local(1.0f / z);
    return z / (1.0f + 0.28f * z * z);
}

double fabs(double x) { return x < 0.0 ? -x : x; }

float fabsf(float x) { return x < 0.0f ? -x : x; }

float floorf(float x) {
    int i = (int)x;
    if ((float)i > x) i--;
    return (float)i;
}

float ceilf(float x) {
    int i = (int)x;
    if ((float)i < x) i++;
    return (float)i;
}

float roundf(float x) {
    return (x >= 0.0f) ? floorf(x + 0.5f) : ceilf(x - 0.5f);
}

int isnan(double x) { return x != x; }

float tanf(float x) {
    float c = cosf_local(x);
    if (c == 0.0f) return 0.0f;
    return sinf_local(x) / c;
}

float atan2f(float y, float x) { return atan2f_local(y, x); }

float fmodf(float x, float y) { return fmodf_local(x, y); }

// qsort implementation
static void swap_bytes(unsigned char *a, unsigned char *b, size_t size) {
    while (size--) {
        unsigned char tmp = *a;
        *a++ = *b;
        *b++ = tmp;
    }
}

void qsort(void *base, size_t nmemb, size_t size, int (*compar)(const void *, const void *)) {
    unsigned char *arr = (unsigned char *)base;
    for (size_t i = 1; i < nmemb; ++i) {
        size_t j = i;
        while (j > 0) {
            unsigned char *a = arr + (j - 1) * size;
            unsigned char *b = arr + j * size;
            if (compar(a, b) <= 0) break;
            swap_bytes(a, b, size);
            --j;
        }
    }
}

// Stub file I/O functions (not used in kernel)
FILE *fopen(const char *path, const char *mode) { (void)path; (void)mode; return NULL; }
int fclose(FILE *stream) { (void)stream; return 0; }
size_t fread(void *ptr, size_t size, size_t nmemb, FILE *stream) { (void)ptr; (void)size; (void)nmemb; (void)stream; return 0; }
int fseek(FILE *stream, long offset, int whence) { (void)stream; (void)offset; (void)whence; return -1; }
long ftell(FILE *stream) { (void)stream; return -1; }
int sscanf(const char *str, const char *format, ...) { (void)str; (void)format; return 0; }

// Timer phase
void timer_phase(int hz) {
    int divisor = 1193180 / hz;
    outb(0x43, 0x36);
    outb(0x40, divisor & 0xFF);
    outb(0x40, (divisor >> 8) & 0xFF);
}

// Cursor init stub
void cursor_init(void) { }

// Stub for warp_ui_mod_init - implemented in storage.c
void warp_ui_mod_init(struct multiboot_info *mbi) {
    (void)mbi;
}

// Stub for svg_init
int svg_init(layer_t *layer, int mode) {
    (void)layer; (void)mode;
    return 1;
}
