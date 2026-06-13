#include "hal.h"
#include "../hal/hal.h"

extern framebuffer_t g_fb;

void arm64_hal_init(void *framebuffer_addr, uint32_t width, uint32_t height, uint32_t pitch) {
    g_fb.base = framebuffer_addr;
    g_fb.width = width;
    g_fb.height = height;
    g_fb.pitch = pitch;
    g_fb.bpp = 32;
}

// Simple UART output for ARM (placeholder addresses - varies by board)
#define MMIO_BASE       0x3F000000
#define GPFSEL1         ((volatile unsigned int*)(MMIO_BASE + 0x200004))
#define GPSET0          ((volatile unsigned int*)(MMIO_BASE + 0x20001C))
#define GPCLR0          ((volatile unsigned int*)(MMIO_BASE + 0x200028))
#define GPPUD           ((volatile unsigned int*)(MMIO_BASE + 0x200094))
#define GPPUDCLK0       ((volatile unsigned int*)(MMIO_BASE + 0x200098))
#define AUX_ENABLES     ((volatile unsigned int*)(MMIO_BASE + 0x215004))
#define AUX_MU_IO_REG   ((volatile unsigned int*)(MMIO_BASE + 0x215040))
#define AUX_MU_IER_REG  ((volatile unsigned int*)(MMIO_BASE + 0x215044))
#define AUX_MU_IIR_REG  ((volatile unsigned int*)(MMIO_BASE + 0x215048))
#define AUX_MU_LCR_REG  ((volatile unsigned int*)(MMIO_BASE + 0x21504C))
#define AUX_MU_MCR_REG  ((volatile unsigned int*)(MMIO_BASE + 0x215050))
#define AUX_MU_LSR_REG  ((volatile unsigned int*)(MMIO_BASE + 0x215054))

static int g_cursor_x = 0;
static int g_cursor_y = 0;

void arm64_putchar(char c) {
    // Placeholder - actual UART implementation depends on specific ARM board
    (void)c;
}

void hal_init(void) {
    arm64_hal_init(0, 0, 0, 0);
}

void hal_putchar(char c) {
    arm64_putchar(c);
}
