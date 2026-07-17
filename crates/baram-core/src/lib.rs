#![no_std]

extern crate alloc;

pub mod color;
pub mod screen;
pub mod layer;
pub mod key_event;
pub mod subsystem;

pub use color::Color;
pub use screen::{Screen, FramebufferInfo};
pub use layer::LayerSystem;
pub use key_event::KeyEvent;
