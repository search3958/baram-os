#ifndef HAL_H
#define HAL_H

#include <stdint.h>
#include <stddef.h>

// HAL Interface Functions
void hal_init(void);
void hal_putchar(char c);
void hal_clear_screen(void);
void hal_set_cursor(int x, int y);
void hal_draw_pixel(int x, int y, uint32_t color);
void hal_fill_rect(int x, int y, int w, int h, uint32_t color);
void hal_draw_mouse(int x, int y);

// Framebuffer structure
typedef struct {
    void *base;
    uint32_t width;
    uint32_t height;
    uint32_t pitch;
    uint32_t bpp;
} framebuffer_t;

extern framebuffer_t g_fb;

#endif // HAL_H
