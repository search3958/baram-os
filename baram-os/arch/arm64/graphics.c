/*
 * ARM64 Graphics driver
 * License: MIT License
 */

#include <stdint.h>
#include "graphics.h"

static uint32_t* framebuffer = NULL;
static uint32_t fb_width = 0;
static uint32_t fb_height = 0;
static uint32_t fb_pitch = 0;

void init_graphics(uint32_t* fb, uint32_t width, uint32_t height, uint32_t pitch) {
    framebuffer = fb;
    fb_width = width;
    fb_height = height;
    fb_pitch = pitch;
}

void draw_pixel(uint32_t x, uint32_t y, uint32_t color) {
    if (x < fb_width && y < fb_height) {
        framebuffer[y * fb_pitch + x] = color;
    }
}

void clear_screen(uint32_t color) {
    for (uint32_t y = 0; y < fb_height; y++) {
        for (uint32_t x = 0; x < fb_width; x++) {
            framebuffer[y * fb_pitch + x] = color;
        }
    }
}

void draw_cursor(int x, int y) {
    const int CURSOR_SIZE = 16;
    
    for (int dy = 0; dy < CURSOR_SIZE; dy++) {
        for (int dx = 0; dx < CURSOR_SIZE; dx++) {
            int px = x + dx;
            int py = y + dy;
            
            if (px >= 0 && px < (int)fb_width && py >= 0 && py < (int)fb_height) {
                framebuffer[py * fb_pitch + px] = 0xFFFFFFFF;
            }
        }
    }
}

void draw_string(const char* str, uint32_t x, uint32_t y, uint32_t color) {
    while (*str && x < fb_width) {
        framebuffer[y * fb_pitch + x] = color;
        str++;
        x++;
    }
}
