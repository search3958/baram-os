#include "drivers.h"

// 必要なmultiboot_info構造体の簡易定義
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
    // フレームバッファ情報
    uint64_t framebuffer_addr;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint8_t  framebuffer_bpp;
    uint8_t  framebuffer_type;
    uint8_t  reserved[2];
};

// レイヤー用バッファを静的に確保 (mallocがないため)
static uint32_t desktop_buf[SCREEN_WIDTH * SCREEN_HEIGHT];
static uint32_t msg_buf[400 * 50];
static uint32_t input_buf[SCREEN_WIDTH * 20];

// SVGサンプルデータ（ユーザー指定のSVGアイコン）
static const char* svg_sample =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" height=\"24px\" viewBox=\"0 -960 960 960\" width=\"24px\" fill=\"#FFFFFF\">"
    "<path d=\"m370-80-16-128q-13-5-24.5-12T307-235l-119 50L78-375l103-78q-1-7-1-13.5v-27q0-6.5 1-13.5L78-585l110-190 119 50q11-8 23-15t24-12l16-128h220l16 128q13 5 24.5 12t22.5 15l119-50 110 190-103 78q1 7 1 13.5v27q0 6.5-2 13.5l103 78-110 190-118-50q-11 8-23 15t-24 12L590-80H370Zm70-80h79l14-106q31-8 57.5-23.5T639-327l99 41 39-68-86-65q5-14 7-29.5t2-31.5q0-16-2-31.5t-7-29.5l86-65-39-68-99 42q-22-23-48.5-38.5T533-694l-13-106h-79l-14 106q-31 8-57.5 23.5T321-633l-99-41-39 68 86 64q-5 15-7 30t-2 32q0 16 2 31t7 30l-86 65 39 68 99-42q22 23 48.5 38.5T427-266l13 106Zm42-180q58 0 99-41t41-99q0-58-41-99t-99-41q-59 0-99.5 41T342-480q0 58 40.5 99t99.5 41Zm-2-140Z\"/></svg>";

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
    layer_fill(&desktop, 0x00FB0000); // Dark Red
    // SVG描画テスト（画面中央に400px幅で表示）
    float svg_scale = 400.0f / 960.0f; // viewBox幅960を400pxにスケール
    int svg_x = (SCREEN_WIDTH - 400) / 2;
    int svg_y = (SCREEN_HEIGHT - 400) / 2;
    // 一時バッファを用意してSVGをラスタライズ
    uint32_t svg_buf[400 * 400] = {0};
    layer_t svg_layer;
    svg_layer.buffer = svg_buf;
    svg_layer.x = svg_x;
    svg_layer.y = svg_y;
    svg_layer.width = 400;
    svg_layer.height = 400;
    svg_layer.transparent = TRANSPARENT_COLOR;
    svg_layer.active = 1;
    layer_fill(&svg_layer, TRANSPARENT_COLOR);
    layer_draw_svg(&svg_layer, svg_sample, svg_scale);
    register_layer(&desktop);
    register_layer(&svg_layer);

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