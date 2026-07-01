/**
 * @file framebuffer.h
 * @brief FrameBuffer描画機能
 * @license MIT
 */

#ifndef FRAMEBUFFER_H
#define FRAMEBUFFER_H

#include "kernel.h"

// UEFIから受け取るFrameBuffer情報
struct FramebufferInfo {
    void* base_address;
    uint32_t width;
    uint32_t height;
    uint32_t pitch;       // 1ラインのバイト数
    uint32_t pixel_format; // 0: PixelRGB888, 1: PixelBGR888, 2: PixelRGBX8888, 3: PixelBGRX8888
};

// FrameBuffer構造体
struct Framebuffer {
    void* base;
    uint32_t width;
    uint32_t height;
    uint32_t pitch;
    uint32_t bytes_per_pixel;
    uint32_t pixel_format;
};

// ピクセルフォーマット
#define PIXEL_RGB888    0
#define PIXEL_BGR888    1
#define PIXEL_RGBX8888  2
#define PIXEL_BGRX8888  3

/**
 * @brief FrameBufferの初期化
 * @param fb Framebuffer構造体へのポインタ
 * @param info UEFIから受け取った情報
 */
void framebuffer_init(struct Framebuffer* fb, struct FramebufferInfo* info);

/**
 * @brief 画面全体のクリア
 * @param fb Framebuffer構造体へのポインタ
 * @param color 32ビットカラー (0xAARRGGBB)
 */
void framebuffer_clear(struct Framebuffer* fb, uint32_t color);

/**
 * @brief 1ピクセル描画
 * @param fb Framebuffer構造体へのポインタ
 * @param x X座標
 * @param y Y座標
 * @param color 32ビットカラー (0xAARRGGBB)
 */
void framebuffer_draw_pixel(struct Framebuffer* fb, uint32_t x, uint32_t y, uint32_t color);

/**
 * @brief 四角形描画
 * @param fb Framebuffer構造体へのポインタ
 * @param x 左上X座標
 * @param y 左上Y座標
 * @param w 幅
 * @param h 高さ
 * @param color 32ビットカラー (0xAARRGGBB)
 */
void framebuffer_draw_rectangle(struct Framebuffer* fb, uint32_t x, uint32_t y, 
                                 uint32_t w, uint32_t h, uint32_t color);

/**
 * @brief 線描画
 * @param fb Framebuffer構造体へのポインタ
 * @param x0 始点X座標
 * @param y0 始点Y座標
 * @param x1 終点X座標
 * @param y1 終点Y座標
 * @param color 32ビットカラー (0xAARRGGBB)
 */
void framebuffer_draw_line(struct Framebuffer* fb, uint32_t x0, uint32_t y0,
                            uint32_t x1, uint32_t y1, uint32_t color);

#endif // FRAMEBUFFER_H
