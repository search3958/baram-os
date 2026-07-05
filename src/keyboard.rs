//! UEFI Simple Text Input wrapper.
//!
//! This module exposes a tiny `Keyboard` struct that the main loop polls
//! each frame.  Returned `KeyEvent`s carry the pressed scan code and an
//! optional printable ASCII character.

use uefi::proto::console::text::{Input, Key, ScanCode};
use uefi::system::with_stdin;


#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    
    pub printable: Option<u8>,
    
    pub scancode: u16,
}

impl KeyEvent {
    #[allow(dead_code)]
    pub fn is_special(&self) -> bool { self.scancode != 0 }

    
    
    pub fn label(&self) -> &'static str {
        if let Some(c) = self.printable {
            return match c {
                b' '  => "SPC",
                b'\t' => "TAB",
                b'\r' | b'\n' => "ENT",
                0x1B  => "ESC",
                0x7F  => "DEL",
                _     => "ASCII",
            };
        }
        
        let sc = ScanCode(self.scancode);
        if sc == ScanCode::UP         { return "UP"; }
        if sc == ScanCode::DOWN       { return "DOWN"; }
        if sc == ScanCode::LEFT       { return "LEFT"; }
        if sc == ScanCode::RIGHT      { return "RIGHT"; }
        if sc == ScanCode::ESCAPE     { return "ESC"; }
        if sc == ScanCode::DELETE     { return "DEL"; }
        if sc == ScanCode::HOME       { return "HOME"; }
        if sc == ScanCode::END        { return "END"; }
        if sc == ScanCode::INSERT     { return "INS"; }
        if sc == ScanCode::PAGE_UP    { return "PGUP"; }
        if sc == ScanCode::PAGE_DOWN  { return "PGDN"; }
        if sc == ScanCode::FUNCTION_1 { return "F1"; }
        if sc == ScanCode::FUNCTION_2 { return "F2"; }
        if sc == ScanCode::FUNCTION_3 { return "F3"; }
        if sc == ScanCode::FUNCTION_4 { return "F4"; }
        if sc == ScanCode::FUNCTION_5 { return "F5"; }
        if sc == ScanCode::FUNCTION_6 { return "F6"; }
        if sc == ScanCode::FUNCTION_7 { return "F7"; }
        if sc == ScanCode::FUNCTION_8 { return "F8"; }
        if sc == ScanCode::FUNCTION_9 { return "F9"; }
        if sc == ScanCode::FUNCTION_10 { return "F10"; }
        if sc == ScanCode::FUNCTION_11 { return "F11"; }
        if sc == ScanCode::FUNCTION_12 { return "F12"; }
        "???"
    }
}

pub struct Keyboard;

impl Keyboard {
    
    
    
    pub fn is_present() -> bool {
        
        uefi::boot::get_handle_for_protocol::<Input>().is_ok()
    }

    
    pub fn reset() {
        with_stdin(|input| { let _ = input.reset(false); });
    }

    
    
    pub fn poll() -> Option<KeyEvent> {
        with_stdin(|input| {
            match input.read_key() {
                Ok(Some(Key::Printable(ch))) => {
                    
                    let v: u16 = ch.into();
                    let printable = if v < 0x80 { Some(v as u8) } else { None };
                    Some(KeyEvent { printable, scancode: 0 })
                }
                Ok(Some(Key::Special(sc))) => {
                    Some(KeyEvent { printable: None, scancode: sc.0 })
                }
                _ => None,
            }
        })
    }
}
