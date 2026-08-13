//! OS-owned software keyboard.
//!
//! The controls are rendered by Warp, but this surface is deliberately not a
//! window or an application. It stays outside app discovery/navigation and
//! reports typed keys directly to the OS input router.

use baram_core::{Color, LayerSystem};

use crate::warp::WarpEngine;

pub const WIDTH: usize = 720;
pub const HEIGHT: usize = 280;

const LETTERS_LOWER: &str = r#"
screen { id: (main),
  hStack {
    tonalButton { id: (q), text: ("q"), width: (52) }
    tonalButton { id: (w), text: ("w"), width: (52) }
    tonalButton { id: (e), text: ("e"), width: (52) }
    tonalButton { id: (r), text: ("r"), width: (52) }
    tonalButton { id: (t), text: ("t"), width: (52) }
    tonalButton { id: (y), text: ("y"), width: (52) }
    tonalButton { id: (u), text: ("u"), width: (52) }
    tonalButton { id: (i), text: ("i"), width: (52) }
    tonalButton { id: (o), text: ("o"), width: (52) }
    tonalButton { id: (p), text: ("p"), width: (52) }
  }
  hStack {
    tonalButton { id: (a), text: ("a"), width: (52) }
    tonalButton { id: (s), text: ("s"), width: (52) }
    tonalButton { id: (d), text: ("d"), width: (52) }
    tonalButton { id: (f), text: ("f"), width: (52) }
    tonalButton { id: (g), text: ("g"), width: (52) }
    tonalButton { id: (h), text: ("h"), width: (52) }
    tonalButton { id: (j), text: ("j"), width: (52) }
    tonalButton { id: (k), text: ("k"), width: (52) }
    tonalButton { id: (l), text: ("l"), width: (52) }
    button { id: (backspace), text: ("Back"), width: (64) }
  }
  hStack {
    button { id: (shift), text: ("Shift"), width: (76) }
    tonalButton { id: (z), text: ("z"), width: (52) }
    tonalButton { id: (x), text: ("x"), width: (52) }
    tonalButton { id: (c), text: ("c"), width: (52) }
    tonalButton { id: (v), text: ("v"), width: (52) }
    tonalButton { id: (b), text: ("b"), width: (52) }
    tonalButton { id: (n), text: ("n"), width: (52) }
    tonalButton { id: (m), text: ("m"), width: (52) }
    button { id: (enter), text: ("Enter"), width: (82) }
  }
  hStack {
    button { id: (symbols), text: ("123"), width: (76) }
    tonalButton { id: (comma), text: (","), width: (52) }
    tonalButton { id: (space), text: ("Space"), width: (300) }
    tonalButton { id: (period), text: ("."), width: (52) }
    button { id: (close), text: ("Close"), width: (92) }
  }
}
"#;

const LETTERS_UPPER: &str = r#"
screen { id: (main),
  hStack {
    tonalButton { id: (q), text: ("Q"), width: (52) } tonalButton { id: (w), text: ("W"), width: (52) }
    tonalButton { id: (e), text: ("E"), width: (52) } tonalButton { id: (r), text: ("R"), width: (52) }
    tonalButton { id: (t), text: ("T"), width: (52) } tonalButton { id: (y), text: ("Y"), width: (52) }
    tonalButton { id: (u), text: ("U"), width: (52) } tonalButton { id: (i), text: ("I"), width: (52) }
    tonalButton { id: (o), text: ("O"), width: (52) } tonalButton { id: (p), text: ("P"), width: (52) }
  }
  hStack {
    tonalButton { id: (a), text: ("A"), width: (52) } tonalButton { id: (s), text: ("S"), width: (52) }
    tonalButton { id: (d), text: ("D"), width: (52) } tonalButton { id: (f), text: ("F"), width: (52) }
    tonalButton { id: (g), text: ("G"), width: (52) } tonalButton { id: (h), text: ("H"), width: (52) }
    tonalButton { id: (j), text: ("J"), width: (52) } tonalButton { id: (k), text: ("K"), width: (52) }
    tonalButton { id: (l), text: ("L"), width: (52) } button { id: (backspace), text: ("Back"), width: (64) }
  }
  hStack {
    button { id: (shift), text: ("Shift"), width: (76) }
    tonalButton { id: (z), text: ("Z"), width: (52) } tonalButton { id: (x), text: ("X"), width: (52) }
    tonalButton { id: (c), text: ("C"), width: (52) } tonalButton { id: (v), text: ("V"), width: (52) }
    tonalButton { id: (b), text: ("B"), width: (52) } tonalButton { id: (n), text: ("N"), width: (52) }
    tonalButton { id: (m), text: ("M"), width: (52) } button { id: (enter), text: ("Enter"), width: (82) }
  }
  hStack {
    button { id: (symbols), text: ("123"), width: (76) } tonalButton { id: (comma), text: (","), width: (52) }
    tonalButton { id: (space), text: ("Space"), width: (300) } tonalButton { id: (period), text: ("."), width: (52) }
    button { id: (close), text: ("Close"), width: (92) }
  }
}
"#;

const SYMBOLS: &str = r#"
screen { id: (main),
  hStack {
    tonalButton { id: (1), text: ("1"), width: (52) } tonalButton { id: (2), text: ("2"), width: (52) }
    tonalButton { id: (3), text: ("3"), width: (52) } tonalButton { id: (4), text: ("4"), width: (52) }
    tonalButton { id: (5), text: ("5"), width: (52) } tonalButton { id: (6), text: ("6"), width: (52) }
    tonalButton { id: (7), text: ("7"), width: (52) } tonalButton { id: (8), text: ("8"), width: (52) }
    tonalButton { id: (9), text: ("9"), width: (52) } tonalButton { id: (0), text: ("0"), width: (52) }
  }
  hStack {
    tonalButton { id: (minus), text: ("-"), width: (52) } tonalButton { id: (slash), text: ("/"), width: (52) }
    tonalButton { id: (colon), text: (":"), width: (52) } tonalButton { id: (semicolon), text: (";"), width: (52) }
    tonalButton { id: (lparen), text: ("("), width: (52) } tonalButton { id: (rparen), text: (")"), width: (52) }
    tonalButton { id: (dollar), text: ("$"), width: (52) } tonalButton { id: (amp), text: ("&"), width: (52) }
    tonalButton { id: (at), text: ("@"), width: (52) } button { id: (backspace), text: ("Back"), width: (64) }
  }
  hStack {
    tonalButton { id: (quote), text: ("'"), width: (52) } tonalButton { id: (doublequote), text: ("”"), width: (52) }
    tonalButton { id: (question), text: ("?"), width: (52) } tonalButton { id: (bang), text: ("!"), width: (52) }
    tonalButton { id: (plus), text: ("+"), width: (52) } tonalButton { id: (equals), text: ("="), width: (52) }
    tonalButton { id: (underscore), text: ("_"), width: (52) } button { id: (enter), text: ("Enter"), width: (82) }
  }
  hStack {
    button { id: (letters), text: ("ABC"), width: (76) } tonalButton { id: (comma), text: (","), width: (52) }
    tonalButton { id: (space), text: ("Space"), width: (300) } tonalButton { id: (period), text: ("."), width: (52) }
    button { id: (close), text: ("Close"), width: (92) }
  }
}
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Character(u8),
    Backspace,
    Enter,
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page { Lower, Upper, Symbols }

pub struct SoftKeyboard {
    engine: WarpEngine,
    page: Page,
    open: bool,
}

impl SoftKeyboard {
    pub fn new() -> Self {
        let mut keyboard = Self { engine: WarpEngine::new(LETTERS_LOWER), page: Page::Lower, open: false };
        keyboard.engine.update(WIDTH as i32, HEIGHT as i32);
        keyboard
    }

    pub fn is_open(&self) -> bool { self.open }
    pub fn open(&mut self) { self.open = true; }
    pub fn close(&mut self) { self.open = false; self.engine.clear_hover(); }
    pub fn toggle(&mut self) { if self.open { self.close() } else { self.open() } }

    pub fn bounds(&self, screen_w: usize, screen_h: usize) -> (i32, i32, i32, i32) {
        let w = WIDTH.min(screen_w.saturating_sub(24));
        let x = screen_w.saturating_sub(w) / 2;
        let y = screen_h.saturating_sub(crate::compositor::TASKBAR_H + HEIGHT + 16);
        (x as i32, y as i32, w as i32, HEIGHT as i32)
    }

    pub fn contains(&self, x: i32, y: i32, screen_w: usize, screen_h: usize) -> bool {
        if !self.open { return false; }
        let (kx, ky, kw, kh) = self.bounds(screen_w, screen_h);
        x >= kx && x < kx + kw && y >= ky && y < ky + kh
    }

    fn select_page(&mut self, page: Page, width: i32) {
        self.page = page;
        self.engine = WarpEngine::new(match page { Page::Lower => LETTERS_LOWER, Page::Upper => LETTERS_UPPER, Page::Symbols => SYMBOLS });
        self.engine.update(width, HEIGHT as i32);
    }

    pub fn click(&mut self, x: i32, y: i32, screen_w: usize, screen_h: usize) -> Option<Key> {
        if !self.contains(x, y, screen_w, screen_h) { return None; }
        let (kx, ky, kw, _) = self.bounds(screen_w, screen_h);
        self.engine.click(x - kx, y - ky);
        let id = self.engine.take_clicked_id()?;
        match id.as_str() {
            "close" => Some(Key::Close),
            "backspace" => Some(Key::Backspace),
            "enter" => Some(Key::Enter),
            "space" => Some(Key::Character(b' ')),
            "comma" => Some(Key::Character(b',')),
            "period" => Some(Key::Character(b'.')),
            "shift" => { let next = if self.page == Page::Upper { Page::Lower } else { Page::Upper }; self.select_page(next, kw); None }
            "symbols" => { self.select_page(Page::Symbols, kw); None }
            "letters" => { self.select_page(Page::Lower, kw); None }
            other => {
                let c = symbol_for_id(other).or_else(|| other.as_bytes().first().copied())?;
                let c = if self.page == Page::Upper { c.to_ascii_uppercase() } else { c };
                if self.page == Page::Upper { self.select_page(Page::Lower, kw); }
                Some(Key::Character(c))
            }
        }
    }

    pub fn set_hover(&mut self, x: i32, y: i32, screen_w: usize, screen_h: usize) -> bool {
        let previous = self.engine.hover_idx;
        if self.contains(x, y, screen_w, screen_h) {
            let (kx, ky, _, _) = self.bounds(screen_w, screen_h);
            self.engine.set_hover(x - kx, y - ky);
        } else {
            self.engine.clear_hover();
        }
        previous != self.engine.hover_idx
    }

    pub fn draw(&mut self, layer: &mut LayerSystem) {
        if !self.open { return; }
        let (x, y, w, h) = self.bounds(layer.width(), layer.height());
        self.engine.update(w, h);
        layer.fill_rounded_rect(x as usize, y as usize, w as usize, h as usize, 18, Color::rgb(0x24, 0x24, 0x28));
        self.engine.draw_to_layer(layer, x, y);
        self.engine.draw_texts(layer, x, y, 1.0);
    }
}

fn symbol_for_id(id: &str) -> Option<u8> {
    Some(match id {
        "minus" => b'-', "slash" => b'/', "colon" => b':', "semicolon" => b';',
        "lparen" => b'(', "rparen" => b')', "dollar" => b'$', "amp" => b'&',
        "at" => b'@', "quote" => b'\'', "doublequote" => b'"', "question" => b'?',
        "bang" => b'!', "plus" => b'+', "equals" => b'=', "underscore" => b'_',
        _ => return None,
    })
}

impl Default for SoftKeyboard {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen_point(local_x: i32, local_y: i32) -> (i32, i32) {
        let keyboard = SoftKeyboard::new();
        let (x, y, _, _) = keyboard.bounds(1280, 720);
        (x + local_x, y + local_y)
    }

    #[test]
    fn emits_letters_and_one_shot_shift_without_commands() {
        let mut keyboard = SoftKeyboard::new();
        keyboard.open();
        let (qx, qy) = screen_point(50, 66);
        assert_eq!(keyboard.click(qx, qy, 1280, 720), Some(Key::Character(b'q')));

        let (sx, sy) = screen_point(50, 170);
        assert_eq!(keyboard.click(sx, sy, 1280, 720), None);
        assert_eq!(keyboard.click(qx, qy, 1280, 720), Some(Key::Character(b'Q')));
        assert_eq!(keyboard.click(qx, qy, 1280, 720), Some(Key::Character(b'q')));
    }

    #[test]
    fn exposes_editing_and_symbol_keys_as_os_events() {
        let mut keyboard = SoftKeyboard::new();
        keyboard.open();
        let (bx, by) = screen_point(596, 118);
        assert_eq!(keyboard.click(bx, by, 1280, 720), Some(Key::Backspace));

        let (symbols_x, symbols_y) = screen_point(50, 222);
        assert_eq!(keyboard.click(symbols_x, symbols_y, 1280, 720), None);
        let (one_x, one_y) = screen_point(50, 66);
        assert_eq!(keyboard.click(one_x, one_y, 1280, 720), Some(Key::Character(b'1')));
    }
}
