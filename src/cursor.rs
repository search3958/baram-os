//! Hardware-style mouse cursor.
//!
//! We draw an arrow-shaped cursor using a 16x16 1-bit sprite with an XOR
//! outline so it is visible on any background.  To avoid artefacts, the
//! cursor module saves the rectangle it is about to draw on, then restores
//! it next frame before drawing the new position.

use crate::gop::{Color, Screen};

const CURSOR_W: usize = 13;
const CURSOR_H: usize = 18;

/// 1-bit arrow cursor.  `1` = filled (white), `0` = transparent.
/// Outline (drawn in black) is implicit: we render a black shadow first
/// then a white body, producing a clear two-tone cursor on any backdrop.
const CURSOR_MASK: [[u8; CURSOR_W]; CURSOR_H] = [
    [1,0,0,0,0,0,0,0,0,0,0,0,0],
    [1,1,0,0,0,0,0,0,0,0,0,0,0],
    [1,1,1,0,0,0,0,0,0,0,0,0,0],
    [1,1,1,1,0,0,0,0,0,0,0,0,0],
    [1,1,1,1,1,0,0,0,0,0,0,0,0],
    [1,1,1,1,1,1,0,0,0,0,0,0,0],
    [1,1,1,1,1,1,1,0,0,0,0,0,0],
    [1,1,1,1,1,1,1,1,0,0,0,0,0],
    [1,1,1,1,1,1,1,1,1,0,0,0,0],
    [1,1,1,1,1,1,1,1,1,1,0,0,0],
    [1,1,1,1,1,1,1,1,1,1,1,0,0],
    [1,1,1,1,1,1,1,1,1,0,0,0,0],
    [1,1,1,1,1,1,1,0,0,0,0,0,0],
    [1,1,1,0,1,1,1,1,0,0,0,0,0],
    [1,1,0,0,0,1,1,1,1,0,0,0,0],
    [1,0,0,0,0,0,1,1,1,1,0,0,0],
    [0,0,0,0,0,0,0,1,1,1,1,0,0],
    [0,0,0,0,0,0,0,0,1,1,1,1,0],
];

/// Manages cursor position + background save/restore.
pub struct Cursor {
    pub x: i32,
    pub y: i32,
    /// Saved background under the cursor (linear RGBA32).
    saved: Option<&'static mut [u32]>,
    saved_x: usize,
    saved_y: usize,
    saved_w: usize,
    saved_h: usize,
}

impl Cursor {
    pub fn new(x: i32, y: i32) -> Cursor {
        Cursor {
            x, y,
            saved: None,
            saved_x: 0, saved_y: 0, saved_w: 0, saved_h: 0,
        }
    }

    /// Allocate the save buffer.  Must be called once with the allocator
    /// available (we just leak the buffer; it lives for the lifetime of
    /// the OS).
    pub fn init_save_buffer(&mut self) {
        // The buffer only needs to cover the cursor rectangle, but the
        // cursor may sit near the bottom-right corner.  Allocate a worst
        // case buffer = (CURSOR_W+1) * (CURSOR_H+1) to allow shadow.
        let size = (CURSOR_W + 2) * (CURSOR_H + 2);
        let buf: &'static mut [u32] = {
            use alloc::vec;
            use alloc::boxed::Box;
            let v = vec![0u32; size].into_boxed_slice();
            Box::leak(v)
        };
        self.saved = Some(buf);
    }

    /// Apply a relative movement, clamping to the visible area.
    pub fn move_by(&mut self, dx: i32, dy: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 { return; }
        self.x = (self.x + dx).clamp(0, w - 1);
        self.y = (self.y + dy).clamp(0, h - 1);
    }

    /// Restore the previously-saved background.  Should be called before
    /// the framebuffer is redrawn each frame.
    pub fn restore_bg(&mut self, screen: &mut Screen) {
        if let Some(buf) = self.saved.as_mut() {
            for yy in 0..self.saved_h {
                for xx in 0..self.saved_w {
                    let px = self.saved_x + xx;
                    let py = self.saved_y + yy;
                    if px < screen.width() && py < screen.height() {
                        let c = buf[yy * self.saved_w + xx];
                        screen.put_pixel(px, py, Color(c));
                    }
                }
            }
        }
    }

    /// Save the background under the cursor then draw the cursor.
    pub fn draw(&mut self, screen: &mut Screen) {
        // Compute clipped save rectangle.
        let x0 = (self.x as usize).min(screen.width().saturating_sub(1));
        let y0 = (self.y as usize).min(screen.height().saturating_sub(1));
        let x1 = (x0 + CURSOR_W + 1).min(screen.width());
        let y1 = (y0 + CURSOR_H + 1).min(screen.height());
        self.saved_x = x0;
        self.saved_y = y0;
        self.saved_w = x1.saturating_sub(x0);
        self.saved_h = y1.saturating_sub(y0);

        // Save background.
        if let Some(buf) = self.saved.as_mut() {
            for yy in 0..self.saved_h {
                for xx in 0..self.saved_w {
                    let px = x0 + xx;
                    let py = y0 + yy;
                    buf[yy * self.saved_w + xx] = screen.read_pixel(px, py).0;
                }
            }
        }

        // Draw a 1px black drop shadow (offset +1,+1) for contrast on
        // bright backgrounds, then draw the white body.
        for yy in 0..CURSOR_H {
            for xx in 0..CURSOR_W {
                if CURSOR_MASK[yy][xx] == 1 {
                    let px = (self.x as usize) + xx;
                    let py = (self.y as usize) + yy;
                    // Shadow
                    let sx = px + 1;
                    let sy = py + 1;
                    if sx < screen.width() && sy < screen.height() {
                        screen.put_pixel(sx, sy, Color::BLACK);
                    }
                    // Body
                    if px < screen.width() && py < screen.height() {
                        screen.put_pixel(px, py, Color::CURSOR);
                    }
                }
            }
        }
    }
}
