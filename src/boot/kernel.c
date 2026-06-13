#include "boot.h"
#include "../hal/hal.h"

void kernel_main(boot_info_t *info) {
    // Initialize HAL with framebuffer info from bootloader
    if (info && info->framebuffer_addr) {
        g_fb.base = info->framebuffer_addr;
        g_fb.width = info->framebuffer_width;
        g_fb.height = info->framebuffer_height;
        g_fb.pitch = info->framebuffer_pitch;
        g_fb.bpp = info->framebuffer_bpp;
    }
    
    // Clear screen to black
    hal_clear_screen();
    
    // Fill background with a nice blue color
    hal_fill_rect(0, 0, g_fb.width, g_fb.height, 0x00008080);
    
    // Draw mouse cursor at center of screen
    int mouse_x = g_fb.width / 2;
    int mouse_y = g_fb.height / 2;
    hal_draw_mouse(mouse_x, mouse_y);
    
    // Infinite loop - OS is running!
    while (1) {
#if defined(__aarch64__) || defined(__arm__)
        __asm__ volatile ("wfi");
#else
        __asm__ volatile ("hlt");
#endif
    }
}
