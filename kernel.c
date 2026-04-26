#include <math.h>   // sinf, cosf
#include <stdarg.h> // va_list
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>  // FILE
#include <stdlib.h> // malloc, free, realloc
#include <string.h> // memcpy, memset
#ifdef __SSE2__
#include <emmintrin.h>
#define ALIGN16 __attribute__((aligned(16)))
#else
#define ALIGN16
#endif

#include "drivers.h"
#include "font/fonts.h"
#include "ui/svg_data.h"
#include "storage.h"
#include "fs.h"

#include <stddef.h>

// GPU blur support
#include "gpu/gpu_driver.h"
#include "gpu/gpu_blur.h"
#include "gpu/gpu_svg.h"

#define NANOSVG_IMPLEMENTATION
#include "nanosvg/nanosvg.h"
#define NANOSVGRAST_IMPLEMENTATION
#include "nanosvg/nanosvgrast.h"

// Distance to line segment
static float dist_to_line_segment(float px, float py, float ax, float ay, float bx, float by) {
  float dx = bx - ax;
  float dy = by - ay;
  float len2 = dx*dx + dy*dy;
  if (len2 == 0.0f) return sqrtf((px-ax)*(px-ax) + (py-ay)*(py-ay));
  float t = ((px - ax)*dx + (py - ay)*dy) / len2;
  t = t < 0.0f ? 0.0f : (t > 1.0f ? 1.0f : t);
  float cx = ax + t*dx;
  float cy = ay + t*dy;
  return sqrtf((px-cx)*(px-cx) + (py-cy)*(py-cy));
}
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
typedef enum {
  LOCK_TRANSITION_IDLE,
  LOCK_TRANSITION_LOCKING,
  LOCK_TRANSITION_UNLOCKING
} lock_transition_t;

typedef struct {
  int is_locked;
  int target_locked;
  lock_transition_t transition;
  float transition_progress;
  uint32_t transition_started_at;
  uint32_t transition_duration_ticks;
  uint32_t last_clock_tick;
  char time_label[16];
} os_lock_state_t;

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

static volatile uint32_t idle_ticks = 0;
static volatile int cpu_idle = 0;

// --- グローバル変数 (Nextgen/Warp) ---
static os_mode_t current_os_mode = OS_MODE_CLASSIC;
static char g_last_svg_parse_status[64] = "None";
static os_lock_state_t g_lock_state = {0, 0, LOCK_TRANSITION_IDLE, 0.0f, 0, 0, 0, "00:00"};

typedef struct {
  char name[64];
  uintptr_t start;
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
    int i = 0;
    // Skip leading spaces or nulls
    while (i < len && (s[i] == ' ' || s[i] == '\0')) i++;
    while (i < len) {
        if (s[i] < '0' || s[i] > '7') break;
        res = res * 8 + (s[i] - '0');
        i++;
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
static int g_dev_scroll_speed = 5;

static int g_svg_dirty = 1;
static char g_hud_status[64] = "Idle";

// --- 前方宣言 ---
struct window_struct;
static uint32_t lerp_color(uint32_t c1, uint32_t c2, float t);
static void apply_conic_gradient(unsigned char *data, int w, int h, int rx,
                                 int ry, int rw, int rh, uint32_t c1,
                                 uint32_t c2);
static uint32_t blend_rgb_over_opaque(uint32_t bg, uint32_t fg, uint8_t alpha);
static void svg_render_full(layer_t *layer);
static void redraw_warp_svg(layer_t *layer);
static void draw_wallpaper(layer_t *layer);
static void bg_preview_update(layer_t *preview);
static void window_redraw(struct window_struct *win);
static void request_window_interaction_refresh(struct window_struct *win);
static uint32_t sample_backdrop_pixel(struct window_struct *target, int px, int py);
static char *append_uint(char *p, unsigned int v);
static void lock_state_enter(void);
static void lock_state_request_unlock(void);
static void lock_state_update(void);
static int lock_state_is_visible(void);
void layer_draw_ttf(layer_t *layer, int px, int py, const char *str,
                    float font_size, uint32_t color);
int measure_ttf_width(const char *str, float font_size);
extern volatile uint32_t timer_ticks;

typedef struct {
  uint32_t burst_first_tick;
  uint32_t last_tick;
  uint32_t last_serial;
  int last_dir;
  int burst_events;
  int burst_steps;
} scroll_input_state_t;

static scroll_input_state_t g_classic_scroll_input = {0, 0, 0, 0, 0, 0};

static void reset_scroll_input_state(scroll_input_state_t *state) {
  if (!state) return;
  state->burst_first_tick = 0;
  state->last_tick = 0;
  state->last_serial = 0;
  state->last_dir = 0;
  state->burst_events = 0;
  state->burst_steps = 0;
}

static float clamp_scroll_offset(float scroll_y, int viewport_h, int content_h) {
  int min_scroll = viewport_h - content_h;
  if (min_scroll > 0) min_scroll = 0;
  if (scroll_y > 0.0f) return 0.0f;
  if (scroll_y < (float)min_scroll) return (float)min_scroll;
  return scroll_y;
}

static float convert_scroll_event_to_delta(scroll_input_state_t *state,
                                           const wheel_scroll_event_t *event) {
  if (!state || !event || event->steps == 0) return 0.0f;

  int dir = (event->steps > 0) ? 1 : -1;
  int steps = (event->steps > 0) ? event->steps : -event->steps;
  uint32_t tick_gap = state->last_tick ? (event->tick - state->last_tick) : 0;
  uint32_t serial_gap = state->last_serial ? (event->serial - state->last_serial) : 0;
  int same_burst = state->last_serial != 0 &&
                   dir == state->last_dir &&
                   tick_gap <= 18 &&
                   serial_gap <= 8;

  if (!same_burst) {
    state->burst_first_tick = event->tick;
    state->burst_events = 0;
    state->burst_steps = 0;
  }

  state->burst_events++;
  state->burst_steps += steps;

  float base_per_step = (float)g_dev_scroll_speed;
  if (base_per_step < 1.0f) base_per_step = 1.0f;

  float per_step = base_per_step;
  if (state->burst_events > 1) {
    float span = (float)(event->tick - state->burst_first_tick);
    float avg_gap = span / (float)(state->burst_events - 1);
    float cadence_gain = 1.0f + 10.0f / (avg_gap + 3.0f);
    float burst_gain = 1.0f + (float)(state->burst_events - 1) * 0.08f;
    per_step *= cadence_gain * burst_gain;
  }
  if (per_step > base_per_step * 14.4f) per_step = base_per_step * 14.4f;

  float step_gain = 1.0f + (float)(steps - 1) * 0.35f;
  float delta = per_step * (float)steps * step_gain;
  if (delta > base_per_step * 144.0f) delta = base_per_step * 144.0f;

  state->last_tick = event->tick;
  state->last_serial = event->serial;
  state->last_dir = dir;
  return (float)dir * delta;
}

static float consume_scroll_events(scroll_input_state_t *state) {
  wheel_scroll_event_t event;
  float total = 0.0f;
  while (mouse_pop_scroll_event(&event)) {
    total += convert_scroll_event_to_delta(state, &event);
  }
  return total;
}

static void discard_scroll_events(void) {
  wheel_scroll_event_t event;
  while (mouse_pop_scroll_event(&event)) {
  }
}

int atoi(const char *nptr) {
  return (int)strtol(nptr, (char **)NULL, 10);
}

static uint32_t parse_hex_color(const char *hex) {
  if (hex[0] == '#') hex++;
  int len = strlen(hex);
  uint32_t val = (uint32_t)strtoll(hex, NULL, 16);
  if (len == 6) {
    // RRGGBB -> FFRRGGBB
    return 0xFF000000 | val;
  } else if (len == 8) {
    // RRGGBBAA -> AARRGGBB
    uint8_t r = (val >> 24) & 0xFF;
    uint8_t g = (val >> 16) & 0xFF;
    uint8_t b = (val >> 8) & 0xFF;
    uint8_t a = val & 0xFF;
    return (a << 24) | (r << 16) | (g << 8) | b;
  }
  return val;
}

// Window Management
typedef struct window_struct {
  int x, y, w, h;
  int old_w, old_h;
  int old_is_resizing_enabled;
  char title[64];
  warp_context_t *warp_ctx;
  warp1_context_t *warp1_ctx;
  int is_warp1;
  unsigned char *rgba_buffer;
  int buffer_w, buffer_h;
  int is_dirty;
  int is_dragging;
  int is_resizing;
  int resize_mode; // 1:BR, 2:BL, 3:TR, 4:TL
  int is_movable;
  int is_resizing_enabled;
  int is_always_full_res;
  int is_sticky;
  uint32_t background_color;
  int force_dark;
  int resize_w, resize_h; // Frozen dimensions during resize
  float fade_alpha;      // Fade to white: 0.0 (content) to 1.0 (white)
  int is_calculating;    // Calculation state after resize
  float scroll_x, scroll_y;
  int no_decoration;
  int is_menubar;
  int is_slider_dragging;

  // Caching for performance
  uint8_t *shadow_cache;   // Alpha mask for the shadow
  int shadow_cache_w, shadow_cache_h;
  uint32_t *frame_cache;   // Title bar + rounded corners frame
  int frame_cache_w, frame_cache_h;
  uint8_t *window_mask;    // Alpha mask for the entire window shape (squircle)
  
  // SVG caching for performance
  char *last_svg_str;      // Cached SVG string to detect changes
  NSVGimage *svg_image_cache; // Parsed SVG cache reused across rerasterization
  uint32_t *raster_cache;  // Rasterized SVG cache
  int raster_cache_w, raster_cache_h;
  float render_scale;      // Scale at which the buffer was rendered
  void *dynamic_file_ptr;  // Pointer to on-demand loaded file data from storage
  uint32_t interaction_refresh_until_tick;

  // Text overlay cache
  uint32_t *text_overlay_cache;
  int text_overlay_cache_w, text_overlay_cache_h;
  float text_overlay_last_scroll_y;
  scroll_input_state_t scroll_input;

  // Blur cache
  uint32_t *blur_cache;
  int blur_cache_cols, blur_cache_rows;
  int blur_last_x, blur_last_y, blur_last_w, blur_last_h;
} window_t;

static float get_window_context_scroll_y(window_t *win) {
  if (!win) return 0.0f;
  if (win->is_warp1 && win->warp1_ctx) return warp1_context_get_scroll_y(win->warp1_ctx);
  if (win->warp_ctx) return warp_context_get_scroll_y(win->warp_ctx);
  return 0.0f;
}

static int get_window_content_height(window_t *win) {
  if (!win) return 0;
  if (win->is_warp1 && win->warp1_ctx) return warp1_context_get_content_height(win->warp1_ctx);
  if (win->warp_ctx) return warp_context_get_content_height(win->warp_ctx);
  return 0;
}

static void set_window_context_scroll_y(window_t *win, float scroll_y) {
  if (!win) return;
  if (win->is_warp1 && win->warp1_ctx) {
    warp1_context_set_scroll_y(win->warp1_ctx, scroll_y);
  } else if (win->warp_ctx) {
    warp_context_set_scroll_y(win->warp_ctx, scroll_y);
  }
}

static void sync_window_scroll_from_context(window_t *win) {
  if (!win) return;
  float scroll_y = get_window_context_scroll_y(win);
  scroll_y = clamp_scroll_offset(scroll_y, win->h, get_window_content_height(win));
  set_window_context_scroll_y(win, scroll_y);
  win->scroll_y = scroll_y;
}

static int apply_window_scroll_delta(window_t *win, float delta) {
  if (!win || delta == 0.0f) return 0;
  float current = get_window_context_scroll_y(win);
  float next = clamp_scroll_offset(current + delta, win->h, get_window_content_height(win));
  if (fabsf(next - current) < 0.01f) return 0;
  set_window_context_scroll_y(win, next);
  win->scroll_y = next;
  return 1;
}

static int g_critical_error_mode = 0;
static const char* g_error_svg = 
"<svg width=\"1280\" height=\"720\" viewBox=\"0 0 1280 720\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"1280\" height=\"720\" fill=\"black\"/><path d=\"M700.469 332.006C702.695 332.006 704.5 333.797 704.5 336.006C704.5 338.215 702.695 340.006 700.469 340.006H579.531C577.305 340.006 575.5 338.215 575.5 336.006C575.5 333.797 577.305 332.006 579.531 332.006H700.469ZM670.636 244.006C679.667 244.006 684.186 244.004 687.635 245.748C690.668 247.282 693.136 249.731 694.682 252.741C696.439 256.163 696.438 260.647 696.438 269.608V298.405C696.438 307.365 696.439 311.849 694.682 315.272L694.06 316.373C692.511 318.879 690.289 320.922 687.635 322.264L686.281 322.834C682.982 324.002 678.537 324.006 670.636 324.006H609.364L603.38 323.983C599.014 323.915 596.075 323.668 593.719 322.834L592.365 322.264C589.711 320.922 587.489 318.879 585.94 316.373L585.318 315.272C583.561 311.849 583.562 307.365 583.562 298.405V269.608C583.562 260.647 583.561 256.163 585.318 252.741C586.864 249.731 589.332 247.282 592.365 245.748C594.952 244.441 598.14 244.111 603.38 244.03L609.364 244.006H670.636ZM609.364 252.006C604.716 252.006 601.712 252.016 599.428 252.201C597.238 252.379 596.425 252.68 596.026 252.881C594.511 253.648 593.28 254.87 592.507 256.373C592.304 256.769 592.001 257.576 591.822 259.748C591.635 262.015 591.625 264.995 591.625 269.608V298.405C591.625 303.017 591.635 305.997 591.822 308.264C592.001 310.437 592.304 311.243 592.507 311.639C593.28 313.143 594.511 314.364 596.026 315.131C596.425 315.333 597.238 315.633 599.428 315.811C601.712 315.996 604.716 316.006 609.364 316.006H670.636C675.284 316.006 678.288 315.996 680.572 315.811C682.762 315.633 683.575 315.333 683.974 315.131C685.489 314.364 686.72 313.143 687.493 311.639C687.696 311.243 687.999 310.437 688.178 308.264C688.365 305.997 688.375 303.017 688.375 298.405V269.608C688.375 264.995 688.365 262.015 688.178 259.748C687.999 257.576 687.696 256.769 687.493 256.373C686.72 254.87 685.489 253.648 683.974 252.881C683.575 252.68 682.762 252.379 680.572 252.201C678.288 252.016 675.284 252.006 670.636 252.006H609.364ZM653.275 265.178C654.849 263.616 657.401 263.616 658.975 265.178C660.549 266.74 660.549 269.272 658.975 270.834L645.7 284.006L658.975 297.178C660.549 298.74 660.549 301.272 658.975 302.834C657.401 304.396 654.849 304.396 653.275 302.834L640 289.662L626.725 302.834C625.151 304.396 622.599 304.396 621.025 302.834C619.45 301.272 619.45 298.74 621.025 297.178L634.3 284.006L621.025 270.834C619.45 269.272 619.45 266.74 621.025 265.178C622.599 263.616 625.151 263.616 626.725 265.178L640 278.35L653.275 265.178Z\" fill=\"white\"/><path d=\"M553.748 377.657H572.002\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M557.094 384.046C557.5 386.784 558.311 394.755 558.311 404.734\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M568.351 384.35C568.148 385.466 567.499 391.044 566.526 404.43\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M552.531 405.647H572.61\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M581.434 374.31V408.994\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M588.127 392.26H582.346\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M622.202 374.006V391.348\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M681.833 374.006V392.869\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M673.314 383.742H680.92\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M656.581 394.999V406.864H666.621V394.999\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M657.19 400.171H666.013\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M677.574 394.39C676.357 404.43 670.576 407.168 669.968 407.777\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M676.661 399.258C677.574 402.605 683.902 407.107 684.876 408.081\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M595.125 394.694H621.898V400.779H597.254V406.864H624.027\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M603.338 375.506C608.081 375.506 611.575 378.83 611.575 382.525C611.575 386.219 608.081 389.543 603.338 389.543C598.596 389.543 595.103 386.219 595.103 382.525C595.103 378.83 598.596 375.506 603.338 375.506Z\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M662.361 375.506C666.528 375.506 669.989 379.011 669.989 383.438C669.989 387.865 666.528 391.369 662.361 391.369C658.195 391.369 654.734 387.865 654.734 383.438C654.734 379.011 658.195 375.506 662.361 375.506Z\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M722.602 381.004C722.602 382.525 722.602 385.567 709.824 385.567C697.045 385.567 697.045 382.525 697.045 381.004C697.045 379.482 697.045 376.44 709.824 376.44C722.602 376.44 722.602 379.736 722.602 381.004Z\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M691.873 391.956H727.47\" stroke=\"white\" stroke-width=\"3\"/><rect x=\"697.328\" y=\"398.628\" width=\"25.5987\" height=\"8.56116\" stroke=\"white\" stroke-width=\"3\"/><path d=\"M617 459.294C617 453.41 617 450.468 618.199 448.245C619.126 446.528 620.534 445.12 622.251 444.193C624.474 442.994 627.416 442.994 633.3 442.994H646.7C652.584 442.994 655.526 442.994 657.749 444.193C659.466 445.12 660.874 446.528 661.801 448.245C663 450.468 663 453.41 663 459.294V459.694C663 465.578 663 468.52 661.801 470.742C660.874 472.46 659.466 473.868 657.749 474.795C655.526 475.994 652.584 475.994 646.7 475.994H633.3C627.416 475.994 624.474 475.994 622.251 474.795C620.534 473.868 619.126 472.46 618.199 470.742C617 468.52 617 465.578 617 459.694V459.294Z\" stroke=\"#6F6F6F\" stroke-width=\"3\"/><path d=\"M649.149 462.661L641.547 470.068L639.453 467.92L643.311 464.161H638C633.5 464.161 632.5 461.698 632.5 458.661V449.994H635.5V458.661C635.5 460.041 635.5 461.161 638 461.161H643.312L639.453 457.401L641.547 455.253L649.149 462.661Z\" fill=\"white\"/></svg>";

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
  if (strcmp(key, "~~dev/scrollSpeed") == 0) {
    static char scroll_speed_buf[16];
    snprintf(scroll_speed_buf, sizeof(scroll_speed_buf), "%d", g_dev_scroll_speed);
    return scroll_speed_buf;
  }

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

static int parse_json_int_or_string(const char *p, int *out_value) {
    if (!p || !out_value) return 0;

    p = json_skip_ws(p);
    if (*p == '\"') p++;

    char num_buf[16];
    int len = 0;
    while (*p && len < (int)(sizeof(num_buf) - 1)) {
        if (*p < '0' || *p > '9') break;
        num_buf[len++] = *p++;
    }
    num_buf[len] = '\0';

    if (len == 0) return 0;
    *out_value = atoi(num_buf);
    return 1;
}


static void parse_os_settings() {
  g_os_settings_found = (g_os_settings_ptr != NULL && g_os_settings_size > 0);
  
  if (!g_os_settings_found) {
      g_critical_error_mode = 1;
      set_w1_global("--warpSystemLog", "CRITICAL: os_settings.json NOT FOUND.");
      return;
  }

  const char* buf = g_os_settings_ptr;
  set_w1_global("--warpSystemLog", "SettingsLoaded.");
  
  
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

          // Fallback to reading from the filesystem if not found in initrd modules
          if (!g_wallpaper_found) {
              uint32_t size = 0;
              void *file_ptr = fs_read_file(wp_name, &size);
              if (file_ptr) {
                  g_wallpaper_ptr = file_ptr;
                  g_wallpaper_size = size;
                  g_wallpaper_found = 1;
                  set_w1_global("--warpSystemLog", "WallpaperSetFromStorage.");
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

  const char *scroll_key = strstr(buf, "\"scrollSpeed\"");
  if (scroll_key) {
    const char *p = json_skip_ws(scroll_key + 13);
    if (*p == ':') {
      int scroll_speed = 0;
      p = json_skip_ws(p + 1);
      if (parse_json_int_or_string(p, &scroll_speed)) {
        if (scroll_speed < 1) scroll_speed = 1;
        if (scroll_speed > 64) scroll_speed = 64;

        char scroll_speed_buf[16];
        snprintf(scroll_speed_buf, sizeof(scroll_speed_buf), "%d", scroll_speed);
        set_w1_global("~~dev/scrollSpeed", scroll_speed_buf);
      }
    }
  }

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
  
  // 設定ファイルが読めたかどうかは fs_read_file の結果を維持する
}

// FPU有効化
void enable_fpu() {
#ifdef __aarch64__
  // Handled in boot_arm64.s
#else
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
#endif
}

// ソフトウェア浮動小数点のヘルパー関数をハードウェアFPU(インラインアセンブリ)で実装
#ifndef __aarch64__
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
  __asm__("fildl %1; fstps %0" : "=m"(r) : "m"(i));
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
#endif

// レイヤー用
static uint32_t main_screen_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
static uint32_t desktop_composite_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
#define BLUR_W (SCREEN_WIDTH / 2)
#define BLUR_H (SCREEN_HEIGHT / 2)

// GPU blur context
// static gpu_blur_context_t g_gpu_blur_ctx;
// static int g_gpu_blur_initialized = 0;

static int desktop_composite_dirty = 1;
static int desktop_composite_last_active_index = -1;
static uint32_t *svg_buf = NULL; // 動的確保に変更
static uint32_t svg_base_buf[SVG_WIDTH * SVG_HEIGHT];
static uint32_t blink_buf[50 * 50];
#define HUD_W 320
#define HUD_H_MAX 240
static uint32_t hud_buf[HUD_W * HUD_H_MAX];
static int g_hud_current_h = 64;
#define WINDOW_BLUR_SAMPLE_SIZE 60
#define WINDOW_BLUR_SPACING 15
// 文字レイヤー (透過処理用)
#define TEXT_LAYER_W SCREEN_WIDTH
#define TEXT_LAYER_H SCREEN_HEIGHT
static uint32_t text_layer_buf[TEXT_LAYER_W * TEXT_LAYER_H];
// stbtt フォント
static stbtt_fontinfo g_font;
static stbtt_fontinfo g_emoji_font;
static int g_font_ready = 0;
static int g_emoji_font_ready = 0;
static const char *g_font_error = NULL;
static const char *g_emoji_font_error = NULL;

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

char *strcpy(char *dst, const char *src) {
  char *d = dst;
  while ((*d++ = *src++))
    ;
  return dst;
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

size_t strspn(const char *s, const char *accept) {
  const char *p;
  const char *a;
  size_t count = 0;
  for (p = s; *p != '\0'; ++p) {
    for (a = accept; *a != '\0'; ++a) {
      if (*p == *a)
        break;
    }
    if (*a == '\0')
      return count;
    ++count;
  }
  return count;
}

size_t strcspn(const char *s, const char *reject) {
  const char *p;
  const char *r;
  size_t count = 0;
  for (p = s; *p != '\0'; ++p) {
    for (r = reject; *r != '\0'; ++r) {
      if (*p == *r)
        return count;
    }
    ++count;
  }
  return count;
}

char *strpbrk(const char *s, const char *accept) {
  while (*s != '\0') {
    const char *a = accept;
    while (*a != '\0') {
      if (*a++ == *s)
        return (char *)s;
    }
    ++s;
  }
  return NULL;
}

char *strtok(char *str, const char *delim) {
  static char *last;
  if (str == NULL)
    str = last;
  if (str == NULL)
    return NULL;

  str += strspn(str, delim);
  if (*str == '\0') {
    last = NULL;
    return NULL;
  }

  char *token = str;
  str = strpbrk(token, delim);
  if (str == NULL) {
    last = NULL;
  } else {
    *str = '\0';
    last = str + 1;
  }
  return token;
}

char *strerror(int errnum) {
  (void)errnum;
  return "Unknown error";
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

int memcmp(const void *s1, const void *s2, size_t n) {
  const unsigned char *p1 = (const unsigned char *)s1;
  const unsigned char *p2 = (const unsigned char *)s2;
  while (n--) {
    if (*p1 != *p2)
      return *p1 - *p2;
    p1++;
    p2++;
  }
  return 0;
}

void *memmove(void *dest, const void *src, size_t n) {
  unsigned char *d = (unsigned char *)dest;
  const unsigned char *s = (const unsigned char *)src;
  if (d < s) {
    while (n--)
      *d++ = *s++;
  } else {
    d += n;
    s += n;
    while (n--)
      *--d = *--s;
  }
  return dest;
}

void *memchr(const void *s, int c, size_t n) {
  const unsigned char *p = (const unsigned char *)s;
  while (n--) {
    if (*p == (unsigned char)c)
      return (void *)p;
    p++;
  }
  return NULL;
}

long strtol(const char *nptr, char **endptr, int base) {
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

  if (base == 0) {
    if (*s == '0') {
      if (s[1] == 'x' || s[1] == 'X') {
        base = 16;
        s += 2;
      } else {
        base = 8;
        s++;
      }
    } else {
      base = 10;
    }
  } else if (base == 16) {
    if (*s == '0' && (s[1] == 'x' || s[1] == 'X')) {
      s += 2;
    }
  }

  long val = 0;
  while (*s) {
    int digit;
    if (*s >= '0' && *s <= '9')
      digit = *s - '0';
    else if (*s >= 'a' && *s <= 'f')
      digit = *s - 'a' + 10;
    else if (*s >= 'A' && *s <= 'F')
      digit = *s - 'A' + 10;
    else
      break;

    if (digit >= base)
      break;

    val = val * base + digit;
    s++;
  }
  if (endptr)
    *endptr = (char *)s;
  return val * sign;
}

long long strtoll(const char *nptr, char **endptr, int base) {
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

  if (base == 0) {
    if (*s == '0') {
      if (s[1] == 'x' || s[1] == 'X') {
        base = 16;
        s += 2;
      } else {
        base = 8;
        s++;
      }
    } else {
      base = 10;
    }
  } else if (base == 16) {
    if (*s == '0' && (s[1] == 'x' || s[1] == 'X')) {
      s += 2;
    }
  }

  long long val = 0;
  while (*s) {
    int digit;
    if (*s >= '0' && *s <= '9')
      digit = *s - '0';
    else if (*s >= 'a' && *s <= 'f')
      digit = *s - 'a' + 10;
    else if (*s >= 'A' && *s <= 'F')
      digit = *s - 'A' + 10;
    else
      break;

    if (digit >= base)
      break;

    val = val * base + digit;
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

float powf(float base, float exp) {
  return (float)pow((double)base, (double)exp);
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

int vsnprintf(char *str, size_t size, const char *format, va_list args) {
  int n = 0;
  const char *p = format;
  while (*p && (size_t)n < size - 1) {
    if (*p == '%') {
      p++;
      if (*p == 's') {
        const char *s = va_arg(args, const char *);
        if (!s)
          s = "(null)";
        while (*s && (size_t)n < size - 1)
          str[n++] = *s++;
      } else if (*p == 'd') {
        int d = va_arg(args, int);
        if (d < 0) {
          if ((size_t)n < size - 1)
            str[n++] = '-';
          d = -d;
        }
        char buf[16];
        int i = 0;
        if (d == 0)
          buf[i++] = '0';
        while (d > 0) {
          buf[i++] = (d % 10) + '0';
          d /= 10;
        }
        while (i > 0 && (size_t)n < size - 1)
          str[n++] = buf[--i];
      } else if (*p == 'x') {
        unsigned int x = va_arg(args, unsigned int);
        char buf[16];
        int i = 0;
        if (x == 0)
          buf[i++] = '0';
        while (x > 0) {
          int r = x % 16;
          buf[i++] = (r < 10) ? (r + '0') : (r - 10 + 'a');
          x /= 16;
        }
        while (i > 0 && (size_t)n < size - 1)
          str[n++] = buf[--i];
      } else if (*p == '%') {
        str[n++] = '%';
      }
      p++;
    } else {
      str[n++] = *p++;
    }
  }
  str[n] = '\0';
  return n;
}

int snprintf(char *str, size_t size, const char *format, ...) {
  va_list args;
  va_start(args, format);
  int n = vsnprintf(str, size, format, args);
  va_end(args);
  return n;
}

int sprintf(char *str, const char *format, ...) {
  va_list args;
  va_start(args, format);
  int n = vsnprintf(str, 1024 * 64, format, args); // Use a large buffer size
  va_end(args);
  return n;
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

#ifdef __SSE2__
static inline __m128i div255_epu16_sse2(__m128i v) {
  v = _mm_add_epi16(v, _mm_set1_epi16(128));
  v = _mm_add_epi16(v, _mm_srli_epi16(v, 8));
  return _mm_srli_epi16(v, 8);
}

static inline __m128i replicate_alpha_words_sse2(__m128i px) {
  px = _mm_shufflelo_epi16(px, _MM_SHUFFLE(3, 3, 3, 3));
  return _mm_shufflehi_epi16(px, _MM_SHUFFLE(3, 3, 3, 3));
}

static inline __m128i swap_rb_words_sse2(__m128i px) {
  px = _mm_shufflelo_epi16(px, _MM_SHUFFLE(3, 0, 1, 2));
  return _mm_shufflehi_epi16(px, _MM_SHUFFLE(3, 0, 1, 2));
}
#endif

static inline void fill_u32_span(uint32_t *dst, int count, uint32_t color) {
#ifdef __SSE2__
  __m128i packed = _mm_set1_epi32((int)color);
  int x = 0;
  for (; x + 4 <= count; x += 4) {
    _mm_storeu_si128((__m128i *)(dst + x), packed);
  }
  for (; x < count; ++x) {
    dst[x] = color;
  }
#else
  for (int x = 0; x < count; ++x) {
    dst[x] = color;
  }
#endif
}

static inline uint32_t blend_rgba_over_opaque_bg_scalar(const unsigned char *rgba,
                                                        uint32_t bg,
                                                        uint8_t bg_r,
                                                        uint8_t bg_g,
                                                        uint8_t bg_b) {
  uint8_t a = rgba[3];
  if (a == 0) {
    return bg;
  }
  if (a == 255) {
    return (0xFFu << 24) | ((uint32_t)rgba[0] << 16) |
           ((uint32_t)rgba[1] << 8) | (uint32_t)rgba[2];
  }

  uint8_t out_r = (uint8_t)((rgba[0] * a + bg_r * (255 - a)) / 255);
  uint8_t out_g = (uint8_t)((rgba[1] * a + bg_g * (255 - a)) / 255);
  uint8_t out_b = (uint8_t)((rgba[2] * a + bg_b * (255 - a)) / 255);
  return (0xFFu << 24) | ((uint32_t)out_r << 16) | ((uint32_t)out_g << 8) |
         (uint32_t)out_b;
}

#ifdef __SSE2__
static void blend_rgba_span_over_opaque_bg_sse2(uint32_t *dst,
                                                const unsigned char *src,
                                                int count, uint32_t bg,
                                                uint8_t bg_r, uint8_t bg_g,
                                                uint8_t bg_b) {
  const __m128i zero = _mm_setzero_si128();
  const __m128i bg_vec =
      _mm_setr_epi16(bg_r, bg_g, bg_b, 255, bg_r, bg_g, bg_b, 255);
  const __m128i alpha_max = _mm_set1_epi16(255);
  const __m128i rgb_mask =
      _mm_setr_epi16(0xFFFF, 0xFFFF, 0xFFFF, 0, 0xFFFF, 0xFFFF, 0xFFFF, 0);
  const __m128i alpha_fill =
      _mm_setr_epi16(0, 0, 0, 255, 0, 0, 0, 255);

  int x = 0;
  for (; x + 4 <= count; x += 4) {
    __m128i rgba = _mm_loadu_si128((const __m128i *)(src + x * 4));
    __m128i lo = _mm_unpacklo_epi8(rgba, zero);
    __m128i hi = _mm_unpackhi_epi8(rgba, zero);

    __m128i alpha_lo = replicate_alpha_words_sse2(lo);
    __m128i alpha_hi = replicate_alpha_words_sse2(hi);
    __m128i inv_alpha_lo = _mm_sub_epi16(alpha_max, alpha_lo);
    __m128i inv_alpha_hi = _mm_sub_epi16(alpha_max, alpha_hi);

    lo = _mm_add_epi16(_mm_mullo_epi16(lo, alpha_lo),
                       _mm_mullo_epi16(bg_vec, inv_alpha_lo));
    hi = _mm_add_epi16(_mm_mullo_epi16(hi, alpha_hi),
                       _mm_mullo_epi16(bg_vec, inv_alpha_hi));

    lo = div255_epu16_sse2(lo);
    hi = div255_epu16_sse2(hi);

    lo = swap_rb_words_sse2(lo);
    hi = swap_rb_words_sse2(hi);

    lo = _mm_or_si128(_mm_and_si128(lo, rgb_mask), alpha_fill);
    hi = _mm_or_si128(_mm_and_si128(hi, rgb_mask), alpha_fill);

    _mm_storeu_si128((__m128i *)(dst + x), _mm_packus_epi16(lo, hi));
  }

  for (; x < count; ++x) {
    dst[x] = blend_rgba_over_opaque_bg_scalar(src + x * 4, bg, bg_r, bg_g, bg_b);
  }
}
#endif

// GPU SVG texture
static gpu_resource_t g_svg_gpu_texture;
static int g_svg_gpu_ready = 0;
static gpu_svg_renderer_t g_gpu_svg_renderer;
static int g_gpu_svg_initialized = 0;

static void svg_render_full(layer_t *layer) {
  if (!g_svg_full_rgba)
    return;

  // GPU-accelerated SVG render: upload to GPU texture once, then blit
  if (g_gpu_available && !g_svg_gpu_ready) {
      gpu_create_resource(g_svg_full_w, g_svg_full_h, &g_svg_gpu_texture);
      gpu_upload_texture(&g_svg_gpu_texture, g_svg_full_rgba, 
                         g_svg_full_w * g_svg_full_h * 4);
      g_svg_gpu_ready = 1;
  }

  const uint32_t bg = BASE_BG_COLOR;
  uint8_t bg_r = (bg >> 16) & 0xFF;
  uint8_t bg_g = (bg >> 8) & 0xFF;
  uint8_t bg_b = bg & 0xFF;

  int scroll_x = (int)roundf(g_scroll_x);
  int scroll_y = (int)roundf(g_scroll_y);
  int visible_x0 = scroll_x;
  int visible_x1 = scroll_x + g_svg_full_w;
  if (visible_x0 < 0)
    visible_x0 = 0;
  if (visible_x1 > layer->width)
    visible_x1 = layer->width;
  if (visible_x0 > layer->width)
    visible_x0 = layer->width;
  if (visible_x1 < 0)
    visible_x1 = 0;

  for (int y = 0; y < layer->height; ++y) {
    uint32_t *line_dst = &layer->buffer[y * layer->width];
    int src_y = y - scroll_y;
    if (src_y < 0 || src_y >= g_svg_full_h) {
      fill_u32_span(line_dst, layer->width, bg);
      continue;
    }

    if (visible_x0 > 0) {
      fill_u32_span(line_dst, visible_x0, bg);
    }

    if (visible_x1 > visible_x0) {
      // GPU download with scroll offset (blit from GPU texture)
      if (g_svg_gpu_ready) {
          unsigned char *gpu_line = &((unsigned char*)g_svg_gpu_texture.cpu_ptr)[
              (src_y * g_svg_full_w + (visible_x0 - scroll_x)) * 4];
#ifdef __SSE2__
          blend_rgba_span_over_opaque_bg_sse2(line_dst + visible_x0, gpu_line,
                                              visible_x1 - visible_x0, bg, bg_r,
                                              bg_g, bg_b);
#else
          for (int x = visible_x0; x < visible_x1; ++x) {
              const unsigned char *rgba = gpu_line + (x - visible_x0) * 4;
              line_dst[x] = blend_rgba_over_opaque_bg_scalar(rgba, bg, bg_r, bg_g, bg_b);
          }
#endif
      } else {
          unsigned char *line_src =
              &g_svg_full_rgba[(src_y * g_svg_full_w + (visible_x0 - scroll_x)) * 4];
#ifdef __SSE2__
          blend_rgba_span_over_opaque_bg_sse2(line_dst + visible_x0, line_src,
                                              visible_x1 - visible_x0, bg, bg_r,
                                              bg_g, bg_b);
#else
          for (int x = visible_x0; x < visible_x1; ++x) {
              const unsigned char *rgba = line_src + (x - visible_x0) * 4;
              line_dst[x] =
                  blend_rgba_over_opaque_bg_scalar(rgba, bg, bg_r, bg_g, bg_b);
          }
#endif
      }
    }

    if (visible_x1 < layer->width) {
      fill_u32_span(line_dst + visible_x1, layer->width - visible_x1, bg);
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
    // "Center Cover" logic (with 103% zoom)
    float scale_x = (float)g_svg_full_w / g_svg_image->width;
    float scale_y = (float)g_svg_full_h / g_svg_image->height;
    scale = ((scale_x > scale_y) ? scale_x : scale_y) * 1.03f;
    tx = (g_svg_full_w - g_svg_image->width * scale) / 2.0f;
    ty = (g_svg_full_h - g_svg_image->height * scale) / 2.0f;
  } else {
    // Center logic for logo
    tx = (g_svg_full_w - g_svg_image->width) / 2.0f;
    ty = (g_svg_full_h - g_svg_image->height) / 2.0f;
  }

  // GPU SVG rendering: Bezier→Tessellation→GPU triangles
  if (g_gpu_available && !g_gpu_svg_initialized) {
      gpu_svg_init(&g_gpu_svg_renderer, g_svg_full_w, g_svg_full_h);
      g_gpu_svg_initialized = 1;
  }
  
  if (g_gpu_svg_initialized) {
      // GPU path render: Uses nanosvgrast for high-quality SVG rasterization
      gpu_svg_render(&g_gpu_svg_renderer, g_svg_image, scale, tx, ty,
                     (uint32_t*)g_svg_full_rgba, g_svg_full_w, g_svg_full_h);
  } else {
      // Fallback to nanosvg CPU rasterizer
      nsvgRasterize(g_svg_rast, g_svg_image, tx, ty, scale, g_svg_full_rgba,
                    g_svg_full_w, g_svg_full_h, g_svg_full_w * 4);
  }

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
          char msg[128];
          snprintf(msg, sizeof(msg), "INITRD: Found, size %d", ram_tar_size);
          set_w1_global("--warpSystemLog", msg);
          
          fs_format();
          const char *p = ram_tar_ptr;
          const char *end = ram_tar_ptr + ram_tar_size;
          int extracted_count = 0;
          while (p + 512 <= end) {
              tar_header_t *h = (tar_header_t *)p;
              if (h->name[0] == '\0') break;
              
              // macOSのメタデータファイル (._*) をスキップ
              if (h->name[0] == '.' && h->name[1] == '_') {
                  uint32_t skip_size = octal_to_int(h->size, 12);
                  p += 512 + ((skip_size + 511) & ~511);
                  continue;
              }

              uint32_t f_size = octal_to_int(h->size, 12);
              if (h->typeflag == '0' || h->typeflag == '\0') {
                  fs_write_file(h->name, p + 512, f_size);
                  extracted_count++;
              }
              p += 512 + ((f_size + 511) & ~511);
          }
          snprintf(msg, sizeof(msg), "INITRD: Extracted %d files to Disk.", extracted_count);
          set_w1_global("--warpSystemLog", msg);
          
          // RAMディスクの参照を即座に消す（これでメモリ計算から除外される）
          multiboot_module_t *mods = (multiboot_module_t *)(uintptr_t)mbi->mods_addr;
          for (uint32_t i = 0; i < mbi->mods_count; i++) {
              const char *s = (const char *)(uintptr_t)mods[i].string;
              if (s && (strstr(s, "initrd") || strstr(s, "tar"))) {
                  mods[i].mod_end = mods[i].mod_start;
              }
          }
      } else {
          set_w1_global("--warpSystemLog", "INITRD: Error - Module not found in Multiboot info.");
      }
  }

  // 3. ストレージ（またはインストール直後のディスク）から必要なファイルだけをロード
  g_warp_ptr = fs_read_file("main.warpc", &g_warp_size);
  g_terminal_warp_ptr = fs_read_file("terminal.warp", &g_terminal_warp_size);
  g_menubar_warp_ptr = fs_read_file("menubar.warp", &g_menubar_warp_size);
  g_bootlogo_ptr = fs_read_file("bootlogo.svg", &g_bootlogo_size);
  if (g_bootlogo_ptr) g_bootlogo_found = 1;
  g_os_settings_ptr = fs_read_file("os_settings.json", &g_os_settings_size);
  
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
      if (strcmp(fe->name, "main.warpc") == 0) g_warp_modules[g_warp_module_count].start = (uintptr_t)g_warp_ptr;
      else if (strcmp(fe->name, "terminal.warp") == 0) g_warp_modules[g_warp_module_count].start = (uintptr_t)g_terminal_warp_ptr;
      
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
  if (!cmd || !cmd[0]) return;

  char trimmed[256];
  strncpy(trimmed, cmd, 255);
  trimmed[255] = '\0';

  // 先頭と末尾の空白・改行を除去
  char *start_ptr = trimmed;
  while (*start_ptr == ' ' || *start_ptr == '\t' || *start_ptr == '\n' || *start_ptr == '\r') start_ptr++;
  char *end_ptr = start_ptr + strlen(start_ptr) - 1;
  while (end_ptr > start_ptr && (*end_ptr == ' ' || *end_ptr == '\t' || *end_ptr == '\n' || *end_ptr == '\r')) {
    *end_ptr = '\0';
    end_ptr--;
  }

  const char *file = NULL;
  if (strncmp(start_ptr, "warp ", 5) == 0) {
    file = start_ptr + 5;
  } else if (strncmp(start_ptr, "./", 2) == 0) {
    file = start_ptr + 2;
  } else if (strstr(start_ptr, ".warp") || strstr(start_ptr, ".warpc")) {
    file = start_ptr;
  }

  if (file) {
    // 引用符があれば除去する
    char filename[128];
    strncpy(filename, file, 127);
    filename[127] = '\0';
    
    char *f_ptr = filename;
    if (f_ptr[0] == '\"' || f_ptr[0] == '\'') {
        char q = f_ptr[0];
        f_ptr++;
        char *q_end = strrchr(f_ptr, q);
        if (q_end) *q_end = '\0';
    }

    // モジュールリストから検索
    int mod_idx = -1;
    for (uint32_t i = 0; i < g_warp_module_count; i++) {
      if (strcasecmp(g_warp_modules[i].name, f_ptr) == 0) {
        mod_idx = i;
        break;
      }
    }
    // 完全一致がなければ部分一致を探す（ただし ._ 隠しファイルは除外）
    if (mod_idx == -1) {
      for (uint32_t i = 0; i < g_warp_module_count; i++) {
        if (strstr(g_warp_modules[i].name, f_ptr) && g_warp_modules[i].name[0] != '.') {
          mod_idx = i;
          break;
        }
      }
    }

    if (mod_idx != -1) {
      const char *canonical_name = g_warp_modules[mod_idx].name;
      int is_warp1 = (strstr(canonical_name, ".warpc") == NULL);
      add_window(canonical_name, 200, 200, 640, 480, is_warp1);
    } else if (strcasecmp(f_ptr, "terminal.warp") == 0 || strcasecmp(f_ptr, "terminal") == 0) {
      add_window("Terminal", 200, 200, 600, 400, 1);
    } else if (strcasecmp(f_ptr, "menubar.warp") == 0 || strcasecmp(f_ptr, "topbar.warp") == 0 || strcasecmp(f_ptr, "menubar") == 0) {
      // menubar.warp が見つからない場合は topbar.warp を探す
      add_window("Menubar", 0, 0, 1280, 32, 1);
    } else {
      char err[512] = "Not found: ";
      strlcat(err, f_ptr, 511);
      set_w1_global("--warpSystemLog", err);
    }
  } else if (strcmp(start_ptr, "ls") == 0 || strcmp(start_ptr, "list") == 0) {
    char list_buf[512] = "Mods: ";
    for (uint32_t i = 0; i < g_warp_module_count; i++) {
      if (i > 0) strlcat(list_buf, ", ", 511);
      strlcat(list_buf, g_warp_modules[i].name, 511);
    }
    set_w1_global("--warpSystemLog", list_buf);
  } else if (strncmp(start_ptr, "os_delete_file:", 15) == 0) {
    // ファイル削除コマンド
    const char *filename = start_ptr + 15;
    char msg[256];
    // 実際には fs_delete_file(filename) などを実装する必要がある
    // 現在はログ出力のみ
    snprintf(msg, sizeof(msg), "Delete requested: %s", filename);
    set_w1_global("--warpSystemLog", msg);
  } else if (strncmp(start_ptr, "os_open_file:", 13) == 0) {
    // ファイルを開くコマンド
    const char *filename = start_ptr + 13;
    // 引用符を除去
    char clean_name[128];
    strncpy(clean_name, filename, 127);
    clean_name[127] = '\0';
    char *p = clean_name;
    if (p[0] == '"') {
      memmove(p, p+1, strlen(p));
      char *end = strrchr(p, '"');
      if (end) *end = '\0';
    }
    // モジュールリストから検索してウィンドウを開く
    int mod_idx = -1;
    for (uint32_t i = 0; i < g_warp_module_count; i++) {
      if (strcasecmp(g_warp_modules[i].name, clean_name) == 0) {
        mod_idx = i;
        break;
      }
    }
    if (mod_idx != -1) {
      const char *canonical_name = g_warp_modules[mod_idx].name;
      int is_warp1 = (strstr(canonical_name, ".warpc") == NULL);
      add_window(canonical_name, 200, 200, 640, 480, is_warp1);
      char msg[256];
      snprintf(msg, sizeof(msg), "Opened: %s", clean_name);
      set_w1_global("--warpSystemLog", msg);
    } else {
      char err[256];
      snprintf(err, sizeof(err), "File not found: %s", clean_name);
      set_w1_global("--warpSystemLog", err);
    }
  } else if (strncmp(start_ptr, "os_show_log:", 12) == 0) {
    // ログ表示コマンド
    const char *msg = start_ptr + 12;
    set_w1_global("--warpSystemLog", msg);
  } else if (strcmp(start_ptr, "vlock") == 0 || strcmp(start_ptr, "lock") == 0) {
    lock_state_enter();
    set_w1_global("--warpSystemLog", "Screen locked.");
  } else if (strcmp(start_ptr, "unlock") == 0) {
    lock_state_request_unlock();
    set_w1_global("--warpSystemLog", "Unlock requested.");
  } else if (strcmp(start_ptr, "reboot") == 0) {
    extern void sys_restart(void);
    sys_restart();
  } else if (strcmp(start_ptr, "exit") == 0) {
    close_active_window();
  } else if (strcmp(start_ptr, "help") == 0) {
    set_w1_global("--warpSystemLog", "Commands: <file.warp>, warp <file>, vlock, reboot, exit, help, ls");
  } else if (strncmp(start_ptr, "dev pointerCheck=", 17) == 0) {
    const char *val = start_ptr + 17;
    set_w1_global("~~dev/pointerCheck", (strcmp(val, "true") == 0) ? "true" : "false");
    strncpy(g_hud_status, g_dev_pointer_check ? "PtrCheck:ON" : "PtrCheck:OFF", 63);
  } else if (strncmp(start_ptr, "dev eventCheck=", 15) == 0) {
    const char *val = start_ptr + 15;
    set_w1_global("~~dev/eventCheck", (strcmp(val, "true") == 0) ? "true" : "false");
    strncpy(g_hud_status, g_dev_event_check ? "EvtCheck:ON" : "EvtCheck:OFF", 63);
  } else if (strncmp(start_ptr, "dev showHUD=", 12) == 0 || strncmp(start_ptr, "dev showhud=", 12) == 0) {
    const char *val = start_ptr + 12;
    set_w1_global("~~dev/showHUD", (strcmp(val, "true") == 0) ? "true" : "false");
    strncpy(g_hud_status, g_dev_show_hud ? "HUD:ON" : "HUD:OFF", 63);
  } else if (strncmp(start_ptr, "dev dark=", 9) == 0) {
    const char *val = start_ptr + 9;
    set_w1_global("~~json/main/dark", val);
    strncpy(g_hud_status, (strcmp(val, "true") == 0) ? "Dark:ON" : "Dark:OFF", 63);
    for (int i = 0; i < g_window_count; i++) {
      window_update_caches(&g_windows[i]);
      g_windows[i].is_dirty = 1;
    }
    g_svg_dirty = 1;
  } else if (strcmp(start_ptr, "storage sync") == 0) {
    if (!mbi_ptr) {
        set_w1_global("--warpSystemLog", "Error: No multiboot info");
        return;
    }
    ata_init();
    fs_init();
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
  } else if (strcmp(start_ptr, "storage ls") == 0) {
    ata_init();
    fs_init();
    fs_list_files();
  } else {
    // 未知のコマンド
    char err[256] = "Unknown: ";
    strlcat(err, start_ptr, 255);
    set_w1_global("--warpSystemLog", err);
  }
}

static void sync_all_window_themes() {
  static int last_is_dark = -1;
  const char *dark_val = get_w1_global("~~main/dark");
  int system_dark = (strcmp(dark_val, "true") == 0);

  if (system_dark != last_is_dark) {
    for (int i = 0; i < g_window_count; i++) {
      window_t *win = &g_windows[i];
      int win_dark = (win->force_dark != -1) ? win->force_dark : system_dark;
      const char *target = win_dark ? "true" : "false";

      if (win->is_warp1 && win->warp1_ctx) {
        warp1_context_set_state(win->warp1_ctx, "~~main/dark", target);
        warp1_context_mark_dirty(win->warp1_ctx);
      } else if (win->warp_ctx) {
        warp_context_set_state(win->warp_ctx, "~~main/dark", target);
      }

      window_update_caches(win);
      win->is_dirty = 1;
    }
    last_is_dark = system_dark;
  }
}
void set_w1_global(const char *key, const char *val) {
  if (strcmp(key, "~~json/main/dark") == 0) {
    set_w1_global("~~main/dark", val);
  }

  // Sync dev flags
  if (strcmp(key, "~~dev/pointerCheck") == 0) {
    g_dev_pointer_check = (strcmp(val, "true") == 0);
  } else if (strcmp(key, "~~dev/eventCheck") == 0) {
    g_dev_event_check = (strcmp(val, "true") == 0);
  } else if (strcmp(key, "~~dev/showHUD") == 0) {
    g_dev_show_hud = (strcmp(val, "true") == 0);
  } else if (strcmp(key, "~~dev/scrollSpeed") == 0) {
    int scroll_speed = atoi(val);
    if (scroll_speed < 1) scroll_speed = 1;
    if (scroll_speed > 64) scroll_speed = 64;
    g_dev_scroll_speed = scroll_speed;
  }

  int is_log = (strcmp(key, "--warpSystemLog") == 0);
  int theme_changed = (strcmp(key, "~~main/dark") == 0);

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
  win->buffer_w = 0;
  win->buffer_h = 0;
  win->raster_cache_w = 0;
  win->raster_cache_h = 0;
}

static uint32_t *build_window_text_overlay(window_t *win, int *out_w, int *out_h) {
  if (!win || (!win->warp_ctx && !win->warp1_ctx))
    return NULL;

  int title_h = win->no_decoration ? 0 : 60;
  int overlay_w = win->w;
  int overlay_h = win->h + title_h; // Full window area
  if (overlay_w <= 0 || overlay_h <= 0)
    return NULL;

  uint32_t *overlay = (uint32_t *)malloc((size_t)overlay_w * (size_t)overlay_h * sizeof(uint32_t));
  if (!overlay)
    return NULL;

  for (int i = 0; i < overlay_w * overlay_h; i++)
    overlay[i] = 0x00000000u;

  layer_t text_layer;
  text_layer.buffer = overlay;
  text_layer.width = overlay_w;
  text_layer.height = overlay_h;

  if (win->is_warp1) {
    warp1_context_draw_texts(win->warp1_ctx, &text_layer, 0, (int)(win->scroll_y * win->render_scale), win->render_scale);
  } else {
    warp_context_draw_texts(win->warp_ctx, &text_layer, 0, (int)(win->scroll_y * win->render_scale), win->render_scale);
  }

  if (out_w) *out_w = overlay_w;
  if (out_h) *out_h = overlay_h;
  return overlay;
}

static void window_update_caches(window_t *win) {
  float scale = win->render_scale;
  if (scale <= 0.0f) scale = 1.0f;

  int title_h = win->no_decoration ? 0 : 60;
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

  for (int i = 0; i < fw * fh; i++) win->frame_cache[i] = 0x00000000;

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
      layer_draw_ttf(&frame_l, (int)(111.0f * scale), (int)(23.0f * scale), header_text, 20.8f * scale, is_dark ? 0xFFEEEEEE : 0xFF333333);
      int ax = win->w - 16;
      for (int j = 0; j < action_count; j++) {
        char act_text[64];
        if (win->is_warp1) warp1_context_get_header_action_info(win->warp1_ctx, j, act_text, sizeof(act_text));
        else warp_context_get_header_action_info(win->warp_ctx, j, act_text, sizeof(act_text));
         int text_w = measure_ttf_width(act_text, 18.2f); 
         int btn_w = text_w + 32;
         int btn_h = 42; 
         ax -= btn_w;

        // Draw pill-shaped button background (capsule SDF, max corner radius)
        int bx = (int)((float)ax * scale);
        int by = (int)(14.0f * scale);
        int bw = (int)((float)btn_w * scale);
        int bh = (int)((float)btn_h * scale);
        float pr = bh / 2.0f;
        
        uint32_t marker_rgb = is_dark ? 0x444444 : 0xFFFFFF;
        for (int dy_i = 0; dy_i < bh; dy_i++) {
          for (int dx_i = 0; dx_i < bw; dx_i++) {
            float ffx = (float)dx_i + 0.5f;
            float ffy = (float)dy_i + 0.5f;
            float seg_x = (ffx < pr) ? pr : ((ffx > bw - pr) ? bw - pr : ffx);
            float ddx = ffx - seg_x;
            float ddy = ffy - pr;
            float dist_to_seg = sqrtf(ddx*ddx + ddy*ddy);
            float alpha_f = pr + 0.5f - dist_to_seg;
            if (alpha_f > 1.0f) alpha_f = 1.0f;
            if (alpha_f > 0.0f) {
              uint8_t a = (uint8_t)(alpha_f * 255.0f);
              // 内部は alpha=1 (レンズマーカー), 縁は alpha=2..255 (AAマーカー)
              uint8_t marker_a = (a >= 254) ? 1 : (a < 2 ? 2 : a);
              frame_l.buffer[(by + dy_i) * fw + (bx + dx_i)] = (marker_a << 24) | marker_rgb;
            }
          }
        }
        layer_draw_ttf(&frame_l, bx + (int)(14.0f * scale), by + (int)(8.0f * scale), act_text, 18.2f * scale, is_dark ? 0xFFEEEEEE : 0xFF000000);
        ax -= 10;
      }
    } else {
      layer_draw_ttf(&frame_l, (int)(111.0f * scale), (int)(20.0f * scale), win->title, 16.0f * scale, is_dark ? 0xFFEEEEEE : 0xFF333333);
    }

    // Control buttons - 42x42 capsule
    int ctrl_size = 42;
    int ctrl_y = 13;
    int ctrl_gap = 10;
    int ctrl_positions[] = {14, 14 + ctrl_size + ctrl_gap}; 
    uint32_t marker_rgb = is_dark ? 0x444444 : 0xFFFFFF;
    uint32_t ctrl_icon_color = is_dark ? 0xFFEEEEEE : 0xFF333333;
    for (int k = 0; k < 2; k++) {
      int bx = (int)((float)ctrl_positions[k] * scale);
      int by = (int)((float)ctrl_y * scale);
      int bw = (int)((float)ctrl_size * scale);
      int bh = (int)((float)ctrl_size * scale);
      float pr = bh / 2.0f;
      for (int dy_i = 0; dy_i < bh; dy_i++) {
        for (int dx_i = 0; dx_i < bw; dx_i++) {
          float ffx = (float)dx_i + 0.5f;
          float ffy = (float)dy_i + 0.5f;
          float seg_x = (ffx < pr) ? pr : ((ffx > bw - pr) ? bw - pr : ffx);
          float ddx = ffx - seg_x;
          float ddy = ffy - pr;
          float dist_to_seg = sqrtf(ddx*ddx + ddy*ddy);
          float alpha_f = pr + 0.5f - dist_to_seg;
          if (alpha_f > 1.0f) alpha_f = 1.0f;
          if (alpha_f > 0.0f) {
            uint8_t a = (uint8_t)(alpha_f * 255.0f);
            uint8_t marker_a = (a >= 254) ? 1 : (a < 2 ? 2 : a);
            frame_l.buffer[(by + dy_i) * fw + (bx + dx_i)] = (marker_a << 24) | marker_rgb;
          }
        }
      }
    }
    // Close button X icon (two 45-degree lines with AA)
    {
      float cx = (float)(ctrl_positions[0] + ctrl_size / 2) * scale;
      float cy = (float)(ctrl_y + ctrl_size / 2) * scale;
      float h = 4.5f * scale; // half size (1.3x)
      float stroke_r = 1.0f * scale;
      int extent = (int)(h + stroke_r + 2.0f);
      for (int dy = -extent; dy <= extent; dy++) {
        for (int dx = -extent; dx <= extent; dx++) {
          float px = cx + (float)dx;
          float py = cy + (float)dy;
          if ((int)px < 0 || (int)px >= fw || (int)py < 0 || (int)py >= fh) continue;

          // Line 1: 45 degrees (cx-h, cy-h) to (cx+h, cy+h)
          float p1x = cx - h, p1y = cy - h, p2x = cx + h, p2y = cy + h;
          float d1 = dist_to_line_segment(px, py, p1x, p1y, p2x, p2y);

          // Line 2: -45 degrees (cx-h, cy+h) to (cx+h, cy-h)
          float p3x = cx - h, p3y = cy + h, p4x = cx + h, p4y = cy - h;
          float d2 = dist_to_line_segment(px, py, p3x, p3y, p4x, p4y);

          float min_d = (d1 < d2) ? d1 : d2;
          float alpha_f = stroke_r + 0.5f - min_d;
          if (alpha_f > 1.0f) alpha_f = 1.0f;
          if (alpha_f > 0.0f) {
            int ipx = (int)px, ipy = (int)py;
            frame_l.buffer[ipy * fw + ipx] = blend_colors(frame_l.buffer[ipy * fw + ipx], ctrl_icon_color, (uint8_t)(alpha_f * 255.0f));
          }
        }
      }
    }
    // Maximize button - icon (one horizontal line)
    {
      float cx = (float)(ctrl_positions[1] + ctrl_size / 2) * scale;
      float cy = (float)(ctrl_y + ctrl_size / 2) * scale;
      float h = 4.5f * scale; // half size
      float stroke_r = 1.0f * scale;
      int extent = (int)(h + stroke_r + 2.0f);
      for (int dy = -extent; dy <= extent; dy++) {
        for (int dx = -extent; dx <= extent; dx++) {
          float px = cx + (float)dx;
          float py = cy + (float)dy;
          if ((int)px < 0 || (int)px >= fw || (int)py < 0 || (int)py >= fh) continue;

          // Horizontal line: (cx-h, cy) to (cx+h, cy)
          float p1x = cx - h, p1y = cy, p2x = cx + h, p2y = cy;
          float d = dist_to_line_segment(px, py, p1x, p1y, p2x, p2y);

          float alpha_f = stroke_r + 0.5f - d;
          if (alpha_f > 1.0f) alpha_f = 1.0f;
          if (alpha_f > 0.0f) {
            int ipx = (int)px, ipy = (int)py;
            frame_l.buffer[ipy * fw + ipx] = blend_colors(frame_l.buffer[ipy * fw + ipx], ctrl_icon_color, (uint8_t)(alpha_f * 255.0f));
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
    float r = 40.0f; // Corner radius (0.5px inward correction)
    for (int y = 0; y < mh; y++) {
      float fy = (float)y / scale + 0.5f; 
      for (int x = 0; x < mw; x++) {
        float fx = (float)x / scale + 0.5f;
        float dx = fabsf(fx - rw/2.0f) - (rw/2.0f - r);
        float dy = fabsf(fy - rh/2.0f) - (rh/2.0f - r);

        float dist;
        if (dx > 0 && dy > 0) {
          // Blend L4 (squircle) and L2 (circle) for softer squircle effect
          float l2 = sqrtf(dx*dx + dy*dy);
          float l4 = sqrtf(sqrtf(dx*dx*dx*dx + dy*dy*dy*dy));
          dist = (l2 * 0.4f + l4 * 0.6f) - r;
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

  float target_scale = 1.0f;

  // If resolution scale changed, force update
  if (win->render_scale != target_scale) {
    win->is_dirty = 1;
    win->render_scale = target_scale;
    window_update_caches(win);
  }

  // ★ 重要な変更: リサイズ(layout) vs スクロール(render)を区別
  int needs_layout_update = 0;
  if (win->is_warp1) {
    needs_layout_update = win->is_dirty || (win->rgba_buffer == NULL);
  } else {
    needs_layout_update = win->is_dirty || (win->rgba_buffer == NULL);
  }

  if (needs_layout_update) {
    strncpy(g_hud_status, "LayoutUpdate", 63);
    int title_h = win->no_decoration ? 0 : 60;
    const char *has_header_str = (title_h > 0) ? "true" : "false";
    const char *minimized_str = (win->w == 300 && win->h == 240) ? "true" : "false";
    if (win->is_warp1) {
      warp1_context_set_state(win->warp1_ctx, "~~internal/has_header", has_header_str);
      warp1_context_set_state(win->warp1_ctx, "~~internal/minimized", minimized_str);
      warp1_context_update(win->warp1_ctx, win->w, win->h + title_h);
    } else {
      warp_context_set_state(win->warp_ctx, "~~internal/has_header", has_header_str);
      warp_context_set_state(win->warp_ctx, "~~internal/minimized", minimized_str);
      warp_context_update(win->warp_ctx, win->w, win->h + title_h);
    }
    win->is_dirty = 0; // レイアウト計算完了
  }

  sync_window_scroll_from_context(win);

  // Check if rendering is needed (engine_dirty flag)
  int needs_render = 0;
  if (win->is_warp1) {
    needs_render = warp1_context_is_dirty(win->warp1_ctx) || (win->rgba_buffer == NULL);
  } else {
    needs_render = warp_context_is_dirty(win->warp_ctx) || (win->rgba_buffer == NULL);
  }

  if (!needs_render) {
    strncpy(g_hud_status, "Cached", 63);
    win->is_calculating = 0;
    return;
  }

  strncpy(g_hud_status, "SVGGen", 63);
  const char *svg = win->is_warp1 ? warp1_context_get_svg(win->warp1_ctx) : warp_context_get_svg(win->warp_ctx);

  // Check if SVG string changed
  int svg_changed = 1;
  if (win->last_svg_str && strcmp(win->last_svg_str, svg) == 0) {
    svg_changed = 0;
  }

  if (svg_changed) {
    strncpy(g_hud_status, "NSVGParse", 63);
    NSVGimage *img = nsvgParse((char*)svg, "px", 96.0f);
    if (!img) {
      strncpy(g_hud_status, "ParseErr", 63);
      return;
    }

    if (win->svg_image_cache) nsvgDelete(win->svg_image_cache);
    win->svg_image_cache = img;

    if (win->last_svg_str) free(win->last_svg_str);
    size_t svg_len = strlen(svg);
    win->last_svg_str = (char*)malloc(svg_len + 1);
    if (win->last_svg_str) memcpy(win->last_svg_str, svg, svg_len + 1);
  }

  if (!win->svg_image_cache) {
    strncpy(g_hud_status, "ParseMiss", 63);
    return;
  }

  // Content height is determined by the cached SVG itself
  int title_h = win->no_decoration ? 0 : 60;
  int content_h = (int)win->svg_image_cache->height;
  if (content_h < win->h + title_h) content_h = win->h + title_h;

  int scaled_w = (int)((float)win->w * target_scale);
  int scaled_h = (int)((float)content_h * target_scale);
  if (scaled_w < 1) scaled_w = 1;
  if (scaled_h < 1) scaled_h = 1;

  int raster_size_changed = (!win->raster_cache || win->raster_cache_w != scaled_w || win->raster_cache_h != scaled_h);
  if (raster_size_changed) {
    if (win->raster_cache) free(win->raster_cache);
    win->raster_cache = (uint32_t *)malloc((size_t)scaled_w * (size_t)scaled_h * 4);
    win->raster_cache_w = scaled_w;
    win->raster_cache_h = scaled_h;
  }

  if (win->raster_cache && (svg_changed || raster_size_changed)) {
    strncpy(g_hud_status, "ClearCache", 63);
    for (int i = 0; i < scaled_w * scaled_h; i++) win->raster_cache[i] = 0x00000000;

    strncpy(g_hud_status, "NSVGRast", 63);
    if (!g_svg_rast) g_svg_rast = nsvgCreateRasterizer();
    nsvgRasterize(g_svg_rast, win->svg_image_cache, 0, 0, target_scale, (unsigned char*)win->raster_cache, scaled_w, scaled_h, scaled_w * 4);

    strncpy(g_hud_status, "RBSwap+PreMul", 63);
    unsigned char *p = (unsigned char*)win->raster_cache;
    for (int i = 0; i < scaled_w * scaled_h; i++) {
      uint8_t a = p[3];
      if (a == 255) {
        // 不透明：RGB入れ替えのみ
        uint8_t r = p[0], b = p[2];
        p[0] = b; p[2] = r;
      } else if (a == 0) {
        // 完全透明：クリア
        p[0] = 0; p[1] = 0; p[2] = 0;
      } else {
        // 乗算済みアルファ：(color * alpha) >> 8
        uint8_t r = p[0], g = p[1], b = p[2];
        p[0] = (uint8_t)((uint32_t)b * a >> 8);
        p[1] = (uint8_t)((uint32_t)g * a >> 8);
        p[2] = (uint8_t)((uint32_t)r * a >> 8);
      }
      p += 4;
    }
  }

  // Prepare RGBA buffer (Flatten content: SVG + Text)
  if (win->raster_cache) {
    if (!win->rgba_buffer || win->buffer_w != win->raster_cache_w || win->buffer_h != win->raster_cache_h) {
      if (win->rgba_buffer) free(win->rgba_buffer);
      win->rgba_buffer = (unsigned char *)malloc((size_t)win->raster_cache_w * (size_t)win->raster_cache_h * 4);
      win->buffer_w = win->raster_cache_w;
      win->buffer_h = win->raster_cache_h;
      win->is_dirty = 1; // Force redraw after allocation
    }

    if (svg_changed || win->is_dirty || needs_layout_update) {
      memcpy(win->rgba_buffer, win->raster_cache, (size_t)win->buffer_w * (size_t)win->buffer_h * 4);
      
      // ここでテキストもバッファに直接描き込む。これで「表示内容そのまま」が完成する。
      layer_t temp_l = { (uint32_t*)win->rgba_buffer, 0, 0, win->buffer_w, win->buffer_h, 0, 1, 0 };
      if (win->is_warp1) warp1_context_draw_texts(win->warp1_ctx, &temp_l, 0, 0, target_scale);
      else warp_context_draw_texts(win->warp_ctx, &temp_l, 0, 0, target_scale);
    }
  }

  // Keep caches for inactive windows to render same as active
  // No baking - use normal render path for both active and inactive

  win->is_dirty = 0;
  win->is_calculating = 0;
  strncpy(g_hud_status, "Idle", 63);
}

static void request_window_interaction_refresh(window_t *win) {
  if (!win) return;
  win->interaction_refresh_until_tick = timer_ticks + 20;
  win->is_dirty = 1;
  g_svg_dirty = 1;
}

typedef struct {
  const char *name;
  void (*init_func)(void *ctx);
} app_registry_t;

static const app_registry_t g_app_registry[] = {
  {NULL, NULL}
};

static void parse_baram_config(window_t *win, const char *code) {
  if (!code) return;
  const char *tag = "baram-os-config";
  const char *pos = strstr(code, tag);
  if (!pos) return;

  const char *p = strstr(pos, "{");
  if (!p) return;
  p++; // Skip '{'

  int brace_level = 1;
  while (*p && brace_level > 0) {
    while (*p && (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r' || *p == ',')) p++;
    if (!*p || *p == '}') break;

    // key:("value") の形式を簡易パース
    char key[64] = {0};
    int i = 0;
    while (*p && *p != ':' && *p != ' ' && i < 63) {
        if (*p == '}') break;
        key[i++] = *p++;
    }
    key[i] = '\0';

    while (*p && (*p == ':' || *p == ' ' || *p == '(' || *p == '\"')) p++;
    
    char val[64] = {0};
    i = 0;
    while (*p && *p != '\"' && *p != ')' && *p != ' ' && *p != ',' && *p != '}' && i < 63) val[i++] = *p++;
    val[i] = '\0';

    // 設定の適用
    if (strcmp(key, "height") == 0) win->h = atoi(val);
    else if (strcmp(key, "width") == 0) {
      if (strstr(val, "vw")) win->w = (atoi(val) * SCREEN_WIDTH) / 100;
      else win->w = atoi(val);
    }
    else if (strcmp(key, "left") == 0) win->x = atoi(val);
    else if (strcmp(key, "top") == 0) win->y = atoi(val);
    else if (strcmp(key, "showBar") == 0) win->no_decoration = (strcmp(val, "false") == 0);
    else if (strcmp(key, "move") == 0) win->is_movable = (strcmp(val, "true") == 0);
    else if (strcmp(key, "resize") == 0) win->is_resizing_enabled = (strcmp(val, "true") == 0); 
    else if (strcmp(key, "front") == 0) {
      win->is_always_full_res = (strcmp(val, "true") == 0);
      win->is_sticky = (strcmp(val, "true") == 0);
    }
    else if (strcmp(key, "background") == 0) win->background_color = parse_hex_color(val);
    else if (strcmp(key, "dark") == 0) win->force_dark = (strcmp(val, "true") == 0);
    else if (strcmp(key, "lua") == 0) {
      if (win->warp1_ctx) run_lua_script(win->warp1_ctx, val);
    }
    else if (strcmp(key, "c") == 0) {
      // App binding: Find and call the init function from registry
      for (int r = 0; g_app_registry[r].name != NULL; r++) {
        if (strcmp(g_app_registry[r].name, val) == 0) {
          if (win->warp1_ctx) g_app_registry[r].init_func(win->warp1_ctx);
          break;
        }
      }
    }

    // 次の項目へ
    while (*p && *p != ',' && *p != '}') {
        if (*p == '{') brace_level++;
        if (*p == '}') { brace_level--; if (brace_level <= 0) break; }
        p++;
    }
    if (*p == ',') p++;
  }
}

// External from files.c (Removed redundant extern, now at top with registry)

static void add_window(const char *title, int x, int y, int w, int h, int is_warp1) {
  if (g_window_count >= MAX_WINDOWS) return;

  void *dynamic_ptr = NULL;
  const char *buf_to_use = NULL;
  int previous_active = g_active_window_index;
  
  if (strcasecmp(title, "Terminal") == 0) {
    if (g_terminal_mod_found) buf_to_use = g_terminal_warp_ptr;
  } else if (strcasecmp(title, "Menubar") == 0) {
    if (g_menubar_mod_found) buf_to_use = g_menubar_warp_ptr;
  }

  if (!buf_to_use) {
    for (uint32_t i = 0; i < g_warp_module_count; i++) {
      // 完全一致または拡張子を含めた一致を確認し、._ で始まるファイルは除外する
      if (strcasecmp(g_warp_modules[i].name, title) == 0 || 
          (strstr(g_warp_modules[i].name, title) && g_warp_modules[i].name[0] != '.')) {
        if (g_warp_modules[i].start != 0) {
            buf_to_use = (const char *)(uintptr_t)g_warp_modules[i].start;
        } else {
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
  strncpy(win->title, title, 63);
  win->is_warp1 = is_warp1;
  win->dynamic_file_ptr = dynamic_ptr; // トラック

  if (is_warp1) {
    win->warp1_ctx = warp1_context_create(buf_to_use);
    if (!win->warp1_ctx) set_w1_global("--warpSystemLog", "Warp1: Context creation FAILED.");
    win->warp_ctx = NULL;
  } else {
    win->warp_ctx = warp_context_create(buf_to_use);
    if (!win->warp_ctx) set_w1_global("--warpSystemLog", "ClassicWarp: Context creation FAILED.");
    win->warp1_ctx = NULL;
  }

  win->rgba_buffer = NULL;
  win->buffer_w = 0;
  win->buffer_h = 0;
  win->last_svg_str = NULL;
  win->svg_image_cache = NULL;
  win->raster_cache = NULL;
  win->raster_cache_w = 0;
  win->raster_cache_h = 0;
  win->shadow_cache = NULL;
  win->frame_cache = NULL;
  win->window_mask = NULL;
  win->text_overlay_cache = NULL;
  win->text_overlay_cache_w = 0;
  win->text_overlay_cache_h = 0;
  win->text_overlay_last_scroll_y = -9999.0f;
  reset_scroll_input_state(&win->scroll_input);
  win->blur_cache = NULL;
  win->blur_cache_cols = 0;
  win->blur_cache_rows = 0;
  win->blur_last_x = -1; win->blur_last_y = -1; win->blur_last_w = -1; win->blur_last_h = -1;
  win->is_dirty = 1;
  win->is_dragging = 0;
  win->is_resizing = 0;
  win->is_slider_dragging = 0;
  win->fade_alpha = 0.0f;
  win->is_calculating = 0;
  win->render_scale = 1.0f;
  win->no_decoration = 0;
  win->is_menubar = 0;
  win->interaction_refresh_until_tick = 0;
  win->is_movable = 1; // Default
  win->is_resizing_enabled = 1; // Default
  win->is_always_full_res = 0; // Default
  win->is_sticky = 0; // Default
  win->background_color = 0xFFFFFFFF; // Default opaque white
  win->force_dark = -1; // -1 means follow system

  // Apply baram-os-config from source code if available
  parse_baram_config(win, buf_to_use);

  if (strstr(title, "Menubar") || strstr(title, "menubar")) {
    win->is_menubar = 1;
    win->no_decoration = 1;
    win->x = 0; win->y = 0; win->w = SCREEN_WIDTH; win->h = 32;
    win->is_warp1 = 1;
  }

  if (!win->is_sticky) g_active_window_index = g_window_count - 1;
  else g_active_window_index = previous_active;

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
  if (win->svg_image_cache) nsvgDelete(win->svg_image_cache);
  if (win->raster_cache) free(win->raster_cache);
  if (win->shadow_cache) free(win->shadow_cache);
  if (win->frame_cache) free(win->frame_cache);
  if (win->window_mask) free(win->window_mask);
  if (win->dynamic_file_ptr) free(win->dynamic_file_ptr);
  if (win->text_overlay_cache) free(win->text_overlay_cache);
  if (win->blur_cache) free(win->blur_cache);
  
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

static uint32_t sample_wallpaper_pixel(int x, int y) {
  if (!g_svg_ready || x < 0 || y < 0 || x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT)
    return BASE_BG_COLOR;
  return svg_base_buf[y * SCREEN_WIDTH + x];
}

static uint32_t get_window_background_color(window_t *win) {
  if (!win)
    return 0xFFF2F2F6u;
  if (win->is_menubar) return 0x00000000u;
  if (win->background_color != 0xFFFFFFFFu)
    return win->background_color;

  const char *dark_val = get_w1_global("~~main/dark");
  int system_dark = (strcmp(dark_val, "true") == 0);
  int win_dark = (win->force_dark != -1) ? win->force_dark : system_dark;
  return win_dark ? 0xFF000000u : 0xFFF2F2F6u;
}

static int window_index_of(window_t *target) {
  if (!target)
    return -1;
  for (int i = 0; i < g_window_count; i++) {
    if (&g_windows[i] == target)
      return i;
  }
  return -1;
}

static uint32_t blend_colors_fixed(uint32_t c0, uint32_t c1, int alpha255) {
  if (alpha255 <= 0) return c0;
  if (alpha255 >= 255) return c1;
  return blend_colors(c0, c1, (uint8_t)alpha255);
}

static uint32_t blend_rgb_over_opaque_premul(uint32_t bg, uint32_t fg_premul) {
  uint32_t alpha = (fg_premul >> 24) & 0xFFu;
  if (alpha == 0) return bg | 0xFF000000u;
  if (alpha == 255) return fg_premul | 0xFF000000u;

  uint32_t inv_alpha = 255 - alpha;
  uint32_t bg_r = (bg >> 16) & 0xFFu;
  uint32_t bg_g = (bg >> 8) & 0xFFu;
  uint32_t bg_b = bg & 0xFFu;

  uint32_t fg_r = (fg_premul >> 16) & 0xFFu;
  uint32_t fg_g = (fg_premul >> 8) & 0xFFu;
  uint32_t fg_b = fg_premul & 0xFFu;

  uint32_t out_r = fg_r + ((bg_r * inv_alpha) >> 8);
  uint32_t out_g = fg_g + ((bg_g * inv_alpha) >> 8);
  uint32_t out_b = fg_b + ((bg_b * inv_alpha) >> 8);

  if (out_r > 255) out_r = 255;
  if (out_g > 255) out_g = 255;
  if (out_b > 255) out_b = 255;

  return 0xFF000000u | (out_r << 16) | (out_g << 8) | out_b;
}

static uint32_t blend_rgb_over_opaque(uint32_t bg, uint32_t fg, uint8_t alpha) {
  if (alpha == 0) return bg | 0xFF000000u;
  if (alpha == 255) return (fg & 0x00FFFFFFu) | 0xFF000000u;

  uint32_t inv_alpha = 255 - alpha;
  uint32_t bg_r = (bg >> 16) & 0xFFu;
  uint32_t bg_g = (bg >> 8) & 0xFFu;
  uint32_t bg_b = bg & 0xFFu;
  uint32_t fg_r = (fg >> 16) & 0xFFu;
  uint32_t fg_g = (fg >> 8) & 0xFFu;
  uint32_t fg_b = fg & 0xFFu;

  uint32_t out_r = (fg_r * alpha + bg_r * inv_alpha) >> 8;
  uint32_t out_g = (fg_g * alpha + bg_g * inv_alpha) >> 8;
  uint32_t out_b = (fg_b * alpha + bg_b * inv_alpha) >> 8;

  if (out_r > 255) out_r = 255;
  if (out_g > 255) out_g = 255;
  if (out_b > 255) out_b = 255;

  return 0xFF000000u | (out_r << 16) | (out_g << 8) | out_b;
}

static uint32_t sample_blur_cache(const uint32_t *blur_cache, int blur_cols,
                                  int blur_rows, int dx, int dy) {
  if (!blur_cache || blur_cols <= 0 || blur_rows <= 0)
    return BASE_BG_COLOR;

  int fx = dx % WINDOW_BLUR_SPACING;
  int fy = dy % WINDOW_BLUR_SPACING;
  int x0 = dx / WINDOW_BLUR_SPACING;
  int y0 = dy / WINDOW_BLUR_SPACING;
  int x1 = x0 + 1;
  int y1 = y0 + 1;
  if (x0 < 0) x0 = 0;
  if (y0 < 0) y0 = 0;
  if (x0 >= blur_cols) x0 = blur_cols - 1;
  if (y0 >= blur_rows) y0 = blur_rows - 1;
  if (x1 >= blur_cols) x1 = blur_cols - 1;
  if (y1 >= blur_rows) y1 = blur_rows - 1;

  uint32_t c00 = blur_cache[y0 * blur_cols + x0];
  uint32_t c10 = blur_cache[y0 * blur_cols + x1];
  uint32_t c01 = blur_cache[y1 * blur_cols + x0];
  uint32_t c11 = blur_cache[y1 * blur_cols + x1];

  uint32_t top = blend_colors_fixed(c00, c10, fx * 255 / (WINDOW_BLUR_SPACING - 1));
  uint32_t bottom = blend_colors_fixed(c01, c11, fx * 255 / (WINDOW_BLUR_SPACING - 1));
  return blend_colors_fixed(top, bottom, fy * 255 / (WINDOW_BLUR_SPACING - 1));
}

static void fill_rect_rgba(layer_t *layer, int x, int y, int w, int h, uint32_t color) {
  if (!layer || !layer->buffer || w <= 0 || h <= 0)
    return;
  int x0 = x < 0 ? 0 : x;
  int y0 = y < 0 ? 0 : y;
  int x1 = x + w;
  int y1 = y + h;
  if (x1 > layer->width) x1 = layer->width;
  if (y1 > layer->height) y1 = layer->height;
  if (x0 >= x1 || y0 >= y1)
    return;
  uint8_t alpha = (uint8_t)(color >> 24);
  for (int py = y0; py < y1; py++) {
    uint32_t *dst = &layer->buffer[py * layer->width + x0];
    for (int px = x0; px < x1; px++) {
      if (alpha == 255) *dst = color;
      else *dst = blend_colors(*dst, color, alpha);
      dst++;
    }
  }
}

static void stroke_rect(layer_t *layer, int x, int y, int w, int h, uint32_t color) {
  fill_rect_rgba(layer, x, y, w, 1, color);
  fill_rect_rgba(layer, x, y + h - 1, w, 1, color);
  fill_rect_rgba(layer, x, y, 1, h, color);
  fill_rect_rgba(layer, x + w - 1, y, 1, h, color);
}

static int bcd_to_int(uint8_t v) {
  return ((v >> 4) * 10) + (v & 0x0F);
}

static int read_display_clock(int *out_hour, int *out_minute) {
  if (!out_hour || !out_minute)
    return 0;

#ifdef __aarch64__
  uint32_t seconds = timer_ticks / 100;
  *out_hour = (int)((seconds / 3600) % 24);
  *out_minute = (int)((seconds / 60) % 60);
  return 1;
#else
  outb(0x70, 0x0A);
  if (inb(0x71) & 0x80)
    return 0;

  outb(0x70, 0x04);
  uint8_t hour = inb(0x71);
  outb(0x70, 0x02);
  uint8_t minute = inb(0x71);
  outb(0x70, 0x0B);
  uint8_t status_b = inb(0x71);

  if ((status_b & 0x04) == 0) {
    hour = (uint8_t)bcd_to_int(hour);
    minute = (uint8_t)bcd_to_int(minute);
  }

  if ((status_b & 0x02) == 0) {
    int is_pm = hour & 0x80;
    hour &= 0x7F;
    if (is_pm && hour < 12) hour = (uint8_t)(hour + 12);
    if (!is_pm && hour == 12) hour = 0;
  }

  *out_hour = hour % 24;
  *out_minute = minute % 60;
  return 1;
#endif
}

static void lock_state_refresh_clock(void) {
  uint32_t second_tick = timer_ticks / 100;
  if (g_lock_state.last_clock_tick == second_tick && g_lock_state.time_label[0] != '\0')
    return;

  int hour = 0;
  int minute = 0;
  if (!read_display_clock(&hour, &minute)) {
    uint32_t seconds = timer_ticks / 100;
    hour = (int)((seconds / 3600) % 24);
    minute = (int)((seconds / 60) % 60);
  }

  snprintf(g_lock_state.time_label, sizeof(g_lock_state.time_label), "%02d:%02d", hour, minute);
  g_lock_state.last_clock_tick = second_tick;
  g_svg_dirty = 1;
}

static void lock_state_start_transition(lock_transition_t transition, int target_locked) {
  g_lock_state.transition = transition;
  g_lock_state.target_locked = target_locked;
  g_lock_state.transition_started_at = timer_ticks;
  g_lock_state.transition_duration_ticks = 0;
  g_lock_state.transition_progress = target_locked ? 1.0f : 0.0f;

  if (g_lock_state.transition_duration_ticks == 0) {
    g_lock_state.transition = LOCK_TRANSITION_IDLE;
    g_lock_state.is_locked = target_locked;
  }
}

static void lock_state_enter(void) {
  if (g_lock_state.is_locked && g_lock_state.target_locked)
    return;
  g_lock_state.is_locked = 1;
  lock_state_refresh_clock();
  lock_state_start_transition(LOCK_TRANSITION_LOCKING, 1);
  set_cursor_type(CURSOR_TYPE_DEFAULT);
  g_svg_dirty = 1;
  screen_mark_all_dirty();
}

static void lock_state_request_unlock(void) {
  if (!g_lock_state.is_locked && !g_lock_state.target_locked)
    return;
  lock_state_start_transition(LOCK_TRANSITION_UNLOCKING, 0);
  g_svg_dirty = 1;
  screen_mark_all_dirty();
}

static void lock_state_update(void) {
  if (g_lock_state.is_locked || g_lock_state.target_locked)
    lock_state_refresh_clock();

  if (g_lock_state.transition == LOCK_TRANSITION_IDLE)
    return;

  if (g_lock_state.transition_duration_ticks == 0) {
    g_lock_state.is_locked = g_lock_state.target_locked;
    g_lock_state.transition = LOCK_TRANSITION_IDLE;
    return;
  }

  uint32_t elapsed = timer_ticks - g_lock_state.transition_started_at;
  if (elapsed >= g_lock_state.transition_duration_ticks) {
    g_lock_state.is_locked = g_lock_state.target_locked;
    g_lock_state.transition_progress = g_lock_state.target_locked ? 1.0f : 0.0f;
    g_lock_state.transition = LOCK_TRANSITION_IDLE;
    g_svg_dirty = 1;
    return;
  }

  float t = (float)elapsed / (float)g_lock_state.transition_duration_ticks;
  g_lock_state.transition_progress =
      (g_lock_state.transition == LOCK_TRANSITION_LOCKING) ? t : (1.0f - t);
  g_svg_dirty = 1;
}

static int lock_state_is_visible(void) {
  return g_lock_state.is_locked || g_lock_state.transition != LOCK_TRANSITION_IDLE;
}

static void lock_screen_get_unlock_button_rect(int *x, int *y, int *w, int *h) {
  int bw = 220;
  int bh = 58;
  if (x) *x = (SCREEN_WIDTH - bw) / 2;
  if (y) *y = SCREEN_HEIGHT - 112;
  if (w) *w = bw;
  if (h) *h = bh;
}

static int lock_screen_hit_unlock_button(int px, int py) {
  int x, y, w, h;
  lock_screen_get_unlock_button_rect(&x, &y, &w, &h);
  return px >= x && px < x + w && py >= y && py < y + h;
}

static void draw_capsule_outline(layer_t *layer, int x, int y, int w, int h,
                                 uint32_t fill_color, uint32_t stroke_color) {
  if (!layer || !layer->buffer || w <= 0 || h <= 0)
    return;

  float radius = (float)h * 0.5f;
  float cx0 = (float)x + radius;
  float cx1 = (float)(x + w) - radius;
  float cy = (float)y + radius;
  float inner_radius = radius - 1.5f;

  for (int py = y; py < y + h; py++) {
    if (py < 0 || py >= layer->height)
      continue;
    for (int px = x; px < x + w; px++) {
      if (px < 0 || px >= layer->width)
        continue;

      float fx = (float)px + 0.5f;
      float fy = (float)py + 0.5f;
      float seg_x = fx;
      if (seg_x < cx0) seg_x = cx0;
      if (seg_x > cx1) seg_x = cx1;

      float dx = fx - seg_x;
      float dy = fy - cy;
      float dist = sqrtf(dx * dx + dy * dy);
      float fill_alpha = radius + 0.5f - dist;
      if (fill_alpha > 1.0f) fill_alpha = 1.0f;
      if (fill_alpha < 0.0f) fill_alpha = 0.0f;
      if (fill_alpha > 0.0f) {
        layer->buffer[py * layer->width + px] =
            blend_colors(layer->buffer[py * layer->width + px], fill_color,
                         (uint8_t)(fill_alpha * ((fill_color >> 24) & 0xFF)));
      }

      float stroke_alpha = inner_radius + 0.5f - dist;
      if (stroke_alpha > 1.0f) stroke_alpha = 1.0f;
      if (stroke_alpha < 0.0f) stroke_alpha = 0.0f;
      if (fill_alpha > 0.0f && stroke_alpha < 1.0f) {
        float ring = 1.0f - stroke_alpha;
        if (ring > 1.0f) ring = 1.0f;
        layer->buffer[py * layer->width + px] =
            blend_colors(layer->buffer[py * layer->width + px], stroke_color,
                         (uint8_t)(ring * ((stroke_color >> 24) & 0xFF)));
      }
    }
  }
}

static void draw_lock_screen(layer_t *layer) {
  if (!layer || !layer->buffer)
    return;

  draw_wallpaper(layer);

  const char *dark_val = get_w1_global("~~main/dark");
  int is_dark = (strcmp(dark_val, "true") == 0);
  fill_rect_rgba(layer, 0, 0, layer->width, layer->height,
                 is_dark ? 0x66000000u : 0x48FFFFFFu);

  lock_state_refresh_clock();

  int time_w = measure_ttf_width(g_lock_state.time_label, 48.0f);
  int time_x = (SCREEN_WIDTH - time_w) / 2;
  int time_y = 54;
  layer_draw_ttf(layer, time_x, time_y, g_lock_state.time_label, 48.0f,
                 is_dark ? 0xFFFFFFFFu : 0xFF111111u);

  const char *button_label = "ロック解除";
  int bx, by, bw, bh;
  lock_screen_get_unlock_button_rect(&bx, &by, &bw, &bh);
  draw_capsule_outline(layer, bx, by, bw, bh,
                       is_dark ? 0x66353535u : 0xB8FFFFFFu,
                       is_dark ? 0x66FFFFFFu : 0x22000000u);

  int label_w = measure_ttf_width(button_label, 22.0f);
  int label_x = bx + (bw - label_w) / 2;
  int label_y = by + 12;
  layer_draw_ttf(layer, label_x, label_y, button_label, 22.0f,
                 is_dark ? 0xFFFFFFFFu : 0xFF111111u);
}

static void get_window_draw_bounds(window_t *win, int *x0, int *y0, int *x1, int *y1) {
  int title_h = win->no_decoration ? 0 : 60;
  int shadow_size = win->no_decoration ? 0 : 48;
  *x0 = win->x - shadow_size;
  *y0 = win->y - title_h - shadow_size;
  *x1 = win->x + win->w + shadow_size;
  *y1 = win->y + win->h + shadow_size;
}

static int rects_intersect(int ax0, int ay0, int ax1, int ay1, int bx0, int by0, int bx1, int by1) {
  return ax0 < bx1 && ax1 > bx0 && ay0 < by1 && ay1 > by0;
}

static void redraw_warp_region(layer_t *layer, int rx0, int ry0, int rx1, int ry1);

static void draw_single_window(layer_t *layer, window_t *win, int clip_x0, int clip_y0, int clip_x1, int clip_y1) {
    // Use normal render path for all windows (active and inactive)
    if (win->rgba_buffer && (win->no_decoration || (win->shadow_cache && win->frame_cache))) {
      int title_h = win->no_decoration ? 0 : 60;
      float scale = win->render_scale;

      int full_y0 = win->y - title_h;
      int full_h = win->h + title_h;

      if (!win->no_decoration && win->shadow_cache) {
        int shadow_size = win->no_decoration ? 0 : 48;
        int sx_start = win->x - shadow_size;
        int sy_start = win->y - title_h - shadow_size + 8;
        int y0 = (sy_start < clip_y0) ? (clip_y0 - sy_start) : 0;
        int y1 = (sy_start + (win->h + title_h + shadow_size * 2) > clip_y1) ? clip_y1 - sy_start : (win->h + title_h + shadow_size * 2);
        int x0 = (sx_start < clip_x0) ? (clip_x0 - sx_start) : 0;
        int x1 = (sx_start + (win->w + shadow_size * 2) > clip_x1) ? clip_x1 - sx_start : (win->w + shadow_size * 2);
        if (x0 < 0) x0 = 0;
        if (y0 < 0) y0 = 0;
        if (x1 <= x0 || y1 <= y0) goto skip_shadow;
        for (int dy = y0; dy < y1; dy++) {
          int py = sy_start + dy;
          uint32_t *dst_line = &layer->buffer[py * layer->width];
          int scaled_dy = (int)((float)dy * scale);
          if (scaled_dy >= win->shadow_cache_h) scaled_dy = win->shadow_cache_h - 1;
          uint8_t *src_mask = &win->shadow_cache[scaled_dy * win->shadow_cache_w];
          for (int dx = x0; dx < x1; dx++) {
            int scaled_dx = (int)((float)dx * scale);
            if (scaled_dx >= win->shadow_cache_w) scaled_dx = win->shadow_cache_w - 1;
            uint8_t alpha = src_mask[scaled_dx];
            if (alpha == 0) continue;
            dst_line[sx_start + dx] = blend_colors(dst_line[sx_start + dx], 0, alpha);
          }
        }
      }

skip_shadow:;
      int cy0 = (full_y0 < clip_y0) ? (clip_y0 - full_y0) : 0;
      int cy1 = (full_y0 + full_h > clip_y1) ? (clip_y1 - full_y0) : full_h;
      int mw = (int)((float)win->w * scale);
      if (mw < 1 && win->w > 0) mw = 1;
      int mh = (int)((float)full_h * scale);
      
      // ★ 修正1: ループ外でヘッダー情報を一度だけ取得
      int action_count = 0;
      int action_btn_x[16];
      int action_btn_w[16];
      if (!win->no_decoration) {
          char dummy[128];
          if (win->is_warp1) warp1_context_get_header_info(win->warp1_ctx, dummy, 128, &action_count);
          else warp_context_get_header_info(win->warp_ctx, dummy, 128, &action_count);
          
          int cur_ax = win->w - 16;
          for (int j = 0; j < action_count && j < 16; j++) {
              char at[64];
              if (win->is_warp1) warp1_context_get_header_action_info(win->warp1_ctx, j, at, 64);
              else warp_context_get_header_action_info(win->warp_ctx, j, at, 64);
              action_btn_w[j] = measure_ttf_width(at, 18.2f) + 32;
              cur_ax -= action_btn_w[j];
              action_btn_x[j] = cur_ax;
              cur_ax -= 10;
          }
      }

      // テキストは事前合成済みのため、ループ内のオーバーレイ処理は不要。
      int is_dark = (strcmp(get_w1_global("~~main/dark"), "true") == 0);

      int scroll_offset_y = (int)roundf(-win->scroll_y * scale);
      if (scroll_offset_y < 0) scroll_offset_y = 0;
      for (int dy = cy0; dy < cy1; dy++) {
        int py = full_y0 + dy;
        uint32_t *dst_line = &layer->buffer[py * layer->width];

        uint32_t grad_base = is_dark ? 0x00000000u : 0x00FFFFFFu;
        uint8_t header_grad_alpha = 0;
        if (dy < title_h || win->is_menubar) {
            float alpha_f = 1.0f - ((float)dy / (float)(win->is_menubar ? full_h : title_h));
            if (alpha_f < 0.0f) alpha_f = 0.0f;
            header_grad_alpha = (uint8_t)(alpha_f * 255.0f);
        }
        
        int scaled_mask_y = (int)((float)dy * scale);
        if (scaled_mask_y >= mh) scaled_mask_y = mh - 1;
        uint8_t *mask_line = &win->window_mask[scaled_mask_y * mw];
        uint8_t fade_alpha_u8 = (uint8_t)(win->fade_alpha * 255);
        int dx0 = (win->x < clip_x0) ? (clip_x0 - win->x) : 0;
        int dx1 = (win->x + win->w > clip_x1) ? (clip_x1 - win->x) : win->w;
        
        for (int dx = dx0; dx < dx1; dx++) {
          int px = win->x + dx;
          if (px < 0 || px >= layer->width) continue;

          // ウィンドウ全体の背景：背後を完全に無視し、指定色の単色塗り潰しとする
          uint32_t win_bg = get_window_background_color(win);
          uint32_t surface_bg = win_bg;

          // 1. Base Content Rendering (Warp UI)
          int src_x = (int)((float)dx * scale);
          int src_y = scroll_offset_y + (int)((float)dy * scale);
          if (src_x >= win->buffer_w) src_x = win->buffer_w - 1;
          if (src_y >= win->buffer_h) src_y = win->buffer_h - 1;
          uint32_t content_color;
          if (win->is_resizing || win->is_calculating) {
              // 超高品質円形ブラー (25点 Poisson-like Spiral Sampling)
              // タイトルバー背面も含め全体に適用
              float r = 30.0f * scale; 
              static const float ox[] = {
                  0.00f, 0.12f, -0.10f, 0.04f, 0.04f, -0.10f, 0.25f, -0.24f, 0.20f, -0.15f, 0.08f, 0.00f, -0.08f, 0.15f, -0.20f, 0.24f, -0.25f, 0.50f, -0.49f, 0.46f, -0.42f, 0.35f, -0.28f, 0.19f, -0.10f
              };
              static const float oy[] = {
                  0.00f, 0.00f, 0.07f, -0.12f, 0.12f, -0.07f, 0.00f, 0.08f, -0.15f, 0.20f, -0.24f, 0.25f, -0.24f, 0.20f, -0.15f, 0.08f, -0.00f, 0.00f, 0.10f, -0.19f, 0.28f, -0.35f, 0.42f, -0.46f, 0.49f
              };
              
              uint32_t rs = 0, gs = 0, bs = 0, as = 0;
              for (int i = 0; i < 25; i++) {
                  int sx = src_x + (int)(ox[i] * r);
                  int sy = src_y + (int)(oy[i] * r);
                  
                  // ウィンドウバッファ内でのクランプ
                  if (sx < 0) sx = 0; 
                  if (sx >= win->buffer_w) sx = win->buffer_w - 1;
                  if (sy < 0) sy = 0; 
                  if (sy >= win->buffer_h) sy = win->buffer_h - 1;
                  
                  uint32_t c = ((uint32_t*)win->rgba_buffer)[sy * win->buffer_w + sx];
                  as += (c >> 24) & 0xFF;
                  rs += (c >> 16) & 0xFF;
                  gs += (c >> 8) & 0xFF;
                  bs += c & 0xFF;
              }
              content_color = ((as / 25) << 24) | ((rs / 25) << 16) | ((gs / 25) << 8) | (bs / 25);
          } else {
              content_color = ((uint32_t*)win->rgba_buffer)[src_y * win->buffer_w + src_x];
          }

          // 超高速パス: ウィンドウ中央、非リサイズ時、不透明ピクセルの場合
          uint8_t mask_a = mask_line[(int)((float)dx * scale)];
          if (mask_a == 255 && dy >= title_h && fade_alpha_u8 == 0 && !win->is_resizing && !win->is_calculating) {
              uint8_t content_a = (content_color >> 24);
              if (content_a == 255) {
                  dst_line[px] = content_color | 0xFF000000u;
                  continue;
              }
          }
          
          uint32_t color = blend_rgb_over_opaque_premul(surface_bg, content_color);
          if (header_grad_alpha > 0) color = blend_colors(color, grad_base, header_grad_alpha);

          // 3. Shadow Layer (SDFの計算コストを削減するため、範囲外ならスキップ)
          if (dy < title_h + 30 && !win->no_decoration) {
              float s_alpha_val = 0.0f;
              float s_blur = 24.0f;
              float sy = (float)dy;
              int cps[] = {14, 14 + 42 + 10};
              for (int k = 0; k < 2; k++) {
                  float dx_c = (float)dx - (cps[k] + 21);
                  float dy_c = sy - 34;
                  float dist_sq = dx_c * dx_c + dy_c * dy_c;
                  if (dist_sq < 1849.0f) { // (19+24)^2 = 1849 
                      float inv_d = 1.0f - (sqrtf(dist_sq) - 19.0f) / 24.0f;
                      if (inv_d > 0) { float sa = inv_d * inv_d * inv_d; if (sa > s_alpha_val) s_alpha_val = sa; }
                  }
              }
              
              // ★ 修正2: 事前取得した情報を使い、必要な範囲のみ影計算を実行
              if (action_count > 0 && dx > win->w - 200) {
                  for (int j = 0; j < action_count; j++) {
                      int ax = action_btn_x[j];
                      int bw = action_btn_w[j];
                      if (dx < ax - 30 || dx > ax + bw + 30) continue;
                      float bbx = (float)ax, bbw = (float)bw, bbr = 21.0f;
                      float seg_x = ((float)dx < bbx + bbr) ? bbx + bbr : (((float)dx > bbx + bbw - bbr) ? bbx + bbw - bbr : (float)dx);
                      float ddx = (float)dx - seg_x;
                      float ddy = sy - 35.0f;
                      float dist_sq = ddx * ddx + ddy * ddy;
                      if (dist_sq < 1849.0f) {
                          float d_norm = (sqrtf(dist_sq) - 19.0f) / s_blur;
                          if (d_norm < 1.0f) {
                              float inv_d = 1.0f - (d_norm < 0.0f ? 0.0f : d_norm);
                              float sa = inv_d * inv_d * inv_d;
                              if (sa > s_alpha_val) s_alpha_val = sa;
                          }
                      }
                  }
              }
              if (s_alpha_val > 0.0f) color = blend_colors(color, 0xFF000000, (uint8_t)(s_alpha_val * 13.0f));
        }

          // 4. システムボタンとグラス歪み
          if (dy < title_h && !win->no_decoration) {
              int f_sx = (int)((float)dx * scale);
              int f_sy = (int)((float)dy * scale);
              if (f_sx >= win->frame_cache_w) f_sx = win->frame_cache_w - 1;
              if (f_sy >= win->frame_cache_h) f_sy = win->frame_cache_h - 1;
              uint32_t frame_px = win->frame_cache[f_sy * win->frame_cache_w + f_sx];
              uint8_t frame_a = (uint8_t)(frame_px >> 24);

              if (frame_a > 0) {
                   int cx = -1, bcy = 0;
                   float btn_half_width = 21.0f;
                   
                   // Left side control buttons
                   int ctrl_positions[] = {14, 14 + 42 + 10};
                   for (int k = 0; k < 2; k++) {
                       if (dx >= ctrl_positions[k] && dx < ctrl_positions[k] + 42) {
                           cx = ctrl_positions[k] + 21;
                           bcy = 13 + 21;
                           break;
                       }
                   }
                  
                   // Right side custom buttons
                   if (cx == -1) {
                       for (int j = 0; j < action_count; j++) {
                           int ax = action_btn_x[j];
                           int bw = action_btn_w[j];
                           if (dx >= ax && dx < ax + bw) {
                               cx = ax + bw / 2;
                               bcy = 13 + 21;
                               btn_half_width = (float)bw / 2.0f;
                               break;
                           }
                       }
                   }

                  uint32_t glass_base = color; // デフォルトは歪みなし
                  if (cx != -1) {
                       float max_dist = 21.0f * scale; 
                       float rx_l = (float)(dx - cx), ry_l = (float)(dy - bcy);
                       float f_px = fabsf(rx_l), f_py = fabsf(ry_l);
                       float h_rect = btn_half_width - 21.0f;
                       if (h_rect < 0.0f) h_rect = 0.0f;
                       f_px = (f_px > h_rect) ? f_px - h_rect : 0.0f;
                       
                       // ★ 修正3: powf(sqrt(x), 4) を x*x に置き換えて高速化
                       float d_sq_norm = (f_px*f_px + f_py*f_py) / (max_dist * max_dist);
                       float d_scale = 1.0f + (d_sq_norm * d_sq_norm) * 0.7f; 

                       float fgx = (float)cx * scale + (rx_l * scale / d_scale);
                       float fgy = (float)scroll_offset_y + (float)bcy * scale + (ry_l * scale / d_scale);
                       
                       // グラスエフェクト用のブラーサンプリング (3x3 grid)
                       // 加工（歪み計算）前のソースから周辺ピクセルを混ぜることで、曇りガラス表現を実現
                       float gr = 1.0f * scale; // サンプリング間隔
                       uint32_t rs = 0, gs = 0, bs = 0, as = 0;
                       for (int iy = -1; iy <= 1; iy++) {
                           for (int ix = -1; ix <= 1; ix++) {
                               int sx = (int)(fgx + (float)ix * gr);
                               int sy = (int)(fgy + (float)iy * gr);
                               if (sx < 0) sx = 0; if (sx >= win->buffer_w) sx = win->buffer_w - 1;
                               if (sy < 0) sy = 0; if (sy >= win->buffer_h) sy = win->buffer_h - 1;
                               uint32_t c = ((uint32_t*)win->rgba_buffer)[sy * win->buffer_w + sx];
                               as += (c >> 24) & 0xFF; rs += (c >> 16) & 0xFF; gs += (c >> 8) & 0xFF; bs += c & 0xFF;
                           }
                       }
                       uint32_t blurred_px = ((as / 9) << 24) | ((rs / 9) << 16) | ((gs / 9) << 8) | (bs / 9);
                       
                       uint32_t g_color = blend_rgb_over_opaque_premul(win_bg, blurred_px);
                       glass_base = blend_colors(g_color, win_bg, 180);

                       // グラスボタンの色にもグラデーションを適用して背景と完全に馴染ませる
                       if (header_grad_alpha > 0) {
                           glass_base = blend_colors(glass_base, grad_base, header_grad_alpha);
                       }
                  }

                      uint32_t marker_rgb = is_dark ? 0x444444 : 0xFFFFFF;
                      if (frame_a == 1) {
                          color = glass_base;
                      } else if ((frame_px & 0x00FFFFFF) == marker_rgb) {
                          // アンチエイリアス：現在の背景(color)と歪み後の色をブレンド
                          color = blend_colors(color, glass_base, frame_a);
                      } else {
                          // ボタン上のアイコン等
                          color = blend_colors(glass_base, frame_px, frame_a);
                      }
              }
          }
          if (fade_alpha_u8 > 0) {
              uint32_t overlay_color = is_dark ? 0x00000000u : 0x00FFFFFFu;
              color = blend_colors(color, overlay_color, fade_alpha_u8);
          }
          
          uint8_t final_alpha = 255;
          if (!win->no_decoration) {
              int scaled_dx = (int)((float)dx * scale);

              if (scaled_dx >= mw) scaled_dx = mw - 1;
              uint8_t mask_a = mask_line[scaled_dx];
              if (mask_a == 255) {
                  // 角丸の範囲外（中央付近）ならブレンドせずに直接書き込む
                  if (fade_alpha_u8 == 0) {
                      dst_line[px] = color | 0xFF000000u;
                      continue;
                  }
                  final_alpha = 255;
              } else {
                  final_alpha = (uint8_t)((uint32_t)final_alpha * mask_a / 255);
              }
          }
          dst_line[px] = blend_colors(dst_line[px], color, final_alpha);
        }
      }
    }
}

static void redraw_warp_region(layer_t *layer, int rx0, int ry0, int rx1, int ry1) {
  if (!g_svg_ready) return;
  if (rx0 < 0) rx0 = 0;
  if (ry0 < 0) ry0 = 0;
  if (rx1 > layer->width) rx1 = layer->width;
  if (ry1 > layer->height) ry1 = layer->height;
  if (rx0 >= rx1 || ry0 >= ry1) return;

  for (int y = ry0; y < ry1; y++) {
    uint32_t *dst = &layer->buffer[y * layer->width + rx0];
    uint32_t *src = &svg_base_buf[y * layer->width + rx0];
    for (int x = rx0; x < rx1; x++) *dst++ = *src++;
  }

  for (int pass = 0; pass < 2; pass++) {
    for (int i = 0; i < g_window_count; i++) {
      window_t *win = &g_windows[i];
      if ((pass == 0 && win->is_sticky) || (pass == 1 && !win->is_sticky)) continue;
      int wx0, wy0, wx1, wy1;
      get_window_draw_bounds(win, &wx0, &wy0, &wx1, &wy1);
      if (!rects_intersect(rx0, ry0, rx1, ry1, wx0, wy0, wx1, wy1)) continue;
      if (win->is_dirty && !win->is_resizing) window_redraw(win);
      draw_single_window(layer, win, rx0, ry0, rx1, ry1);
    }
  }
  screen_mark_dirty_rect(rx0, ry0, rx1 - rx0, ry1 - ry0);
}

static void redraw_warp_svg(layer_t *layer) {
  if (!g_svg_dirty) return;
  sync_all_window_themes();

  if (lock_state_is_visible()) {
    draw_lock_screen(layer);
    g_svg_dirty = 0;
    screen_mark_layer_dirty(layer);
    return;
  }

  int active_idx = g_active_window_index;
  int below_active_dirty = 0;
  for (int i = 0; i < g_window_count; i++) {
    window_t *win = &g_windows[i];
    // エンジン内部で状態（画面等）が変わっていたらOS側のDirtyを立てる
    if (win->is_warp1 && win->warp1_ctx && warp1_context_is_dirty(win->warp1_ctx)) win->is_dirty = 1;
    else if (!win->is_warp1 && win->warp_ctx && warp_context_is_dirty(win->warp_ctx)) win->is_dirty = 1;

    int active_is_sticky = (active_idx >= 0 && g_windows[active_idx].is_sticky);
    int contributes_to_bg = (!win->is_sticky && (active_is_sticky || i < active_idx));
    if (contributes_to_bg && win->is_dirty && !win->is_resizing) below_active_dirty = 1;
  }

  if (desktop_composite_dirty || below_active_dirty || active_idx != desktop_composite_last_active_index) {
    layer_t temp_layer = *layer;
    temp_layer.buffer = desktop_composite_buf;
    
    draw_wallpaper(&temp_layer);
    for (int i = 0; i < g_window_count; i++) {
      window_t *win = &g_windows[i];
      if (win->is_sticky) continue;
      if (i == active_idx) break; // アクティブなウィンドウの手前までを描画（アクティブがスティッキーなら全通常窓を描画）
      if (win->is_dirty && !win->is_resizing) window_redraw(win);
      draw_single_window(&temp_layer, win, 0, 0, temp_layer.width, temp_layer.height);
    }
    desktop_composite_dirty = 0;
    desktop_composite_last_active_index = active_idx;
  }

  // 全画面コピーを避け、アクティブウィンドウとその周辺のみを更新
  // ただし、初回や大きな変更時は全画面コピーが必要
  if (desktop_composite_last_active_index != active_idx || g_svg_dirty) {
     memcpy(layer->buffer, desktop_composite_buf, (size_t)layer->width * (size_t)layer->height * 4);
  }

  // 1. Draw sticky windows
  // 2. Draw active window
  if (active_idx >= 0 && active_idx < g_window_count) {
    window_t *win = &g_windows[active_idx];
    if (win->is_dirty && !win->is_resizing) window_redraw(win);
    draw_single_window(layer, win, 0, 0, layer->width, layer->height);
  }

  // 3. Draw sticky windows LAST if active window is NOT maximized (standard behavior)
  // 最大化時でもスティッキーウィンドウ（メニューバー等）は常に最前面に表示する
  for (int i = 0; i < g_window_count; i++) {
    window_t *win = &g_windows[i];
    if (!win->is_sticky) continue;
    if (i == active_idx) continue;
    if (win->is_dirty && !win->is_resizing) window_redraw(win);
    draw_single_window(layer, win, 0, 0, layer->width, layer->height);
  }

  g_svg_dirty = 0;
  screen_mark_layer_dirty(layer);
}
static int svg_init_nextgen(layer_t *layer) {
  svg_init(layer, 1); // Load and render wallpaper
  
  // Execute startup commands
  handle_terminal_command("warp new.warp");
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
#ifndef __aarch64__
  outb(0x43, 0x36);
  outb(0x40, divisor & 0xFF);
  outb(0x40, (divisor >> 8) & 0xFF);
#endif
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
  screen_mark_layer_dirty(hud);

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
#ifdef __aarch64__
  uint32_t size = 0;
  void *data = fs_read_file("HarmonyOS_Sans_Regular.ttf", &size);
  if (data) {
    if (stbtt_InitFont(&g_font, (unsigned char *)data, 0)) {
      g_font_ready = 1;
    } else {
      g_font_error = "ERR:stbtt_InitFont ARM";
    }
  } else {
    g_font_error = "ERR:font not found in FS";
  }

  size = 0;
  void *emoji_data = fs_read_file("NotoEmoji-Regular.ttf", &size);
  if (emoji_data) {
    if (stbtt_InitFont(&g_emoji_font, (unsigned char *)emoji_data, 0)) {
      g_emoji_font_ready = 1;
    } else {
      g_emoji_font_error = "ERR:stbtt_InitFont Emoji ARM";
    }
  }

  return g_font_ready;
#else
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

  multiboot_module_t *mods = (multiboot_module_t *)(uintptr_t)mbi->mods_addr;
  for (uint32_t i = 0; i < mbi->mods_count; i++) {
    unsigned char *ttf = (unsigned char *)(uintptr_t)mods[i].mod_start;
    uint32_t ttf_size = mods[i].mod_end - mods[i].mod_start;
    const char *cmdline = (const char *)(uintptr_t)mods[i].string;

    if (ttf_size > 12 && ttf[0] == 0x00 && ttf[1] == 0x01 && ttf[2] == 0x00 && ttf[3] == 0x00) { // Simple TTF magic check
      if (!g_font_ready) {
        if (stbtt_InitFont(&g_font, ttf, stbtt_GetFontOffsetForIndex(ttf, 0))) {
          g_font_ready = 1;
        }
      } else if (!g_emoji_font_ready) {
        // Assume second TTF is emoji font if not already loaded
        // Or check cmdline if available
        if (cmdline && strstr(cmdline, "NotoEmoji")) {
           if (stbtt_InitFont(&g_emoji_font, ttf, stbtt_GetFontOffsetForIndex(ttf, 0))) {
             g_emoji_font_ready = 1;
           }
        } else if (!cmdline || !strstr(cmdline, "HarmonyOS")) {
           if (stbtt_InitFont(&g_emoji_font, ttf, stbtt_GetFontOffsetForIndex(ttf, 0))) {
             g_emoji_font_ready = 1;
           }
        }
      }
    }
  }

  if (!g_font_ready) g_font_error = "ERR:stbtt_InitFont";
  return g_font_ready;
#endif
}

// 2つの色をアルファ値(0-255)で合成するヘルパー
static inline uint32_t blend_colors(uint32_t bg, uint32_t fg, uint8_t alpha) {
  if (alpha == 0) return bg;
  uint32_t bg_alpha = (bg >> 24) & 0xFFu;
  if (alpha == 255 && bg_alpha == 0) return fg;

  uint32_t out_alpha = alpha + ((bg_alpha * (255 - alpha)) >> 8);
  if (out_alpha == 0) return 0;

  uint32_t fg_r = (fg >> 16) & 0xFFu;
  uint32_t fg_g = (fg >> 8) & 0xFFu;
  uint32_t fg_b = fg & 0xFFu;
  uint32_t bg_r = (bg >> 16) & 0xFFu;
  uint32_t bg_g = (bg >> 8) & 0xFFu;
  uint32_t bg_b = bg & 0xFFu;

  // 高速なアルファ合成（割り算を回避）
  // 背景が不透明(out_alpha=255)な場合が多いため、より単純化可能ですが、
  // 汎用性を維持しつつ計算コストを下げます。
  uint32_t inv_a = 255 - alpha;
  uint32_t out_r = (fg_r * alpha + bg_r * inv_a + 128) >> 8;
  uint32_t out_g = (fg_g * alpha + bg_g * inv_a + 128) >> 8;
  uint32_t out_b = (fg_b * alpha + bg_b * inv_a + 128) >> 8;

  if (out_alpha > 255) out_alpha = 255;

  return (out_alpha << 24) | (out_r << 16) | (out_g << 8) | out_b;
}

static int point_in_titlebar_button(int hx, int hy, window_t *win, int center_x) {
    int x = win->x + center_x;
    int y = win->y - 60 + 13 + 21;
    int dx = hx - x;
    int dy = hy - y;
    return dx*dx + dy*dy < 21*21;
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

// UTF-8→Unicode変換 (堅牢版)
static uint32_t utf8_next(const char **p) {
  const unsigned char *s = (const unsigned char *)*p;
  if (!s || !*s) return 0;
  uint32_t c = 0;
  int len = 0;
  if (s[0] < 0x80) { c = s[0]; len = 1; }
  else if ((s[0] & 0xE0) == 0xC0) { c = s[0] & 0x1F; len = 2; }
  else if ((s[0] & 0xF0) == 0xE0) { c = s[0] & 0x0F; len = 3; }
  else if ((s[0] & 0xF8) == 0xF0) { c = s[0] & 0x07; len = 4; }
  else { (*p)++; return 0; }

  for (int i = 1; i < len; i++) {
    if ((s[i] & 0xC0) != 0x80) { (*p)++; return 0; }
    c = (c << 6) | (s[i] & 0x3F);
  }
  *p += len;
  return c;
}

// --- グリフキャッシュ ---
typedef struct {
  uint32_t codepoint;
  float size;
  int bw, bh, bx, by, adv;
  unsigned char *bitmap;
  stbtt_fontinfo *font;
} glyph_cache_t;
#define MAX_GLYPH_CACHE 1024
static glyph_cache_t g_glyph_cache[MAX_GLYPH_CACHE];
static int g_glyph_cache_count = 0;

static int is_emoji(uint32_t cp) {
  if (cp < 0x2000) return 0;
  if (cp >= 0x1F300 && cp <= 0x1F9FF) return 1;
  if (cp >= 0x2600 && cp <= 0x27BF) return 1;
  if (cp >= 0x1FA70 && cp <= 0x1FAFF) return 1;
  if (cp >= 0x1F000 && cp <= 0x1F2FF) return 1;
  if (cp >= 0x1F680 && cp <= 0x1F6FF) return 1;
  return 0;
}

static glyph_cache_t* get_glyph(uint32_t codepoint, float size) {
  if (codepoint == 0) return NULL;
  for (int i = 0; i < g_glyph_cache_count; i++) {
    if (g_glyph_cache[i].codepoint == codepoint && g_glyph_cache[i].size == size)
      return &g_glyph_cache[i];
  }

  // FIFO方式で確実にメモリを解放
  static int evict_idx = 0;
  glyph_cache_t *gc;
  if (g_glyph_cache_count < MAX_GLYPH_CACHE) {
    gc = &g_glyph_cache[g_glyph_cache_count++];
  } else {
    gc = &g_glyph_cache[evict_idx];
    if (gc->bitmap) {
      STBTT_free(gc->bitmap, NULL);
      gc->bitmap = NULL;
    }
    evict_idx = (evict_idx + 1) % MAX_GLYPH_CACHE;
  }

  stbtt_fontinfo *font = &g_font;
  if (g_emoji_font_ready && is_emoji(codepoint)) {
    font = &g_emoji_font;
  }

  if (stbtt_FindGlyphIndex(font, (int)codepoint) == 0) {
    if (font == &g_font && g_emoji_font_ready) {
      if (stbtt_FindGlyphIndex(&g_emoji_font, (int)codepoint) != 0) font = &g_emoji_font;
    } else if (font == &g_emoji_font && g_font_ready) {
      if (stbtt_FindGlyphIndex(&g_font, (int)codepoint) != 0) font = &g_font;
    }
  }

  float scale = stbtt_ScaleForPixelHeight(font, size);
  gc->bitmap = stbtt_GetCodepointBitmap(font, scale, scale, (int)codepoint, &gc->bw, &gc->bh, &gc->bx, &gc->by);
  
  int adv_tmp, lsb_tmp;
  stbtt_GetCodepointHMetrics(font, (int)codepoint, &adv_tmp, &lsb_tmp);
  gc->adv = (int)(adv_tmp * scale);
  if (gc->adv < 0) gc->adv = 0;

  gc->codepoint = codepoint;
  gc->size = size;
  gc->font = font;
  return gc;
}

void layer_draw_ttf(layer_t *layer, int px, int py, const char *str,
                     float font_size, uint32_t color) {
  if (!g_font_ready || !str || !layer || !layer->buffer)
    return;
  
  int cx = px;
  const char *p = str;
  while (*p) {
    uint32_t cp = utf8_next(&p);
    glyph_cache_t *gc = get_glyph(cp, font_size);
    if (gc && gc->bitmap) {
      // Get baseline for THIS glyph's font
      float scale = stbtt_ScaleForPixelHeight(gc->font, font_size);
      int ascent, descent, line_gap;
      stbtt_GetFontVMetrics(gc->font, &ascent, &descent, &line_gap);
      int baseline = (int)(ascent * scale);

#ifdef __SSE2__
      // SIMD-accelerated glyph rendering: 8 pixels at once
      for (int dy = 0; dy < gc->bh; dy++) {
        int dpy = py + baseline + gc->by + dy;
        if (dpy < 0 || dpy >= (int)layer->height)
          continue;
        int dx = 0;
        while (dx + 8 <= gc->bw) {
          int dpx = cx + gc->bx + dx;
          if (dpx < 0 || dpx + 7 >= (int)layer->width) { dx++; continue; }
          
          // Load 8 alpha values
          uint8_t *alpha_ptr = &gc->bitmap[dy * gc->bw + dx];
          __m128i alphas = _mm_loadl_epi64((__m128i*)alpha_ptr);
          
          // Check if any alpha is non-zero (fast skip)
          __m128i zero = _mm_setzero_si128();
          __m128i cmp = _mm_cmpeq_epi8(alphas, zero);
          int mask = _mm_movemask_epi8(cmp);
          
          if (mask != 0xFF) {
            // At least one pixel needs blending
            uint32_t *dst = &layer->buffer[dpy * layer->width + dpx];
            for (int i = 0; i < 8; i++) {
              uint8_t a = alpha_ptr[i];
              if (a) {
                uint32_t bg = dst[i];
                dst[i] = blend_colors(bg, color, a);
              }
            }
          }
          dx += 8;
        }
        // Scalar remainder
        for (; dx < gc->bw; dx++) {
          int dpx = cx + gc->bx + dx;
          if (dpx < 0 || dpx >= (int)layer->width)
            continue;
          uint8_t alpha = gc->bitmap[dy * gc->bw + dx];
          if (alpha == 0)
            continue;
          uint32_t bg = layer->buffer[dpy * layer->width + dpx];
          layer->buffer[dpy * layer->width + dpx] = blend_colors(bg, color, alpha);
        }
      }
#else
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
          layer->buffer[dpy * layer->width + dpx] = blend_colors(bg, color, alpha);
        }
      }
#endif
      cx += gc->adv;
    }
  }
}

int measure_ttf_width(const char *str, float font_size) {
  if (!(g_font_ready || g_emoji_font_ready) || !str)
    return 0;

  int width = 0;
  const char *p = str;
  while (*p) {
    uint32_t cp = utf8_next(&p);
    glyph_cache_t *gc = get_glyph(cp, font_size);
    if (gc)
      width += gc->adv;
  }
  
  return width;
}
// SVGパスを使ったグリフ描画（ダミー: 枠のみ）
static void layer_draw_glyph(layer_t *layer, int x, int y, uint32_t code,
                             uint32_t color) {
  // fonts.h の font_glyphs[] から code を検索
  extern const Glyph font_glyphs[];
  for (int i = 0; font_glyphs[i].code != 0; ++i) {
    if (font_glyphs[i].code == (uint16_t)code) {
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
    uint32_t code = utf8_next(&str);
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
  if (!mbi || !(mbi->flags & (1 << 12)))
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

static void resize_cursor_init(void) {
  const char *resize_svg =
      "<svg width=\"319\" height=\"307\" viewBox=\"0 0 319 307\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">"
      "<path d=\"M96.7002 73V121.123C96.7002 133.239 96.7002 139.297 99.0961 142.102C101.175 144.536 104.294 145.828 107.485 145.577C111.163 145.288 115.446 141.004 124.014 132.436L172.137 84.3137C180.704 75.7462 184.988 71.4624 185.277 67.7846C185.528 64.5934 184.237 61.4749 181.803 59.3959C178.997 57 172.939 57 160.823 57H112.7C107.1 57 104.299 57 102.16 58.0899C100.279 59.0487 98.7489 60.5785 97.7901 62.4601C96.7002 64.5992 96.7002 67.3995 96.7002 73Z\" stroke=\"white\" stroke-width=\"27\" stroke-linecap=\"round\"/>"
      "<path d=\"M218.991 179.072L113.825 74.3452\" stroke=\"white\" stroke-width=\"56\"/>"
      "<path d=\"M96.7002 73V121.123C96.7002 133.239 96.7002 139.297 99.0961 142.102C101.175 144.536 104.294 145.828 107.485 145.577C111.163 145.288 115.446 141.004 124.014 132.436L172.137 84.3137C180.704 75.7462 184.988 71.4624 185.277 67.7846C185.528 64.5934 184.237 61.4749 181.803 59.3959C178.997 57 172.939 57 160.823 57H112.7C107.1 57 104.299 57 102.16 58.0899C100.279 59.0487 98.7489 60.5785 97.7901 62.4601C96.7002 64.5992 96.7002 67.3995 96.7002 73Z\" fill=\"black\"/>"
      "<path d=\"M233.7 178V129.877C233.7 117.761 233.7 111.703 231.304 108.898C229.225 106.464 226.107 105.172 222.916 105.423C219.238 105.712 214.954 109.996 206.386 118.564L158.264 166.686C149.696 175.254 145.413 179.538 145.123 183.215C144.872 186.407 146.164 189.525 148.598 191.604C151.403 194 157.461 194 169.578 194H217.7C223.301 194 226.101 194 228.24 192.91C230.122 191.951 231.652 190.422 232.61 188.54C233.7 186.401 233.7 183.601 233.7 178Z\" stroke=\"white\" stroke-width=\"27\" stroke-linecap=\"round\"/>"
      "<path d=\"M233.7 178V129.877C233.7 117.761 233.7 111.703 231.304 108.898C229.225 106.464 226.107 105.172 222.916 105.423C219.238 105.712 214.954 109.996 206.386 118.564L158.264 166.686C149.696 175.254 145.413 179.538 145.123 183.215C144.872 186.407 146.164 189.525 148.598 191.604C151.403 194 157.461 194 169.578 194H217.7C223.301 194 226.101 194 228.24 192.91C230.122 191.951 231.652 190.422 232.61 188.54C233.7 186.401 233.7 183.601 233.7 178Z\" fill=\"black\"/>"
      "<path d=\"M218.991 179.072L113.825 74.3452\" stroke=\"black\" stroke-width=\"29\"/>"
      "</svg>";

  NSVGimage *img = nsvgParse((char *)resize_svg, "px", 96.0f);
  if (!img) return;

  int target_h = 42;
  float scale = (float)target_h / img->height;
  int target_w = (int)(img->width * scale);
  int padding = 12;
  int w = target_w + padding * 2;
  int h = target_h + padding * 2;

  uint32_t *buf = (uint32_t *)malloc((size_t)w * (size_t)h * 4);
  NSVGrasterizer *rast = nsvgCreateRasterizer();
  unsigned char *rgba = (unsigned char *)malloc((size_t)w * (size_t)h * 4);
  unsigned char *shadow_rgba = (unsigned char *)malloc((size_t)w * (size_t)h * 4);

  if (rast && rgba && shadow_rgba && buf) {
    memset(shadow_rgba, 0, (size_t)w * (size_t)h * 4);
    nsvgRasterize(rast, img, (float)padding + 2.0f, (float)padding + 3.0f, scale, shadow_rgba, w, h, w * 4);
    box_blur_alpha(shadow_rgba, w, h, 4);

    memset(rgba, 0, (size_t)w * (size_t)h * 4);
    nsvgRasterize(rast, img, (float)padding, (float)padding, scale, rgba, w, h, w * 4);

    for (int i = 0; i < w * h; i++) {
      uint8_t shadow_a = (uint8_t)(shadow_rgba[i * 4 + 3] * 0.4f);
      uint8_t r = rgba[i * 4 + 0], g = rgba[i * 4 + 1], b = rgba[i * 4 + 2], a = rgba[i * 4 + 3];
      if (a == 255) {
        buf[i] = (0xFFu << 24) | ((uint32_t)r << 16) | ((uint32_t)g << 8) | (uint32_t)b;
      } else {
        uint8_t out_a = a + (shadow_a * (255 - a) / 255);
        if (out_a == 0) buf[i] = 0;
        else {
          uint8_t out_r = (uint8_t)((r * a) / out_a);
          uint8_t out_g = (uint8_t)((g * a) / out_a);
          uint8_t out_b = (uint8_t)((b * a) / out_a);
          buf[i] = ((uint32_t)out_a << 24) | ((uint32_t)out_r << 16) | ((uint32_t)out_g << 8) | (uint32_t)out_b;
        }
      }
    }
    set_resize_cursor_bitmap(buf, w, h);

    // Create flipped version for BL/TR corners (NE-SW)
    uint32_t *flipped_buf = (uint32_t *)malloc((size_t)w * (size_t)h * 4);
    if (flipped_buf) {
      for (int fy = 0; fy < h; fy++) {
        for (int fx = 0; fx < w; fx++) {
          flipped_buf[fy * w + (w - 1 - fx)] = buf[fy * w + fx];
        }
      }
      set_resize_nesw_cursor_bitmap(flipped_buf, w, h);
    }
  }
  if (rast) nsvgDeleteRasterizer(rast);
  if (rgba) free(rgba);
  if (shadow_rgba) free(shadow_rgba);
  nsvgDelete(img);
}

#ifdef __aarch64__
extern void uart_puts(const char *s);
#endif

#ifdef __aarch64__
extern unsigned char _binary_output_initrd_tar_start[];
extern size_t _binary_output_initrd_tar_size;
#endif

static void warp_ui_mod_init_embedded() {
#ifdef __aarch64__
  const char *tar_start = (const char *)_binary_output_initrd_tar_start;
  size_t tar_size = _binary_output_initrd_tar_size;
  
  uart_puts("Loading embedded initrd...\r\n");
  
  // Format and extract to in-memory FS
  fs_format();
  const char *p = tar_start;
  const char *end = tar_start + tar_size;
  while (p + 512 <= end) {
      tar_header_t *h = (tar_header_t *)p;
      if (h->name[0] == '\0') break;
      if (h->name[0] == '.' && h->name[1] == '_') {
          uint32_t skip_size = octal_to_int(h->size, 12);
          p += 512 + ((skip_size + 511) & ~511);
          continue;
      }
      uint32_t f_size = octal_to_int(h->size, 12);
      if (h->typeflag == '0' || h->typeflag == '\0') {
          fs_write_file(h->name, p + 512, f_size);
      }
      p += 512 + ((f_size + 511) & ~511);
  }

  // Load essential files
  g_warp_ptr = fs_read_file("main.warpc", &g_warp_size);
  g_terminal_warp_ptr = fs_read_file("terminal.warp", &g_terminal_warp_size);
  g_menubar_warp_ptr = fs_read_file("menubar.warp", &g_menubar_warp_size);
  g_bootlogo_ptr = fs_read_file("bootlogo.svg", &g_bootlogo_size);
  g_os_settings_ptr = fs_read_file("os_settings.json", &g_os_settings_size);
  
  if (g_warp_ptr) g_warp_mod_found = 1;
  parse_os_settings();
#endif
}

void kmain(uint32_t magic, struct multiboot_info *mbi) {
  (void)magic;
#ifdef __aarch64__
  uart_puts("\r\n--- BaramOS ARM64 Booting ---\r\n");
  // Test fw_cfg
  extern void fw_cfg_select(uint16_t key);
  extern void fw_cfg_read(void *buf, size_t len);
  fw_cfg_select(0x0000);
  char sig[4];
  fw_cfg_read(sig, 4);
  if (sig[0] == 'Q' && sig[1] == 'E' && sig[2] == 'M' && sig[3] == 'U') {
    uart_puts("fw_cfg OK\n");
  } else {
    uart_puts("fw_cfg FAIL\n");
  }
#endif
  mbi_ptr = mbi;
  uint32_t mem_total_kb = 0;
#ifdef __aarch64__
  g_mbi_flags = 0;
  mem_total_kb = 1024 * 1024; // 1GB
#else
  if (mbi) {
    g_mbi_flags = mbi->flags;
    mem_total_kb = mbi->mem_upper;
  } else {
    mem_total_kb = 65536;
  }
  // kernel.c は -msse2 でビルドされているため、最適化で初期化コードにも
  // SSE 命令が出る。ログ出力や文字列処理より前に有効化しておく。
  enable_fpu();
#endif

  // 動的ヒープの初期化
  uintptr_t heap_start = (uintptr_t)_kernel_end;
#ifdef __aarch64__
  // ARM64 QEMU virt: assume at least 1GB RAM
  uintptr_t heap_end = 0x40000000ULL + (uintptr_t)mem_total_kb * 1024;
  heap_init((void*)heap_start, heap_end - heap_start);
#else
  if (mbi && (mbi->flags & 0x8) && mbi->mods_count > 0) {
      multiboot_module_t *mods = (multiboot_module_t *)(uintptr_t)mbi->mods_addr;
      for (uint32_t i = 0; i < mbi->mods_count; i++) {
          if (mods[i].mod_end > heap_start) {
              heap_start = mods[i].mod_end;
          }
      }
  }
  // 4KBアライメント
  heap_start = (heap_start + 4095) & ~4095;
  uintptr_t heap_floor = heap_start;
  uintptr_t heap_end = 0;

  // メモリマップの解析 (UEFI対応への準備)
  if (mbi && (mbi->flags & (1 << 6))) {
    struct multiboot_mmap_entry *mmap = (struct multiboot_mmap_entry *)(uintptr_t)mbi->mmap_addr;
    uint32_t mmap_len = mbi->mmap_length;
    int found_heap = 0;
    
    struct multiboot_mmap_entry *mmap_curr = mmap;
    uintptr_t mmap_limit = (uintptr_t)mmap + mmap_len;

    set_w1_global("--warpSystemLog", "Parsing Memory Map...");

    while ((uintptr_t)mmap_curr < mmap_limit) {
      if (mmap_curr->size < 20) break; // Safety check

      // Type 1: Available RAM
      if (mmap_curr->type == 1) {
        uint64_t entry_start = mmap_curr->addr;
        uint64_t entry_end = mmap_curr->addr + mmap_curr->len;

        // カーネル末尾を含む利用可能領域だけをヒープ候補にする。
        // ここで heap_start 自体を別の領域へ飛ばすと、未マップ領域を掴んで
        // 起動直後の malloc でクラッシュしやすい。
        if (entry_start <= (uint64_t)heap_floor &&
            entry_end > (uint64_t)heap_floor) {
          uintptr_t actual_end =
              (entry_end > 0xFFFFFFFFULL) ? 0xFFFFFFFFU : (uintptr_t)entry_end;

          if (actual_end > heap_floor &&
              (!found_heap || actual_end > heap_end)) {
            heap_end = actual_end;
            found_heap = 1;
          }
        }
      }

      // デバッグログ: 特定の条件（大きい領域など）のみ記録
      if (mmap_curr->len > 1024 * 1024) {
          char mmap_msg[128];
          snprintf(mmap_msg, sizeof(mmap_msg), "MMAP: %x-%x T%d", (uint32_t)mmap_curr->addr, (uint32_t)(mmap_curr->addr + mmap_curr->len), mmap_curr->type);
          set_w1_global("--warpSystemLog", mmap_msg);
      }

      mmap_curr = (struct multiboot_mmap_entry *)((uintptr_t)mmap_curr + mmap_curr->size + 4);
    }
    if (found_heap) {
      char msg[128];
      snprintf(msg, sizeof(msg), "Heap Settled: %x - %x (%d MB)",
               heap_floor, heap_end,
               (uint32_t)((heap_end - heap_floor) / (1024 * 1024)));
      set_w1_global("--warpSystemLog", msg);
      heap_init((void*)heap_floor, heap_end - heap_floor);
    }
  } 
  
  // メモリマップが見つからない、または失敗した場合は従来の mem_upper を信じる
  if (!heap_initialized) {
    heap_end = 0x100000 + (uintptr_t)mem_total_kb * 1024;
    if (heap_end > heap_floor) {
        heap_init((void*)heap_floor, heap_end - heap_floor);
    }
  }
#endif

  // SVG描画などの初期化より前に、まず赤画面を出す
#ifndef __aarch64__
  for (int i = 0; i < 30; ++i) { // 約0.3秒間、赤で塗りつぶし続ける
    fill_framebuffer_red_early(mbi);
    for (volatile int j = 0; j < 1000000; ++j) {
      __asm__ __volatile__("nop");
    }
  }

  fill_framebuffer_red_early(mbi); // 最後にもう一度赤で塗る
#endif
  // 割り込み初期化
  idt_install();
  irq_install();
#ifndef __aarch64__
  irq_install_handler(0, timer_handler);
  timer_phase(100); // 100Hz
#endif
  keyboard_install();
  mouse_install();
  enable_interrupts();

  // モジュールとフォントの初期化を最優先で行う
#ifdef __aarch64__
  warp_ui_mod_init_embedded();
  font_init(NULL);
#else
  font_init(mbi);
  warp_ui_mod_init(mbi);
#endif

#ifdef __aarch64__
  // ARM64 QEMU virt: use ramfb
  extern void ramfb_init(uint32_t *fb, uint32_t w, uint32_t h);
  extern void arm_timer_init(uint32_t hz);
  // メインスクリーンバッファを直接 RamFB に紐付ける
  set_framebuffer_info(main_screen_buf, SCREEN_WIDTH, SCREEN_HEIGHT, SCREEN_WIDTH * 4);
  ramfb_init(main_screen_buf, SCREEN_WIDTH, SCREEN_HEIGHT);
  arm_timer_init(100);
#else
  if (mbi->flags & (1 << 12)) {
    // Multiboot 提供のフレームバッファを使用
    set_framebuffer_info((uint32_t *)(uintptr_t)mbi->framebuffer_addr,
                         mbi->framebuffer_width, mbi->framebuffer_height,
                         mbi->framebuffer_pitch);
  } else {
    // フレームバッファがない場合はメインバッファで代用（描画は見えないがクラッシュ回避）
    set_framebuffer_info(main_screen_buf, SCREEN_WIDTH, SCREEN_HEIGHT, SCREEN_WIDTH * 4);
    set_w1_global("--warpSystemLog", "NoMultibootFB!");
  }
#endif

  // カーソル初期化
  cursor_init();
  resize_cursor_init();

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
    if (g_critical_error_mode) {
        layer_fill(&desktop, 0xFF000000); // 背景を黒に固定
        svg_layer.active = 0;
        blink_layer.active = 0;
        nextgen_ui_layer.active = 0;
        text_layer.active = 0;
        hud_layer.active = 0;

        static int error_rendered = 0;
        if (!error_rendered) {
            NSVGimage *img = nsvgParse((char*)g_error_svg, "px", 96.0f);
            if (img) {
                if (!g_svg_rast) g_svg_rast = nsvgCreateRasterizer();
                float tx = (float)(SCREEN_WIDTH - (int)img->width) / 2.0f;
                float ty = (float)(SCREEN_HEIGHT - (int)img->height) / 2.0f;
                // 直接メインバッファにラスタライズ
                nsvgRasterize(g_svg_rast, img, tx, ty, 1.0f, (unsigned char*)main_screen_buf, SCREEN_WIDTH, SCREEN_HEIGHT, SCREEN_WIDTH * 4);
                
                // RGBA -> BGRA (Little Endian ARGB) 変換
                unsigned char *p = (unsigned char*)main_screen_buf;
                for (int i = 0; i < SCREEN_WIDTH * SCREEN_HEIGHT; i++) {
                    uint8_t r = p[0], b = p[2];
                    p[0] = b; p[2] = r;
                    p += 4;
                }
                nsvgDelete(img);
            }
            screen_mark_static_dirty();
            error_rendered = 1;
        }

        if (keybuf_len > 0) {
            for (int i = 0; i < keybuf_len; i++) {
                if (keybuf[i] == '\n') {
                    g_critical_error_mode = 0;
                    // 通常モード（Classic）のレイヤーを復旧
                    svg_layer.active = 1;
                    blink_layer.active = 1;
                    text_layer.active = 1;
                    hud_layer.active = g_dev_show_hud;
                    screen_mark_static_dirty();
                    break;
                }
            }
            keybuf_len = 0;
        }

        screen_refresh();
        continue;
    }

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
      g_scroll_x = g_scroll_y = 0.0f;
      reset_scroll_input_state(&g_classic_scroll_input);
      discard_scroll_events();
      
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

      // Classic モードのスクロールを入力からその場で反映
      {
        float scroll_delta = consume_scroll_events(&g_classic_scroll_input);
        if (scroll_delta != 0.0f) {
          g_scroll_y += scroll_delta;
          g_scroll_y = clamp_scroll_offset(g_scroll_y, SCREEN_HEIGHT, g_svg_full_h);
          svg_render_full(&svg_layer);
          memcpy(svg_base_buf, svg_layer.buffer,
                 sizeof(uint32_t) * svg_layer.width * svg_layer.height);
          screen_mark_static_dirty();
        }
      }

      if (keybuf_len > 0) {
        for (int i = 0; i < keybuf_len; i++) {
          char c = (char)keybuf[i];
          if (c == KEY_UP) {
            g_scroll_y = clamp_scroll_offset(g_scroll_y + 120.0f, SCREEN_HEIGHT, g_svg_full_h);
            svg_render_full(&svg_layer);
            memcpy(svg_base_buf, svg_layer.buffer,
                   sizeof(uint32_t) * svg_layer.width * svg_layer.height);
            screen_mark_static_dirty();
          } else if (c == KEY_DOWN) {
            g_scroll_y = clamp_scroll_offset(g_scroll_y - 120.0f, SCREEN_HEIGHT, g_svg_full_h);
            svg_render_full(&svg_layer);
            memcpy(svg_base_buf, svg_layer.buffer,
                   sizeof(uint32_t) * svg_layer.width * svg_layer.height);
            screen_mark_static_dirty();
          }
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
      lock_state_update();

      for (int i = 0; i < g_window_count; i++) {
        window_t *win = &g_windows[i];
        if (win->interaction_refresh_until_tick > timer_ticks) {
          win->is_dirty = 1;
          g_svg_dirty = 1;
        }
      }

      if (lock_state_is_visible()) {
        uint8_t curr_btns = mouse_buttons;
        if (curr_btns != prev_mouse_buttons) {
          if ((curr_btns & 1) && !(prev_mouse_buttons & 1)) {
            int hx = mouse_x + MOUSE_HOTSPOT_X;
            int hy = mouse_y + MOUSE_HOTSPOT_Y;
            if (lock_screen_hit_unlock_button(hx, hy)) {
              lock_state_request_unlock();
            }
          }
          prev_mouse_buttons = curr_btns;
        }

        if (keybuf_len > 0) {
          keybuf_len = 0;
          keybuf_str[0] = '\0';
        }

        set_cursor_type(CURSOR_TYPE_DEFAULT);
        g_scroll_y = 0.0f;
        discard_scroll_events();
        reset_scroll_input_state(&g_classic_scroll_input);

        if (g_svg_dirty) {
          redraw_warp_svg(&nextgen_ui_layer);
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
        goto warpdesktop_frame_done;
      }

      // 1. Scroll Handling (Active Window)
      if (g_active_window_index >= 0) {
        window_t *win = &g_windows[g_active_window_index];
        float scroll_amount = consume_scroll_events(&win->scroll_input);
        if (apply_window_scroll_delta(win, scroll_amount)) {
          int bx0, by0, bx1, by1;
          get_window_draw_bounds(win, &bx0, &by0, &bx1, &by1);
          redraw_warp_region(&nextgen_ui_layer, bx0, by0, bx1, by1);
        }
      } else {
        discard_scroll_events();
      }

      // 2. Non-scroll invalidation
      int moved = 0;
      int scroll_rx0 = nextgen_ui_layer.width;
      int scroll_ry0 = nextgen_ui_layer.height;
      int scroll_rx1 = 0;
      int scroll_ry1 = 0;
      for (int i = 0; i < g_window_count; i++) {
        window_t *win = &g_windows[i];

        // Resize/Calculating Fade (Instant semi-transparent overlay)
        if (win->is_resizing || win->is_calculating) {
            if (win->fade_alpha != 0.5f) {
                int bx0, by0, bx1, by1;
                get_window_draw_bounds(win, &bx0, &by0, &bx1, &by1);
                if (bx0 < scroll_rx0) scroll_rx0 = bx0;
                if (by0 < scroll_ry0) scroll_ry0 = by0;
                if (bx1 > scroll_rx1) scroll_rx1 = bx1;
                if (by1 > scroll_ry1) scroll_ry1 = by1;
                win->fade_alpha = 0.5f; // 即座に50%の透明度へ
                moved = 1;
                g_svg_dirty = 1;
            }
        } else if (win->fade_alpha != 0.0f) {
            int bx0, by0, bx1, by1;
            get_window_draw_bounds(win, &bx0, &by0, &bx1, &by1);
            if (bx0 < scroll_rx0) scroll_rx0 = bx0;
            if (by0 < scroll_ry0) scroll_ry0 = by0;
            if (bx1 > scroll_rx1) scroll_rx1 = bx1;
            if (by1 > scroll_ry1) scroll_ry1 = by1;
            win->fade_alpha = 0.0f; // 即座に元に戻す
            moved = 1;
            g_svg_dirty = 1;
        }
      }
      if (moved) redraw_warp_region(&nextgen_ui_layer, scroll_rx0, scroll_ry0, scroll_rx1, scroll_ry1);

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
          
          // 1. Check Sticky Windows first (top-most)
          for (int i = g_window_count - 1; i >= 0; i--) {
            window_t *win = &g_windows[i];
            if (!win->is_sticky) continue;
            if (hx >= win->x && hx < win->x + win->w && hy >= (win->y - (win->no_decoration ? 0 : 40)) && hy < win->y + win->h) {
                hit_index = i; break;
            }
          }

          // 2. Check Regular Windows
          if (hit_index == -1) {
            for (int i = g_window_count - 1; i >= 0; i--) {
              window_t *win = &g_windows[i];
              if (win->is_sticky) continue;
              int title_h = win->no_decoration ? 0 : 60;
              
              // Resize Handle
              if (!win->is_menubar && i == g_active_window_index && win->is_resizing_enabled &&
                  hx >= win->x + win->w - 24 && hx < win->x + win->w + 8 &&
                  hy >= win->y + win->h - 24 && hy < win->y + win->h + 8) {
                hit_index = i; g_windows[i].is_resizing = 1; g_windows[i].resize_mode = 1; break;
              }
              if (!win->is_menubar && i == g_active_window_index && win->is_resizing_enabled &&
                  hx >= win->x - 8 && hx < win->x + 24 &&
                  hy >= win->y + win->h - 24 && hy < win->y + win->h + 8) {
                hit_index = i; g_windows[i].is_resizing = 1; g_windows[i].resize_mode = 2; break;
              }
              if (!win->is_menubar && i == g_active_window_index && win->is_resizing_enabled &&
                  hx >= win->x + win->w - 24 && hx < win->x + win->w + 8 &&
                  hy >= win->y - title_h - 8 && hy < win->y - title_h + 8) {
                hit_index = i; g_windows[i].is_resizing = 1; g_windows[i].resize_mode = 3; break;
              }
              if (!win->is_menubar && i == g_active_window_index && win->is_resizing_enabled &&
                  hx >= win->x - 8 && hx < win->x + 24 &&
                  hy >= win->y - title_h - 8 && hy < win->y - title_h + 8) {
                hit_index = i; g_windows[i].is_resizing = 1; g_windows[i].resize_mode = 4; break;
              }
              // Window area (Title + Content)
              if (hx >= win->x && hx < win->x + win->w && hy >= (win->y - title_h) && hy < win->y + win->h) {
                hit_index = i; break;
              }
            }
          }
          
          if (hit_index >= 0) {
            if (!g_windows[hit_index].is_sticky && hit_index != g_active_window_index) {
              int target_pos = g_window_count - 1;
              while (target_pos > 0 && g_windows[target_pos].is_sticky) target_pos--;
              if (hit_index < target_pos) {
                window_t tmp = g_windows[hit_index];
                for (int j = hit_index; j < target_pos; j++) g_windows[j] = g_windows[j+1];
                g_windows[target_pos] = tmp;
                g_active_window_index = target_pos;
                reset_scroll_input_state(&g_windows[target_pos].scroll_input);
                window_set_all_dirty();
                request_window_interaction_refresh(&g_windows[target_pos]);
              } else {
                g_active_window_index = hit_index;
                reset_scroll_input_state(&g_windows[hit_index].scroll_input);
                request_window_interaction_refresh(&g_windows[hit_index]);
              }
              hit_index = -2;
            }
          }
          
          if (hit_index >= 0 && g_windows[hit_index].is_resizing) {
            g_windows[hit_index].resize_w = g_windows[hit_index].w;
            g_windows[hit_index].resize_h = g_windows[hit_index].h;
            g_svg_dirty = 1;
            hit_index = -2;
          }

          if (hit_index >= 0) {
            window_t *hwin = &g_windows[hit_index];
            int handled = 0;
            
            // Title Bar check (Top Overlay Layer)
            if (hy < hwin->y && !hwin->no_decoration) {
              if (point_in_titlebar_button(hx, hy, hwin, 35)) { 
                  g_active_window_index = hit_index; 
                  close_active_window(); 
                  hit_index = -2; 
                  handled = 1;
              } else if (point_in_titlebar_button(hx, hy, hwin, 35 + 42 + 10)) {
                  // Pure dimension resize toggle
                  if (hwin->w == 300 && hwin->h == 240) {
                      // Already 300x300, restore original size
                      if (hwin->old_w > 0 && hwin->old_h > 0) {
                          hwin->w = hwin->old_w; hwin->h = hwin->old_h;
                          hwin->is_resizing_enabled = hwin->old_is_resizing_enabled;
                      }
                  } else {
                      // Not 300x300, save current dimensions and resize to 300x300
                      hwin->old_w = hwin->w; hwin->old_h = hwin->h;
                      hwin->old_is_resizing_enabled = hwin->is_resizing_enabled;
                      hwin->w = 300; hwin->h = 240; 
                      hwin->is_resizing_enabled = 0;
                  }
                  window_update_caches(hwin);
                  request_window_interaction_refresh(hwin);
                  handled = 1;
              } else {
                char header_text[128]; int action_count = 0; int has_header = 0;
                if (hwin->is_warp1) { if (hwin->warp1_ctx) has_header = warp1_context_get_header_info(hwin->warp1_ctx, header_text, sizeof(header_text), &action_count); } 
                else { if (hwin->warp_ctx) has_header = warp_context_get_header_info(hwin->warp_ctx, header_text, sizeof(header_text), &action_count); }
                if (has_header) {
                  int ax = hwin->x + hwin->w - 12;
                  for (int j = 0; j < action_count; j++) {
                    char act_text[64];
                    if (hwin->is_warp1) warp1_context_get_header_action_info(hwin->warp1_ctx, j, act_text, sizeof(act_text));
                    else warp_context_get_header_action_info(hwin->warp_ctx, j, act_text, sizeof(act_text));
                    int text_w = measure_ttf_width(act_text, 18.2f); int btn_w = text_w + 32; ax -= btn_w;
                    if (hx >= ax && hx < ax + btn_w) { if (hwin->is_warp1) { warp1_context_invalidate_layout(hwin->warp1_ctx); warp1_context_click_header_action(hwin->warp1_ctx, j); } else warp_context_click_header_action(hwin->warp_ctx, j); request_window_interaction_refresh(hwin); handled = 1; hit_index = -2; break; }
                    ax -= 10;
                  }
                }
              }
            }

            // If not handled by system buttons, check Warp UI content (Base Layer)
            if (!handled) {
              int title_h = hwin->no_decoration ? 0 : 60;
              // Pass click to Warp engine, coordinates relative to full window top (including header)
              if (hwin->is_warp1) { warp1_context_invalidate_layout(hwin->warp1_ctx); warp1_context_click(hwin->warp1_ctx, hx - hwin->x, hy - (hwin->y - title_h) - (int)hwin->scroll_y); }
              else warp_context_click(hwin->warp_ctx, hx - hwin->x, hy - (hwin->y - title_h) - (int)hwin->scroll_y);
              request_window_interaction_refresh(hwin);
              hwin->is_slider_dragging = 0;
              if (hwin->is_warp1) {
                int sx, sy, sw, sh;
                if (warp1_context_get_active_slider_rect(hwin->warp1_ctx, &sx, &sy, &sw, &sh)) hwin->is_slider_dragging = 1;
              } else {
                int sx, sy, sw, sh;
                if (warp_context_get_active_slider_rect(hwin->warp_ctx, &sx, &sy, &sw, &sh)) hwin->is_slider_dragging = 1;
              }

              // Also check for dragging if in header but didn't hit a button
              if (!hwin->is_slider_dragging && hy < hwin->y && !hwin->no_decoration && hx >= hwin->x + 56 && hwin->is_movable) {
                hwin->is_dragging = 1;
              }
            }
          }
          
          if (hit_index >= 0) {
            if (!g_windows[hit_index].is_sticky && hit_index != g_window_count - 1) {
              window_t tmp = g_windows[hit_index];
              int target_pos = g_window_count - 1;
              while (target_pos > 0 && g_windows[target_pos].is_sticky) target_pos--;
              if (hit_index < target_pos) {
                for (int j = hit_index; j < target_pos; j++) g_windows[j] = g_windows[j+1];
                g_windows[target_pos] = tmp; g_active_window_index = target_pos; window_set_all_dirty();
              }
            } else { g_active_window_index = hit_index; g_svg_dirty = 1; }
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
            if (g_windows[i].is_slider_dragging) {
              if (g_windows[i].is_warp1) warp1_context_end_slider_drag(g_windows[i].warp1_ctx);
              else warp_context_end_slider_drag(g_windows[i].warp_ctx);
            }
            g_windows[i].is_slider_dragging = 0;
          }
        }
        prev_mouse_buttons = curr_btns;
      }

      // Drag/Resize movement or pointcheck mouse update
      if (g_active_window_index >= 0 && (mouse_dx != 0 || mouse_dy != 0)) {
        window_t *awin = &g_windows[g_active_window_index];
        int hx = mouse_x + MOUSE_HOTSPOT_X;
        int hy = mouse_y + MOUSE_HOTSPOT_Y;

        // ポインター種別の更新
        int cursor_type = CURSOR_TYPE_DEFAULT;
        for (int i = g_window_count - 1; i >= 0; i--) {
            window_t *win = &g_windows[i];
            if (!win->is_menubar && i == g_active_window_index && win->is_resizing_enabled) {
                int th = win->no_decoration ? 0 : 60;
                if ((hx >= win->x + win->w - 24 && hx < win->x + win->w + 8 && hy >= win->y + win->h - 24 && hy < win->y + win->h + 8) ||
                    (hx >= win->x - 8 && hx < win->x + 24 && hy >= win->y - th - 8 && hy < win->y - th + 8)) {
                    cursor_type = CURSOR_TYPE_RESIZE_NWSE; break;
                }
                if ((hx >= win->x - 8 && hx < win->x + 24 && hy >= win->y + win->h - 24 && hy < win->y + win->h + 8) ||
                    (hx >= win->x + win->w - 24 && hx < win->x + win->w + 8 && hy >= win->y - th - 8 && hy < win->y - th + 8)) {
                    cursor_type = CURSOR_TYPE_RESIZE_NESW; break;
                }
            }
        }
        set_cursor_type(cursor_type);
        
        if (!awin->is_slider_dragging && !awin->is_dragging && !awin->is_resizing) {
          if (awin->is_warp1) {
            if (awin->warp1_ctx) {
              warp1_context_set_mouse(awin->warp1_ctx, hx - awin->x, hy - awin->y - (int)awin->scroll_y);
              // マウス移動だけでは dirty にしない（ホバー等の状態変化時のみエンジンが dirty を返す）
              if (warp1_context_is_dirty(awin->warp1_ctx)) awin->is_dirty = 1;
            }
          } else {
            if (awin->warp_ctx) {
              warp_context_set_mouse(awin->warp_ctx, hx - awin->x, hy - awin->y - (int)awin->scroll_y);
              if (warp_context_is_dirty(awin->warp_ctx)) awin->is_dirty = 1;
            }
          }
        }

        if (awin->is_slider_dragging) {
          int local_x = hx - awin->x;
          int local_y = hy - awin->y - (int)awin->scroll_y;
          int changed = 0;
          int sx, sy, sw, sh;
          if (awin->is_warp1) {
            changed = warp1_context_drag_active_slider(awin->warp1_ctx, local_x, local_y);
            if (changed && warp1_context_get_active_slider_rect(awin->warp1_ctx, &sx, &sy, &sw, &sh)) {
              request_window_interaction_refresh(awin);
              int title_h = awin->no_decoration ? 0 : 60;
              redraw_warp_region(&nextgen_ui_layer,
                                 awin->x + sx - 24,
                                 (awin->y - title_h) + sy - (int)awin->scroll_y - 24,
                                 awin->x + sx + sw + 24,
                                 (awin->y - title_h) + sy + sh - (int)awin->scroll_y + 24);
            }
          } else {
            changed = warp_context_drag_active_slider(awin->warp_ctx, local_x, local_y);
            if (changed && warp_context_get_active_slider_rect(awin->warp_ctx, &sx, &sy, &sw, &sh)) {
              request_window_interaction_refresh(awin);
              int title_h = awin->no_decoration ? 0 : 60;
              redraw_warp_region(&nextgen_ui_layer,
                                 awin->x + sx - 24,
                                 (awin->y - title_h) + sy - (int)awin->scroll_y - 24,
                                 awin->x + sx + sw + 24,
                                 (awin->y - title_h) + sy + sh - (int)awin->scroll_y + 24);
            }
          }
        } else if (awin->is_dragging) {
          awin->x += mouse_dx; awin->y += mouse_dy;
          g_svg_dirty = 1;
        } else if (awin->is_resizing) {
          if (awin->resize_mode == 1) { // BR
            awin->w += mouse_dx; awin->h += mouse_dy;
          } else if (awin->resize_mode == 2) { // BL
            awin->x += mouse_dx; awin->w -= mouse_dx; awin->h += mouse_dy;
          } else if (awin->resize_mode == 3) { // TR
            awin->w += mouse_dx; awin->y += mouse_dy; awin->h -= mouse_dy;
          } else if (awin->resize_mode == 4) { // TL
            awin->x += mouse_dx; awin->w -= mouse_dx; awin->y += mouse_dy; awin->h -= mouse_dy;
          }
          
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

          if (c == KEY_UP || c == KEY_DOWN) {
            if (g_active_window_index >= 0) {
              window_t *swin = &g_windows[g_active_window_index];
              float delta = (c == KEY_UP) ? 120.0f : -120.0f;
              if (apply_window_scroll_delta(swin, delta)) {
                int bx0, by0, bx1, by1;
                get_window_draw_bounds(swin, &bx0, &by0, &bx1, &by1);
                redraw_warp_region(&nextgen_ui_layer, bx0, by0, bx1, by1);
              }
            }
          } else if (c == '\b') {
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
      }

      if (g_svg_dirty) {
        redraw_warp_svg(&nextgen_ui_layer);
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
warpdesktop_frame_done:;
    }

    // 常時再描画
    cpu_idle = 0;
    screen_refresh();
  }
}
