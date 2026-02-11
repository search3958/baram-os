#include <stdint.h>

/* Multiboot 1 構造体の定義（ビデオ情報の取得に必要） */
struct multiboot_info {
    uint32_t flags;
    uint32_t unused[15]; // 前半の不要なフィールドをスキップ
    /* Framebuffer info (flags & 1 << 12 が真のとき有効) */
    uint64_t framebuffer_addr;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint8_t  framebuffer_bpp;
    uint8_t  framebuffer_type;
};

void kmain(uint32_t magic, struct multiboot_info* mbi) {
    // 1. グラフィック情報があるか確認 (bit 12)
    if (!(mbi->flags & (1 << 12))) {
        return;
    }

    // 2. 正しいビデオメモリのアドレスを取得
    // 0xA0000 ではなく、mbi->framebuffer_addr を使う
    uint8_t* video_mem = (uint8_t*)(uintptr_t)mbi->framebuffer_addr;
    
    uint32_t width  = mbi->framebuffer_width;
    uint32_t height = mbi->framebuffer_height;

    // 3. 画面全体を塗りつぶす (8bitモードなら 1 = 青)
    // ※もし画面が真っ暗なままなら、試しに i < 1000000 くらいまで大きく回してみてください
    for (uint32_t i = 0; i < width * height; i++) {
        video_mem[i] = 1; 
    }

    // 4. 真ん中に白い四角を描く
    uint32_t cx = width / 2;
    uint32_t cy = height / 2;
    for (uint32_t y = cy - 10; y < cy + 10; y++) {
        for (uint32_t x = cx - 10; x < cx + 10; x++) {
            video_mem[y * width + x] = 15; // 15: 白
        }
    }

    while (1) {
        __asm__ __volatile__("hlt");
    }
}