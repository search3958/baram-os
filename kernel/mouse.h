/**
 * @file mouse.h
 * @brief マウスカーソル機能
 * @license MIT
 */

#ifndef MOUSE_H
#define MOUSE_H

#include "kernel.h"
#include "framebuffer.h"

// マウスカーソルのサイズ
#define MOUSE_CURSOR_WIDTH  16
#define MOUSE_CURSOR_HEIGHT 16

// マウス状態構造体
struct MouseState {
    int32_t x;              // X座標
    int32_t y;              // Y座標
    bool left_button;       // 左ボタン
    bool right_button;      // 右ボタン
    bool middle_button;     // 中ボタン
    bool visible;           // 表示フラグ
    uint32_t color;         // カーソル色
};

/**
 * @brief マウスの初期化
 * @param mouse MouseState構造体へのポインタ
 */
void mouse_init(struct MouseState* mouse);

/**
 * @brief マウスカーソルの描画
 * @param fb Framebuffer構造体へのポインタ
 * @param mouse MouseState構造体へのポインタ
 */
void mouse_draw_cursor(struct Framebuffer* fb, struct MouseState* mouse);

/**
 * @brief マウス状態の更新
 * @param mouse MouseState構造体へのポインタ
 */
void mouse_update(struct MouseState* mouse);

/**
 * @brief マウスカーソルの移動
 * @param mouse MouseState構造体へのポインタ
 * @param dx X方向の移動量
 * @param dy Y方向の移動量
 * @param fb_width 画面幅
 * @param fb_height 画面高さ
 */
void mouse_move(struct MouseState* mouse, int32_t dx, int32_t dy, 
                uint32_t fb_width, uint32_t fb_height);

#endif // MOUSE_H
