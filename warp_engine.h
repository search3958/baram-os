#ifndef WARP_ENGINE_H
#define WARP_ENGINE_H

#include "drivers.h"
#include <stdint.h>

void warp_engine_init(const char* code);
void warp_engine_update(void);
const char* warp_engine_get_svg(void);
void warp_engine_draw_texts(layer_t* layer);
void warp_engine_click(int x, int y);

#endif
