#include <stdint.h>
#include "font.h"

struct multiboot_info {
    uint32_t flags;
    uint32_t unused1[10];
    uint32_t mmap_length;
    uint32_t mmap_addr;
    uint32_t unused2[9];
    uint64_t framebuffer_addr;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint8_t  framebuffer_bpp;
    uint8_t  framebuffer_type;
} __attribute__((packed));

void draw_char(uint8_t* fb, uint32_t pitch, int x, int y, unsigned char c, uint32_t fg_color) {
    const unsigned char* glyph = font_table[c];
    
    uint32_t r = (fg_color >> 16) & 0xFF;
    uint32_t g = (fg_color >> 8) & 0xFF;
    uint32_t b = fg_color & 0xFF;

    // 8x16ピクセル: 16行ループ
    for (int row = 0; row < 16; row++) {
        // 1行は2バイト (8ピクセル分)
        for (int b_idx = 0; b_idx < 2; b_idx++) {
            unsigned char data = glyph[row * 2 + b_idx];

            // 1バイト内の4ピクセルを処理
            for (int p = 0; p < 4; p++) {
                // 左から2ビットずつ抽出 (MSB側から)
                int level = (data >> (6 - p * 2)) & 0x03;

                if (level > 0) {
                    // アンチエイリアス計算 (輝度調整)
                    uint32_t cr = (r * level) / 3;
                    uint32_t cg = (g * level) / 3;
                    uint32_t cb = (b * level) / 3;
                    uint32_t final_color = (cr << 16) | (cg << 8) | cb;
                    
                    // 書き込み先の座標計算
                    uint32_t* target = (uint32_t*)(fb + ((y + row) * pitch) + ((x + (b_idx * 4) + p) * 4));
                    *target = final_color;
                }
            }
        }
    }
}

void kmain(uint32_t magic, struct multiboot_info* mbi) {
    // グラフィック情報チェック
    if (!(mbi->flags & (1 << 12))) return;

    uint8_t* fb = (uint8_t*)(uintptr_t)mbi->framebuffer_addr;
    uint32_t pitch = mbi->framebuffer_pitch;
    uint32_t width = mbi->framebuffer_width;
    uint32_t height = mbi->framebuffer_height;

    if (fb == 0) return;

    // 1. 画面を真っ赤にする (確実に反映を確認するため)
    for (uint32_t i = 0; i < height; i++) {
        uint32_t* row_ptr = (uint32_t*)(fb + (i * pitch));
        for (uint32_t j = 0; j < width; j++) {
            row_ptr[j] = 0x00FF0000; 
        }
    }

    // 2. 文字表示 ("Hello, GUI!")
    const char* msg = "Hello, GUI!";
    int start_x = (width / 2) - 40;
    int start_y = (height / 2) - 8;

    for (int i = 0; msg[i] != '\0'; i++) {
        draw_char(fb, pitch, start_x + (i * 8), start_y, (unsigned char)msg[i], 0xFFFFFFFF);
    }

    while (1) { __asm__ __volatile__("hlt"); }
}