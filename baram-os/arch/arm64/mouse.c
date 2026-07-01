/*
 * ARM64 Mouse driver
 * License: MIT License
 */

#include <stdint.h>
#include "mouse.h"

static int mouse_x = 100;
static int mouse_y = 100;

void init_mouse(void) {
    mouse_x = 100;
    mouse_y = 100;
}

void get_mouse_position(int* x, int* y) {
    if (x) *x = mouse_x;
    if (y) *y = mouse_y;
}

void set_mouse_position(int x, int y) {
    mouse_x = x;
    mouse_y = y;
}
