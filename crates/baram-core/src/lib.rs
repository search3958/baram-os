#![no_std]

extern crate alloc;

pub mod color;
pub mod config;
pub mod key_event;
pub mod layer;
pub mod parallel;
pub mod screen;
pub mod subsystem;

pub use color::Color;
pub use key_event::KeyEvent;
pub use layer::LayerSystem;
pub use screen::{FramebufferInfo, Screen};
