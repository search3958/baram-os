/**
 * @file main.c
 * @brief カーネルエントリーポイント
 * @license MIT
 */

#include "kernel.h"
#include "framebuffer.h"
#include "mouse.h"
#include "keyboard.h"

// メモリ操作関数の実装（リンカエラー回避）
void* memset(void* dest, int val, size_t count) {
    uint8_t* ptr = (uint8_t*)dest;
    for (size_t i = 0; i < count; i++) {
        ptr[i] = (uint8_t)val;
    }
    return dest;
}

void* memcpy(void* dest, const void* src, size_t count) {
    uint8_t* d = (uint8_t*)dest;
    const uint8_t* s = (const uint8_t*)src;
    for (size_t i = 0; i < count; i++) {
        d[i] = s[i];
    }
    return dest;
}

int memcmp(const void* ptr1, const void* ptr2, size_t count) {
    const uint8_t* p1 = (const uint8_t*)ptr1;
    const uint8_t* p2 = (const uint8_t*)ptr2;
    for (size_t i = 0; i < count; i++) {
        if (p1[i] != p2[i]) {
            return (int)p1[i] - (int)p2[i];
        }
    }
    return 0;
}

/**
 * @brief カーネルメインエントリポイント
 * @param framebuffer_info UEFI から受け取る FrameBuffer 情報
 */
void kernel_main(struct FramebufferInfo* fb_info) {
    // FrameBuffer の初期化
    struct Framebuffer fb;
    framebuffer_init(&fb, fb_info);
    
    // 画面をクリア (黒色)
    framebuffer_clear(&fb, 0x00000000);
    
    // マウスの初期化
    struct MouseState mouse;
    mouse_init(&mouse);
    
    // キーボードの初期化
    struct KeyboardState keyboard;
    keyboard_init(&keyboard);
    
    // デモ：画面中央にマウスカーソルを表示
    uint32_t demo_x = fb.width / 2 - 8;
    uint32_t demo_y = fb.height / 2 - 8;
    
    // マウス位置を設定
    mouse.x = (int32_t)demo_x;
    mouse.y = (int32_t)demo_y;
    
    // メインループ
    while (1) {
        // 画面クリア
        framebuffer_clear(&fb, 0x00000000);
        
        // マウスカーソルの描画（簡単な四角形）
        mouse_draw_cursor(&fb, &mouse);
        
        // キーボード入力の処理
        keyboard_process(&keyboard);
        
        // キー入力に応じてマウスを移動（デモ用）
        if (keyboard_has_key(&keyboard)) {
            uint8_t key = keyboard_get_key(&keyboard);
            int32_t dx = 0, dy = 0;
            
            switch (key) {
                case KEY_UP:    dy = -10; break;
                case KEY_DOWN:  dy = 10; break;
                case KEY_LEFT:  dx = -10; break;
                case KEY_RIGHT: dx = 10; break;
            }
            
            if (dx != 0 || dy != 0) {
                mouse_move(&mouse, dx, dy, fb.width, fb.height);
            }
        }
        
        // 次のフレーム準備
        mouse_update(&mouse);
        
        // CPU を休める
        halt();
    }
}
