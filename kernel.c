#include "drivers.h"
#include "font8x8_basic.h"

#define CURSOR_WIDTH 10
#define CURSOR_HEIGHT 15

struct multiboot_info {
    uint32_t flags;
    uint32_t unused[21];
    uint64_t framebuffer_addr;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint8_t  framebuffer_bpp;
    uint8_t  framebuffer_type;
};

#define ARGB(a, r, g, b) (((uint32_t)(a) << 24) | ((uint32_t)(r) << 16) | ((uint32_t)(g) << 8) | (uint32_t)(b))

static uint32_t* video_mem_global;
static uint32_t screen_width, screen_height, screen_pitch;
static uint32_t cursor_bg[CURSOR_WIDTH * CURSOR_HEIGHT];
static int32_t old_mouse_x = -1, old_mouse_y = -1;
static int prev_keybuf_len = 0;
static int hide_mouse = 0;

extern void set_framebuffer_info(uint32_t* fb, uint32_t width, uint32_t height, uint32_t pitch);
extern volatile char keybuf[];
extern volatile int keybuf_len;
extern void keyboard_install();
extern void draw_char8x8(int x, int y, char c, uint32_t color);

void draw_debug_rect(int x, int y, int w, int h, uint32_t color) {
    for (int py = 0; py < h; py++) {
        for (int px = 0; px < w; px++) {
            int target_x = x + px;
            int target_y = y + py;
            if (target_x >= 0 && target_x < (int)screen_width && 
                target_y >= 0 && target_y < (int)screen_height) {
                uint32_t* row = (uint32_t*)((uint8_t*)video_mem_global + target_y * screen_pitch);
                row[target_x] = color;
            }
        }
    }
}

void restore_cursor_bg() {
    if (old_mouse_x < 0) return;
    for (int y = 0; y < CURSOR_HEIGHT; y++) {
        for (int x = 0; x < CURSOR_WIDTH; x++) {
            int target_x = old_mouse_x + x;
            int target_y = old_mouse_y + y;
            if (target_x >= 0 && target_x < (int)screen_width && 
                target_y >= 0 && target_y < (int)screen_height) {
                uint32_t* row = (uint32_t*)((uint8_t*)video_mem_global + target_y * screen_pitch);
                row[target_x] = cursor_bg[y * CURSOR_WIDTH + x];
            }
        }
    }
}

void save_cursor_bg() {
    for (int y = 0; y < CURSOR_HEIGHT; y++) {
        for (int x = 0; x < CURSOR_WIDTH; x++) {
            int target_x = mouse_x + x;
            int target_y = mouse_y + y;
            if (target_x >= 0 && target_x < (int)screen_width && 
                target_y >= 0 && target_y < (int)screen_height) {
                uint32_t* row = (uint32_t*)((uint8_t*)video_mem_global + target_y * screen_pitch);
                cursor_bg[y * CURSOR_WIDTH + x] = row[target_x];
            } else {
                cursor_bg[y * CURSOR_WIDTH + x] = ARGB(255, 139, 0, 0);
            }
        }
    }
}

void draw_cursor() {
    uint32_t white = ARGB(255, 255, 255, 255);
    for (int y = 0; y < CURSOR_HEIGHT; y++) {
        for (int x = 0; x < CURSOR_WIDTH; x++) {
            int target_x = mouse_x + x;
            int target_y = mouse_y + y;
            if (target_x >= 0 && target_x < (int)screen_width && 
                target_y >= 0 && target_y < (int)screen_height) {
                uint32_t* row = (uint32_t*)((uint8_t*)video_mem_global + target_y * screen_pitch);
                row[target_x] = white;
            }
        }
    }
}

void draw_char8x8(int x, int y, char c, uint32_t color) {
    if (c < 0 || c > 127) return;
    for (int row = 0; row < 8; row++) {
        uint8_t bits = font8x8_basic[(int)c][row];
        for (int col = 0; col < 8; col++) {
            if (bits & (1 << col)) {
                int px = x + col;
                int py = y + row;
                if (px >= 0 && px < (int)screen_width && py >= 0 && py < (int)screen_height) {
                    uint32_t* vrow = (uint32_t*)((uint8_t*)video_mem_global + py * screen_pitch);
                    vrow[px] = color;
                }
            }
        }
    }
}

void draw_string8x8(int x, int y, const char* str, uint32_t color) {
    int cx = x;
    while (*str) {
        draw_char8x8(cx, y, *str, color);
        cx += 8;
        str++;
    }
}

void kmain(uint32_t magic, struct multiboot_info* mbi) {
    if (!(mbi->flags & (1 << 12))) {
        return;
    }

    video_mem_global = (uint32_t*)(uintptr_t)mbi->framebuffer_addr;
    screen_width  = mbi->framebuffer_width;
    screen_height = mbi->framebuffer_height;
    screen_pitch  = mbi->framebuffer_pitch;

    set_framebuffer_info(video_mem_global, screen_width, screen_height, screen_pitch);

    uint32_t dark_red = ARGB(255, 139, 0, 0);
    for (uint32_t y = 0; y < screen_height; y++) {
        uint32_t* row = (uint32_t*)((uint8_t*)video_mem_global + y * screen_pitch);
        for (uint32_t x = 0; x < screen_width; x++) {
            row[x] = dark_red;
        }
    }

    idt_install();
    irq_install();
    keyboard_install();
    mouse_install();

    enable_interrupts(); // 全ての準備が整ってから割り込みを許可

    /* Step 2: idt_install() & irq_install() 後 */
    draw_debug_rect(35, screen_height - 20, 20, 20, ARGB(255, 255, 255, 0));

    /* Step 4: mouse_install() 後 */
    draw_debug_rect(85, screen_height - 20, 20, 20, ARGB(255, 0, 255, 255));
    
    while (1) {
        if (mouse_x > (int32_t)(screen_width - CURSOR_WIDTH)) {
            mouse_x = screen_width - CURSOR_WIDTH;
        }
        if (mouse_y > (int32_t)(screen_height - CURSOR_HEIGHT)) {
            mouse_y = screen_height - CURSOR_HEIGHT;
        }

        // キーボード入力バッファを画面中央下に表示
        int buf_x = (screen_width - keybuf_len * 8) / 2;
        int buf_y = (screen_height - 8) / 2 + 16;
        for (int i = 0; i < keybuf_len; i++) {
            draw_char8x8(buf_x + i * 8, buf_y, keybuf[i], ARGB(255, 255, 255, 0));
        }

        // 入力が増えたらマウス非表示
        if (keybuf_len > prev_keybuf_len) {
            hide_mouse = 1;
        }
        prev_keybuf_len = keybuf_len;

        // マウスポインター描画
        if (!hide_mouse) {
            if (old_mouse_x != mouse_x || old_mouse_y != mouse_y) {
                restore_cursor_bg();
                save_cursor_bg();
                draw_cursor();
                old_mouse_x = mouse_x;
                old_mouse_y = mouse_y;
            }
        }

        uint8_t red_val = (mouse_interrupt_counter % 256);
        uint32_t debug_color = ARGB(255, red_val, 0, 255 - red_val);

        for (int y = 0; y < 5; y++) {
            for (int x = 0; x < 5; x++) {
                int target_x = screen_width - 6 + x;
                int target_y = y;
                if (target_x >= 0 && target_x < (int)screen_width && 
                    target_y >= 0 && target_y < (int)screen_height) {
                    uint32_t* row = (uint32_t*)((uint8_t*)video_mem_global + target_y * screen_pitch);
                    row[target_x] = debug_color;
                }
            }
        }
    }
}