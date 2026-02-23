#include "drivers.h"

// レイヤー用バッファを静的に確保 (mallocがないため)
static uint32_t desktop_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
static uint32_t msg_buf[400 * 50];
static uint32_t input_buf[SCREEN_WIDTH * 20];

extern void register_layer(layer_t* layer);
extern volatile char keybuf[];
extern volatile int keybuf_len;

void kmain(uint32_t magic, struct multiboot_info* mbi) {
    set_framebuffer_info((uint32_t*)(uintptr_t)mbi->framebuffer_addr, 
                         mbi->framebuffer_width, 
                         mbi->framebuffer_height, 
                         mbi->framebuffer_pitch);

    // 1. 背景レイヤー (Desktop)
    layer_t desktop;
    desktop.buffer = desktop_buf;
    desktop.x = 0; desktop.y = 0;
    desktop.width = SCREEN_WIDTH; desktop.height = SCREEN_HEIGHT;
    desktop.transparent = 0;
    desktop.active = 1;
    layer_fill(&desktop, 0x008B0000); // Dark Red
    register_layer(&desktop);

    // 2. メッセージレイヤー
    layer_t msg;
    msg.buffer = msg_buf;
    msg.x = (SCREEN_WIDTH - 400) / 2; msg.y = 100;
    msg.width = 400; msg.height = 50;
    msg.transparent = TRANSPARENT_COLOR;
    msg.active = 1;
    layer_fill(&msg, TRANSPARENT_COLOR);
    layer_draw_string(&msg, 0, 0, "Hello, baram! Layer System Ready.", 0xFFFFFFFF, TRANSPARENT_COLOR);
    register_layer(&msg);

    // 3. 入力バッファレイヤー
    layer_t input;
    input.buffer = input_buf;
    input.x = 0; input.y = SCREEN_HEIGHT - 40;
    input.width = SCREEN_WIDTH; input.height = 20;
    input.transparent = TRANSPARENT_COLOR;
    input.active = 1;
    layer_fill(&input, TRANSPARENT_COLOR);
    register_layer(&input);

    idt_install();
    irq_install();
    keyboard_install();
    mouse_install();
    enable_interrupts();

    int prev_key_len = 0;

    while (1) {
        // キー入力があれば描画を更新
        if (keybuf_len != prev_key_len) {
            layer_fill(&input, TRANSPARENT_COLOR);
            // 文字の重なりを防ぐため、背景色（または透明色）でクリアしてから描画
            // 今回は layer_draw_string で bg_color を指定することで古い文字を塗りつぶす
            layer_draw_string(&input, (SCREEN_WIDTH - keybuf_len * 8) / 2, 0, 
                              (const char*)keybuf, 0xFFFFFF00, 0xFF000000); // 黄色文字、黒背景
            prev_key_len = keybuf_len;
        }

        // すべてを合成して画面更新
        screen_refresh();
    }
}