#ifndef X86_64_HAL_H
#define X86_64_HAL_H

#include "../hal/hal.h"

void x86_64_hal_init(void *framebuffer_addr, uint32_t width, uint32_t height, uint32_t pitch);
void x86_64_putchar(char c);

#endif // X86_64_HAL_H
