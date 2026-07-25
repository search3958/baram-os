//! A small X11-compatible core used by the native BaramOS window server.
//!
//! This is intentionally independent of a transport.  It gives the compositor
//! the same resource model as X11 (windows, pixmaps and graphics contexts),
//! while the current in-process Warp/HTML clients use `X11Surface`.  A socket
//! or kernel IPC frontend can later feed `Request` values into `X11Server`.

#![allow(dead_code)]

use alloc::vec::Vec;
use baram_core::{Color, LayerSystem};

pub type Xid = u32;

pub const ROOT_WINDOW: Xid = 1;
pub const COPY_FROM_PARENT: u8 = 0;
pub const INPUT_OUTPUT: u8 = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Window,
    Pixmap,
    GraphicsContext,
}

pub struct X11Window {
    pub id: Xid,
    pub parent: Xid,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub mapped: bool,
    pub event_mask: u32,
    pub surface: X11Surface,
}

pub struct X11Surface {
    layer: LayerSystem,
}

impl X11Surface {
    pub fn new(width: usize, height: usize) -> Self {
        Self { layer: LayerSystem::new_transparent(width, height) }
    }

    pub fn width(&self) -> usize { self.layer.width() }
    pub fn height(&self) -> usize { self.layer.height() }
    pub fn layer(&mut self) -> &mut LayerSystem { &mut self.layer }
    pub fn clear(&mut self) { self.layer.clear(Color::TRANSPARENT); }

    /// Copies the client pixmap into the compositor's window surface.
    pub fn present_into(&self, dst: &mut LayerSystem) {
        let w = self.width().min(dst.width());
        let h = self.height().min(dst.height());
        let dst_w = dst.width();
        for y in 0..h {
            let src = &self.layer.buf_ref()[y * self.width()..y * self.width() + w];
            let out = &mut dst.buf_mut()[y * dst_w..y * dst_w + w];
            for x in 0..w {
                let px = src[x];
                if (px >> 24) != 0 { out[x] = px; }
            }
        }
    }
}

pub struct X11Server {
    next_xid: Xid,
    pub windows: Vec<X11Window>,
    gc_ids: Vec<Xid>,
    pixmap_ids: Vec<Xid>,
}

impl X11Server {
    pub fn new() -> Self {
        Self { next_xid: ROOT_WINDOW + 1, windows: Vec::new(), gc_ids: Vec::new(), pixmap_ids: Vec::new() }
    }

    fn alloc_id(&mut self) -> Xid {
        let id = self.next_xid;
        self.next_xid = self.next_xid.wrapping_add(1).max(ROOT_WINDOW + 1);
        id
    }

    pub fn create_window(&mut self, parent: Xid, x: i16, y: i16, width: u16, height: u16) -> Xid {
        let id = self.alloc_id();
        self.windows.push(X11Window {
            id, parent, x, y, width, height, mapped: false, event_mask: 0,
            surface: X11Surface::new(width as usize, height as usize),
        });
        id
    }

    pub fn map_window(&mut self, id: Xid) -> bool {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) { w.mapped = true; true } else { false }
    }

    pub fn unmap_window(&mut self, id: Xid) -> bool {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) { w.mapped = false; true } else { false }
    }

    pub fn destroy_window(&mut self, id: Xid) -> bool {
        let old = self.windows.len();
        self.windows.retain(|w| w.id != id && w.parent != id);
        old != self.windows.len()
    }

    pub fn create_gc(&mut self) -> Xid { let id = self.alloc_id(); self.gc_ids.push(id); id }
    pub fn create_pixmap(&mut self) -> Xid { let id = self.alloc_id(); self.pixmap_ids.push(id); id }
}

/// Minimal wire requests. Values are deliberately close to core X11 opcodes.
pub enum Request { CreateWindow, MapWindow, UnmapWindow, DestroyWindow, CreateGc, CreatePixmap, PutImage }

pub fn decode_request(opcode: u8) -> Option<Request> {
    match opcode { 1 => Some(Request::CreateWindow), 8 => Some(Request::MapWindow),
        10 => Some(Request::UnmapWindow), 4 => Some(Request::DestroyWindow),
        55 => Some(Request::CreateGc), 53 => Some(Request::CreatePixmap),
        72 => Some(Request::PutImage), _ => None }
}
