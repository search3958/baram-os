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
    
    // Show mouse cursor at center of screen
    int mouse_x = g_fb.width / 2;
    int mouse_y = g_fb.height / 2;
    hal_mouse_show(mouse_x, mouse_y);
    
    // Simple mouse movement demo - move in a circle
    int center_x = g_fb.width / 2;
    int center_y = g_fb.height / 2;
    int radius = 100;
    int angle = 0;
    
    // Infinite loop - OS is running!
    while (1) {
        // Calculate circular motion
        int new_x = center_x + (radius * ((angle % 360) < 180 ? 1 : -1));
        int new_y = center_y + (radius * ((angle % 360) < 90 || (angle % 360) >= 270 ? 1 : -1));
        
        // Move mouse (simple demo pattern)
        if (angle % 10 == 0) {
            hal_mouse_move(new_x, new_y);
        }
        
        angle++;
        
        // Small delay
        for (volatile int i = 0; i < 100000; i++);
        
#if defined(__aarch64__) || defined(__arm__)
        __asm__ volatile ("wfi");
#else
        __asm__ volatile ("hlt");
#endif
    }
}
