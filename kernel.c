#include <stdint.h>
#include "font.h"

// 構造体のパディングを禁止し、Multibootの仕様とメモリ配置を完全に一致させる
struct multiboot_info {
    uint32_t flags;
    uint32_t mem_lower;
    uint32_t mem_upper;
    uint32_t boot_device;
    uint32_t cmdline;
    uint32_t mods_count;
    uint32_t mods_addr;
    uint32_t syms[4];
    uint32_t mmap_length;
    uint32_t mmap_addr;
    uint32_t drives_length;
    uint32_t drives_addr;
    uint32_t config_table;
    uint32_t boot_loader_name;
    uint32_t apm_table;
    uint32_t vbe_control_info;
    uint32_t vbe_mode_info;
    uint16_t vbe_mode;
    uint16_t vbe_interface_seg;
    uint16_t vbe_interface_off;
    uint16_t vbe_interface_len;
    uint64_t framebuffer_addr;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint8_t  framebuffer_bpp;
    uint8_t  framebuffer_type;
    uint8_t  color_info[6];
} __attribute__((packed));

// シンプルで確実なピクセル描画
static inline void draw_pixel(uint8_t* fb, uint32_t pitch, int x, int y, uint32_t color) {
    // 32bitカラー(4バイト/ピクセル)であることを前提に計算
    uint32_t* target = (uint32_t*)(fb + (y * pitch) + (x * 4));
    *target = color;
}

// 仕様通りの2bitアンチエイリアス描画
void draw_char(uint8_t* fb, uint32_t pitch, int x, int y, unsigned char c, uint32_t fg_color) {
    const unsigned char* glyph = font_table[c];
    
    // RGB抽出
    uint32_t r = (fg_color >> 16) & 0xFF;
    uint32_t g = (fg_color >> 8) & 0xFF;
    uint32_t b = fg_color & 0xFF;

    for (int i = 0; i < 16; i++) {
        for (int b_idx = 0; b_idx < 2; b_idx++) {
            // 1バイト(4ピクセル分)を取得
            unsigned char data = glyph[i * 2 + b_idx];

            for (int p = 0; p < 4; p++) {
                // ビットシフト: p=0(左)なら6bitシフト, p=3(右)なら0bitシフト
                int level = (data >> (6 - p * 2)) & 0x03;

                if (level > 0) {
                    // levelに合わせて輝度を計算
                    uint32_t cr = (r * level) / 3;
                    uint32_t cg = (g * level) / 3;
                    uint32_t cb = (b * level) / 3;
                    uint32_t color = (cr << 16) | (cg << 8) | cb;
                    
                    draw_pixel(fb, pitch, x + (b_idx * 4) + p, y + i, color);
                }
            }
        }
    }
}

void draw_string(uint8_t* fb, uint32_t pitch, int x, int y, const char* str, uint32_t color) {
    for (int i = 0; str[i] != '\0'; i++) {
        draw_char(fb, pitch, x + (i * 8), y, (unsigned char)str[i], color);
    }
}

void kmain(uint32_t magic, struct multiboot_info* mbi) {
    // グラフィック情報の有無をチェック
    if (!(mbi->flags & (1 << 12))) return;

    uint8_t* fb = (uint8_t*)(uintptr_t)mbi->framebuffer_addr;
    uint32_t pitch  = mbi->framebuffer_pitch;
    uint32_t width  = mbi->framebuffer_width;
    uint32_t height = mbi->framebuffer_height;

    // 1. 画面を紺色でクリア
    for (uint32_t y = 0; y < height; y++) {
        uint32_t* row = (uint32_t*)(fb + (y * pitch));
        for (uint32_t x = 0; x < width; x++) {
            row[x] = 0x00003366; 
        }
    }

    // 2. 画面中央に文字を表示
    // width=854なら (427 - 44), height=480なら (240 - 8) あたり
    int start_x = (width / 2) - 44;
    int start_y = (height / 2) - 8;
    draw_string(fb, pitch, start_x, start_y, "Hello, GUI!", 0xFFFFFF);

    while (1) {
        __asm__ __volatile__("hlt");
    }
}