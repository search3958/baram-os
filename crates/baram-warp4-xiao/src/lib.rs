#![no_std]

// Keep the Xiao renderer byte-for-byte aligned with Warp4's runtime behavior,
// while giving Cargo a separate package identity. This is important: the
// normal desktop and Xiao kiosk must be buildable in one workspace command
// without Cargo unifying their mutually exclusive rendering features.
#[path = "../../baram-warp4/src/lib.rs"]
mod implementation;

pub use implementation::*;
