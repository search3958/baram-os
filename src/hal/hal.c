#include "hal.h"

framebuffer_t g_fb = {0};

// Weak default implementations - architecture specific versions override these
__attribute__((weak))
void hal_init(void) {
    // Default empty implementation
}

__attribute__((weak))
void hal_putchar(char c) {
    // Default empty implementation
}

__attribute__((weak))
void hal_clear_screen(void) {
    if (g_fb.base) {
        uint32_t *pixels = (uint32_t *)g_fb.base;
        size_t count = (g_fb.width * g_fb.height);
        for (size_t i = 0; i < count; i++) {
            pixels[i] = 0x00000000; // Black
        }
    }
}

__attribute__((weak))
void hal_set_cursor(int x, int y) {
    // Default empty implementation
}

__attribute__((weak))
void hal_draw_pixel(int x, int y, uint32_t color) {
    if (!g_fb.base || x < 0 || y < 0 || x >= (int)g_fb.width || y >= (int)g_fb.height) {
        return;
    }
    
    uint32_t *pixels = (uint32_t *)g_fb.base;
    pixels[y * (g_fb.pitch / 4) + x] = color;
}

__attribute__((weak))
void hal_fill_rect(int x, int y, int w, int h, uint32_t color) {
    for (int iy = y; iy < y + h && iy < (int)g_fb.height; iy++) {
        for (int ix = x; ix < x + w && ix < (int)g_fb.width; ix++) {
            hal_draw_pixel(ix, iy, color);
        }
    }
}

// Simple mouse cursor (16x16 white arrow) with double buffering support
__attribute__((weak))
void hal_draw_mouse(int x, int y) {
    // Draw a simple 8x8 mouse cursor
    static const uint8_t mouse_bitmap[] = {
        0x80, 0xC0, 0xE0, 0xF0, 0xF8, 0xFC, 0xFE, 0xFF,
        0x7E, 0x3E, 0x1E, 0x0E, 0x06, 0x06, 0x00, 0x00
    };
    
    for (int dy = 0; dy < 16; dy++) {
        for (int dx = 0; dx < 8; dx++) {
            if (mouse_bitmap[dy] & (0x80 >> dx)) {
                hal_draw_pixel(x + dx, y + dy, 0xFFFFFFFF); // White
            } else {
                hal_draw_pixel(x + dx, y + dy, 0x00000000); // Black
            }
        }
    }
}

// Mouse state tracking
static int g_mouse_x = 0;
static int g_mouse_y = 0;
static int g_mouse_visible = 0;

__attribute__((weak))
void hal_mouse_move(int x, int y) {
    // Erase old mouse cursor if visible
    if (g_mouse_visible) {
        // Redraw background where mouse was (simplified - just clear area)
        for (int dy = 0; dy < 16; dy++) {
            for (int dx = 0; dx < 8; dx++) {
                hal_draw_pixel(g_mouse_x + dx, g_mouse_y + dy, 0x00008080); // Background color
            }
        }
    }
    
    // Update position
    g_mouse_x = x;
    g_mouse_y = y;
    g_mouse_visible = 1;
    
    // Draw new mouse cursor
    hal_draw_mouse(x, y);
}

__attribute__((weak))
void hal_mouse_show(int x, int y) {
    g_mouse_x = x;
    g_mouse_y = y;
    g_mouse_visible = 1;
    hal_draw_mouse(x, y);
}

__attribute__((weak))
void hal_mouse_hide(void) {
    if (g_mouse_visible) {
        // Erase mouse cursor
        for (int dy = 0; dy < 16; dy++) {
            for (int dx = 0; dx < 8; dx++) {
                hal_draw_pixel(g_mouse_x + dx, g_mouse_y + dy, 0x00008080); // Background color
            }
        }
        g_mouse_visible = 0;
    }
}
