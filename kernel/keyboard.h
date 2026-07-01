/**
 * @file keyboard.h
 * @brief キーボード入力機能
 * @license MIT
 */

#ifndef KEYBOARD_H
#define KEYBOARD_H

#include "kernel.h"

// キーボードバッファサイズ
#define KEYBOARD_BUFFER_SIZE 256

// キーコード定義 (US配列基準)
#define KEY_ESCAPE      0x01
#define KEY_1           0x02
#define KEY_2           0x03
#define KEY_3           0x04
#define KEY_4           0x05
#define KEY_5           0x06
#define KEY_6           0x07
#define KEY_7           0x08
#define KEY_8           0x09
#define KEY_9           0x0A
#define KEY_0           0x0B
#define KEY_Q           0x10
#define KEY_W           0x11
#define KEY_E           0x12
#define KEY_R           0x13
#define KEY_T           0x14
#define KEY_Y           0x15
#define KEY_U           0x16
#define KEY_I           0x17
#define KEY_O           0x18
#define KEY_P           0x19
#define KEY_A           0x1E
#define KEY_S           0x1F
#define KEY_D           0x20
#define KEY_F           0x21
#define KEY_G           0x22
#define KEY_H           0x23
#define KEY_J           0x24
#define KEY_K           0x25
#define KEY_L           0x26
#define KEY_Z           0x2C
#define KEY_X           0x2D
#define KEY_C           0x2E
#define KEY_V           0x2F
#define KEY_B           0x30
#define KEY_N           0x31
#define KEY_M           0x32
#define KEY_ENTER       0x1C
#define KEY_BACKSPACE   0x0E
#define KEY_TAB         0x0F
#define KEY_SPACE       0x39
#define KEY_UP          0xC8
#define KEY_DOWN        0xD0
#define KEY_LEFT        0xCB
#define KEY_RIGHT       0xCD

// キーボード状態構造体
struct KeyboardState {
    uint8_t buffer[KEYBOARD_BUFFER_SIZE];
    uint32_t head;
    uint32_t tail;
    uint32_t count;
    bool shift_pressed;
    bool ctrl_pressed;
    bool alt_pressed;
};

/**
 * @brief キーボードの初期化
 * @param keyboard KeyboardState構造体へのポインタ
 */
void keyboard_init(struct KeyboardState* keyboard);

/**
 * @brief キーボード入力の処理
 * @param keyboard KeyboardState構造体へのポインタ
 */
void keyboard_process(struct KeyboardState* keyboard);

/**
 * @brief キー入力を取得
 * @param keyboard KeyboardState構造体へのポインタ
 * @return キーコード、ない場合は0
 */
uint8_t keyboard_get_key(struct KeyboardState* keyboard);

/**
 * @brief バッファにキーがあるか確認
 * @param keyboard KeyboardState構造体へのポインタ
 * @return true: キーあり, false: キーなし
 */
bool keyboard_has_key(struct KeyboardState* keyboard);

#endif // KEYBOARD_H
