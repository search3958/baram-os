#ifndef BOOT_H
#define BOOT_H

#include <stdint.h>

// Boot information structure passed from bootloader
typedef struct {
    void *framebuffer_addr;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_bpp;
    uint32_t mmap_addr;
    uint32_t mmap_length;
} boot_info_t;

// Entry point for the kernel
void kernel_main(boot_info_t *info);

#endif // BOOT_H
