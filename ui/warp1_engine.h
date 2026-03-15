#ifndef WARP1_ENGINE_H
#define WARP1_ENGINE_H

#include "drivers.h"
#include <stdint.h>

#define MAX_VARS 128
#define MAX_NODES 512
#define MAX_TEXTS 256
#define MAX_TOKENS 4096
#define MAX_SCRIPTS 16
#define MAX_SCRIPT_BLOCKS 16
#define MAX_DYNAMIC_NODES 32

typedef struct warp1_context warp1_context_t;

warp1_context_t* warp1_context_create(const char* code);
void warp1_context_destroy(warp1_context_t* ctx);
void warp1_context_update(warp1_context_t* ctx, int width, int height);
const char* warp1_context_get_svg(warp1_context_t* ctx);
void warp1_context_draw_texts(warp1_context_t* ctx, layer_t* layer, int off_x, int off_y);
void warp1_context_click(warp1_context_t* ctx, int x, int y);
int warp1_context_is_dirty(warp1_context_t* ctx);
void warp1_context_clear_dirty(warp1_context_t* ctx);
void warp1_context_set_state(warp1_context_t* ctx, const char* key, const char* val);
void warp1_context_set_mouse(warp1_context_t* ctx, int x, int y);
int warp1_context_get_node_count(warp1_context_t* ctx);
void warp1_context_get_node_info(warp1_context_t* ctx, int index, int* x, int* y, int* w, int* h, int* is_dirty);
const char* warp1_context_get_node_svg(warp1_context_t* ctx, int index);
void warp1_context_get_node_prev_rect(warp1_context_t* ctx, int index, int* x, int* y, int* w, int* h);
const char* warp1_context_get_status(warp1_context_t* ctx);
int warp1_context_get_header_info(warp1_context_t* ctx, char* out_text, int max_len, int* out_action_count);
void warp1_context_get_header_action_info(warp1_context_t* ctx, int action_index, char* out_text, int max_len);
void warp1_context_click_header_action(warp1_context_t* ctx, int action_index);

#endif
