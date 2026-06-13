#include "hal.h"
#include "../hal/hal.h"

extern framebuffer_t g_fb;

void i386_hal_init(void *framebuffer_addr, uint32_t width, uint32_t height, uint32_t pitch) {
    g_fb.base = framebuffer_addr;
    g_fb.width = width;
    g_fb.height = height;
    g_fb.pitch = pitch;
    g_fb.bpp = 32;
}

static void *g_vga_buffer = (void *)0xB8000;
static int g_cursor_x = 0;
static int g_cursor_y = 0;

void i386_putchar(char c) {
    if (c == '\n') {
        g_cursor_x = 0;
        g_cursor_y++;
        return;
    }
    
    if (g_cursor_x >= 80) {
        g_cursor_x = 0;
        g_cursor_y++;
    }
    
    if (g_cursor_y >= 25) {
        g_cursor_y = 0;
    }
    
    uint16_t *buffer = (uint16_t *)g_vga_buffer;
    buffer[g_cursor_y * 80 + g_cursor_x] = (0x0F << 8) | c;
    g_cursor_x++;
}

void hal_init(void) {
    i386_hal_init(0, 0, 0, 0);
}

void hal_putchar(char c) {
    i386_putchar(c);
}
