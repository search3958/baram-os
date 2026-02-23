#ifndef MOUSE_H
#define MOUSE_H

#include <stdint.h>

/* PS/2マウスを初期化し、ハンドラをインストールする */
void mouse_install();

/* マウスの座標 (外部から参照可能) */
extern int32_t mouse_x;
extern int32_t mouse_y;

#endif /* MOUSE_H */
