/**
 * @file keyboard.c
 * @brief キーボード入力機能の実装
 * @license MIT
 */

#include "keyboard.h"

#ifdef __x86_64__
// x86_64用のPS/2キーボードコントローラ操作

#define PS2_DATA_PORT   0x60
#define PS2_STATUS_PORT 0x64
#define PS2_COMMAND_PORT 0x64

// PS/2ステータスレジスタのビット
#define PS2_STATUS_OUTPUT_FULL 0x01
#define PS2_STATUS_INPUT_FULL  0x02

static uint8_t keyboard_read_scancode(void) {
    // データが来るまで待つ
    while ((inb(PS2_STATUS_PORT) & PS2_STATUS_OUTPUT_FULL) == 0) {
        // 待機
    }
    return inb(PS2_DATA_PORT);
}

void keyboard_init(struct KeyboardState* keyboard) {
    keyboard->head = 0;
    keyboard->tail = 0;
    keyboard->count = 0;
    keyboard->shift_pressed = false;
    keyboard->ctrl_pressed = false;
    keyboard->alt_pressed = false;
    
    // キーボードを有効化
    // 実際にはPS/2コントローラの初期化が必要
}

void keyboard_process(struct KeyboardState* keyboard) {
    // PS/2ポートからスキャンコードを読み取る
    if (inb(PS2_STATUS_PORT) & PS2_STATUS_OUTPUT_FULL) {
        uint8_t scancode = inb(PS2_DATA_PORT);
        
        // キー解放コード (0x80が追加される)
        bool key_released = (scancode & 0x80) != 0;
        scancode &= 0x7F;
        
        if (!key_released) {
            // キー押下処理
            switch (scancode) {
                case 0x2A: // Left Shift
                case 0x36: // Right Shift
                    keyboard->shift_pressed = true;
                    break;
                case 0x1D: // Ctrl
                    keyboard->ctrl_pressed = true;
                    break;
                case 0x38: // Alt
                    keyboard->alt_pressed = true;
                    break;
                default:
                    // バッファに追加
                    if (keyboard->count < KEYBOARD_BUFFER_SIZE) {
                        keyboard->buffer[keyboard->tail] = scancode;
                        keyboard->tail = (keyboard->tail + 1) % KEYBOARD_BUFFER_SIZE;
                        keyboard->count++;
                    }
                    break;
            }
        } else {
            // キー解放処理
            switch (scancode) {
                case 0x2A: // Left Shift
                case 0x36: // Right Shift
                    keyboard->shift_pressed = false;
                    break;
                case 0x1D: // Ctrl
                    keyboard->ctrl_pressed = false;
                    break;
                case 0x38: // Alt
                    keyboard->alt_pressed = false;
                    break;
            }
        }
    }
}

#else
// ARM64用 (ダミー実装 - 実際のハードウェアに依存)

void keyboard_init(struct KeyboardState* keyboard) {
    keyboard->head = 0;
    keyboard->tail = 0;
    keyboard->count = 0;
    keyboard->shift_pressed = false;
    keyboard->ctrl_pressed = false;
    keyboard->alt_pressed = false;
}

void keyboard_process(struct KeyboardState* keyboard) {
    // ARM64ではUARTや他のインターフェースを使用
    // 実際の実装はプラットフォームに依存
}

#endif

uint8_t keyboard_get_key(struct KeyboardState* keyboard) {
    if (keyboard->count == 0) {
        return 0;
    }
    
    uint8_t key = keyboard->buffer[keyboard->head];
    keyboard->head = (keyboard->head + 1) % KEYBOARD_BUFFER_SIZE;
    keyboard->count--;
    
    return key;
}

bool keyboard_has_key(struct KeyboardState* keyboard) {
    return keyboard->count > 0;
}
