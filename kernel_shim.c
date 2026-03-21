// Kernel shim - Complete C implementation for BaramOS
// Based on original kernel_runtime.c functionality
#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include "drivers.h"
#include "ui/warp_engine.h"
#include "ui/warp1_engine.h"
#include "font/fonts.h"
#include "storage.h"
#include "fs.h"

// Forward declarations
void register_layer(layer_t *layer);
void screen_mark_static_dirty(void);
void screen_mark_all_dirty(void);
void layer_fill(layer_t *layer, uint32_t color);
void layer_draw_string(layer_t *layer, int x, int y, const char *str, uint32_t color, uint32_t bg_color);
void ata_init(void);
void fs_init(void);
void *fs_read_file(const char *name, uint32_t *size);
void rust_graphics_apply_conic_gradient(unsigned char *data, int w, int h, int rx, int ry, int rw, int rh, uint32_t c1, uint32_t c2);
void rust_graphics_box_blur_alpha(unsigned char *data, int w, int h, int radius);
void rust_kernel_append_uint(char *p, unsigned int v);
void hud_update(layer_t *hud, unsigned int cpu_percent, unsigned int mem_total_kb);

// =====================================================
// Constants and static buffers
// =====================================================
#define SCREEN_WIDTH 1280
#define SCREEN_HEIGHT 720
#define SVG_WIDTH 1024
#define SVG_HEIGHT 768
#define BASE_BG_COLOR 0xFF000000u
#define HUD_W 320
#define HUD_H_MAX 240
#define MOUSE_HOTSPOT_X 28
#define MOUSE_HOTSPOT_Y 21
#define SCROLL_EASE 0.4f

static uint32_t main_screen_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
static uint32_t svg_base_buf[SVG_WIDTH * SVG_HEIGHT];
static uint32_t blink_buf[50 * 50];
static uint32_t hud_buf[HUD_W * HUD_H_MAX];
static int g_hud_current_h = 64;
static uint32_t text_layer_buf[SCREEN_WIDTH * SCREEN_HEIGHT];

// Static layer structures
static layer_t g_desktop_layer;
static layer_t g_svg_layer;
static layer_t g_blink_layer;
static layer_t g_hud_layer;
static layer_t g_text_layer;

// Global state
static volatile uint32_t timer_ticks = 0;
static volatile int cpu_idle = 0;
static volatile uint32_t idle_ticks = 0;

// SVG state (matching original kernel_runtime.c)
static void *g_svg_image = NULL;
static void *g_svg_rast = NULL;
static unsigned char *g_svg_full_rgba = NULL;
static int g_svg_full_w = 0;
static int g_svg_full_h = 0;
static int g_svg_ready = 0;
static float g_scroll_x = 0.0f;
static float g_scroll_y = 0.0f;
static float g_target_scroll_x = 0.0f;
static float g_target_scroll_y = 0.0f;

// File pointers from initrd
static const char *g_bootlogo_ptr = NULL;
static uint32_t g_bootlogo_size = 0;
static const char *g_wallpaper_ptr = NULL;
static uint32_t g_wallpaper_size = 0;
static int g_bootlogo_found = 0;
static int g_wallpaper_found = 0;

// Classic mode state
static volatile uint32_t g_kmain_last_blink_tick = 0;
static int g_kmain_blink_state = 0;
static int g_kmain_classic_last_mouse_x = -1;
static int g_kmain_classic_last_mouse_y = -1;

// External references
extern volatile int32_t mouse_x;
extern volatile int32_t mouse_y;
extern volatile int32_t mouse_scroll;
extern volatile uint8_t mouse_buttons;
extern volatile char keybuf[];
extern volatile int keybuf_len;

// =====================================================
// Memory allocator
// =====================================================
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

// =====================================================
// Memory and string functions
// =====================================================
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

// =====================================================
// Math functions
// =====================================================
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

static float fabsf_local(float x) {
    return x < 0.0f ? -x : x;
}

static float fmodf_local(float x, float y) {
    if (y == 0.0f) return 0.0f;
    int q = (int)(x / y);
    return x - (float)q * y;
}

static float roundf_local(float x) {
    int i = (int)x;
    if ((float)i > x) return (float)(i - 1);
    if ((float)i < x) return (float)(i + 1);
    return (float)i;
}

static float atan2f_local(float y, float x);
static float atan_approx_local(float z);

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

// Standard math function wrappers
float sqrtf(float x) { return sqrtf_local(x); }
float cosf(float x) { return cosf_local(x); }
float sinf(float x) { return sinf_local(x); }
float acosf(float x) {
    const float pi = 3.14159265358979323846f;
    if (x <= -1.0f) return pi;
    if (x >= 1.0f) return 0.0f;
    return atan2f_local(sqrtf_local(1.0f - x * x), x);
}
float fabsf(float x) { return fabsf_local(x); }
float floorf(float x) {
    int i = (int)x;
    if ((float)i > x) return (float)(i - 1);
    return (float)i;
}
float ceilf(float x) {
    int i = (int)x;
    if ((float)i < x) return (float)(i + 1);
    return (float)i;
}
float roundf(float x) { return roundf_local(x); }
float tanf(float x) {
    float c = cosf_local(x);
    if (c == 0.0f) return 0.0f;
    return sinf_local(x) / c;
}
float atan2f(float y, float x) { return atan2f_local(y, x); }
float fmodf(float x, float y) { return fmodf_local(x, y); }
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
double sqrt(double x) { return (double)sqrtf_local((float)x); }
double fabs(double x) { return x < 0.0 ? -x : x; }
int isnan(double x) { return x != x; }

// =====================================================
// stb_truetype and nanosvg
// =====================================================
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
#define STBTT_acos(x) acosf(x)
#define STBTT_fabs(x) fabsf_local(x)
#include "font/stb_truetype.h"

#define NANOSVG_IMPLEMENTATION
#include "nanosvg/nanosvg.h"
#define NANOSVGRAST_IMPLEMENTATION
#include "nanosvg/nanosvgrast.h"

// =====================================================
// Multiboot module entry
// =====================================================
typedef struct {
    uint32_t mod_start;
    uint32_t mod_end;
    uint32_t string;
    uint32_t reserved;
} __attribute__((packed)) multiboot_module_t;

// =====================================================
// Timer handler
// =====================================================
void timer_handler(struct regs *r) {
    (void)r;
    timer_ticks++;
    if (cpu_idle)
        idle_ticks++;
}

// =====================================================
// Font functions
// =====================================================
static stbtt_fontinfo g_font;
static int g_font_ready = 0;

int font_init(struct multiboot_info *mbi) {
    if (!mbi) return 0;
    if (!(mbi->flags & 0x8)) return 0;
    if (mbi->mods_count == 0) return 0;

    multiboot_module_t *mod = (multiboot_module_t *)(uintptr_t)mbi->mods_addr;
    unsigned char *ttf = (unsigned char *)(uintptr_t)mod->mod_start;
    
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

// =====================================================
// Graphics helpers
// =====================================================
static void box_blur_alpha(unsigned char *data, int w, int h, int radius) {
    rust_graphics_box_blur_alpha(data, w, h, radius);
}

static uint32_t parse_rgba_smart(const char *str, int color_index) {
    if (!str) return 0xFFFFFFFF;
    const char *p = str;
    for (int i = 0; i <= color_index; i++) {
        const char *next = strstr(p, "rgba(");
        if (!next) return (i == 0) ? 0xFF5CA8FF : 0xFFFFFFFF;
        p = next + 5;
    }
    int r = (int)strtoll(p, (char **)&p, 10);
    while (*p == ',' || *p == ' ') p++;
    int g = (int)strtoll(p, (char **)&p, 10);
    while (*p == ',' || *p == ' ') p++;
    int b = (int)strtoll(p, (char **)&p, 10);
    return (0xFFu << 24) | ((uint32_t)r << 16) | ((uint32_t)g << 8) | (uint32_t)b;
}

// =====================================================
// SVG render full (with scroll support)
// =====================================================
static void svg_render_full(layer_t *layer) {
    if (!g_svg_full_rgba) return;

    const uint32_t bg = BASE_BG_COLOR;
    uint8_t bg_r = (bg >> 16) & 0xFF;
    uint8_t bg_g = (bg >> 8) & 0xFF;
    uint8_t bg_b = bg & 0xFF;

    int scroll_x = (int)roundf_local(g_scroll_x);
    int scroll_y = (int)roundf_local(g_scroll_y);

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

// =====================================================
// SVG init
// =====================================================
int svg_init(layer_t *layer, int load_wallpaper) {
    if (g_svg_ready && !load_wallpaper)
        return 1;
    
    if (load_wallpaper) {
        g_svg_ready = 0;
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

    if (g_svg_image) nsvgDelete((NSVGimage *)g_svg_image);
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
        g_svg_full_rgba = (unsigned char *)malloc((size_t)g_svg_full_w * (size_t)g_svg_full_h * 4);
    }
    if (!g_svg_full_rgba)
        return 0;
    memset(g_svg_full_rgba, 0, (size_t)g_svg_full_w * (size_t)g_svg_full_h * 4);

    float scale = 1.0f;
    float tx = 0.0f, ty = 0.0f;

    if (load_wallpaper && svg_data == g_wallpaper_ptr) {
        // "Center Cover" logic
        float scale_x = (float)g_svg_full_w / ((NSVGimage *)g_svg_image)->width;
        float scale_y = (float)g_svg_full_h / ((NSVGimage *)g_svg_image)->height;
        scale = (scale_x > scale_y) ? scale_x : scale_y;
        tx = (g_svg_full_w - ((NSVGimage *)g_svg_image)->width * scale) / 2.0f;
        ty = (g_svg_full_h - ((NSVGimage *)g_svg_image)->height * scale) / 2.0f;
    } else {
        // Center logic for logo - center in the layer
        float logo_scale = 3.0f; // Scale up the logo
        tx = (g_svg_full_w - ((NSVGimage *)g_svg_image)->width * logo_scale) / 2.0f;
        ty = (g_svg_full_h - ((NSVGimage *)g_svg_image)->height * logo_scale) / 2.0f;
        scale = logo_scale;
    }

    nsvgRasterize((NSVGrasterizer *)g_svg_rast, (NSVGimage *)g_svg_image, tx, ty, scale, g_svg_full_rgba,
                  g_svg_full_w, g_svg_full_h, g_svg_full_w * 4);

    // Render to layer buffer
    svg_render_full(layer);
    
    // Copy to svg_base_buf
    memcpy(svg_base_buf, layer->buffer, sizeof(uint32_t) * layer->width * layer->height);
    
    g_svg_ready = 1;
    return 1;
}

// =====================================================
// Global variable store
// =====================================================
#define MAX_GLOBAL_VARS 128
static struct { char key[64]; char val[512]; } g_global_vars[MAX_GLOBAL_VARS];
static int g_global_var_count = 0;

// Export for drivers.c
int g_dev_pointer_check = 1;
int g_dev_event_check = 0;
int g_dev_show_hud = 1;

const char *get_w1_global(const char *key) {
    if (!key) return "";
    if (strcmp(key, "~~dev/pointerCheck") == 0) return g_dev_pointer_check ? "true" : "false";
    if (strcmp(key, "~~dev/eventCheck") == 0) return g_dev_event_check ? "true" : "false";
    if (strcmp(key, "~~dev/showHUD") == 0) return g_dev_show_hud ? "true" : "false";
    for (int i = 0; i < g_global_var_count; i++) {
        if (strcmp(g_global_vars[i].key, key) == 0) {
            return g_global_vars[i].val;
        }
    }
    return "";
}

void set_w1_global(const char *key, const char *val) {
    if (!key || !val) return;
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
    const char *effective_key = key;
    int is_log = (strcmp(key, "--warpSystemLog") == 0);
    if (strcmp(key, "~~json/main/dark") == 0) effective_key = "~~main/dark";
    for (int i = 0; i < g_global_var_count; i++) {
        if (strcmp(g_global_vars[i].key, effective_key) == 0) {
            if (is_log) {
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

// =====================================================
// Timer and utilities
// =====================================================
void timer_phase(int hz) {
    int divisor = 1193180 / hz;
    outb(0x43, 0x36);
    outb(0x40, divisor & 0xFF);
    outb(0x40, (divisor >> 8) & 0xFF);
}

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

// Stub file I/O
FILE *fopen(const char *path, const char *mode) { (void)path; (void)mode; return NULL; }
int fclose(FILE *stream) { (void)stream; return 0; }
size_t fread(void *ptr, size_t size, size_t nmemb, FILE *stream) { (void)ptr; (void)size; (void)nmemb; (void)stream; return 0; }
int fseek(FILE *stream, long offset, int whence) { (void)stream; (void)offset; (void)whence; return -1; }
long ftell(FILE *stream) { (void)stream; return -1; }
int sscanf(const char *str, const char *format, ...) { (void)str; (void)format; return 0; }

// =====================================================
// Cursor with SVG
// =====================================================
void cursor_init(void) {
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
    if (!img) return;

    int target_h = 48;
    float scale = (float)target_h / img->height;
    int target_w = (int)(img->width * scale);

    int padding = 16;
    int w = target_w + padding * 2;
    int h = target_h + padding * 2;

    uint32_t *buf = (uint32_t *)malloc((size_t)w * (size_t)h * 4);
    if (!buf) { nsvgDelete(img); return; }

    NSVGrasterizer *rast = nsvgCreateRasterizer();
    unsigned char *rgba = (unsigned char *)malloc((size_t)w * (size_t)h * 4);
    unsigned char *shadow_rgba = (unsigned char *)malloc((size_t)w * (size_t)h * 4);

    if (rast && rgba && shadow_rgba) {
        memset(shadow_rgba, 0, (size_t)w * (size_t)h * 4);
        nsvgRasterize(rast, img, (float)padding + 2.0f, (float)padding + 4.0f,
                      scale, shadow_rgba, w, h, w * 4);
        box_blur_alpha(shadow_rgba, w, h, 4);

        memset(rgba, 0, (size_t)w * (size_t)h * 4);
        nsvgRasterize(rast, img, (float)padding, (float)padding, scale, rgba, w, h, w * 4);

        for (int i = 0; i < w * h; i++) {
            uint8_t shadow_a = (uint8_t)(shadow_rgba[i * 4 + 3] * 0.5f);
            uint8_t r = rgba[i * 4 + 0], g = rgba[i * 4 + 1], b = rgba[i * 4 + 2], a = rgba[i * 4 + 3];

            if (a == 255) {
                buf[i] = (0xFFu << 24) | ((uint32_t)r << 16) | ((uint32_t)g << 8) | (uint32_t)b;
            } else {
                uint8_t out_a = a + (shadow_a * (255 - a) / 255);
                if (out_a == 0) { buf[i] = 0; }
                else {
                    uint8_t out_r = (uint8_t)((r * a + 0 * (out_a - a)) / out_a);
                    uint8_t out_g = (uint8_t)((g * a + 0 * (out_a - a)) / out_a);
                    uint8_t out_b = (uint8_t)((b * a + 0 * (out_a - a)) / out_a);
                    buf[i] = ((uint32_t)out_a << 24) | ((uint32_t)out_r << 16) | ((uint32_t)out_g << 8) | (uint32_t)out_b;
                }
            }
        }
        set_cursor_bitmap(buf, w, h);
    }

    if (rast) nsvgDeleteRasterizer(rast);
    if (rgba) free(rgba);
    if (shadow_rgba) free(shadow_rgba);
    nsvgDelete(img);
}

// =====================================================
// warp_ui_mod_init
// =====================================================
void warp_ui_mod_init(struct multiboot_info *mbi) {
    (void)mbi;
    ata_init();
    fs_init();
    
    g_bootlogo_ptr = fs_read_file("bootlogo.svg", &g_bootlogo_size);
    g_wallpaper_ptr = fs_read_file("wallpaper_1.svg", &g_wallpaper_size);
    if (g_bootlogo_ptr) g_bootlogo_found = 1;
    if (g_wallpaper_ptr) g_wallpaper_found = 1;
}

// =====================================================
// Main iteration functions
// =====================================================
void kernel_finish_iteration(void) {
    cpu_idle = 0;
}

void kernel_main_iteration_classic(void) {
    // Blink every 0.1 second (10 ticks)
    if (timer_ticks - g_kmain_last_blink_tick >= 10) {
        g_kmain_blink_state = !g_kmain_blink_state;
        g_blink_layer.active = g_kmain_blink_state;
        g_kmain_last_blink_tick = timer_ticks;
    }

    // Mouse tracking
    int mx = (int)mouse_x + MOUSE_HOTSPOT_X;
    int my = (int)mouse_y + MOUSE_HOTSPOT_Y;
    if (mx != g_kmain_classic_last_mouse_x || my != g_kmain_classic_last_mouse_y) {
        g_kmain_classic_last_mouse_x = mx;
        g_kmain_classic_last_mouse_y = my;
    }

    // Scroll handling
    if (mouse_scroll != 0) {
        g_target_scroll_y += (float)mouse_scroll * 30.0f;
        mouse_scroll = 0;
        int content_h = g_svg_full_h;
        int min_scroll = SCREEN_HEIGHT - content_h;
        if (min_scroll > 0) min_scroll = 0;
        if (g_target_scroll_y > 0.0f) g_target_scroll_y = 0.0f;
        if (g_target_scroll_y < (float)min_scroll) g_target_scroll_y = (float)min_scroll;
    }

    // Scroll animation
    if (g_target_scroll_y != g_scroll_y) {
        float dy = (g_target_scroll_y - g_scroll_y) * SCROLL_EASE;
        if (fabsf_local(dy) < 1.0f) {
            g_scroll_y = g_target_scroll_y;
        } else {
            g_scroll_y += dy;
        }
        svg_render_full(&g_svg_layer);
        memcpy(svg_base_buf, g_svg_layer.buffer, sizeof(uint32_t) * g_svg_layer.width * g_svg_layer.height);
        screen_mark_static_dirty();
    }

    // Keyboard
    if (keybuf_len > 0) {
        keybuf_len = 0;
    }
    
    // Update HUD
    hud_update(&g_hud_layer, 0, 0);
}

void kernel_main_iteration_desktop(void) {
    if (timer_ticks - g_kmain_last_blink_tick >= 10) {
        g_kmain_blink_state = !g_kmain_blink_state;
        g_blink_layer.active = g_kmain_blink_state;
        g_kmain_last_blink_tick = timer_ticks;
    }
}

int kernel_process_pending_commands(void) { return 0; }
int kernel_try_autoboot_to_desktop(void) { return 0; }
int kernel_get_current_os_mode(void) { return 0; }

// =====================================================
// Missing Rust interop functions
// =====================================================
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

void kernel_load_wallpaper_from_settings(const char *wp_name) { (void)wp_name; }
void kernel_list_warp_modules(char *out_buf, int out_buf_len) {
    if (!out_buf || out_buf_len <= 0) return;
    strlcpy(out_buf, "Mods: (none)", out_buf_len);
}
void kernel_close_active_window_for_command(void) { }
void kernel_storage_sync_command(void) { set_w1_global("--warpSystemLog", "Storage sync not implemented"); }
void kernel_storage_ls_command(void) { fs_list_files(); }

const char *kernel_find_warp_module(const char *name, int *out_is_warp1) {
    (void)name;
    if (out_is_warp1) *out_is_warp1 = 0;
    return NULL;
}

void kernel_add_window_for_command(const char *title, int x, int y, int w, int h, int is_warp1) {
    (void)title; (void)x; (void)y; (void)w; (void)h; (void)is_warp1;
}

#define MAX_PENDING_COMMANDS 8
static char g_pending_commands[MAX_PENDING_COMMANDS][256];
static int g_pending_command_count = 0;

void set_pending_command(const char *cmd) {
    if (!cmd || g_pending_command_count >= MAX_PENDING_COMMANDS) return;
    strlcpy(g_pending_commands[g_pending_command_count], cmd, 256);
    g_pending_command_count++;
}

void kernel_mark_os_settings_ready(void) {
    set_w1_global("--warpSystemLog", "OS settings ready");
}

// =====================================================
// HUD update
// =====================================================
void hud_update(layer_t *hud, unsigned int cpu_percent, unsigned int mem_total_kb) {
    (void)cpu_percent; (void)mem_total_kb;
    
    if (!g_dev_show_hud) {
        hud->active = 0;
        return;
    }
    hud->active = 1;
    
    layer_fill(hud, 0xFF000000);
    
    // Line 1: Build info
    char line1[64];
    char *p = line1;
    const char *title = "BaramOS Build ";
    while (*title) *p++ = *title++;
    
    // Append build number (simplified)
    *p++ = '#';
    *p++ = '6';
    *p++ = '8';
    *p++ = '6';
    *p = '\0';
    
    // Line 2: CPU/MEM
    char line2[64];
    p = line2;
    *p++ = 'C'; *p++ = 'P'; *p++ = 'U'; *p++ = ':'; *p++ = ' ';
    *p++ = '0'; *p++ = '%';
    *p++ = ' '; *p++ = 'R'; *p++ = 'A'; *p++ = 'M'; *p++ = ':'; *p++ = ' ';
    *p++ = '0'; *p++ = 'K'; *p++ = 'B';
    *p = '\0';
    
    // Line 3: Mode
    char line3[64];
    p = line3;
    *p++ = 'M'; *p++ = ':'; *p++ = ' ';
    *p++ = 'C'; *p++ = 'L'; *p++ = 'S';
    *p++ = ' '; *p++ = 'S'; *p++ = ':'; *p++ = ' ';
    *p++ = 'I'; *p++ = 'd'; *p++ = 'l'; *p++ = 'e';
    *p = '\0';
    
    // Line 4: Mouse
    char line4[64];
    p = line4;
    *p++ = 'M'; *p++ = ':'; *p++ = ' ';
    // Simple mouse position
    int mx = (int)mouse_x;
    int my = (int)mouse_y;
    if (mx > 99) { *p++ = (mx / 100) + '0'; mx %= 100; }
    if (mx > 9) { *p++ = (mx / 10) + '0'; mx %= 10; }
    *p++ = mx + '0';
    *p++ = ',';
    if (my > 99) { *p++ = (my / 100) + '0'; my %= 100; }
    if (my > 9) { *p++ = (my / 10) + '0'; my %= 10; }
    *p++ = my + '0';
    *p = '\0';
    
    layer_draw_string(hud, 2, 0, line1, 0xFFFFFFFF, 0x00000000);
    layer_draw_string(hud, 2, 8, line2, 0xFFFFFFFF, 0x00000000);
    layer_draw_string(hud, 2, 16, line3, 0xFFFFFF00, 0x00000000);
    layer_draw_string(hud, 2, 24, line4, 0xFFFFFFFF, 0x00000000);
}

// =====================================================
// Accessor functions for Rust
// =====================================================
layer_t *get_desktop_layer(void) { return &g_desktop_layer; }
layer_t *get_svg_layer(void) { return &g_svg_layer; }
layer_t *get_blink_layer(void) { return &g_blink_layer; }
layer_t *get_hud_layer(void) { return &g_hud_layer; }
layer_t *get_text_layer(void) { return &g_text_layer; }
uint32_t *get_main_screen_buf(void) { return main_screen_buf; }
uint32_t *get_svg_base_buf(void) { return svg_base_buf; }
uint32_t *get_blink_buf(void) { return blink_buf; }
uint32_t *get_hud_buf(void) { return hud_buf; }
uint32_t *get_text_layer_buf(void) { return text_layer_buf; }
int get_hud_current_h(void) { return g_hud_current_h; }
int get_screen_width(void) { return SCREEN_WIDTH; }
int get_screen_height(void) { return SCREEN_HEIGHT; }

// =====================================================
// Initialize static layers
// =====================================================
void init_static_layers(void) {
    // Desktop layer
    g_desktop_layer.buffer = main_screen_buf;
    g_desktop_layer.x = 0;
    g_desktop_layer.y = 0;
    g_desktop_layer.width = SCREEN_WIDTH;
    g_desktop_layer.height = SCREEN_HEIGHT;
    g_desktop_layer.transparent = 0;
    g_desktop_layer.active = 1;
    g_desktop_layer.dynamic = 0;
    layer_fill(&g_desktop_layer, 0xFF000000);
    register_layer(&g_desktop_layer);

    // SVG layer
    g_svg_layer.buffer = svg_base_buf;
    g_svg_layer.x = 0;
    g_svg_layer.y = 0;
    g_svg_layer.width = SVG_WIDTH;
    g_svg_layer.height = SVG_HEIGHT;
    g_svg_layer.transparent = 0;
    g_svg_layer.active = 1;
    g_svg_layer.dynamic = 0;
    svg_init(&g_svg_layer, 0);
    register_layer(&g_svg_layer);

    // Blink layer
    g_blink_layer.buffer = blink_buf;
    g_blink_layer.x = SCREEN_WIDTH - 60;
    g_blink_layer.y = SCREEN_HEIGHT - 60;
    g_blink_layer.width = 50;
    g_blink_layer.height = 50;
    g_blink_layer.transparent = 0;
    g_blink_layer.active = 1;
    g_blink_layer.dynamic = 1;
    layer_fill(&g_blink_layer, 0xFF0000FF);
    register_layer(&g_blink_layer);

    // HUD layer
    g_hud_layer.buffer = hud_buf;
    g_hud_layer.x = 10;
    g_hud_layer.y = SCREEN_HEIGHT - (g_hud_current_h + 10);
    g_hud_layer.width = HUD_W;
    g_hud_layer.height = g_hud_current_h;
    g_hud_layer.transparent = 0;
    g_hud_layer.active = 1;
    g_hud_layer.dynamic = 1;
    layer_fill(&g_hud_layer, 0xFF000000);
    register_layer(&g_hud_layer);

    // Text layer
    g_text_layer.buffer = text_layer_buf;
    g_text_layer.x = 0;
    g_text_layer.y = 0;
    g_text_layer.width = SCREEN_WIDTH;
    g_text_layer.height = SCREEN_HEIGHT;
    g_text_layer.transparent = 0x00000000;
    g_text_layer.active = 1;
    g_text_layer.dynamic = 1;
    for (int i = 0; i < SCREEN_WIDTH * SCREEN_HEIGHT; i++) {
        text_layer_buf[i] = 0x00000000;
    }
    register_layer(&g_text_layer);
    
    screen_mark_static_dirty();
    screen_mark_all_dirty();
}
