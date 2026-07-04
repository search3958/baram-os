//! UEFI Simple Pointer Protocol wrapper (mouse driver).
//!
//! UEFI exposes mouse devices via the Simple Pointer Protocol.  Each event
//! is relative (`dx`, `dy`) plus button bits.  We accumulate deltas into an
//! absolute cursor position, clamped to the framebuffer.

use uefi::boot;
use uefi::proto::console::pointer::Pointer;

/// One relative mouse event.
#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
pub struct MouseEvent {
    pub dx: i32,
    pub dy: i32,
    pub left:   bool,
    pub right:  bool,
    pub middle: bool,
}

/// Speed divisor used to scale UEFI pointer units to something usable.
const SPEED_DIV: i32 = 2;

/// Test whether a Simple Pointer Protocol handle exists in the system.
pub fn mouse_present() -> bool {
    boot::get_handle_for_protocol::<Pointer>().is_ok()
}

/// Poll every available mouse device once, merging events into a single
/// `MouseEvent`.  Returns `None` if no events were queued.
///
/// We open the protocol on each poll.  This is slightly wasteful but
/// avoids the lifetime gymnastics of holding a `ScopedProtocol<Pointer>`
/// across the main loop.
pub fn poll_mouse() -> Option<MouseEvent> {
    let handles = boot::find_handles::<Pointer>().ok()?;
    let mut acc = MouseEvent::default();
    let mut got = false;
    for h in handles {
        if let Ok(mut ptr) = boot::open_protocol_exclusive::<Pointer>(h) {
            // Drain all queued events from this device.
            loop {
                match ptr.read_state() {
                    Ok(Some(state)) => {
                        acc.dx += state.relative_movement[0] / SPEED_DIV;
                        acc.dy += state.relative_movement[1] / SPEED_DIV;
                        if state.button[0] { acc.left = true; }
                        if state.button[1] { acc.right = true; }
                        // UEFI Simple Pointer only has 2 buttons; the
                        // middle field is always false here.
                        got = true;
                    }
                    _ => break,
                }
            }
        }
    }
    if got { Some(acc) } else { None }
}

/// Reset all pointer devices (flush queued events at startup).
pub fn reset_all() {
    if let Ok(handles) = boot::find_handles::<Pointer>() {
        for h in handles {
            if let Ok(mut ptr) = boot::open_protocol_exclusive::<Pointer>(h) {
                let _ = ptr.reset(false);
            }
        }
    }
}
