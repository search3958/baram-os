/*
 * Keyboard driver for x86_64
 * License: MIT License
 */

#include <stdint.h>
#include <stdbool.h>
#include "keyboard.h"

#define KBD_DATA_PORT 0x60
#define KBD_STATUS_PORT 0x64

static uint8_t last_scancode = 0;
static bool key_released = false;

void init_keyboard(void) {
    // In real implementation, would initialize PS/2 controller
    // For UEFI, we'll use simple polling
    last_scancode = 0;
}

uint8_t poll_keyboard(void) {
    // Simplified keyboard polling
    // In real implementation, would check status register and read data
    if (key_released) {
        key_released = false;
        return 0; // Key release event
    }
    
    // Return last scancode if available
    if (last_scancode != 0) {
        uint8_t code = last_scancode;
        last_scancode = 0;
        key_released = true;
        return code;
    }
    
    return 0;
}

// For testing - simulate key press
void simulate_key_press(uint8_t scancode) {
    last_scancode = scancode;
}
