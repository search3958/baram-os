#ifndef GRAPHICS_H
#define GRAPHICS_H

#include <stdint.h>

void clear_screen(uint32_t color);
void draw_cursor(int x, int y);
void draw_pixel(uint32_t x, uint32_t y, uint32_t color);
void draw_string(const char* str, uint32_t x, uint32_t y, uint32_t color);

#endif
