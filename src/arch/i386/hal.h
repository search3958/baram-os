#ifndef I386_HAL_H
#define I386_HAL_H

#include "../hal/hal.h"

void i386_hal_init(void *framebuffer_addr, uint32_t width, uint32_t height, uint32_t pitch);
void i386_putchar(char c);

#endif // I386_HAL_H
