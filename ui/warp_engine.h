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

const char* warp_engine_get_status(void);
void warp_engine_init(const char* code);
void warp_engine_update(int width, int height);
const char* warp_engine_get_svg(void);
void warp_engine_draw_texts(layer_t* layer, int off_x, int off_y);
void warp_engine_click(int x, int y);

int warp_engine_is_dirty(void);
void warp_engine_clear_dirty(void);
int warp_engine_get_node_count(void);
void warp_engine_get_node_info(int index, int* x, int* y, int* w, int* h, int* is_dirty);
const char* warp_engine_get_node_svg(int index);
void warp_engine_get_node_prev_rect(int index, int* x, int* y, int* w, int* h);

#endif
