/**
 * @file framebuffer.c
 * @brief FrameBuffer描画機能の実装
 * @license MIT
 */

#include "framebuffer.h"

void framebuffer_init(struct Framebuffer* fb, struct FramebufferInfo* info) {
    fb->base = info->base_address;
    fb->width = info->width;
    fb->height = info->height;
    fb->pitch = info->pitch;
    fb->pixel_format = info->pixel_format;
    
    // ピクセルフォーマットに基づくバイト数設定
    switch (info->pixel_format) {
        case PIXEL_RGB888:
        case PIXEL_BGR888:
            fb->bytes_per_pixel = 3;
            break;
        case PIXEL_RGBX8888:
        case PIXEL_BGRX8888:
            fb->bytes_per_pixel = 4;
            break;
        default:
            fb->bytes_per_pixel = 4; // デフォルトは32ビット
            break;
    }
}

void framebuffer_clear(struct Framebuffer* fb, uint32_t color) {
    uint8_t* ptr = (uint8_t*)fb->base;
    size_t total_pixels = fb->width * fb->height;
    
    for (size_t i = 0; i < total_pixels; i++) {
        switch (fb->bytes_per_pixel) {
            case 3:
                if (fb->pixel_format == PIXEL_BGR888) {
                    ptr[i * 3 + 0] = (color >> 16) & 0xFF; // B
                    ptr[i * 3 + 1] = (color >> 8) & 0xFF;  // G
                    ptr[i * 3 + 2] = color & 0xFF;         // R
                } else {
                    ptr[i * 3 + 0] = (color >> 16) & 0xFF; // R
                    ptr[i * 3 + 1] = (color >> 8) & 0xFF;  // G
                    ptr[i * 3 + 2] = color & 0xFF;         // B
                }
                break;
            case 4:
                if (fb->pixel_format == PIXEL_BGRX8888) {
                    ptr[i * 4 + 0] = (color >> 16) & 0xFF; // B
                    ptr[i * 4 + 1] = (color >> 8) & 0xFF;  // G
                    ptr[i * 4 + 2] = color & 0xFF;         // R
                    ptr[i * 4 + 3] = 0xFF;                 // X (unused)
                } else {
                    ptr[i * 4 + 0] = (color >> 16) & 0xFF; // R
                    ptr[i * 4 + 1] = (color >> 8) & 0xFF;  // G
                    ptr[i * 4 + 2] = color & 0xFF;         // B
                    ptr[i * 4 + 3] = 0xFF;                 // X (unused)
                }
                break;
        }
    }
}

void framebuffer_draw_pixel(struct Framebuffer* fb, uint32_t x, uint32_t y, uint32_t color) {
    if (x >= fb->width || y >= fb->height) {
        return; // 範囲外チェック
    }
    
    uint8_t* ptr = (uint8_t*)fb->base;
    size_t offset = y * fb->pitch + x * fb->bytes_per_pixel;
    
    switch (fb->bytes_per_pixel) {
        case 3:
            if (fb->pixel_format == PIXEL_BGR888) {
                ptr[offset + 0] = (color >> 16) & 0xFF; // B
                ptr[offset + 1] = (color >> 8) & 0xFF;  // G
                ptr[offset + 2] = color & 0xFF;         // R
            } else {
                ptr[offset + 0] = (color >> 16) & 0xFF; // R
                ptr[offset + 1] = (color >> 8) & 0xFF;  // G
                ptr[offset + 2] = color & 0xFF;         // B
            }
            break;
        case 4:
            if (fb->pixel_format == PIXEL_BGRX8888) {
                ptr[offset + 0] = (color >> 16) & 0xFF; // B
                ptr[offset + 1] = (color >> 8) & 0xFF;  // G
                ptr[offset + 2] = color & 0xFF;         // R
                ptr[offset + 3] = (color >> 24) & 0xFF; // A
            } else {
                ptr[offset + 0] = (color >> 16) & 0xFF; // R
                ptr[offset + 1] = (color >> 8) & 0xFF;  // G
                ptr[offset + 2] = color & 0xFF;         // B
                ptr[offset + 3] = (color >> 24) & 0xFF; // A
            }
            break;
    }
}

void framebuffer_draw_rectangle(struct Framebuffer* fb, uint32_t x, uint32_t y, 
                                 uint32_t w, uint32_t h, uint32_t color) {
    // 単純な実装：四角形の輪郭を描画
    framebuffer_draw_line(fb, x, y, x + w, y, color);           // 上辺
    framebuffer_draw_line(fb, x, y + h, x + w, y + h, color);   // 下辺
    framebuffer_draw_line(fb, x, y, x, y + h, color);           // 左辺
    framebuffer_draw_line(fb, x + w, y, x + w, y + h, color);   // 右辺
}

void framebuffer_draw_line(struct Framebuffer* fb, uint32_t x0, uint32_t y0,
                            uint32_t x1, uint32_t y1, uint32_t color) {
    int dx = (int)x1 - (int)x0;
    int dy = (int)y1 - (int)y0;
    int steps;
    
    // より大きい変化量を使用
    if (dx < 0) dx = -dx;
    if (dy < 0) dy = -dy;
    
    if (dx >= dy) {
        steps = dx;
    } else {
        steps = dy;
    }
    
    float x_increment = (float)(x1 - x0) / steps;
    float y_increment = (float)(y1 - y0) / steps;
    
    float x = (float)x0;
    float y = (float)y0;
    
    for (int i = 0; i <= steps; i++) {
        framebuffer_draw_pixel(fb, (uint32_t)x, (uint32_t)y, color);
        x += x_increment;
        y += y_increment;
    }
}
