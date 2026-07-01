#ifndef MOUSE_H
#define MOUSE_H

#include <stdint.h>

void init_mouse(void);
void get_mouse_position(int* x, int* y);
void set_mouse_position(int x, int y);

#endif
