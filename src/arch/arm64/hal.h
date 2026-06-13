#ifndef ARM64_HAL_H
#define ARM64_HAL_H

#include "../hal/hal.h"

void arm64_hal_init(void *framebuffer_addr, uint32_t width, uint32_t height, uint32_t pitch);
void arm64_putchar(char c);

#endif // ARM64_HAL_H
