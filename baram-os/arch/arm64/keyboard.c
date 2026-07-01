/*
 * ARM64 Keyboard driver
 * License: MIT License
 */

#include <stdint.h>
#include <stdbool.h>
#include "keyboard.h"

static uint8_t last_scancode = 0;
static bool key_released = false;

void init_keyboard(void) {
    last_scancode = 0;
}

uint8_t poll_keyboard(void) {
    if (key_released) {
        key_released = false;
        return 0;
    }
    
    if (last_scancode != 0) {
        uint8_t code = last_scancode;
        last_scancode = 0;
        key_released = true;
        return code;
    }
    
    return 0;
}
