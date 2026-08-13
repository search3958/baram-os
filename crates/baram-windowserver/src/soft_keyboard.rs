//! OS-owned, topmost software keyboard rendered by Warp 3.
//!
//! This is neither a window nor an application. Its embedded Warp 3 resources
//! never enter app discovery or the VFS, while emitted keys go straight to the
//! OS input router.

extern crate alloc;

use alloc::string::String;
use baram_core::LayerSystem;
use crate::window::RoundedShadow;

use crate::warp3::Warp3Engine;

pub const WIDTH: usize = 720;
pub const HEIGHT: usize = 252;
const RADIUS: usize = 18;
const OPEN_DURATION_NS: u64 = 400_000_000;
const OPEN_OFFSET_Y: i32 = 30;

const CONFIG: &str = "version = 3\nscreen = main\nname = Software Keyboard\n";
const LOWER: &str = r#"
config { title("Software Keyboard") }
keyboard { detail.ime-status { text("English") }
keyboard-row { button.q { text("q") type("tonal") } button.w { text("w") type("tonal") } button.e { text("e") type("tonal") } button.r { text("r") type("tonal") } button.t { text("t") type("tonal") } button.y { text("y") type("tonal") } button.u { text("u") type("tonal") } button.i { text("i") type("tonal") } button.o { text("o") type("tonal") } button.p { text("p") type("tonal") } }
keyboard-row { button.a { text("a") type("tonal") } button.s { text("s") type("tonal") } button.d { text("d") type("tonal") } button.f { text("f") type("tonal") } button.g { text("g") type("tonal") } button.h { text("h") type("tonal") } button.j { text("j") type("tonal") } button.k { text("k") type("tonal") } button.l { text("l") type("tonal") } button.backspace { text("Back") type("primary") } }
keyboard-row { button.shift { text("Shift") type("primary") } button.z { text("z") type("tonal") } button.x { text("x") type("tonal") } button.c { text("c") type("tonal") } button.v { text("v") type("tonal") } button.b { text("b") type("tonal") } button.n { text("n") type("tonal") } button.m { text("m") type("tonal") } button.enter { text("Enter") type("primary") } }
keyboard-row { button.symbols { text("123") } button.comma { text(",") type("tonal") } button.space { text("Space") type("tonal") } button.period { text(".") type("tonal") } button.close { text("Close") } }
}
"#;
const UPPER: &str = r#"
config { title("Software Keyboard") }
keyboard { detail.ime-status { text("English") }
keyboard-row { button.q { text("Q") type("tonal") } button.w { text("W") type("tonal") } button.e { text("E") type("tonal") } button.r { text("R") type("tonal") } button.t { text("T") type("tonal") } button.y { text("Y") type("tonal") } button.u { text("U") type("tonal") } button.i { text("I") type("tonal") } button.o { text("O") type("tonal") } button.p { text("P") type("tonal") } }
keyboard-row { button.a { text("A") type("tonal") } button.s { text("S") type("tonal") } button.d { text("D") type("tonal") } button.f { text("F") type("tonal") } button.g { text("G") type("tonal") } button.h { text("H") type("tonal") } button.j { text("J") type("tonal") } button.k { text("K") type("tonal") } button.l { text("L") type("tonal") } button.backspace { text("Back") type("primary") } }
keyboard-row { button.shift { text("Shift") type("primary") } button.z { text("Z") type("tonal") } button.x { text("X") type("tonal") } button.c { text("C") type("tonal") } button.v { text("V") type("tonal") } button.b { text("B") type("tonal") } button.n { text("N") type("tonal") } button.m { text("M") type("tonal") } button.enter { text("Enter") type("primary") } }
keyboard-row { button.symbols { text("123") } button.comma { text(",") type("tonal") } button.space { text("Space") type("tonal") } button.period { text(".") type("tonal") } button.close { text("Close") } }
}
"#;
const SYMBOLS: &str = r#"
config { title("Software Keyboard") }
keyboard { detail.ime-status { text("Symbols") }
keyboard-row { button.1 { text("1") type("tonal") } button.2 { text("2") type("tonal") } button.3 { text("3") type("tonal") } button.4 { text("4") type("tonal") } button.5 { text("5") type("tonal") } button.6 { text("6") type("tonal") } button.7 { text("7") type("tonal") } button.8 { text("8") type("tonal") } button.9 { text("9") type("tonal") } button.0 { text("0") type("tonal") } }
keyboard-row { button.minus { text("-") type("tonal") } button.slash { text("/") type("tonal") } button.colon { text(":") type("tonal") } button.semicolon { text(";") type("tonal") } button.lparen { text("(") type("tonal") } button.rparen { text(")") type("tonal") } button.dollar { text("$") type("tonal") } button.amp { text("&") type("tonal") } button.at { text("@") type("tonal") } button.backspace { text("Back") type("primary") } }
keyboard-row { button.quote { text("'") type("tonal") } button.doublequote { text("\"") type("tonal") } button.question { text("?") type("tonal") } button.bang { text("!") type("tonal") } button.plus { text("+") type("tonal") } button.equals { text("=") type("tonal") } button.underscore { text("_") type("tonal") } button.enter { text("Enter") type("primary") } }
keyboard-row { button.letters { text("ABC") } button.comma { text(",") type("tonal") } button.space { text("Space") type("tonal") } button.period { text(".") type("tonal") } button.close { text("Close") } }
}
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Character(u8),
    Backspace,
    Enter,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardLanguage {
    Latin,
    Japanese,
    KoreanDubeolsik,
    KoreanHancomRoman,
    KoreanChosunDubeolsik,
    ChinesePinyin,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Lower,
    Upper,
    Symbols,
}

pub struct SoftKeyboard {
    engine: Warp3Engine,
    surface: LayerSystem,
    page: Page,
    language: KeyboardLanguage,
    ime_status: String,
    shadow: Option<RoundedShadow>,
    open: bool,
    position: Option<(i32, i32)>,
    dragging: bool,
    drag_offset: (i32, i32),
    open_animating: bool,
    open_started_ns: Option<u64>,
    render_y_offset: i32,
    surface_dirty: bool,
    presented_bounds: Option<(i32, i32, i32, i32)>,
}

impl SoftKeyboard {
    pub fn new() -> Self {
        let mut keyboard = Self {
            engine: engine_for(LOWER),
            surface: LayerSystem::new(WIDTH, HEIGHT),
            page: Page::Lower,
            language: KeyboardLanguage::Latin,
            ime_status: String::from("English"),
            shadow: RoundedShadow::new(WIDTH, HEIGHT, RADIUS),
            open: false,
            position: None,
            dragging: false,
            drag_offset: (0, 0),
            open_animating: false,
            open_started_ns: None,
            render_y_offset: 0,
            surface_dirty: true,
            presented_bounds: None,
        };
        keyboard.refresh_surface();
        keyboard
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }
    pub fn open(&mut self) {
        self.open_animating = true;
        self.open_started_ns = None;
        self.render_y_offset = OPEN_OFFSET_Y;
        self.open = true;
    }
    pub fn close(&mut self) {
        self.open = false;
        self.dragging = false;
        self.open_animating = false;
        self.open_started_ns = None;
        self.render_y_offset = 0;
        self.engine.clear_hover();
    }
    pub fn toggle(&mut self) {
        if self.open {
            self.close()
        } else {
            self.open()
        }
    }

    pub fn bounds(&self, screen_w: usize, screen_h: usize) -> (i32, i32, i32, i32) {
        let w = WIDTH.min(screen_w.saturating_sub(24));
        let max_x = screen_w.saturating_sub(w) as i32;
        let max_y = screen_h.saturating_sub(crate::compositor::TASKBAR_H + HEIGHT) as i32;
        let default = (
            (screen_w.saturating_sub(w) / 2) as i32,
            max_y.saturating_sub(OPEN_OFFSET_Y),
        );
        let (x, y) = self.position.unwrap_or(default);
        (
            x.clamp(0, max_x),
            (y.clamp(0, max_y) + self.render_y_offset).min(max_y),
            w as i32,
            HEIGHT as i32,
        )
    }

    pub fn contains(&self, x: i32, y: i32, sw: usize, sh: usize) -> bool {
        if !self.open {
            return false;
        }
        let (kx, ky, kw, kh) = self.bounds(sw, sh);
        x >= kx && x < kx + kw && y >= ky && y < ky + kh
    }

    fn select_page(&mut self, page: Page) {
        self.page = page;
        self.engine = engine_for(match page {
            Page::Lower => LOWER,
            Page::Upper => UPPER,
            Page::Symbols => SYMBOLS,
        });
        self.apply_language_labels();
        self.engine.set_text("ime-status", &self.ime_status);
        self.surface_dirty = true;
    }

    /// Updates labels and the compact candidate shelf from OS IME state. The
    /// keyboard still emits physical layout bytes; only the OS composition
    /// engine owns the resulting text.
    pub fn set_input_context(
        &mut self,
        language: KeyboardLanguage,
        reading: Option<&str>,
        candidates: &[String],
        selected: usize,
    ) -> bool {
        let status = ime_status(language, reading, candidates, selected);
        let mut changed = false;
        if self.language != language {
            self.language = language;
            self.apply_language_labels();
            changed = true;
        }
        if self.ime_status != status {
            self.ime_status = status;
            self.engine.set_text("ime-status", &self.ime_status);
            changed = true;
        }
        self.surface_dirty |= changed;
        changed
    }

    fn apply_language_labels(&mut self) {
        if self.page == Page::Symbols {
            return;
        }
        let labels = keyboard_labels(self.language, self.page == Page::Upper);
        for (id, label) in KEY_IDS.iter().zip(labels.iter()) {
            self.engine.set_text(id, label);
        }
    }

    pub fn click(&mut self, x: i32, y: i32, sw: usize, sh: usize) -> Option<Key> {
        if !self.contains(x, y, sw, sh) {
            return None;
        }
        let (kx, ky, _, _) = self.bounds(sw, sh);
        let local_x = x - kx;
        let local_y = y - ky;
        if local_y < crate::window::title_bar_h() as i32 {
            self.dragging = true;
            self.drag_offset = (local_x, local_y);
            return None;
        }
        self.engine.click(local_x, local_y);
        self.surface_dirty = true;
        let id = self.engine.take_clicked_class()?;
        match id.as_str() {
            "close" => Some(Key::Close),
            "backspace" => Some(Key::Backspace),
            "enter" => Some(Key::Enter),
            "space" => Some(Key::Character(b' ')),
            "comma" => Some(Key::Character(b',')),
            "period" => Some(Key::Character(b'.')),
            "shift" => {
                self.select_page(if self.page == Page::Upper {
                    Page::Lower
                } else {
                    Page::Upper
                });
                None
            }
            "symbols" => {
                self.select_page(Page::Symbols);
                None
            }
            "letters" => {
                self.select_page(Page::Lower);
                None
            }
            other => {
                let c = symbol_for_id(other).or_else(|| other.as_bytes().first().copied())?;
                let c = if self.page == Page::Upper {
                    c.to_ascii_uppercase()
                } else {
                    c
                };
                if self.page == Page::Upper {
                    self.select_page(Page::Lower);
                }
                Some(Key::Character(c))
            }
        }
    }

    pub fn drag_to(&mut self, x: i32, y: i32, sw: usize, sh: usize) -> bool {
        if !self.dragging {
            return false;
        }
        let max_x = sw.saturating_sub(WIDTH.min(sw.saturating_sub(24))) as i32;
        let max_y = sh.saturating_sub(crate::compositor::TASKBAR_H + HEIGHT) as i32;
        let next = (
            (x - self.drag_offset.0).clamp(0, max_x),
            (y - self.drag_offset.1).clamp(0, max_y),
        );
        let changed = self.position != Some(next);
        self.position = Some(next);
        changed
    }

    pub fn end_drag(&mut self) {
        self.dragging = false;
    }

    pub fn set_hover(&mut self, x: i32, y: i32, sw: usize, sh: usize) -> bool {
        if self.dragging {
            return false;
        }
        let previous = self.engine.hover_token();
        if self.contains(x, y, sw, sh) {
            let (kx, ky, _, _) = self.bounds(sw, sh);
            self.engine.set_hover(x - kx, y - ky);
        } else {
            self.engine.clear_hover();
        }
        let changed = previous != self.engine.hover_token();
        self.surface_dirty |= changed;
        changed
    }

    pub fn tick(&mut self, now_ns: u64) -> bool {
        let engine_changed = self.engine.tick(now_ns);
        let mut changed = engine_changed;
        if self.open_animating {
            let started = *self.open_started_ns.get_or_insert(now_ns);
            let t = (now_ns.saturating_sub(started) as f32 / OPEN_DURATION_NS as f32)
                .clamp(0.0, 1.0);
            let previous = self.render_y_offset;
            let remaining = 1.0 - t;
            self.render_y_offset =
                (OPEN_OFFSET_Y as f32 * remaining * remaining * remaining) as i32;
            changed |= previous != self.render_y_offset || self.open_animating;
            if t >= 1.0 {
                self.open_animating = false;
                self.open_started_ns = None;
            }
        }
        // Translation does not invalidate the cached Warp 3 surface; only
        // Warp's own hover/control animation needs a repaint.
        self.surface_dirty |= engine_changed;
        changed
    }

    /// Returns the union of the old and new overlay bounds. This lets the
    /// compositor restore the background when the cached surface is moved or
    /// closed, instead of repainting the whole screen.
    pub fn take_damage(&mut self, sw: usize, sh: usize) -> Option<(i32, i32, i32, i32)> {
        let current = self.open.then(|| self.bounds(sw, sh));
        let needs_repaint = self.surface_dirty || current != self.presented_bounds;
        if !needs_repaint {
            return None;
        }
        let pad = crate::window::shadow_pad().max(0);
        let expand = |rect: (i32, i32, i32, i32)| {
            (rect.0 - pad, rect.1 - pad, rect.2 + pad * 2, rect.3 + pad * 2)
        };
        let damage = match (self.presented_bounds, current) {
            (Some(a), Some(b)) => Some((
                a.0.min(b.0),
                a.1.min(b.1),
                (a.0 + a.2).max(b.0 + b.2),
                (a.1 + a.3).max(b.1 + b.3),
            )),
            (Some(a), None) | (None, Some(a)) => Some((a.0, a.1, a.0 + a.2, a.1 + a.3)),
            (None, None) => None,
        };
        self.presented_bounds = current;
        damage.map(expand)
    }

    fn refresh_surface(&mut self) {
        if !self.surface_dirty {
            return;
        }
        self.engine
            .update(WIDTH as i32, (HEIGHT - crate::window::title_bar_h()) as i32);
        self.engine.draw_to_layer(&mut self.surface, 0, 0);
        self.surface_dirty = false;
    }

    pub fn draw(&mut self, layer: &mut LayerSystem) {
        if !self.open {
            return;
        }
        self.refresh_surface();
        let (x, y, w, h) = self.bounds(layer.width(), layer.height());
        if let Some(shadow) = &self.shadow {
            shadow.composite_onto(layer, x, y);
        }
        layer.composit_rounded(
            &self.surface,
            x as usize,
            y as usize,
            0,
            0,
            w as usize,
            h as usize,
            RADIUS,
        );
    }
}

fn engine_for(main: &'static str) -> Warp3Engine {
    Warp3Engine::new_embedded(
        "os-soft-keyboard",
        &[("config.ini", CONFIG), ("main.w3u", main)],
    )
}

const KEY_IDS: [&str; 26] = [
    "q", "w", "e", "r", "t", "y", "u", "i", "o", "p", "a", "s", "d", "f", "g", "h",
    "j", "k", "l", "z", "x", "c", "v", "b", "n", "m",
];
const LATIN_LOWER: [&str; 26] = [
    "q", "w", "e", "r", "t", "y", "u", "i", "o", "p", "a", "s", "d", "f", "g", "h",
    "j", "k", "l", "z", "x", "c", "v", "b", "n", "m",
];
const LATIN_UPPER: [&str; 26] = [
    "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "A", "S", "D", "F", "G", "H",
    "J", "K", "L", "Z", "X", "C", "V", "B", "N", "M",
];
const DUBEOL_LOWER: [&str; 26] = [
    "ㅂ", "ㅈ", "ㄷ", "ㄱ", "ㅅ", "ㅛ", "ㅕ", "ㅑ", "ㅐ", "ㅔ", "ㅁ", "ㄴ", "ㅇ", "ㄹ", "ㅎ", "ㅗ",
    "ㅓ", "ㅏ", "ㅣ", "ㅋ", "ㅌ", "ㅊ", "ㅍ", "ㅠ", "ㅜ", "ㅡ",
];
const DUBEOL_UPPER: [&str; 26] = [
    "ㅃ", "ㅉ", "ㄸ", "ㄲ", "ㅆ", "ㅛ", "ㅕ", "ㅑ", "ㅒ", "ㅖ", "ㅁ", "ㄴ", "ㅇ", "ㄹ", "ㅎ", "ㅗ",
    "ㅓ", "ㅏ", "ㅣ", "ㅋ", "ㅌ", "ㅊ", "ㅍ", "ㅠ", "ㅜ", "ㅡ",
];
const CHOSUN: [&str; 26] = [
    "ㅂ", "ㅁ", "ㄷ", "ㄹ", "ㄱ", "ㅕ", "ㅜ", "ㅓ", "ㅐ", "ㅔ", "ㅈ", "ㄱ", "ㅇ", "ㄴ", "ㅅ", "ㅗ",
    "ㅏ", "ㅣ", "ㅡ", "ㅋ", "ㅌ", "ㅊ", "ㅍ", "ㅠ", "ㅛ", "ㅑ",
];
const CHOSUN_UPPER: [&str; 26] = [
    "ㅃ", "ㅁ", "ㄸ", "ㄹ", "ㄱ", "ㅕ", "ㅜ", "ㅓ", "ㅒ", "ㅖ", "ㅉ", "ㄲ", "ㅇ", "ㄴ", "ㅆ", "ㅗ",
    "ㅏ", "ㅣ", "ㅡ", "ㅋ", "ㅌ", "ㅊ", "ㅍ", "ㅠ", "ㅛ", "ㅑ",
];
const HANCOM: [&str; 26] = [
    "ㅂ", "ㅡ", "ㅓ", "ㄹ", "ㅌ", "ㅣ", "ㅜ", "ㅣ", "ㅗ", "ㅍ", "ㅏ", "ㅅ", "ㄷ", "ㅍ", "ㄱ", "ㅎ",
    "ㅈ", "ㅋ", "ㄹ", "ㅋ", "ㅇ", "ㅊ", "", "ㅂ", "ㄴ", "ㅁ",
];
const HANCOM_UPPER: [&str; 26] = [
    "ㅂ", "ㅡ", "ㅓ", "ㄹ", "ㅌ", "ㅣ", "ㅜ", "ㅣ", "ㅗ", "ㅍ", "ㅏ", "ㅆ", "ㄸ", "ㅍ", "ㄲ", "ㅎ",
    "ㅉ", "ㅋ", "ㄹ", "ㅋ", "ㅇ", "ㅊ", "", "ㅃ", "ㄴ", "ㅁ",
];

fn keyboard_labels(language: KeyboardLanguage, upper: bool) -> &'static [&'static str; 26] {
    match language {
        KeyboardLanguage::KoreanDubeolsik if upper => &DUBEOL_UPPER,
        KeyboardLanguage::KoreanDubeolsik => &DUBEOL_LOWER,
        KeyboardLanguage::KoreanChosunDubeolsik if upper => &CHOSUN_UPPER,
        KeyboardLanguage::KoreanChosunDubeolsik => &CHOSUN,
        KeyboardLanguage::KoreanHancomRoman if upper => &HANCOM_UPPER,
        KeyboardLanguage::KoreanHancomRoman => &HANCOM,
        _ if upper => &LATIN_UPPER,
        _ => &LATIN_LOWER,
    }
}

fn ime_status(
    language: KeyboardLanguage,
    reading: Option<&str>,
    candidates: &[String],
    selected: usize,
) -> String {
    let language = match language {
        KeyboardLanguage::Latin => "English",
        KeyboardLanguage::Japanese => "日本語",
        KeyboardLanguage::KoreanDubeolsik => "한국어 · 두벌식",
        KeyboardLanguage::KoreanHancomRoman => "한국어 · 한컴 로마자",
        KeyboardLanguage::KoreanChosunDubeolsik => "조선말 · 두벌식",
        KeyboardLanguage::ChinesePinyin => "中文 · 拼音",
    };
    let Some(reading) = reading else {
        return String::from(language);
    };
    if candidates.is_empty() {
        return alloc::format!("{language} · {reading}");
    }
    let mut status = alloc::format!("{language} · {reading}  ");
    for (index, candidate) in candidates.iter().take(5).enumerate() {
        let mut item = String::new();
        if index == selected { item.push('［'); }
        for ch in candidate.chars().take(6) { item.push(ch); }
        if index == selected {
            item.push('］');
        }
        if status.chars().count() + item.chars().count() + 1 > 30 {
            break;
        }
        status.push_str(&item);
        status.push(' ');
    }
    status
}

fn symbol_for_id(id: &str) -> Option<u8> {
    Some(match id {
        "minus" => b'-',
        "slash" => b'/',
        "colon" => b':',
        "semicolon" => b';',
        "lparen" => b'(',
        "rparen" => b')',
        "dollar" => b'$',
        "amp" => b'&',
        "at" => b'@',
        "quote" => b'\'',
        "doublequote" => b'"',
        "question" => b'?',
        "bang" => b'!',
        "plus" => b'+',
        "equals" => b'=',
        "underscore" => b'_',
        _ => return None,
    })
}

impl Default for SoftKeyboard {
    fn default() -> Self {
        Self::new()
    }
}
