#define VGA_WIDTH 80
#define VGA_HEIGHT 25
#define VIDEO_MEM (char*)0xb8000

// I/Oポート制御用：インラインアセンブリ
static inline unsigned char inb(unsigned short port) {
    unsigned char ret;
    __asm__ __volatile__ ("inb %1, %0" : "=a"(ret) : "Nd"(port));
    return ret;
}

// 画面を特定の色で塗りつぶす
void clear_screen(char color) {
    for (int i = 0; i < VGA_WIDTH * VGA_HEIGHT * 2; i += 2) {
        VIDEO_MEM[i] = ' ';
        VIDEO_MEM[i+1] = color;
    }
}

// 指定位置に文字列表示
void k_print(const char *str, int x, int y, char color) {
    char *vidptr = VIDEO_MEM + ((y * VGA_WIDTH + x) * 2);
    for (int i = 0; str[i] != '\0'; i++) {
        vidptr[i * 2] = str[i];
        vidptr[i * 2 + 1] = color;
    }
}

// GUI: 中央に窓を描画
void draw_window() {
    int w = 30, h = 10;
    int sx = (VGA_WIDTH - w) / 2, sy = (VGA_HEIGHT - h) / 2;

    for (int y = 0; y < h; y++) {
        for (int x = 0; x < w; x++) {
            char *ptr = VIDEO_MEM + ((sy + y) * VGA_WIDTH + (sx + x)) * 2;
            if (y == 0 || y == h - 1 || x == 0 || x == w - 1) {
                *ptr = '#';
            } else {
                *ptr = ' ';
            }
            *(ptr + 1) = 0x1F; // 青背景 / 白文字
        }
    }
    k_print("  COMMAND EXECUTED  ", sx + 5, sy + 4, 0x1F);
    k_print(" [ Press Any Key ]  ", sx + 5, sy + 6, 0x1F);
}

// 簡易的なキーボード入力（スキャンコード取得）
unsigned char get_scancode() {
    while (!(inb(0x64) & 1)); // 入力が来るまで待機（ポーリング）
    return inb(0x60);
}

void kmain(void) {
    clear_screen(0x07);
    k_print("--- Welcome to MyOS v0.1 ---", 2, 1, 0x0A); // 緑色
    k_print("Available commands: 'gui', 'cls'", 2, 2, 0x07);

    while (1) {
        k_print("admin@myos:~$ ", 2, 24, 0x0E); // 黄色プロンプト

        // 入力待ち（ここではデモとして「何かのキー」で反応）
        unsigned char scancode = get_scancode();

        // 0x1C = Enter, 0x01 = ESC, 0x21 = 'F' (仮のキー判定用)
        // 本来はここで文字列バッファに貯めてstrcmpする
        if (scancode == 0x21) { // 'F' キーをGUIトリガーと仮定
            draw_window();
            get_scancode(); // 窓を閉じるための待機
            clear_screen(0x07);
            k_print("Returned to CLI.", 2, 1, 0x07);
        } 
        else if (scancode == 0x01) { // ESCで画面クリア
            clear_screen(0x07);
        }
    }
}