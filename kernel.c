#include <stdint.h>

typedef uint32_t size_t;

// PSF1 ヘッダ構造体
struct psf1_header {
    uint16_t magic;     // 0x3604
    uint8_t  mode;      // 0: 256文字, 1: 512文字
    uint8_t  charsize;  // 1文字あたりの高さ
} __attribute__((packed));

// objcopyによって生成されるシンボル
extern uint8_t _binary_batarde_psf_start[];

struct multiboot_info {
    uint32_t flags;
    uint32_t unused1[10];
    uint32_t mmap_length;
    uint32_t mmap_addr;
    uint32_t unused2[9];
    uint64_t framebuffer_addr;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint8_t  framebuffer_bpp;
} __attribute__((packed));

void put_pixel(uint8_t* fb, uint32_t pitch, int x, int y, uint32_t color, uint8_t bpp) {
    uint8_t* p = fb + (y * pitch) + (x * (bpp / 8));
    if (bpp == 32) *(uint32_t*)p = color;
    else if (bpp == 24) { p[0] = color & 0xFF; p[1] = (color >> 8) & 0xFF; p[2] = (color >> 16) & 0xFF; }
}

// PSFフォント描画関数
void draw_char_psf(uint8_t* fb, uint32_t pitch, uint32_t sw, uint32_t sh, int x, int y, unsigned char c, uint32_t color, uint8_t bpp) {
    struct psf1_header* header = (struct psf1_header*)_binary_batarde_psf_start;
    uint8_t* glyphs = (uint8_t*)_binary_batarde_psf_start + sizeof(struct psf1_header);
    int h = header->charsize;
    uint8_t* glyph = glyphs + (c * h);

    for (int py = 0; py < h; py++) {
        for (int px = 0; px < 8; px++) {
            if (glyph[py] & (0x80 >> px)) {
                int sx = x + px, sy = y + py;
                if (sx >= 0 && sx < (int)sw && sy >= 0 && sy < (int)sh)
                    put_pixel(fb, pitch, sx, sy, color, bpp);
            }
        }
    }
}
void kmain(uint32_t magic, struct multiboot_info* mbi) {
    if (!(mbi->flags & (1 << 12))) return;

    uint8_t* fb = (uint8_t*)(uintptr_t)mbi->framebuffer_addr;
    uint32_t pitch = mbi->framebuffer_pitch;
    uint32_t sw = mbi->framebuffer_width;
    uint32_t sh = mbi->framebuffer_height;
    uint8_t  bpp = mbi->framebuffer_bpp;

    // 背景を「暗い赤」で塗りつぶし
    draw_rect(fb, pitch, sw, sh, 0, 0, sw, sh, 0x440000, bpp);

    // 明るい黄色でテストメッセージを表示
    const char* msg = "Batarde PSF Font Test - Bright Yellow on Dark Red";
    int x = 20;
    int y = 50;
    uint32_t yellow = 0xFFFF00;

    for (int i = 0; msg[i]; i++) {
        draw_char_psf(fb, pitch, sw, sh, x + (i * 9), y, (unsigned char)msg[i], yellow, bpp);
    }

    // ASCII 256文字一覧も表示
    int x_off = 20;
    int y_off = 100;
    for (int i = 0; i < 256; i++) {
        draw_char_psf(fb, pitch, sw, sh, x_off, y_off, (unsigned char)i, yellow, bpp);
        x_off += 10;
        if (x_off + 10 > (int)sw) {
            x_off = 20;
            y_off += 20;
        }
    }

    while (1) { __asm__ __volatile__("hlt"); }
}