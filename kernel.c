#include <stdint.h>
#include "idt.h"
#include "irq.h"
#include "mouse.h"

#define CURSOR_WIDTH 10
#define CURSOR_HEIGHT 15

/* Multiboot 1 構造体の定義（ビデオ情報の取得に必要） */
struct multiboot_info {
    uint32_t flags;
    uint32_t unused[21]; // オフセットを修正し、フレームバッファ情報に正しくアクセスする
    /* Framebuffer info (flags & 1 << 12 が真のとき有効) */
    uint64_t framebuffer_addr;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint8_t  framebuffer_bpp;
    uint8_t  framebuffer_type;
};

/* 32-bit ARGBピクセルを作成するマクロ */
#define ARGB(a, r, g, b) (((uint32_t)(a) << 24) | ((uint32_t)(r) << 16) | ((uint32_t)(g) << 8) | (uint32_t)(b))

// --- 描画関連のグローバル変数 ---
static uint32_t* video_mem_global;
static uint32_t screen_width, screen_height, screen_pitch;
static uint32_t cursor_bg[CURSOR_WIDTH * CURSOR_HEIGHT];
static int32_t old_mouse_x = -1, old_mouse_y = -1;

/* カーソルの下の背景を復元 */
void restore_cursor_bg() {
    if (old_mouse_x < 0) return; // 初回は復元しない
    for (int y = 0; y < CURSOR_HEIGHT; y++) {
        for (int x = 0; x < CURSOR_WIDTH; x++) {
            int target_x = old_mouse_x + x;
            int target_y = old_mouse_y + y;
            if (target_x < screen_width && target_y < screen_height) {
                uint32_t* row = (uint32_t*)((uint8_t*)video_mem_global + target_y * screen_pitch);
                row[target_x] = cursor_bg[y * CURSOR_WIDTH + x];
            }
        }
    }
}

/* カーソルの描画先となる背景を保存 */
void save_cursor_bg() {
    for (int y = 0; y < CURSOR_HEIGHT; y++) {
        for (int x = 0; x < CURSOR_WIDTH; x++) {
            int target_x = mouse_x + x;
            int target_y = mouse_y + y;
            if (target_x < screen_width && target_y < screen_height) {
                uint32_t* row = (uint32_t*)((uint8_t*)video_mem_global + target_y * screen_pitch);
                cursor_bg[y * CURSOR_WIDTH + x] = row[target_x];
            }
        }
    }
}

/* カーソルを描画 */
void draw_cursor() {
    uint32_t white = ARGB(255, 255, 255, 255);
    for (int y = 0; y < CURSOR_HEIGHT; y++) {
        for (int x = 0; x < CURSOR_WIDTH; x++) {
            int target_x = mouse_x + x;
            int target_y = mouse_y + y;
            if (target_x < screen_width && target_y < screen_height) {
                uint32_t* row = (uint32_t*)((uint8_t*)video_mem_global + target_y * screen_pitch);
                row[target_x] = white;
            }
        }
    }
}


void kmain(uint32_t magic, struct multiboot_info* mbi) {
    // 1. グラフィック情報があるか確認
    if (!(mbi->flags & (1 << 12))) {
        return;
    }

    // 2. フレームバッファ情報をグローバル変数に保存
    video_mem_global = (uint32_t*)(uintptr_t)mbi->framebuffer_addr;
    screen_width  = mbi->framebuffer_width;
    screen_height = mbi->framebuffer_height;
    screen_pitch  = mbi->framebuffer_pitch;

    // 3. 割り込みとマウスを初期化
    idt_install();
    irq_install();
    mouse_install();

    // 4. 背景を描画
    uint32_t dark_red = ARGB(255, 139, 0, 0);
    for (uint32_t y = 0; y < screen_height; y++) {
        uint32_t* row = (uint32_t*)((uint8_t*)video_mem_global + y * screen_pitch);
        for (uint32_t x = 0; x < screen_width; x++) {
            row[x] = dark_red;
        }
    }
    
    // 5. メインループ
    while (1) {
        if (old_mouse_x != mouse_x || old_mouse_y != mouse_y) {
            // 以前のカーソル位置の背景を復元
            restore_cursor_bg();

            // 新しいカーソル位置の背景を保存
            save_cursor_bg();

            // 新しい位置にカーソルを描画
            draw_cursor();

            // 座標を更新
            old_mouse_x = mouse_x;
            old_mouse_y = mouse_y;
        }
    }
}