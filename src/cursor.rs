//! Hardware-style mouse cursor.
//!
//! We draw an arrow-shaped cursor using a 16x16 1-bit sprite with an XOR
//! outline so it is visible on any background.  To avoid artefacts, the
//! cursor module saves the rectangle it is about to draw on, then restores
//! it next frame before drawing the new position.

use crate::gop::{Color, Screen};

const CURSOR_W: usize = 13;
const CURSOR_H: usize = 18;




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


pub struct Cursor {
    pub x: i32,
    pub y: i32,
    
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

    
    
    
    pub fn init_save_buffer(&mut self) {
        
        
        
        let size = (CURSOR_W + 2) * (CURSOR_H + 2);
        let buf: &'static mut [u32] = {
            use alloc::vec;
            use alloc::boxed::Box;
            let v = vec![0u32; size].into_boxed_slice();
            Box::leak(v)
        };
        self.saved = Some(buf);
    }

    
    pub fn move_by(&mut self, dx: i32, dy: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 { return; }
        self.x = (self.x + dx).clamp(0, w - 1);
        self.y = (self.y + dy).clamp(0, h - 1);
    }

    
    
    pub fn restore_bg(&mut self, screen: &mut Screen) {
        if let Some(buf) = self.saved.as_mut() {
            for yy in 0..self.saved_h {
                let src_row = yy * self.saved_w;
                let dst_y = self.saved_y + yy;
                for xx in 0..self.saved_w {
                    let px = self.saved_x + xx;
                    if px < screen.width() && dst_y < screen.height() {
                        screen.put_pixel(px, dst_y, Color(buf[src_row + xx]));
                    }
                }
            }
        }
    }

    
    pub fn draw(&mut self, screen: &mut Screen) {
        
        let x0 = (self.x as usize).min(screen.width().saturating_sub(1));
        let y0 = (self.y as usize).min(screen.height().saturating_sub(1));
        let x1 = (x0 + CURSOR_W + 1).min(screen.width());
        let y1 = (y0 + CURSOR_H + 1).min(screen.height());
        self.saved_x = x0;
        self.saved_y = y0;
        self.saved_w = x1.saturating_sub(x0);
        self.saved_h = y1.saturating_sub(y0);

        
        if let Some(buf) = self.saved.as_mut() {
            for yy in 0..self.saved_h {
                for xx in 0..self.saved_w {
                    let px = x0 + xx;
                    let py = y0 + yy;
                    buf[yy * self.saved_w + xx] = screen.read_pixel(px, py).0;
                }
            }
        }

        
        
        for yy in 0..CURSOR_H {
            for xx in 0..CURSOR_W {
                if CURSOR_MASK[yy][xx] == 1 {
                    let px = (self.x as usize) + xx;
                    let py = (self.y as usize) + yy;
                    
                    let sx = px + 1;
                    let sy = py + 1;
                    if sx < screen.width() && sy < screen.height() {
                        screen.put_pixel(sx, sy, Color::BLACK);
                    }
                    
                    if px < screen.width() && py < screen.height() {
                        screen.put_pixel(px, py, Color::CURSOR);
                    }
                }
            }
        }
    }
}
