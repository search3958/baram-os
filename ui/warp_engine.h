#ifndef WARP_ENGINE_H
#define WARP_ENGINE_H

#include "drivers.h"
#include <stdint.h>

#define MAX_VARS 128
#define MAX_NODES 512
#define MAX_TEXTS 256
#define MAX_TOKENS 4096
#define MAX_SCRIPTS 16
#define MAX_SCRIPT_BLOCKS 16
#define MAX_DYNAMIC_NODES 32

typedef struct warp_context warp_context_t;

warp_context_t* warp_context_create(const char* code);
void warp_context_destroy(warp_context_t* ctx);
void warp_context_update(warp_context_t* ctx, int width, int height);
const char* warp_context_get_svg(warp_context_t* ctx);
void warp_context_draw_texts(warp_context_t* ctx, layer_t* layer, int off_x, int off_y);
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

#endif
