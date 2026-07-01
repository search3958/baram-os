/*
 * Baram OS Kernel - Main Entry Point
 * License: MIT License
 * 
 * A simple UEFI-based kernel with mouse pointer, keyboard input, and display output.
 * This is NOT a Linux clone - it's an independent implementation.
 */

#include <stdint.h>
#include <stdbool.h>
#include "kernel.h"
#include "graphics.h"
#include "keyboard.h"
#include "mouse.h"

// Framebuffer info (set by UEFI)
static uint32_t* framebuffer = NULL;
static uint32_t width = 0;
static uint32_t height = 0;
static uint32_t pitch = 0;

// Mouse cursor position
static int mouse_x = 100;
static int mouse_y = 100;
static const int CURSOR_SIZE = 16;

// Simple square cursor bitmap (1 = white, 0 = transparent)
static const uint8_t cursor_bitmap[CURSOR_SIZE][CURSOR_SIZE] = {
    {1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0},
    {1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0},
    {1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0},
    {1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0},
    {1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0},
    {1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0},
    {1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0},
    {1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0},
    {0,0,0,0,0,0,0,0,1,1,0,0,0,0,0,0},
    {0,0,0,0,0,0,0,0,1,1,0,0,0,0,0,0},
    {0,0,0,0,0,0,0,0,1,1,0,0,0,0,0,0},
    {0,0,0,0,0,0,0,0,1,1,0,0,0,0,0,0},
    {0,0,0,0,0,0,0,0,0,0,1,1,0,0,0,0},
    {0,0,0,0,0,0,0,0,0,0,1,1,0,0,0,0},
    {0,0,0,0,0,0,0,0,0,0,0,0,1,1,0,0},
    {0,0,0,0,0,0,0,0,0,0,0,0,1,1,0,0}
};

void clear_screen(uint32_t color) {
    for (uint32_t y = 0; y < height; y++) {
        for (uint32_t x = 0; x < width; x++) {
            framebuffer[y * pitch + x] = color;
        }
    }
}

void draw_cursor() {
    for (int dy = 0; dy < CURSOR_SIZE; dy++) {
        for (int dx = 0; dx < CURSOR_SIZE; dx++) {
            int px = mouse_x + dx;
            int py = mouse_y + dy;
            
            if (px >= 0 && px < (int)width && py >= 0 && py < (int)height) {
                if (cursor_bitmap[dy][dx]) {
                    // Draw white cursor pixel
                    framebuffer[py * pitch + px] = 0xFFFFFFFF;
                }
            }
        }
    }
}

void draw_string(const char* str, uint32_t x, uint32_t y, uint32_t color) {
    // Simple placeholder - in real implementation would use font
    // For now, just indicate text position with a small marker
    for (int i = 0; i < 10 && x + i < width; i++) {
        framebuffer[y * pitch + (x + i)] = color;
    }
}

void handle_keyboard_input(uint8_t scancode) {
    // Simple keyboard handler - move cursor with arrow keys
    // In real implementation, would parse full scancodes
    static bool key_pressed = false;
    
    if (!key_pressed && scancode != 0) {
        key_pressed = true;
        
        // Basic movement based on scancode (simplified)
        switch (scancode) {
            case 0x4B: // Left arrow
                if (mouse_x > 0) mouse_x -= 10;
                break;
            case 0x4D: // Right arrow
                if (mouse_x < width - CURSOR_SIZE) mouse_x += 10;
                break;
            case 0x48: // Up arrow
                if (mouse_y > 0) mouse_y -= 10;
                break;
            case 0x50: // Down arrow
                if (mouse_y < height - CURSOR_SIZE) mouse_y += 10;
                break;
        }
    }
    
    if (scancode == 0) {
        key_pressed = false;
    }
}

void main_kernel(uint32_t* fb, uint32_t w, uint32_t h, uint32_t p) {
    framebuffer = fb;
    width = w;
    height = h;
    pitch = p;
    
    // Clear screen to blue background
    clear_screen(0x000080FF); // BGRA format
    
    // Draw initial cursor
    draw_cursor();
    
    // Display welcome message indicator
    draw_string("Baram OS", 10, 10, 0xFFFFFFFF);
    
    // Main loop - in real implementation would handle interrupts
    while (true) {
        // Poll keyboard (simplified)
        uint8_t keycode = poll_keyboard();
        if (keycode != 0) {
            handle_keyboard_input(keycode);
            
            // Redraw screen
            clear_screen(0x000080FF);
            draw_string("Baram OS", 10, 10, 0xFFFFFFFF);
            draw_cursor();
        }
        
        // Small delay to prevent busy-waiting too fast
        for (volatile int i = 0; i < 100000; i++);
    }
}
