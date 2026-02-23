#include "mouse.h"
#include "irq.h"
#include "io.h"

// マウスの現在の座標
int32_t mouse_x = 0;
int32_t mouse_y = 0;

// マウスパケットのサイクル
static uint8_t mouse_cycle = 0;
// マウスパケットデータ
static int8_t  mouse_packet[3];

// PS/2コントローラにコマンドを送信する前に待機
void mouse_wait(uint8_t a_type) {
    uint32_t timeout = 100000;
    if (a_type == 0) {
        while (timeout--) {
            if ((inb(0x64) & 1) == 1) return;
        }
        return;
    } else {
        while (timeout--) {
            if ((inb(0x64) & 2) == 0) return;
        }
        return;
    }
}

// マウスにコマンドを書き込む
void mouse_write(uint8_t a_write) {
    mouse_wait(1);
    outb(0x64, 0xD4);
    mouse_wait(1);
    outb(0x60, a_write);
}

// マウスからの応答を読み込む
uint8_t mouse_read() {
    mouse_wait(0);
    return inb(0x60);
}

// IRQ12のマウスハンドラ
static void mouse_handler(struct regs *r) {
    // マウスからバイトを読み込む
    uint8_t data = inb(0x60);

    // パケットを解析
    // (https://wiki.osdev.org/PS/2_Mouse)
    switch (mouse_cycle) {
        case 0:
            // 第1バイト: Y overflow, X overflow, Y sign, X sign, 1, Middle Btn, Right Btn, Left Btn
            // 厳密なチェックを一旦削除して、より多くの環境で動作するようにする
            mouse_packet[0] = data;
            mouse_cycle++;
            break;
        case 1:
            // 第2バイト: X movement
            mouse_packet[1] = data;
            mouse_cycle++;
            break;
        case 2:
            // 第3バイト: Y movement
            mouse_packet[2] = data;
            
            // パケットが揃ったので座標を更新
            int32_t delta_x = mouse_packet[1];
            int32_t delta_y = mouse_packet[2];

            // Y方向の動きは逆
            mouse_x += delta_x;
            mouse_y -= delta_y;

            // 画面範囲外に出ないようにクランプ
            // TODO: 画面の幅と高さをどこかから取得する必要がある
            if (mouse_x < 0) mouse_x = 0;
            if (mouse_y < 0) mouse_y = 0;
            if (mouse_x > 1279) mouse_x = 1279; // 仮の幅
            if (mouse_y > 719) mouse_y = 719; // 仮の高さ

            mouse_cycle = 0; // 次のパケットのためにリセット
            break;
    }
}

// マウスドライバをインストール
void mouse_install() {
    uint8_t status;

    // 1. マウス(Auxiliary Device)を有効化
    mouse_wait(1);
    outb(0x64, 0xA8);

    // 2. PS/2コントローラ設定バイトを取得
    mouse_wait(1);
    outb(0x64, 0x20);
    mouse_wait(0);
    status = (inb(0x60) | 2); // 第2PS/2ポートの割り込みを有効化

    // 3. 新しい設定バイトを書き込む
    mouse_wait(1);
    outb(0x64, 0x60);
    mouse_wait(1);
    outb(0x60, status);

    // 4. マウスをデフォルト設定にする
    mouse_write(0xF6);
    mouse_read(); // ACKを待つ

    // 5. データパケットのストリーミングを有効化
    mouse_write(0xF4);
    mouse_read(); // ACKを待つ

    // 6. IRQ12ハンドラをインストール
    irq_install_handler(12, mouse_handler);

    // 7. PICでIRQ12を有効化
    outb(0x21, inb(0x21) & 0xFB); // マスターPIC: IRQ2 (カスケード) を有効化
    outb(0xA1, inb(0xA1) & 0xF7); // スレーブPIC: IRQ12 (マウス) を有効化
}
