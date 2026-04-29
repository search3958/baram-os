#ifndef WARP_ENGINE_H
#define WARP_ENGINE_H

#include "drivers.h"
#include "warp_draw.h"
#include <stdint.h>

#define MAX_VARS 128
#define MAX_NODES 512
#define MAX_TEXTS 256
#define MAX_TOKENS 4096
#define MAX_SCRIPTS 16
#define MAX_SCRIPT_BLOCKS 16
#define MAX_DYNAMIC_NODES 32
#define MAX_SCREENS 8

typedef struct warp_context warp_context_t;

warp_context_t* warp_context_create(const char* code);
void warp_context_destroy(warp_context_t* ctx);
void warp_context_update(warp_context_t* ctx, int width, int height);
const char* warp_context_get_svg(warp_context_t* ctx);
const warp_draw_op_t* warp_context_get_draw_ops(warp_context_t* ctx, int* out_count);
void warp_context_draw_texts(warp_context_t* ctx, layer_t* layer, int off_x, int off_y, float scale);
void warp_context_click(warp_context_t* ctx, int x, int y);
int warp_context_is_dirty(warp_context_t* ctx);
void warp_context_clear_dirty(warp_context_t* ctx);
void warp_context_set_state(warp_context_t* ctx, const char* key, const char* val);
void warp_context_set_mouse(warp_context_t* ctx, int x, int y);
int warp_context_get_node_count(warp_context_t* ctx);
void warp_context_get_node_info(warp_context_t* ctx, int index, int* x, int* y, int* w, int* h, int* is_dirty);
const char* warp_context_get_node_prev_svg(warp_context_t* ctx, int index); // Dummy for compatibility if needed
const char* warp_context_get_node_svg(warp_context_t* ctx, int index);
void warp_context_get_node_prev_rect(warp_context_t* ctx, int index, int* x, int* y, int* w, int* h);
const char* warp_context_get_status(warp_context_t* ctx);
int warp_context_get_header_info(warp_context_t* ctx, char* out_text, int max_len, int* out_action_count);
void warp_context_get_header_action_info(warp_context_t* ctx, int action_index, char* out_text, int max_len);
void warp_context_click_header_action(warp_context_t* ctx, int action_index);
int warp_context_is_dev_event_check(warp_context_t* ctx);
float warp_context_get_scroll_y(warp_context_t* ctx);

// Global State / Settings Sync
void set_w1_global(const char *key, const char *val);
const char *get_w1_global(const char *key);

void warp_context_set_scroll_y(warp_context_t* ctx, float y);
int warp_context_get_content_height(warp_context_t* ctx);
const char* warp_context_get_screen_svg(warp_context_t* ctx, const char* screen_id, int* content_height);
void warp_context_set_screen_scroll(warp_context_t* ctx, const char* screen_id, float scroll_y);
float warp_context_get_screen_scroll(warp_context_t* ctx, const char* screen_id);
int warp_context_drag_active_slider(warp_context_t* ctx, int x, int y);
void warp_context_end_slider_drag(warp_context_t* ctx);
int warp_context_get_active_slider_rect(warp_context_t* ctx, int* x, int* y, int* w, int* h);

// Squircle rendering helpers for consistent UI
char *warp_stpcpy(char *dest, const char *src);
char *warp_strcat(char *dest, const char *src);
char *warp_strncat(char *dest, const char *src, size_t n);
char *append_fixed3(char *p, float v);
char *append_int(char *p, int v);
char* emit_squircle_shape_to(char *p, int x, int y, int w, int h, float radius,
                             const char *fill, const char *extra);
int measure_ttf_width(const char *str, float font_size);

#endif
