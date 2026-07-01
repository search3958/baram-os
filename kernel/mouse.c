/**
 * @file mouse.c
 * @brief マウスカーソル機能の実装
 * @license MIT
 */

#include "mouse.h"

// 簡単な四角形マウスカーソル (16x16)
// 1: 描画するピクセル, 0: 描画しない
static const uint8_t mouse_cursor_bitmap[MOUSE_CURSOR_HEIGHT][MOUSE_CURSOR_WIDTH] = {
    {1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0},
    {1,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0},
    {1,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0},
    {1,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0},
    {1,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0},
    {1,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0},
    {1,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0},
    {1,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0},
    {1,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0},
    {1,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0},
    {1,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0},
    {1,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0},
    {1,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0},
    {1,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0},
    {1,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0},
    {1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1}
};

void mouse_init(struct MouseState* mouse) {
    mouse->x = 100;  // 初期位置
    mouse->y = 100;
    mouse->left_button = false;
    mouse->right_button = false;
    mouse->middle_button = false;
    mouse->visible = true;
    mouse->color = 0xFFFFFFFF; // 白色
}

void mouse_draw_cursor(struct Framebuffer* fb, struct MouseState* mouse) {
    if (!mouse->visible) {
        return;
    }
    
    // マウスカーソルを四角形で描画
    // シンプルな実装：マウス位置に小さな四角形を描画
    uint32_t cursor_size = 16;
    
    // カーソルの背景をクリア（黒色）
    framebuffer_draw_rectangle(fb, 
                               (uint32_t)mouse->x, 
                               (uint32_t)mouse->y, 
                               cursor_size, 
                               cursor_size, 
                               0x00000000);
    
    // カーソル本体を描画（白色の枠線）
    framebuffer_draw_rectangle(fb, 
                               (uint32_t)mouse->x, 
                               (uint32_t)mouse->y, 
                               cursor_size, 
                               cursor_size, 
                               mouse->color);
    
    // 内部を少し埋める（十字型）
    for (int i = 2; i < cursor_size - 2; i++) {
        framebuffer_draw_pixel(fb, (uint32_t)(mouse->x + i), (uint32_t)(mouse->y + i), mouse->color);
    }
}

void mouse_update(struct MouseState* mouse) {
    // 将来的に実際のマウス入力から状態を更新
    // 今はダミー
}

void mouse_move(struct MouseState* mouse, int32_t dx, int32_t dy,
                uint32_t fb_width, uint32_t fb_height) {
    mouse->x += dx;
    mouse->y += dy;
    
    // 画面外に出ないように制限
    if (mouse->x < 0) {
        mouse->x = 0;
    }
    if (mouse->y < 0) {
        mouse->y = 0;
    }
    if (mouse->x >= (int32_t)fb_width) {
        mouse->x = (int32_t)fb_width - 1;
    }
    if (mouse->y >= (int32_t)fb_height) {
        mouse->y = (int32_t)fb_height - 1;
    }
}
