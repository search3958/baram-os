#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::ToString;
use alloc::vec::Vec;

use uefi::prelude::*;
use uefi::runtime;

use baram_bsd::config;
use baram_bsd::shift_key;
use baram_core::{Color, LayerSystem, Screen};
use baram_font::log_line_str;
use baram_windowserver::compositor::*;
use baram_windowserver::cursor;
use baram_windowserver::window::{SmoothScroll, WinId, WindowManager};
use wana_kana::ConvertJapanese;

fn kernel_key_event(event: nano_system::NanoKeyEvent) -> baram_core::KeyEvent {
    baram_core::KeyEvent {
        printable: event.printable,
        scancode: event.scancode,
        modifiers: event.modifiers,
        raw_key: event.raw_key,
    }
}

fn kernel_pointer_event(
    event: nano_system::NanoBasicPointerEvent,
    state: nano_system::NanoInputState,
) -> baram_iokit::mouse::MouseEvent {
    if let Some((x, y, max_x, max_y)) = event.absolute {
        baram_iokit::mouse::MouseEvent {
            abs_x: x,
            abs_y: y,
            abs_max_x: max_x,
            abs_max_y: max_y,
            is_absolute: true,
            left: state.left,
            right: state.right,
            ..baram_iokit::mouse::MouseEvent::default()
        }
    } else {
        baram_iokit::mouse::MouseEvent {
            rel_dx: event.dx,
            rel_dy: event.dy,
            left: state.left,
            right: state.right,
            scroll: state.scroll,
            ..baram_iokit::mouse::MouseEvent::default()
        }
    }
}
use nano_system::NanoSystem;

// Keep this comfortably longer than the normal 16 ms present interval so
// opening an app always has visible intermediate taskbar frames.
const TASKBAR_ADD_ANIMATION_MS: u64 = 180;
const MOZC_DICTIONARY: &str = include_str!("mozc_dictionary.tsv");

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Latin,
    Hiragana,
    Korean(KoreanLayout),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum KoreanLayout {
    Dubeolsik,
    HancomRoman,
    ChosunDubeolsik,
}

/// Keeps the uncommitted romaji so each key can replace the visible
/// composition with Wanakana's current hiragana conversion.
struct JapaneseIme {
    romaji: alloc::string::String,
    visible_chars: usize,
    conversion: Option<JapaneseConversion>,
}

struct JapaneseConversion {
    kana: alloc::string::String,
    candidates: alloc::vec::Vec<alloc::string::String>,
    selected: usize,
}

impl JapaneseIme {
    fn new() -> Self {
        Self {
            romaji: alloc::string::String::new(),
            visible_chars: 0,
            conversion: None,
        }
    }

    fn reset(&mut self) {
        self.romaji.clear();
        self.visible_chars = 0;
        self.conversion = None;
    }

    fn edit(&mut self, key: u8) -> (alloc::string::String, usize) {
        // Starting another romaji run commits the selected candidate already
        // visible in the target field.
        if key != 0x08 && key != 0x7f && self.conversion.is_some() {
            self.conversion = None;
            self.romaji.clear();
            self.visible_chars = 0;
        }
        if key == 0x08 || key == 0x7f {
            if self.romaji.pop().is_some() {
                let text = self.romaji.as_str().to_hiragana();
                let replace = self.visible_chars;
                self.visible_chars = text.chars().count();
                return (text, replace);
            }
            return (alloc::string::String::new(), 1);
        }

        let ch = key as char;
        if ch.is_ascii_alphabetic() || ch == '\'' {
            self.romaji.push(ch);
            let text = self.romaji.as_str().to_hiragana();
            let replace = self.visible_chars;
            self.visible_chars = text.chars().count();
            return (text, replace);
        }

        // A separator commits the romaji run and is inserted verbatim.
        let mut text = self.romaji.as_str().to_hiragana();
        text.push(ch);
        let replace = self.visible_chars;
        self.reset();
        (text, replace)
    }

    /// Starts (or advances) kana-to-kanji conversion. The returned edit
    /// replaces the currently visible composition in the focused input.
    fn convert(&mut self) -> Option<(alloc::string::String, usize)> {
        if let Some(conversion) = self.conversion.as_mut() {
            conversion.selected = (conversion.selected + 1) % conversion.candidates.len();
            let text = conversion.candidates[conversion.selected].clone();
            let replace = self.visible_chars;
            self.visible_chars = text.chars().count();
            return Some((text, replace));
        }

        let kana = self.romaji.as_str().to_hiragana();
        let candidates = mozc_candidates(&kana)?;
        let text = candidates[0].clone();
        let replace = self.visible_chars;
        self.visible_chars = text.chars().count();
        self.conversion = Some(JapaneseConversion {
            kana,
            candidates,
            selected: 0,
        });
        Some((text, replace))
    }

    fn commit_conversion(&mut self) {
        if self.conversion.is_some() {
            self.reset();
        }
    }

    fn edit_for_key(&mut self, key: u8) -> (alloc::string::String, usize) {
        if key == b' ' {
            return self.convert().unwrap_or_else(|| self.edit(key));
        }
        if key == b'\n' || key == b'\r' {
            self.commit_conversion();
            return (alloc::string::String::new(), 0);
        }
        self.edit(key)
    }

    fn conversion_view(&self) -> Option<(&str, &[alloc::string::String], usize)> {
        self.conversion
            .as_ref()
            .map(|conversion| (conversion.kana.as_str(), conversion.candidates.as_slice(), conversion.selected))
    }
}

/// Looks up the compact index generated directly from Mozc's OSS dictionary.
/// Entries are sorted by reading and candidates preserve Mozc's cost ranking.
fn mozc_candidates(kana: &str) -> Option<alloc::vec::Vec<alloc::string::String>> {
    for line in MOZC_DICTIONARY.lines().skip(1) {
        let mut fields = line.split('\t');
        let key = fields.next()?;
        if key == kana {
            let candidates = fields.map(alloc::string::String::from).collect();
            return Some(candidates);
        }
    }
    None
}

/// Incremental modern Hangul composer shared by the three Korean layouts.
/// Keeping the raw input lets the final consonant move to the following
/// syllable when a vowel is typed, as users expect (ㄱㅏㄴㅏ -> 가나).
struct HangulIme {
    raw: alloc::string::String,
    visible_chars: usize,
}

impl HangulIme {
    fn new() -> Self {
        Self { raw: alloc::string::String::new(), visible_chars: 0 }
    }

    fn reset(&mut self) {
        self.raw.clear();
        self.visible_chars = 0;
    }

    fn rendered(&self, layout: KoreanLayout) -> alloc::string::String {
        let jamo = match layout {
            KoreanLayout::HancomRoman => hancom_roman_jamo(&self.raw),
            _ => self.raw.chars().collect(),
        };
        compose_hangul(&jamo)
    }

    fn edit_for_key(&mut self, key: u8, layout: KoreanLayout) -> (alloc::string::String, usize) {
        if key == 0x08 || key == 0x7f {
            if self.raw.pop().is_some() {
                let text = self.rendered(layout);
                let replace = self.visible_chars;
                self.visible_chars = text.chars().count();
                return (text, replace);
            }
            return (alloc::string::String::new(), 1);
        }
        if key == b'\n' || key == b'\r' {
            self.reset();
            return (alloc::string::String::new(), 0);
        }

        let input = key as char;
        // Hancom Roman leaves V unassigned. Consume it without altering the
        // pending syllable rather than leaking a Latin V into the target.
        if layout == KoreanLayout::HancomRoman && matches!(input, 'v' | 'V') {
            return (alloc::string::String::new(), 0);
        }
        let accepted = match layout {
            KoreanLayout::Dubeolsik => dubeolsik_jamo(input),
            KoreanLayout::ChosunDubeolsik => chosun_dubeolsik_jamo(input),
            KoreanLayout::HancomRoman if input.is_ascii_alphabetic() => Some(input),
            KoreanLayout::HancomRoman => None,
        };
        if let Some(jamo) = accepted {
            self.raw.push(jamo);
            let text = self.rendered(layout);
            let replace = self.visible_chars;
            self.visible_chars = text.chars().count();
            return (text, replace);
        }

        let mut text = self.rendered(layout);
        text.push(input);
        let replace = self.visible_chars;
        self.reset();
        (text, replace)
    }
}

fn dubeolsik_jamo(key: char) -> Option<char> {
    Some(match key {
        'q' => 'ㅂ', 'Q' => 'ㅃ', 'w' => 'ㅈ', 'W' => 'ㅉ', 'e' => 'ㄷ', 'E' => 'ㄸ',
        'r' => 'ㄱ', 'R' => 'ㄲ', 't' => 'ㅅ', 'T' => 'ㅆ', 'y' => 'ㅛ', 'u' => 'ㅕ',
        'i' => 'ㅑ', 'o' => 'ㅐ', 'O' => 'ㅒ', 'p' => 'ㅔ', 'P' => 'ㅖ', 'a' => 'ㅁ',
        's' => 'ㄴ', 'd' => 'ㅇ', 'f' => 'ㄹ', 'g' => 'ㅎ', 'h' => 'ㅗ', 'j' => 'ㅓ',
        'k' => 'ㅏ', 'l' => 'ㅣ', 'z' => 'ㅋ', 'x' => 'ㅌ', 'c' => 'ㅊ', 'v' => 'ㅍ',
        'b' => 'ㅠ', 'n' => 'ㅜ', 'm' => 'ㅡ', _ => return None,
    })
}

/// 조선 두벌식, exactly following the layout supplied in the request.
fn chosun_dubeolsik_jamo(key: char) -> Option<char> {
    Some(match key {
        'q' => 'ㅂ', 'Q' => 'ㅃ', 'w' | 'W' => 'ㅁ', 'e' => 'ㄷ', 'E' => 'ㄸ',
        'r' | 'R' => 'ㄹ', 't' | 'T' => 'ㄱ', 'y' | 'Y' => 'ㅕ', 'u' | 'U' => 'ㅜ',
        'i' | 'I' => 'ㅓ', 'o' => 'ㅐ', 'O' => 'ㅒ', 'p' => 'ㅔ', 'P' => 'ㅖ',
        'a' => 'ㅈ', 'A' => 'ㅉ', 's' => 'ㄱ', 'S' => 'ㄲ', 'd' | 'D' => 'ㅇ',
        'f' | 'F' => 'ㄴ', 'g' => 'ㅅ', 'G' => 'ㅆ', 'h' | 'H' => 'ㅗ', 'j' | 'J' => 'ㅏ',
        'k' | 'K' => 'ㅣ', 'l' | 'L' => 'ㅡ', 'z' | 'Z' => 'ㅋ', 'x' | 'X' => 'ㅌ',
        'c' | 'C' => 'ㅊ', 'v' | 'V' => 'ㅍ', 'b' | 'B' => 'ㅠ', 'n' | 'N' => 'ㅛ',
        'm' | 'M' => 'ㅑ', _ => return None,
    })
}

fn hancom_roman_jamo(raw: &str) -> alloc::vec::Vec<char> {
    let lower = raw.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut jamo = alloc::vec::Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let tail = &lower[i..];
        let (len, ch) = if tail.starts_with("yei") || tail.starts_with("iei") { (3, 'ㅖ') }
        else if tail.starts_with("yai") || tail.starts_with("iai") { (3, 'ㅒ') }
        else if tail.starts_with("ya") || tail.starts_with("ia") { (2, 'ㅑ') }
        else if tail.starts_with("yu") || tail.starts_with("iu") { (2, 'ㅠ') }
        else if tail.starts_with("yo") || tail.starts_with("io") { (2, 'ㅛ') }
        else if tail.starts_with("ye") || tail.starts_with("ie") { (2, 'ㅕ') }
        else if tail.starts_with("ai") { (2, 'ㅐ') }
        else if tail.starts_with("ei") { (2, 'ㅔ') }
        else if tail.starts_with("oi") { (2, 'ㅚ') }
        else if tail.starts_with("ui") { (2, 'ㅟ') }
        else if tail.starts_with("wi") { (2, 'ㅢ') }
        else {
            let original = raw.as_bytes()[i] as char;
            let ch = match original {
                // Shift produces the five modern tense consonants.
                'G' => 'ㄲ', 'D' => 'ㄸ', 'B' => 'ㅃ', 'S' => 'ㅆ', 'J' => 'ㅉ',
                'a' | 'A' => 'ㅏ', 'e' | 'E' => 'ㅓ', 'i' | 'I' | 'y' | 'Y' => 'ㅣ',
                'o' | 'O' => 'ㅗ', 'u' | 'U' => 'ㅜ', 'w' | 'W' => 'ㅡ',
                'g' => 'ㄱ', 'n' | 'N' => 'ㄴ', 'd' => 'ㄷ', 'r' | 'l' | 'R' | 'L' => 'ㄹ',
                'm' | 'M' => 'ㅁ', 'b' => 'ㅂ', 's' => 'ㅅ', 'j' => 'ㅈ', 'h' | 'H' => 'ㅎ',
                'f' | 'F' | 'p' | 'P' => 'ㅍ', 't' | 'T' => 'ㅌ', 'k' | 'K' => 'ㅋ',
                'c' | 'C' => 'ㅊ', 'x' | 'X' => 'ㅇ',
                // V is intentionally unmapped in this layout.
                'v' | 'V' => { i += 1; continue; }
                _ => { i += 1; continue; }
            };
            (1, ch)
        };
        jamo.push(ch);
        i += len;
    }
    jamo
}

fn ime_menu_selection(mode: InputMode) -> usize {
    match mode {
        InputMode::Latin => 0,
        InputMode::Hiragana => 1,
        InputMode::Korean(KoreanLayout::Dubeolsik) => 2,
        InputMode::Korean(KoreanLayout::HancomRoman) => 3,
        InputMode::Korean(KoreanLayout::ChosunDubeolsik) => 4,
    }
}

fn input_mode_for_menu_selection(selection: usize) -> InputMode {
    match selection {
        1 => InputMode::Hiragana,
        2 => InputMode::Korean(KoreanLayout::Dubeolsik),
        3 => InputMode::Korean(KoreanLayout::HancomRoman),
        4 => InputMode::Korean(KoreanLayout::ChosunDubeolsik),
        _ => InputMode::Latin,
    }
}

fn ime_edit_for_key(
    mode: InputMode,
    japanese: &mut JapaneseIme,
    hangul: &mut HangulIme,
    key: u8,
) -> Option<(alloc::string::String, usize)> {
    match mode {
        InputMode::Latin => None,
        InputMode::Hiragana => Some(japanese.edit_for_key(key)),
        InputMode::Korean(layout) => Some(hangul.edit_for_key(key, layout)),
    }
}

fn initial_index(ch: char) -> Option<u32> {
    "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ".chars().position(|c| c == ch).map(|i| i as u32)
}
fn vowel_index(ch: char) -> Option<u32> {
    "ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ".chars().position(|c| c == ch).map(|i| i as u32)
}
fn final_index(ch: char) -> Option<u32> {
    "\0ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ".chars().position(|c| c == ch).map(|i| i as u32)
}
fn combined_vowel(a: char, b: char) -> Option<char> {
    match (a, b) { ('ㅗ','ㅏ') => Some('ㅘ'), ('ㅗ','ㅐ') => Some('ㅙ'), ('ㅗ','ㅣ') => Some('ㅚ'),
        ('ㅜ','ㅓ') => Some('ㅝ'), ('ㅜ','ㅔ') => Some('ㅞ'), ('ㅜ','ㅣ') => Some('ㅟ'),
        ('ㅡ','ㅣ') => Some('ㅢ'), _ => None }
}
fn combined_final(a: char, b: char) -> Option<char> {
    match (a, b) { ('ㄱ','ㅅ') => Some('ㄳ'), ('ㄴ','ㅈ') => Some('ㄵ'), ('ㄴ','ㅎ') => Some('ㄶ'),
        ('ㄹ','ㄱ') => Some('ㄺ'), ('ㄹ','ㅁ') => Some('ㄻ'), ('ㄹ','ㅂ') => Some('ㄼ'),
        ('ㄹ','ㅅ') => Some('ㄽ'), ('ㄹ','ㅌ') => Some('ㄾ'), ('ㄹ','ㅍ') => Some('ㄿ'),
        ('ㄹ','ㅎ') => Some('ㅀ'), ('ㅂ','ㅅ') => Some('ㅄ'), _ => None }
}
fn compose_hangul(jamo: &[char]) -> alloc::string::String {
    let mut out = alloc::string::String::new();
    let mut i = 0;
    while i < jamo.len() {
        let Some(l) = initial_index(jamo[i]) else { out.push(jamo[i]); i += 1; continue; };
        if i + 1 >= jamo.len() { out.push(jamo[i]); break; }
        let Some(mut v) = vowel_index(jamo[i + 1]) else { out.push(jamo[i]); i += 1; continue; };
        let vowel = jamo[i + 1];
        i += 2;
        if i < jamo.len() {
            if let Some(next_v) = vowel_index(jamo[i]) {
                if let Some(combined) = combined_vowel(vowel, jamo[i]) {
                    v = vowel_index(combined).unwrap_or(next_v);
                    i += 1;
                }
            }
        }
        let mut t = 0;
        if i < jamo.len() && initial_index(jamo[i]).is_some() {
            let c = jamo[i];
            let followed_by_vowel = i + 1 < jamo.len() && vowel_index(jamo[i + 1]).is_some();
            if !followed_by_vowel {
                if i + 1 < jamo.len() && initial_index(jamo[i + 1]).is_some()
                    && !(i + 2 < jamo.len() && vowel_index(jamo[i + 2]).is_some()) {
                    if let Some(cluster) = combined_final(c, jamo[i + 1]) {
                        t = final_index(cluster).unwrap_or(0);
                        i += 2;
                    }
                }
                if t == 0 {
                    if let Some(final_jamo) = final_index(c) { t = final_jamo; i += 1; }
                }
            }
        }
        if let Some(syllable) = char::from_u32(0xac00 + (l * 21 + v) * 28 + t) { out.push(syllable); }
    }
    out
}

struct UiMonotonicClock {
    last: u64,
    frequency_hz: u64,
}

impl UiMonotonicClock {
    fn new() -> Option<Self> {
        let frequency_hz = monotonic_counter_frequency()?;
        Some(Self {
            last: monotonic_counter(),
            frequency_hz,
        })
    }

    #[inline]
    fn frame_delta_ms(&mut self) -> u64 {
        let now = monotonic_counter();
        let ticks = now.wrapping_sub(self.last);
        self.last = now;
        // Firmware and virtual CPUs occasionally expose an inaccurate TSC
        // ratio or jump the counter. Never let one sample consume the entire
        // animation, and always advance at least one timer quantum.
        let measured = ((ticks as u128 * 1_000) / self.frequency_hz as u128) as u64;
        measured.clamp(1, 16)
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn monotonic_counter() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

#[cfg(target_arch = "x86_64")]
fn monotonic_counter_frequency() -> Option<u64> {
    use core::arch::x86_64::{__cpuid, __cpuid_count};
    unsafe {
        let max_leaf = __cpuid(0).eax;
        if max_leaf >= 0x15 {
            let leaf = __cpuid_count(0x15, 0);
            if leaf.eax != 0 && leaf.ebx != 0 && leaf.ecx != 0 {
                return Some((leaf.ecx as u64).saturating_mul(leaf.ebx as u64) / leaf.eax as u64);
            }
        }
        if max_leaf >= 0x16 {
            let mhz = __cpuid(0x16).eax;
            if mhz != 0 {
                return Some(mhz as u64 * 1_000_000);
            }
        }
        // Some virtual firmware hides leaves 0x15/0x16. Calibrate once at
        // startup instead of falling back to coalesced timer-event counts.
        let start = core::arch::x86_64::_rdtsc();
        uefi::boot::stall(core::time::Duration::from_millis(10));
        let elapsed = core::arch::x86_64::_rdtsc().wrapping_sub(start);
        if elapsed != 0 {
            return Some(elapsed.saturating_mul(100));
        }
    }
    None
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn monotonic_counter() -> u64 {
    let value: u64;
    unsafe {
        core::arch::asm!("mrs {0}, cntvct_el0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

#[cfg(target_arch = "aarch64")]
fn monotonic_counter_frequency() -> Option<u64> {
    let value: u64;
    unsafe {
        core::arch::asm!("mrs {0}, cntfrq_el0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    if value == 0 {
        None
    } else {
        Some(value)
    }
}

fn baram_kernel_main(mut nano: NanoSystem) -> Status {
    let timer_event = nano.take_timer_event();
    let mut screen = match Screen::take() {
        Ok(screen) => screen,
        Err(_) => {
            NanoSystem::paint_failure_screen();
            return Status::UNSUPPORTED;
        }
    };

    unsafe { baram_font::log::init_screen(&screen) };
    log_line_str("BaramOS kernel: starting...");

    unsafe { baram_kern::panic::init_from_screen(&screen) };

    // Nano System has already cleared the framebuffer before input probing;
    // replace that minimal handoff screen with the kernel boot logo now.
    draw_boot_logo(&mut screen);

    let compute_workers = baram_core::parallel::init();
    log_line_str(&alloc::format!(
        "BaramOS: {} compute APs enabled",
        compute_workers
    ));

    config::init_config();
    let mut mouse_motion = baram_iokit::mouse::MouseMotionProcessor::new();
    log_line_str("BaramOS: config loaded");

    baram_font::ttf_font::init();
    baram_font::ttf_font_hud::init();
    log_line_str("BaramOS: fonts initialized");

    unsafe {
        baram_windowserver::cursor::CURSOR_NORMAL = Some(cursor::prerender_cursor(
            cursor::CURSOR_SVG,
            cursor::CURSOR_BOX_W,
            cursor::CURSOR_BOX_H,
            8,
        ));
        baram_windowserver::cursor::CURSOR_RESIZE = Some(cursor::prerender_cursor(
            cursor::CURSOR_SVG_SIZE,
            cursor::CURSOR_BOX_SIZE_W,
            cursor::CURSOR_BOX_SIZE_H,
            8,
        ));
    }

    log_line_str("BaramOS: input is owned by Nano System");
    nano.set_shift_key(shift_key::load_shift_key());

    let mut cursor_x: i32 = (screen.width() / 2) as i32;
    let mut cursor_y: i32 = (screen.height() / 2) as i32;
    let mut display_state = baram_bsd::uri::DisplayState::new();
    baram_bsd::uri::load_settings_from_config(&mut display_state);

    if !baram_bsd::setup::is_setup_done() {
        log_line_str("BaramOS: first boot detected, starting setup wizard");
        let mut wizard = baram_bsd::setup::SetupWizard::new();
        let setup_w = screen.width();
        let setup_h = screen.height();
        let mut setup_engine = baram_windowserver::html::HtmlEngine::new_warp3("setup.w3a");
        let mut setup_scene = LayerSystem::new(setup_w, setup_h);
        let mut setup_present = LayerSystem::new(setup_w, setup_h);
        let mut setup_surface = LayerSystem::new(528, 320);
        let setup_origin = ((setup_w as i32 - 528) / 2, (setup_h as i32 - 320) / 2);
        let setup_card = (setup_origin.0, setup_origin.1, 528usize, 320usize);
        let setup_wallpaper = wallpaper_for_state(&display_state, setup_w, setup_h);
        let setup_background = setup_wallpaper.as_ref().map(|wallpaper| {
            let mut blurred = alloc::vec![0u32; setup_w * setup_h];
            let mut scratch = alloc::vec![0u32; setup_w * setup_h];
            baram_graphics::blur::blur_region_to_with_scratch(
                wallpaper,
                &mut blurred,
                &mut scratch,
                setup_w,
                0,
                setup_h,
                30,
            );
            blurred
        });
        let card_radius = config::get_usize("ui-theme/card/radius", 12);
        let setup_shadow =
            baram_windowserver::window::RoundedShadow::new(setup_card.2, setup_card.3, card_radius);
        let mut setup_scene_dirty = true;
        let mut setup_prev_cursor = (cursor_x, cursor_y);
        let mut setup_now_ns = 0u64;
        let mut setup_next_present_ms = 0u64;
        setup_engine.set_warp3_screen(wizard.warp3_screen());
        setup_engine.update(528, 320);

        loop {
            if let Some(ref timer) = timer_event {
                let mut events = [unsafe { core::ptr::read(timer) }];
                let _ = uefi::boot::wait_for_event(&mut events);
            }
            setup_now_ns = setup_now_ns.saturating_add(1_000_000);

            while let Some(nano_event) = nano.poll_keyboard() {
                let ev = kernel_key_event(nano_event);
                wizard.on_key(&ev);
            }

            {
                while let Some(nano_event) = nano.poll_pointer() {
                    let ev =
                        mouse_motion.process(kernel_pointer_event(nano_event, nano.input_state));
                    baram_iokit::mouse::apply_mouse_event(
                        &mut cursor_x,
                        &mut cursor_y,
                        &ev,
                        screen.width(),
                        screen.height(),
                        nano.pointer_abs_max(),
                    );
                    setup_engine.set_hover(cursor_x - setup_origin.0, cursor_y - setup_origin.1);
                    setup_scene_dirty = true;
                    if ev.left {
                        setup_engine.click(cursor_x - setup_origin.0, cursor_y - setup_origin.1);
                        if let Some(command) = setup_engine.last_command.take() {
                            wizard.on_command(&command);
                        }
                    }
                }
            }

            if wizard.screen == baram_bsd::setup::SetupScreen::Done {
                break;
            }

            cursor_x = cursor_x.max(0).min(screen.width() as i32 - 1);
            cursor_y = cursor_y.max(0).min(screen.height() as i32 - 1);
            if wizard.take_dirty() {
                setup_engine.set_warp3_screen(wizard.warp3_screen());
                setup_engine.update(528, 320);
                setup_scene_dirty = true;
            }
            if setup_engine.tick(setup_now_ns) {
                setup_scene_dirty = true;
            }

            let setup_now_ms = setup_now_ns / 1_000_000;
            let cursor_changed = setup_prev_cursor != (cursor_x, cursor_y);
            if !setup_scene_dirty && !cursor_changed {
                continue;
            }
            if setup_now_ms < setup_next_present_ms && !cursor_changed {
                continue;
            }
            setup_next_present_ms = setup_now_ms.saturating_add(16);

            if setup_scene_dirty {
                if let Some(ref background) = setup_background {
                    setup_scene.copy_from_screen_buffer(background);
                } else {
                    setup_scene.clear(config::get_color("ui-theme/color/bg", Color::BG));
                }
                if let Some(ref shadow) = setup_shadow {
                    shadow.composite_onto(&mut setup_scene, setup_card.0, setup_card.1);
                }
                setup_engine.update(528, 320);
                setup_engine.draw_to_layer(&mut setup_surface, 0, 0);
                setup_scene.composit_rounded(
                    &setup_surface,
                    setup_origin.0.max(0) as usize,
                    setup_origin.1.max(0) as usize,
                    0,
                    0,
                    528,
                    320,
                    card_radius,
                );
                setup_present.copy_from_screen_buffer(setup_scene.buf_ref());
            } else {
                let pad = 32i32;
                let x0 = (setup_prev_cursor.0.min(cursor_x) - pad).max(0) as usize;
                let y0 = (setup_prev_cursor.1.min(cursor_y) - pad).max(0) as usize;
                let x1 = (setup_prev_cursor.0.max(cursor_x) + cursor::CURSOR_BOX_W as i32 + pad)
                    .min(setup_w as i32) as usize;
                let y1 = (setup_prev_cursor.1.max(cursor_y) + cursor::CURSOR_BOX_H as i32 + pad)
                    .min(setup_h as i32) as usize;
                setup_present.push_clip(x0, y0, x1, y1);
                setup_present.copy_from_screen_buffer(setup_scene.buf_ref());
                setup_present.pop_clip();
            }

            cursor::draw_cursor_into_layer(
                &mut setup_present,
                cursor_x,
                cursor_y,
                false,
                display_state.pointer_size,
            );
            if setup_scene_dirty {
                setup_present.flush(&mut screen);
            } else {
                let pad = 32i32;
                let x0 = (setup_prev_cursor.0.min(cursor_x) - pad).max(0) as usize;
                let y0 = (setup_prev_cursor.1.min(cursor_y) - pad).max(0) as usize;
                let x1 = (setup_prev_cursor.0.max(cursor_x) + cursor::CURSOR_BOX_W as i32 + pad)
                    .min(setup_w as i32) as usize;
                let y1 = (setup_prev_cursor.1.max(cursor_y) + cursor::CURSOR_BOX_H as i32 + pad)
                    .min(setup_h as i32) as usize;
                setup_present.flush_rect(&mut screen, x0, y0, x1, y1);
            }
            setup_scene_dirty = false;
            setup_prev_cursor = (cursor_x, cursor_y);
        }
        log_line_str("BaramOS: setup wizard completed");
        nano.set_shift_key(shift_key::load_shift_key());
    }

    let mut wm = WindowManager::new(screen.width(), screen.height());
    let mut layer = LayerSystem::new(screen.width(), screen.height());

    log_line_str("BaramOS: loading index.yaml...");
    let index_yaml = baram_bsd::app::read_index_yaml();
    log_line_str(&alloc::format!(
        "BaramOS: index.yaml {} bytes",
        index_yaml.len()
    ));
    let (autostart_list, app_entries) = parse_index_yaml(&index_yaml);
    let mut warp_engines: alloc::vec::Vec<(WinId, baram_windowserver::warp::WarpEngine)> =
        alloc::vec::Vec::new();
    let mut html_engines: alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)> =
        alloc::vec::Vec::new();
    let mut ui_win_id: Option<WinId> = None;
    let mut ui_commands: alloc::vec::Vec<baram_graphics::uiscript::Command> =
        alloc::vec::Vec::new();

    let mut auto_idx = 0i32;
    for autostart_name in &autostart_list {
        if let Some(entry) = app_entries.iter().find(|e| &e.name == autostart_name) {
            let x = 60 + (auto_idx * 120) % 500;
            let y = 60 + (auto_idx * 80) % 400;
            let w = 400;
            let h = 450;
            if open_app(
                &entry.name,
                &app_entries,
                &mut wm,
                &mut warp_engines,
                &mut html_engines,
                &mut ui_commands,
                &mut ui_win_id,
                x,
                y,
                w,
                h,
            )
            .is_some()
            {
                auto_idx += 1;
            }
        }
    }

    let mut last_keys: Vec<&'static str> = Vec::with_capacity(8);
    let mut mouse_ev_count: u32 = 0;
    let mut key_ev_count: u32 = 0;
    let mut frames: u32 = 0;
    let mut fps: u32 = 0;
    let mut frames_since_tick: u32 = 0;
    let mut start_time = runtime::get_time().unwrap_or_else(|_| runtime::Time::invalid());
    let mut mouse_down = false;
    let mut new_window_idx: u32 = 0;
    let mut keyboard_click: bool = false;
    let mut wasd_first_press: [u64; 4] = [0; 4];
    let mut wasd_moved: [bool; 4] = [false; 4];

    let mut mousekey_mode: bool = false;
    let mut shift_press_times: [u64; 3] = [0; 3];
    let mut shift_press_idx: usize = 0;
    let mut prev_shift_held: bool = false;
    let mut mousekey_win_id: Option<WinId> = None;
    let mut pending_os_permission: Option<PendingOsPermission> = None;

    let mouse_mode_label = if nano.input.absolute_pointer_available {
        "Absolute"
    } else if nano.input.pointer_available {
        "Simple Ptr"
    } else {
        "None"
    };

    let mut cached_wallpaper: Option<Vec<u32>> = None;
    if let Some(bytes) = WALLPAPERS.get(display_state.wallpaper_index) {
        cached_wallpaper = decode_wallpaper(bytes, screen.width(), screen.height());
    }

    let mut scene_dirty = true;
    let mut cached_scene: Vec<u32> = alloc::vec![0u32; screen.width() * screen.height()];
    let mut prev_cursor_x = cursor_x;
    let mut prev_cursor_y = cursor_y;
    let mut prev_is_resizing = false;
    let shadow_pad = 35i32;

    let mut taskbar_surface = TaskbarSurface::new(screen.width());
    let mut cached_launcher_layer: Option<Vec<u32>> = None;
    let mut cached_ime_menu_layer: Option<Vec<u32>> = None;
    let mut prev_window_count: usize = 0;
    let mut prev_focused_id: Option<WinId> = None;
    let mut bg_cache: Option<Vec<u32>> = None;
    let mut prev_wallpaper_idx: usize = display_state.wallpaper_index;
    let mut hud_damage_pending = false;
    // Monotonic UI clock driven by the already-configured 1 ms timer event.
    // Do not query the slow, wall-clock UEFI runtime service per frame.
    let mut ui_time_ms: u64 = 0;
    let mut ui_clock = UiMonotonicClock::new();
    let mut next_present_ms: u64 = 0;
    let mut deferred_dirty = false;

    let mut tb_add_progress: f32 = -1.0f32;
    let mut tb_add_started_ms: Option<u64> = None;
    let mut tb_remove_progress: f32 = -1.0f32;
    let mut tb_shift_x: f32 = 0.0f32;
    let mut show_app_launcher: bool = false;
    let mut app_search_focused: bool = false;
    let mut app_search_query = alloc::string::String::new();
    let mut app_launcher_scroll = SmoothScroll::new();
    let mut app_list: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    let mut app_name_list: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    let mut app_icon_list: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    for entry in &app_entries {
        app_list.push(entry.title.clone());
        app_name_list.push(entry.name.clone());
        app_icon_list.push(entry.icon.clone());
    }
    // The search box has no hover visual; keep this compatibility argument
    // stable without making pointer movement schedule a redraw.
    let hover_apps_icon = false;
    let mut prev_show_app_launcher: bool = false;
    let mut launcher_content_dirty: bool = false;
    let mut launcher_render_visible = false;
    let mut launcher_target_prev = false;
    let mut launcher_anim_phase: i8 = 0; // 1 opening, -1 closing
    let mut launcher_anim_started_ms = 0u64;
    let mut launcher_anim_elapsed_ms = 0u32;
    let mut launcher_cache_drop_after_close = false;
    let mut input_mode = InputMode::Latin;
    let mut japanese_ime = JapaneseIme::new();
    let mut hangul_ime = HangulIme::new();
    let mut show_ime_menu = false;
    let mut prev_show_ime_menu = false;
    let mut prev_ime_conversion_visible = false;

    let timezone_offset: i32 = config::get_config().get_i32("system/timezone").unwrap_or(9);

    let mut battery_info = baram_iokit::battery::read_battery();
    let mut battery_poll_seconds: u8 = 0;

    let (mut clock_hh, mut clock_mm) = {
        let tz = timezone_offset;
        match runtime::get_time() {
            Ok(t) => {
                let total_min = (t.hour() as i32) * 60 + (t.minute() as i32) + tz * 60;
                let day_min = total_min.rem_euclid(24 * 60);
                ((day_min / 60) as u8, (day_min % 60) as u8)
            }
            Err(_) => (0u8, 0u8),
        }
    };

    render_scene(
        &mut layer,
        &mut taskbar_surface,
        &mut wm,
        mouse_ev_count,
        key_ev_count,
        fps,
        mouse_mode_label,
        &ui_commands,
        ui_win_id,
        &mut warp_engines,
        &mut html_engines,
        cached_wallpaper.as_deref(),
        &mut cached_launcher_layer,
        &mut cached_ime_menu_layer,
        true,
        -1.0,
        -1.0,
        0.0,
        display_state.hud_enabled,
        &mut bg_cache,
        false,
        launcher_render_visible,
        &app_list,
        &app_icon_list,
        hover_apps_icon,
        app_search_focused,
        &app_search_query,
        app_launcher_scroll.position.max(0) as usize,
        0,
        0,
        false,
        false,
        false,
        clock_hh,
        clock_mm,
        battery_info.valid_percentage(),
        false,
        ime_menu_selection(input_mode),
        None,
        &[],
        0,
    );
    // Build the hidden launcher once while the boot scene is already hot.
    // With zero layer opacity this leaves the framebuffer unchanged, but the
    // first click never has to decode icons, rasterize labels, or blur glass.
    render_scene(
        &mut layer,
        &mut taskbar_surface,
        &mut wm,
        mouse_ev_count,
        key_ev_count,
        fps,
        mouse_mode_label,
        &ui_commands,
        ui_win_id,
        &mut warp_engines,
        &mut html_engines,
        cached_wallpaper.as_deref(),
        &mut cached_launcher_layer,
        &mut cached_ime_menu_layer,
        false,
        -1.0,
        -1.0,
        0.0,
        display_state.hud_enabled,
        &mut bg_cache,
        true,
        true,
        &app_list,
        &app_icon_list,
        hover_apps_icon,
        app_search_focused,
        &app_search_query,
        app_launcher_scroll.position.max(0) as usize,
        1,
        0,
        false,
        true,
        false,
        clock_hh,
        clock_mm,
        battery_info.valid_percentage(),
        false,
        ime_menu_selection(input_mode),
        None,
        &[],
        0,
    );
    prev_window_count = wm.count();
    prev_focused_id = wm.focused_id;
    cached_scene.copy_from_slice(layer.buf_ref());
    cursor::draw_cursor_into_layer(
        &mut layer,
        cursor_x,
        cursor_y,
        false,
        display_state.pointer_size,
    );
    layer.flush(&mut screen);

    loop {
        let mut dirty = deferred_dirty;
        deferred_dirty = false;
        let mut cursor_moved = false;
        let mut scroll_input = false;
        let mut launcher_scroll_input_changed = false;
        let mut ui_timer_fired = timer_event.is_none();

        if let Some(ref timer) = timer_event {
            let mut events = [unsafe { core::ptr::read(timer) }];
            ui_timer_fired = uefi::boot::wait_for_event(&mut events).is_ok();
        }
        if let Some(ref mut clock) = ui_clock {
            ui_time_ms = ui_time_ms.wrapping_add(clock.frame_delta_ms());
        } else if ui_timer_fired {
            ui_time_ms = ui_time_ms.wrapping_add(1);
        }

        match baram_bsd::uri::check_system_commands(&mut display_state) {
            baram_bsd::uri::SystemCommand::ResetAll => {
                NanoSystem::cold_reset();
            }
            baram_bsd::uri::SystemCommand::None => {}
        }

        while let Some(nano_event) = nano.poll_keyboard() {
            let ev = kernel_key_event(nano_event);
            key_ev_count = key_ev_count.wrapping_add(1);
            if last_keys.len() >= 6 {
                last_keys.remove(0);
            }
            last_keys.push(ev.label());

            match ev.scancode {
                0x01 => wm.scroll_focused(-baram_windowserver::window::scroll_speed()),
                0x02 => wm.scroll_focused(baram_windowserver::window::scroll_speed()),
                _ => {}
            }

            // Nano System reports UEFI Backspace as special scan code 0x08,
            // so it does not always arrive through `printable`.
            if app_search_focused && ev.scancode == 0x08 {
                if let Some((text, replace_chars)) =
                    ime_edit_for_key(input_mode, &mut japanese_ime, &mut hangul_ime, 0x08)
                {
                    for _ in 0..replace_chars {
                        app_search_query.pop();
                    }
                    app_search_query.push_str(&text);
                } else {
                    app_search_query.pop();
                }
                rebuild_filtered_apps(
                    &app_entries,
                    &app_search_query,
                    &mut app_list,
                    &mut app_name_list,
                    &mut app_icon_list,
                );
                app_launcher_scroll.set_max(app_launcher_scroll_max(app_list.len()));
                show_app_launcher = true;
                cached_launcher_layer = None;
                launcher_content_dirty = true;
                taskbar_surface.invalidate_search();
                dirty = true;
                scene_dirty = true;
                continue;
            }

            if ev.ctrl_or_cmd() || (mousekey_mode && nano.shift_held()) {
                if let Some(c) = ev.printable {
                    match c {
                        b' ' => {
                            keyboard_click = true;
                            dirty = true;
                            scene_dirty = true;
                        }
                        _ => {}
                    }
                }
            } else if let Some(c) = ev.printable {
                let mut handled = false;
                if app_search_focused {
                    if let Some((text, replace_chars)) =
                        ime_edit_for_key(input_mode, &mut japanese_ime, &mut hangul_ime, c)
                    {
                        for _ in 0..replace_chars {
                            app_search_query.pop();
                        }
                        app_search_query.push_str(&text);
                    } else {
                        match c {
                            0x08 | 0x7f => {
                                app_search_query.pop();
                            }
                            0x20..=0x7e => app_search_query.push(c as char),
                            _ => {}
                        }
                    }
                    rebuild_filtered_apps(
                        &app_entries,
                        &app_search_query,
                        &mut app_list,
                        &mut app_name_list,
                        &mut app_icon_list,
                    );
                    app_launcher_scroll.set_max(app_launcher_scroll_max(app_list.len()));
                    show_app_launcher = app_search_focused || !app_search_query.is_empty();
                    cached_launcher_layer = None;
                    launcher_content_dirty = true;
                    taskbar_surface.invalidate_search();
                    handled = true;
                    dirty = true;
                    scene_dirty = true;
                }
                if let Some(focused_win) = wm.focused_id {
                    for (wid, engine) in warp_engines.iter_mut() {
                        if handled {
                            break;
                        }
                        if *wid == focused_win
                            && !wm.is_interaction_blocked(focused_win)
                            && !engine.focused_input_var.is_empty()
                        {
                            if let Some((text, replace_chars)) =
                                ime_edit_for_key(input_mode, &mut japanese_ime, &mut hangul_ime, c)
                            {
                                engine.handle_text(&text, replace_chars);
                            } else {
                                engine.handle_key(c);
                            }
                            if let Some((_, _, ww, wh, _)) = wm.get_window_rect(*wid) {
                                let tb_h = baram_windowserver::window::title_bar_h() as i32;
                                let content_h = (wh as i32).saturating_sub(tb_h);
                                engine.update(ww as i32, content_h);
                                wm.clamp_window_scroll(*wid, engine.content_height);
                                wm.set_content_dirty(*wid);
                            }
                            handled = true;
                            dirty = true;
                            scene_dirty = true;
                            break;
                        }
                    }
                }
                if !handled {
                    if let Some(focused_win) = wm.focused_id {
                        for (wid, engine) in html_engines.iter_mut() {
                            if *wid == focused_win
                                && !wm.is_interaction_blocked(focused_win)
                                && engine.has_focused_input()
                            {
                                if let Some((text, replace_chars)) =
                                    ime_edit_for_key(input_mode, &mut japanese_ime, &mut hangul_ime, c)
                                {
                                    engine.handle_text(&text, replace_chars);
                                } else {
                                    engine.handle_key(c);
                                }
                                if let Some((_, _, ww, wh, scroll)) = wm.get_window_rect(*wid) {
                                    let content_h = wh
                                        .saturating_sub(baram_windowserver::window::title_bar_h());
                                    engine.set_scroll(scroll);
                                    engine.update(ww as i32, content_h as i32);
                                    wm.clamp_window_scroll(*wid, engine.content_height);
                                    wm.set_content_dirty(*wid);
                                }
                                handled = true;
                                dirty = true;
                                scene_dirty = true;
                                break;
                            }
                        }
                    }
                }
                if !handled {
                    match c {
                        _ => {}
                    }
                }
            }
            dirty = true;
            scene_dirty = true;
        }

        {
            let shift_held = nano.shift_held();
            let shift_just_pressed = shift_held && !prev_shift_held;
            prev_shift_held = shift_held;

            if shift_just_pressed {
                let now_ns = runtime::get_time()
                    .map(|t| {
                        t.nanosecond() as u64
                            + t.second() as u64 * 1_000_000_000
                            + t.minute() as u64 * 60_000_000_000
                            + t.hour() as u64 * 3_600_000_000_000
                    })
                    .unwrap_or(0);
                let threshold_ns = 1_000_000_000;

                shift_press_times[shift_press_idx % 3] = now_ns;
                shift_press_idx += 1;

                if shift_press_idx >= 3 {
                    let oldest = shift_press_times[(shift_press_idx - 3) % 3];
                    if now_ns.saturating_sub(oldest) <= threshold_ns {
                        mousekey_mode = !mousekey_mode;
                        shift_press_idx = 0;

                        if mousekey_mode {
                            let nx = (screen.width() as i32 - 400) / 2;
                            let ny = (screen.height() as i32 - 300) / 2;
                            let win_id = wm.add("マウスキー", nx, ny, 400, 300);
                            let mut engine = baram_windowserver::html::HtmlEngine::new_warp3(
                                "mousekeydialog.w3a",
                            );
                            engine.update(380, 260);
                            html_engines.push((win_id, engine));
                            mousekey_win_id = Some(win_id);
                            tb_add_progress = 0.0;
                            tb_add_started_ms = None;
                            tb_shift_x = 26.0;
                            dirty = true;
                            scene_dirty = true;
                        } else {
                            if let Some(wid) = mousekey_win_id.take() {
                                wm.remove(wid);
                                warp_engines.retain(|(id, _)| *id != wid);
                                html_engines.retain(|(id, _)| *id != wid);
                                dirty = true;
                                scene_dirty = true;
                            }
                        }
                    }
                }
            }
        }

        if nano.ctrl_or_cmd_held() || mousekey_mode {
            let step = 8i32;
            let now_ns = runtime::get_time()
                .map(|t| {
                    t.nanosecond() as u64
                        + t.second() as u64 * 1_000_000_000
                        + t.minute() as u64 * 60_000_000_000
                        + t.hour() as u64 * 3_600_000_000_000
                })
                .unwrap_or(0);
            let delay_ns = 300_000_000;

            let keys = [
                (0x1A, 0usize),
                (0x04, 1usize),
                (0x16, 2usize),
                (0x07, 3usize),
            ];

            for (usb_code, idx) in keys {
                if nano.key_is_held(usb_code) {
                    if wasd_first_press[idx] == 0 {
                        wasd_first_press[idx] = now_ns;
                        wasd_moved[idx] = true;
                        match idx {
                            0 => {
                                cursor_y = (cursor_y - step).max(0);
                            }
                            1 => {
                                cursor_x = (cursor_x - step).max(0);
                            }
                            2 => {
                                cursor_y = (cursor_y + step).min(screen.height() as i32 - 1);
                            }
                            3 => {
                                cursor_x = (cursor_x + step).min(screen.width() as i32 - 1);
                            }
                            _ => {}
                        }
                        dirty = true;
                        scene_dirty = true;
                    } else {
                        let elapsed = now_ns.saturating_sub(wasd_first_press[idx]);
                        if elapsed >= delay_ns {
                            match idx {
                                0 => {
                                    cursor_y = (cursor_y - step).max(0);
                                }
                                1 => {
                                    cursor_x = (cursor_x - step).max(0);
                                }
                                2 => {
                                    cursor_y = (cursor_y + step).min(screen.height() as i32 - 1);
                                }
                                3 => {
                                    cursor_x = (cursor_x + step).min(screen.width() as i32 - 1);
                                }
                                _ => {}
                            }
                            dirty = true;
                            scene_dirty = true;
                        }
                    }
                } else {
                    wasd_first_press[idx] = 0;
                    wasd_moved[idx] = false;
                }
            }
        }

        {
            while let Some(nano_event) = nano.poll_pointer() {
                let ev = mouse_motion.process(kernel_pointer_event(nano_event, nano.input_state));
                mouse_ev_count = mouse_ev_count.wrapping_add(1);

                let old_cursor = (cursor_x, cursor_y);
                let (cx, cy) = baram_iokit::mouse::apply_mouse_event(
                    &mut cursor_x,
                    &mut cursor_y,
                    &ev,
                    screen.width(),
                    screen.height(),
                    nano.pointer_abs_max(),
                );
                cursor_moved |= (cx, cy) != old_cursor;

                if ev.scroll != 0 {
                    scroll_input = true;
                    let window_scroll_delta = ev
                        .scroll
                        .saturating_neg()
                        .saturating_mul(baram_windowserver::window::scroll_speed());
                    let panel_y = screen.height() as i32 - TASKBAR_H as i32 - (3 * 88 + 24) as i32;
                    let on_launcher = show_app_launcher
                        && cx >= 12
                        && cx < 300
                        && cy >= panel_y
                        && cy < screen.height() as i32 - TASKBAR_H as i32;
                    if on_launcher {
                        app_launcher_scroll.set_max(app_launcher_scroll_max(app_list.len()));
                        launcher_scroll_input_changed |=
                            app_launcher_scroll.scroll(window_scroll_delta);
                        launcher_content_dirty |= launcher_scroll_input_changed;
                        dirty = true;
                        scene_dirty = true;
                    } else if !show_app_launcher {
                        if let Some(id) = wm.window_at(cx, cy) {
                            wm.scroll_window(id, window_scroll_delta);
                            dirty = true;
                            scene_dirty = true;
                        }
                    }
                }

                if ev.left && !mouse_down {
                    mouse_down = true;
                    // Clicking a different input ends the previous composition.
                    japanese_ime.reset();
                    hangul_ime.reset();
                    let sh = screen.height();

                    // The mode picker is modal: a click selects a row or
                    // dismisses it before reaching the window underneath.
                    if show_ime_menu {
                        if let Some(selection) = ime_menu_mode_at(
                            cx,
                            cy,
                            screen.width(),
                            screen.height(),
                            battery_info.valid_percentage(),
                        ) {
                            input_mode = input_mode_for_menu_selection(selection);
                            hangul_ime.reset();
                            taskbar_surface.invalidate();
                        }
                        show_ime_menu = false;
                        scene_dirty = true;
                        dirty = true;
                        continue;
                    }

                    if show_app_launcher {
                        let search_x = 12i32;
                        let search_y = sh as i32 - TASKBAR_H as i32 + (TASKBAR_H as i32 - 40) / 2;
                        if cx >= search_x
                            && cx < search_x + 190
                            && cy >= search_y
                            && cy < search_y + 40
                        {
                            app_search_focused = true;
                            taskbar_surface.invalidate_search();
                            scene_dirty = true;
                            dirty = true;
                            continue;
                        }
                        let cols = 4usize;
                        let icon_size = 52usize;
                        let icon_gap = 16usize;
                        let label_h = 20usize;
                        let cell_w = icon_size + icon_gap;
                        let cell_h = icon_size + label_h + icon_gap;
                        let grid_w = cols * cell_w;
                        let rows = 3usize;
                        let grid_h = rows * cell_h;
                        let grid_x = 20usize;
                        let grid_y = screen.height().saturating_sub(TASKBAR_H + grid_h + 16);
                        let content_y = grid_y + 4;
                        let panel_x = 12i32;
                        let panel_y = grid_y.saturating_sub(8) as i32;
                        let panel_w = (grid_w + 16) as i32;
                        let panel_h = (grid_h.max(40) + 16) as i32;
                        let on_launcher_panel = cx >= panel_x
                            && cx < panel_x + panel_w
                            && cy >= panel_y
                            && cy < panel_y + panel_h;
                        let mut clicked_app = None;
                        for (i, _) in app_list.iter().enumerate() {
                            let col = i % cols;
                            let row = i / cols;
                            let ix = grid_x + col * cell_w + icon_gap / 2;
                            let iy = content_y as i32 + row as i32 * cell_h as i32
                                - app_launcher_scroll.position;
                            if cx >= ix as i32
                                && cx < (ix + icon_size) as i32
                                && cy >= content_y as i32
                                && cy < (content_y + grid_h) as i32
                                && cy >= iy
                                && cy < iy + icon_size as i32
                            {
                                clicked_app = Some(i);
                                break;
                            }
                        }
                        if let Some(idx) = clicked_app {
                            let app_name = app_name_list[idx].clone();
                            let nx = 100 + ((new_window_idx as i32 * 37) % 300);
                            let ny = 60 + ((new_window_idx as i32 * 23) % 200);
                            open_app(
                                &app_name,
                                &app_entries,
                                &mut wm,
                                &mut warp_engines,
                                &mut html_engines,
                                &mut ui_commands,
                                &mut ui_win_id,
                                nx,
                                ny,
                                400,
                                450,
                            );
                            tb_add_progress = 0.0;
                            tb_add_started_ms = None;
                            tb_shift_x = 26.0;
                            new_window_idx = new_window_idx.wrapping_add(1);
                            app_search_query.clear();
                            app_search_focused = false;
                            rebuild_filtered_apps(
                                &app_entries,
                                "",
                                &mut app_list,
                                &mut app_name_list,
                                &mut app_icon_list,
                            );
                            taskbar_surface.invalidate();
                            launcher_cache_drop_after_close = true;
                            show_app_launcher = false;
                        } else if on_launcher_panel {
                            app_search_focused = false;
                            taskbar_surface.invalidate_search();
                            show_app_launcher = true;
                        } else {
                            app_search_query.clear();
                            app_search_focused = false;
                            rebuild_filtered_apps(
                                &app_entries,
                                "",
                                &mut app_list,
                                &mut app_name_list,
                                &mut app_icon_list,
                            );
                            launcher_cache_drop_after_close = true;
                            taskbar_surface.invalidate_search();
                            show_app_launcher = false;
                        }
                        scene_dirty = true;
                    } else if cy >= sh as i32 - TASKBAR_H as i32 {
                        let (ime_x, ime_y, ime_w, ime_h) =
                            ime_button_bounds(screen.width(), battery_info.valid_percentage());
                        let ime_y = sh as i32 - TASKBAR_H as i32 + ime_y;
                        if cx >= ime_x && cx < ime_x + ime_w && cy >= ime_y && cy < ime_y + ime_h {
                            show_ime_menu = true;
                            scene_dirty = true;
                            dirty = true;
                            continue;
                        }
                        let apps_icon_x = 12i32;
                        let apps_icon_size = 190i32;
                        let apps_icon_y =
                            sh as i32 - TASKBAR_H as i32 + (TASKBAR_H as i32 - 40) / 2;
                        let on_apps_icon = cx >= apps_icon_x
                            && cx < apps_icon_x + apps_icon_size
                            && cy >= apps_icon_y
                            && cy < apps_icon_y + 40;
                        if on_apps_icon {
                            app_launcher_scroll.reset();
                            app_launcher_scroll.set_max(app_launcher_scroll_max(app_list.len()));
                            app_search_focused = true;
                            show_app_launcher = true;
                            taskbar_surface.invalidate_search();
                            scene_dirty = true;
                        } else {
                            if show_app_launcher {
                                show_app_launcher = false;
                                scene_dirty = true;
                            }
                            let ids = wm.insertion_ids();
                            let count = ids.len();
                            let btn_d = 40i32;
                            let btn_gap = 12i32;
                            let total_w = count as i32 * (btn_d + btn_gap) - btn_gap;
                            let mut bx = ((screen.width() as i32 - total_w) / 2).max(0);
                            let btn_y =
                                (sh as usize).saturating_sub(TASKBAR_H) + (TASKBAR_H - 40) / 2;
                            for id in &ids {
                                let dx = cx - bx - btn_d / 2;
                                let dy = cy - btn_y as i32 - btn_d / 2;
                                if dx * dx + dy * dy <= (btn_d / 2) * (btn_d / 2) {
                                    if wm.is_minimized(*id) {
                                        wm.restore_minimized(*id);
                                    }
                                    wm.focus(*id);
                                    break;
                                }
                                bx += btn_d + btn_gap;
                            }
                        }
                    } else {
                        let win_under = wm.window_at(cx, cy);
                        if let Some(id) = win_under {
                            wm.focus(id);
                            let btn = wm.button_hit_at(id, cx, cy);
                            match btn {
                                'c' => {
                                    wm.remove(id);
                                    warp_engines.retain(|(wid, _)| *wid != id);
                                    html_engines.retain(|(wid, _)| *wid != id);
                                    cancel_permission_for_closed_window(
                                        id,
                                        &mut wm,
                                        &mut html_engines,
                                        &mut pending_os_permission,
                                    );
                                }
                                'm' => {
                                    wm.toggle_maximize_at(id);
                                }
                                'i' => {
                                    wm.toggle_minimize_at(id);
                                }
                                _ => {
                                    if wm.resize_hit_at(id, cx, cy) {
                                        wm.start_resize_at(id, cx, cy);
                                    } else {
                                        wm.start_drag_at(id, cx, cy);
                                    }
                                }
                            }
                            let after = wm.insertion_ids();
                            let _ = after;
                        }
                        if let Some(clicked_id) = wm.window_at(cx, cy) {
                            for (wid, engine) in warp_engines.iter_mut() {
                                if clicked_id == *wid && !wm.is_interaction_blocked(clicked_id) {
                                    if let Some((wx, wy, ww, wh, scroll)) =
                                        wm.get_window_rect(clicked_id)
                                    {
                                        let rel_x = cx - wx;
                                        let rel_y = cy - wy;
                                        let tb_h = baram_windowserver::window::title_bar_h() as i32;
                                        if rel_y >= tb_h {
                                            let warp_y = rel_y + scroll;
                                            engine.click(rel_x, warp_y);
                                            let content_h = wh.saturating_sub(tb_h as usize);
                                            engine.update(ww as i32, content_h as i32);
                                            wm.set_content_dirty(clicked_id);
                                            scene_dirty = true;

                                            if let Some(cmd) = engine.last_command.take() {
                                                let is_hud_command = baram_bsd::uri::parse(&cmd)
                                                    .map_or(false, |p| {
                                                        p.path.starts_with("display/hud")
                                                    });
                                                let previous_hud = display_state.hud_enabled;
                                                if authorize_os_setting(
                                                    &cmd,
                                                    engine.origin(),
                                                    &mut wm,
                                                    &mut html_engines,
                                                    &mut pending_os_permission,
                                                    Some(clicked_id),
                                                    120,
                                                    80,
                                                ) && baram_bsd::uri::execute(
                                                    &cmd,
                                                    &mut display_state,
                                                ) {
                                                    engine.update(ww as i32, content_h as i32);
                                                    if is_hud_command {
                                                        hud_damage_pending |= previous_hud
                                                            != display_state.hud_enabled;
                                                    } else {
                                                        wm.set_all_dirty();
                                                        taskbar_surface.invalidate();
                                                        cached_launcher_layer = None;
                                                        app_launcher_scroll.reset();
                                                        bg_cache = None;
                                                    }
                                                    scene_dirty = true;
                                                }
                                                if let Some(parsed) = baram_bsd::uri::parse(&cmd) {
                                                    if parsed.path.starts_with("display/wallpaper")
                                                    {
                                                        if display_state.wallpaper_mode
                                                            == baram_bsd::uri::WallpaperMode::Color
                                                        {
                                                            if let Some(color) =
                                                                display_state.wallpaper_color
                                                            {
                                                                cached_wallpaper =
                                                                    Some(make_solid_wallpaper(
                                                                        color,
                                                                        screen.width(),
                                                                        screen.height(),
                                                                    ));
                                                            }
                                                        } else {
                                                            if let Some(bytes) = WALLPAPERS
                                                                .get(display_state.wallpaper_index)
                                                            {
                                                                cached_wallpaper = decode_wallpaper(
                                                                    bytes,
                                                                    screen.width(),
                                                                    screen.height(),
                                                                );
                                                            } else {
                                                                log_line_str("NO WALLPAPER BYTES");
                                                            }
                                                        }
                                                        prev_wallpaper_idx =
                                                            display_state.wallpaper_index;
                                                        scene_dirty = true;
                                                    } else if parsed
                                                        .path
                                                        .starts_with("display/pointer")
                                                        || parsed.path.starts_with("display/hud")
                                                    {
                                                        scene_dirty = true;
                                                    } else {
                                                        scene_dirty = true;
                                                    }
                                                }
                                            }

                                            if let Some(enabled_str) =
                                                engine.get_state_value("--hudEnabled")
                                            {
                                                let new_enabled = enabled_str == "true";
                                                if display_state.hud_enabled != new_enabled {
                                                    display_state.hud_enabled = new_enabled;
                                                    hud_damage_pending = true;
                                                    scene_dirty = true;
                                                }
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                            let mut html_command = None;
                            for (wid, engine) in html_engines.iter_mut() {
                                if clicked_id != *wid || wm.is_interaction_blocked(clicked_id) {
                                    continue;
                                }
                                if let Some((wx, wy, ww, wh, scroll)) =
                                    wm.get_window_rect(clicked_id)
                                {
                                    let rel_x = cx - wx;
                                    let rel_y = cy - wy;
                                    let tb_h = baram_windowserver::window::title_bar_h() as i32;
                                    if rel_y >= tb_h {
                                        engine.set_scroll(scroll);
                                        engine.set_runtime_metrics(
                                            fps,
                                            wm.count(),
                                            key_ev_count,
                                            mouse_ev_count,
                                        );
                                        engine.click(rel_x, rel_y + scroll);
                                        engine.update(
                                            ww as i32,
                                            wh.saturating_sub(tb_h as usize) as i32,
                                        );
                                        if let Some(target) = engine.take_scroll_request() {
                                            wm.set_window_scroll(clicked_id, target);
                                        }
                                        html_command = engine.last_command.take().map(|command| {
                                            (command, engine.origin().to_string(), *wid)
                                        });
                                        wm.set_content_dirty(clicked_id);
                                        scene_dirty = true;
                                    }
                                }
                                break;
                            }
                            if let Some((cmd, origin, source_win_id)) = html_command {
                                let nx = 100 + ((new_window_idx as i32 * 37) % 300);
                                let ny = 60 + ((new_window_idx as i32 * 23) % 200);
                                match handle_navigation(
                                    &cmd,
                                    &app_entries,
                                    &mut wm,
                                    &mut warp_engines,
                                    &mut html_engines,
                                    &mut ui_commands,
                                    &mut ui_win_id,
                                    &mut display_state,
                                    &origin,
                                    source_win_id,
                                    &mut pending_os_permission,
                                    nx,
                                    ny,
                                ) {
                                    NavigationEffect::AppOpened => {
                                        new_window_idx = new_window_idx.wrapping_add(1);
                                        tb_add_progress = 0.0;
                                        tb_add_started_ms = None;
                                        tb_shift_x = 26.0;
                                    }
                                    NavigationEffect::SystemChanged => {
                                        taskbar_surface.invalidate();
                                        cached_launcher_layer = None;
                                        bg_cache = None;
                                        cached_wallpaper = wallpaper_for_state(
                                            &display_state,
                                            screen.width(),
                                            screen.height(),
                                        );
                                        prev_wallpaper_idx = display_state.wallpaper_index;
                                    }
                                    NavigationEffect::None => {}
                                }
                                scene_dirty = true;
                            }
                        }
                    }
                    scene_dirty = true;
                } else if !ev.left && mouse_down {
                    mouse_down = false;
                    wm.on_mouse_up();
                    scene_dirty = true;
                }

                if mouse_down {
                    wm.on_mouse_drag(cx, cy);
                    scene_dirty = true;
                }

                dirty = true;
            }
        }

        if keyboard_click {
            keyboard_click = false;
            let cx = cursor_x;
            let cy = cursor_y;
            let sh = screen.height();
            let search_y = sh as i32 - TASKBAR_H as i32 + (TASKBAR_H as i32 - 40) / 2;
            let on_search = cx >= 12 && cx < 202 && cy >= search_y && cy < search_y + 40;

            if show_ime_menu {
                if let Some(selection) = ime_menu_mode_at(
                    cx,
                    cy,
                    screen.width(),
                    screen.height(),
                    battery_info.valid_percentage(),
                ) {
                    input_mode = input_mode_for_menu_selection(selection);
                    japanese_ime.reset();
                    hangul_ime.reset();
                    taskbar_surface.invalidate();
                }
                show_ime_menu = false;
                scene_dirty = true;
                dirty = true;
            } else if show_app_launcher && on_search {
                app_search_focused = true;
                taskbar_surface.invalidate_search();
                scene_dirty = true;
            } else if show_app_launcher {
                let cols = 4usize;
                let icon_size = 52usize;
                let icon_gap = 16usize;
                let label_h = 20usize;
                let cell_w = icon_size + icon_gap;
                let cell_h = icon_size + label_h + icon_gap;
                let grid_w = cols * cell_w;
                let rows = 3usize;
                let grid_h = rows * cell_h;
                let grid_x = 20usize;
                let grid_y = screen.height().saturating_sub(TASKBAR_H + grid_h + 16);
                let content_y = grid_y + 4;
                let on_launcher_panel = cx >= 12
                    && cx < (12 + grid_w + 16) as i32
                    && cy >= grid_y.saturating_sub(8) as i32
                    && cy < (grid_y.saturating_sub(8) + grid_h.max(40) + 16) as i32;
                let mut clicked_app = None;
                for (i, _) in app_list.iter().enumerate() {
                    let col = i % cols;
                    let row = i / cols;
                    let ix = grid_x + col * cell_w + icon_gap / 2;
                    let iy = content_y as i32 + row as i32 * cell_h as i32
                        - app_launcher_scroll.position;
                    if cx >= ix as i32
                        && cx < (ix + icon_size) as i32
                        && cy >= content_y as i32
                        && cy < (content_y + grid_h) as i32
                        && cy >= iy
                        && cy < iy + icon_size as i32
                    {
                        clicked_app = Some(i);
                        break;
                    }
                }
                if let Some(idx) = clicked_app {
                    let app_name = app_name_list[idx].clone();
                    let nx = 100 + ((new_window_idx as i32 * 37) % 300);
                    let ny = 60 + ((new_window_idx as i32 * 23) % 200);
                    open_app(
                        &app_name,
                        &app_entries,
                        &mut wm,
                        &mut warp_engines,
                        &mut html_engines,
                        &mut ui_commands,
                        &mut ui_win_id,
                        nx,
                        ny,
                        400,
                        450,
                    );
                    tb_add_progress = 0.0;
                    tb_add_started_ms = None;
                    tb_shift_x = 26.0;
                    new_window_idx = new_window_idx.wrapping_add(1);
                    app_search_query.clear();
                    rebuild_filtered_apps(
                        &app_entries,
                        "",
                        &mut app_list,
                        &mut app_name_list,
                        &mut app_icon_list,
                    );
                    show_app_launcher = false;
                    launcher_cache_drop_after_close = true;
                } else if on_launcher_panel {
                    app_search_focused = false;
                    show_app_launcher = true;
                } else {
                    app_search_query.clear();
                    app_search_focused = false;
                    rebuild_filtered_apps(
                        &app_entries,
                        "",
                        &mut app_list,
                        &mut app_name_list,
                        &mut app_icon_list,
                    );
                    launcher_cache_drop_after_close = true;
                    show_app_launcher = false;
                }
                taskbar_surface.invalidate();
                scene_dirty = true;
            } else if cy >= sh as i32 - TASKBAR_H as i32 {
                let (ime_x, ime_y, ime_w, ime_h) =
                    ime_button_bounds(screen.width(), battery_info.valid_percentage());
                let ime_y = sh as i32 - TASKBAR_H as i32 + ime_y;
                if cx >= ime_x && cx < ime_x + ime_w && cy >= ime_y && cy < ime_y + ime_h {
                    show_ime_menu = true;
                    scene_dirty = true;
                    dirty = true;
                    continue;
                }
                let apps_icon_x = 12i32;
                let apps_icon_size = 190i32;
                let apps_icon_y = search_y;
                let on_apps_icon = cx >= apps_icon_x
                    && cx < apps_icon_x + apps_icon_size
                    && cy >= apps_icon_y
                    && cy < apps_icon_y + 40;
                if on_apps_icon {
                    app_launcher_scroll.reset();
                    app_launcher_scroll.set_max(app_launcher_scroll_max(app_list.len()));
                    app_search_focused = true;
                    show_app_launcher = true;
                    taskbar_surface.invalidate_search();
                    scene_dirty = true;
                } else {
                    if show_app_launcher {
                        show_app_launcher = false;
                        scene_dirty = true;
                    }
                    let ids = wm.insertion_ids();
                    let count = ids.len();
                    let btn_d = 40i32;
                    let btn_gap = 12i32;
                    let total_w = count as i32 * (btn_d + btn_gap) - btn_gap;
                    let mut bx = ((screen.width() as i32 - total_w) / 2).max(0);
                    let btn_y = (sh as usize).saturating_sub(TASKBAR_H) + (TASKBAR_H - 40) / 2;
                    for id in &ids {
                        let dx = cx - bx - btn_d / 2;
                        let dy = cy - btn_y as i32 - btn_d / 2;
                        if dx * dx + dy * dy <= (btn_d / 2) * (btn_d / 2) {
                            if wm.is_minimized(*id) {
                                wm.restore_minimized(*id);
                            }
                            wm.focus(*id);
                            break;
                        }
                        bx += btn_d + btn_gap;
                    }
                }
            } else {
                let win_under = wm.window_at(cx, cy);
                if let Some(id) = win_under {
                    wm.focus(id);
                    let btn = wm.button_hit_at(id, cx, cy);
                    match btn {
                        'c' => {
                            wm.remove(id);
                            warp_engines.retain(|(wid, _)| *wid != id);
                            html_engines.retain(|(wid, _)| *wid != id);
                            cancel_permission_for_closed_window(
                                id,
                                &mut wm,
                                &mut html_engines,
                                &mut pending_os_permission,
                            );
                        }
                        'm' => {
                            wm.toggle_maximize_at(id);
                        }
                        'i' => {
                            wm.toggle_minimize_at(id);
                        }
                        _ => {}
                    }
                }
                if let Some(clicked_id) = wm.window_at(cx, cy) {
                    for (wid, engine) in warp_engines.iter_mut() {
                        if clicked_id == *wid && !wm.is_interaction_blocked(clicked_id) {
                            if let Some((wx, wy, ww, wh, scroll)) = wm.get_window_rect(clicked_id) {
                                let rel_x = cx - wx;
                                let rel_y = cy - wy;
                                let tb_h = baram_windowserver::window::title_bar_h() as i32;
                                if rel_y >= tb_h {
                                    let warp_y = rel_y + scroll;
                                    engine.click(rel_x, warp_y);
                                    let content_h = wh.saturating_sub(tb_h as usize);
                                    engine.update(ww as i32, content_h as i32);
                                    wm.set_content_dirty(clicked_id);
                                    scene_dirty = true;

                                    if let Some(cmd) = engine.last_command.take() {
                                        let is_hud_command = baram_bsd::uri::parse(&cmd)
                                            .map_or(false, |p| p.path.starts_with("display/hud"));
                                        let previous_hud = display_state.hud_enabled;
                                        if authorize_os_setting(
                                            &cmd,
                                            engine.origin(),
                                            &mut wm,
                                            &mut html_engines,
                                            &mut pending_os_permission,
                                            Some(clicked_id),
                                            120,
                                            80,
                                        ) && baram_bsd::uri::execute(&cmd, &mut display_state)
                                        {
                                            engine.update(ww as i32, content_h as i32);
                                            if is_hud_command {
                                                hud_damage_pending |=
                                                    previous_hud != display_state.hud_enabled;
                                            } else {
                                                wm.set_all_dirty();
                                                taskbar_surface.invalidate();
                                                cached_launcher_layer = None;
                                                bg_cache = None;
                                            }
                                            scene_dirty = true;
                                        }
                                        if let Some(parsed) = baram_bsd::uri::parse(&cmd) {
                                            if parsed.path.starts_with("display/wallpaper") {
                                                if display_state.wallpaper_mode
                                                    == baram_bsd::uri::WallpaperMode::Color
                                                {
                                                    if let Some(color) =
                                                        display_state.wallpaper_color
                                                    {
                                                        cached_wallpaper =
                                                            Some(make_solid_wallpaper(
                                                                color,
                                                                screen.width(),
                                                                screen.height(),
                                                            ));
                                                    }
                                                } else {
                                                    if let Some(bytes) = WALLPAPERS
                                                        .get(display_state.wallpaper_index)
                                                    {
                                                        cached_wallpaper = decode_wallpaper(
                                                            bytes,
                                                            screen.width(),
                                                            screen.height(),
                                                        );
                                                    }
                                                }
                                                prev_wallpaper_idx = display_state.wallpaper_index;
                                                scene_dirty = true;
                                            } else if parsed.path.starts_with("display/pointer")
                                                || parsed.path.starts_with("display/hud")
                                            {
                                                scene_dirty = true;
                                            } else {
                                                scene_dirty = true;
                                            }
                                        }
                                    }

                                    if let Some(enabled_str) =
                                        engine.get_state_value("--hudEnabled")
                                    {
                                        let new_enabled = enabled_str == "true";
                                        if display_state.hud_enabled != new_enabled {
                                            display_state.hud_enabled = new_enabled;
                                            hud_damage_pending = true;
                                            scene_dirty = true;
                                        }
                                    }
                                }
                            }
                            break;
                        }
                    }
                    let mut html_command = None;
                    for (wid, engine) in html_engines.iter_mut() {
                        if clicked_id != *wid || wm.is_interaction_blocked(clicked_id) {
                            continue;
                        }
                        if let Some((wx, wy, ww, wh, scroll)) = wm.get_window_rect(clicked_id) {
                            let rel_x = cx - wx;
                            let rel_y = cy - wy;
                            let tb_h = baram_windowserver::window::title_bar_h() as i32;
                            if rel_y >= tb_h {
                                engine.set_scroll(scroll);
                                engine.set_runtime_metrics(
                                    fps,
                                    wm.count(),
                                    key_ev_count,
                                    mouse_ev_count,
                                );
                                engine.click(rel_x, rel_y + scroll);
                                engine.update(ww as i32, wh.saturating_sub(tb_h as usize) as i32);
                                if let Some(target) = engine.take_scroll_request() {
                                    wm.set_window_scroll(clicked_id, target);
                                }
                                html_command = engine
                                    .last_command
                                    .take()
                                    .map(|command| (command, engine.origin().to_string(), *wid));
                                wm.set_content_dirty(clicked_id);
                                scene_dirty = true;
                            }
                        }
                        break;
                    }
                    if let Some((cmd, origin, source_win_id)) = html_command {
                        let nx = 100 + ((new_window_idx as i32 * 37) % 300);
                        let ny = 60 + ((new_window_idx as i32 * 23) % 200);
                        match handle_navigation(
                            &cmd,
                            &app_entries,
                            &mut wm,
                            &mut warp_engines,
                            &mut html_engines,
                            &mut ui_commands,
                            &mut ui_win_id,
                            &mut display_state,
                            &origin,
                            source_win_id,
                            &mut pending_os_permission,
                            nx,
                            ny,
                        ) {
                            NavigationEffect::AppOpened => {
                                new_window_idx = new_window_idx.wrapping_add(1);
                                tb_add_progress = 0.0;
                                tb_add_started_ms = None;
                                tb_shift_x = 26.0;
                            }
                            NavigationEffect::SystemChanged => {
                                taskbar_surface.invalidate();
                                cached_launcher_layer = None;
                                bg_cache = None;
                                cached_wallpaper = wallpaper_for_state(
                                    &display_state,
                                    screen.width(),
                                    screen.height(),
                                );
                                prev_wallpaper_idx = display_state.wallpaper_index;
                            }
                            NavigationEffect::None => {}
                        }
                        scene_dirty = true;
                    }
                }
            }
            dirty = true;
        }

        // Scroll positions are sampled from absolute time. The backing
        // document is already rasterized; each sample only changes the source
        // offset used for the viewport copy.
        let transition_now_ns = ui_time_ms * 1_000_000;
        if wm.tick_scroll_animations(transition_now_ns) {
            scene_dirty = true;
            dirty = true;
        }
        let launcher_scroll_changed =
            launcher_scroll_input_changed || app_launcher_scroll.tick(transition_now_ns);
        if launcher_scroll_changed {
            launcher_content_dirty = true;
            scene_dirty = true;
            dirty = true;
        }

        if show_app_launcher != launcher_target_prev {
            launcher_target_prev = show_app_launcher;
            launcher_anim_phase = if show_app_launcher { 1 } else { -1 };
            launcher_anim_started_ms = ui_time_ms;
            launcher_anim_elapsed_ms = 0;
            launcher_render_visible = true;
            scene_dirty = true;
            dirty = true;
        }
        if launcher_anim_phase != 0 {
            launcher_anim_elapsed_ms = ui_time_ms
                .saturating_sub(launcher_anim_started_ms)
                .min(u32::MAX as u64) as u32;
            let duration = 200;
            if launcher_anim_elapsed_ms >= duration {
                if launcher_anim_phase < 0 {
                    launcher_render_visible = false;
                    if launcher_cache_drop_after_close {
                        cached_launcher_layer = None;
                        launcher_cache_drop_after_close = false;
                    }
                }
                launcher_anim_phase = 0;
            }
            launcher_content_dirty = true;
            scene_dirty = true;
            dirty = true;
        }

        {
            let mut hovered_any = false;
            if let Some(hover_id) = wm.window_at(cursor_x, cursor_y) {
                let scrolling = wm.is_scroll_animating(hover_id);
                for (wid, engine) in warp_engines.iter_mut() {
                    if hover_id == *wid {
                        if scrolling {
                            engine.clear_hover();
                            hovered_any = true;
                            break;
                        }
                        if let Some((wx, wy, _ww, _wh, scroll)) = wm.get_window_rect(hover_id) {
                            let rel_x = cursor_x - wx;
                            let rel_y = cursor_y - wy;
                            let tb_h = baram_windowserver::window::title_bar_h() as i32;
                            let prev_hover = engine.hover_idx;
                            if rel_y >= tb_h {
                                let warp_y = rel_y + scroll;
                                engine.set_hover(rel_x, warp_y);
                            } else {
                                engine.set_hover(rel_x, -1);
                            }
                            if engine.hover_idx != prev_hover {
                                wm.set_content_dirty(hover_id);
                                scene_dirty = true;
                                dirty = true;
                            }
                            hovered_any = true;
                        }
                        break;
                    }
                }
                for (wid, engine) in html_engines.iter_mut() {
                    if hover_id == *wid {
                        if scrolling {
                            engine.cancel_hover();
                            hovered_any = true;
                            break;
                        }
                        if let Some((wx, wy, _ww, _wh, scroll)) = wm.get_window_rect(hover_id) {
                            let rel_x = cursor_x - wx;
                            let rel_y = cursor_y - wy;
                            let tb_h = baram_windowserver::window::title_bar_h() as i32;
                            let previous = engine.hovered_node();
                            if rel_y >= tb_h {
                                engine.set_scroll(scroll);
                                engine.set_hover(rel_x, rel_y + scroll);
                            } else {
                                engine.clear_hover();
                            }
                            if engine.hovered_node() != previous {
                                if let Some((x0, y0, x1, y1)) = engine.window_damage() {
                                    wm.set_content_damage(hover_id, x0, y0, x1, y1);
                                } else {
                                    wm.set_content_dirty(hover_id);
                                }
                                scene_dirty = true;
                                dirty = true;
                            }
                            hovered_any = true;
                        }
                        break;
                    }
                }
            }
            if !hovered_any {
                for (_, engine) in warp_engines.iter_mut() {
                    engine.clear_hover();
                }
                for (_, engine) in html_engines.iter_mut() {
                    engine.clear_hover();
                }
            }
        }

        // Absolute monotonic UI time: transitions derive their progress from
        // this clock, without a runtime-service call in the render hot path.
        let mut deferred_html_commands = alloc::vec::Vec::new();
        let runtime_window_count = wm.count();
        for (wid, engine) in html_engines.iter_mut() {
            engine.set_runtime_metrics(fps, runtime_window_count, key_ev_count, mouse_ev_count);
            if engine.tick(transition_now_ns) {
                if let Some((x0, y0, x1, y1)) = engine.window_damage() {
                    wm.set_content_damage(*wid, x0, y0, x1, y1);
                } else {
                    wm.set_content_dirty(*wid);
                }
                scene_dirty = true;
                dirty = true;
            }
            if let Some(command) = engine.last_command.take() {
                deferred_html_commands.push((command, engine.origin().to_string(), *wid));
            }
        }
        for (command, origin, source_win_id) in deferred_html_commands {
            let previous_hud = display_state.hud_enabled;
            let nx = 100 + ((new_window_idx as i32 * 37) % 300);
            let ny = 60 + ((new_window_idx as i32 * 23) % 200);
            match handle_navigation(
                &command,
                &app_entries,
                &mut wm,
                &mut warp_engines,
                &mut html_engines,
                &mut ui_commands,
                &mut ui_win_id,
                &mut display_state,
                &origin,
                source_win_id,
                &mut pending_os_permission,
                nx,
                ny,
            ) {
                NavigationEffect::AppOpened => {
                    new_window_idx = new_window_idx.wrapping_add(1);
                    tb_add_progress = 0.0;
                    tb_add_started_ms = None;
                    tb_shift_x = 26.0;
                }
                NavigationEffect::SystemChanged => {
                    hud_damage_pending |= previous_hud != display_state.hud_enabled;
                    taskbar_surface.invalidate();
                    cached_launcher_layer = None;
                    bg_cache = None;
                    cached_wallpaper =
                        wallpaper_for_state(&display_state, screen.width(), screen.height());
                    prev_wallpaper_idx = display_state.wallpaper_index;
                }
                NavigationEffect::None => {}
            }
            scene_dirty = true;
            dirty = true;
        }

        frames = frames.wrapping_add(1);
        frames_since_tick = frames_since_tick.wrapping_add(1);
        if let Ok(now) = runtime::get_time() {
            let elapsed_ns = time_diff_ns(&start_time, &now);
            if elapsed_ns >= 1_000_000_000 {
                fps = frames_since_tick;
                frames_since_tick = 0;
                start_time = now;

                let total_min =
                    (now.hour() as i32) * 60 + (now.minute() as i32) + timezone_offset * 60;
                let day_min = total_min.rem_euclid(24 * 60);
                let next_hh = (day_min / 60) as u8;
                let next_mm = (day_min % 60) as u8;
                let clock_changed = next_hh != clock_hh || next_mm != clock_mm;
                clock_hh = next_hh;
                clock_mm = next_mm;

                battery_poll_seconds = battery_poll_seconds.saturating_add(1);
                let mut battery_changed = false;
                if battery_poll_seconds >= 60 {
                    battery_poll_seconds = 0;
                    let next_battery = baram_iokit::battery::read_battery();
                    battery_changed =
                        next_battery.valid_percentage() != battery_info.valid_percentage();
                    battery_info = next_battery;
                }
                if clock_changed || battery_changed {
                    taskbar_surface.invalidate();
                }

                dirty = true;
                scene_dirty = true;
            }
        }

        if wm.take_order_changed() {
            scene_dirty = true;
            dirty = true;
        }

        for (wid, engine) in warp_engines.iter_mut() {
            if let Some((_, _, ww, wh, _)) = wm.get_window_rect(*wid) {
                let content_h = wh.saturating_sub(30);
                engine.update(ww as i32, content_h as i32);
                wm.clamp_window_scroll(*wid, engine.content_height);
            }
        }
        for (wid, engine) in html_engines.iter_mut() {
            if let Some((_, _, ww, wh, scroll)) = wm.get_window_rect(*wid) {
                let content_h = wh.saturating_sub(baram_windowserver::window::title_bar_h());
                engine.set_scroll(scroll);
                engine.update(ww as i32, content_h as i32);
                wm.clamp_window_scroll(*wid, engine.content_height);
            }
        }

        if tb_add_progress >= 0.0 {
            if tb_add_started_ms.is_none() {
                tb_add_started_ms = Some(ui_time_ms);
            }
            let started = tb_add_started_ms.unwrap_or(ui_time_ms);
            tb_add_progress = (ui_time_ms.saturating_sub(started) as f32
                / TASKBAR_ADD_ANIMATION_MS as f32)
                .min(1.0);
            let remaining = 1.0 - tb_add_progress;
            let eased = 1.0 - remaining * remaining * remaining;
            tb_shift_x = 26.0 * (1.0 - eased);
            dirty = true;
            scene_dirty = true;
        }

        if tb_remove_progress >= 0.0 {
            tb_remove_progress = (tb_remove_progress + 0.2).min(1.0);
            dirty = true;
            scene_dirty = true;
        }

        let scroll_animating = wm.has_scroll_animation();
        let taskbar_animating = tb_add_progress >= 0.0 || tb_remove_progress >= 0.0;
        let continuous_motion =
            scroll_animating || app_launcher_scroll.is_animating() || launcher_anim_phase != 0;
        // New scroll input is presented immediately. During easing, use a
        // short deadline instead of the normal 16 ms scene deadline.
        if dirty && ui_time_ms < next_present_ms && !cursor_moved && !scroll_input {
            deferred_dirty = true;
            continue;
        }

        if dirty {
            // The taskbar has to rasterize and flush the whole bottom strip.
            // Cap it at the normal 60 Hz cadence, while retaining the tighter
            // interval for lightweight scrolling and launcher motion.
            let present_interval_ms = if taskbar_animating {
                16
            } else if continuous_motion {
                4
            } else {
                16
            };
            next_present_ms = ui_time_ms.saturating_add(present_interval_ms);
            let is_resizing = wm.is_any_resizing() || wm.is_over_resize_handle(cursor_x, cursor_y);

            if scene_dirty {
                let (bx0, by0, bx1, by1) = wm.dirty_bbox(shadow_pad);

                let bg_valid =
                    bg_cache.is_some() && prev_wallpaper_idx == display_state.wallpaper_index;

                if !launcher_render_visible && (bx1 > bx0 || !bg_valid) {
                    cached_launcher_layer = None;
                }

                let taskbar_dirty = !taskbar_surface.is_valid()
                    || tb_add_progress >= 0.0
                    || tb_remove_progress >= 0.0
                    || tb_shift_x.abs() > 0.5
                    || wm.count() != prev_window_count
                    || wm.focused_id != prev_focused_id
                    || by1 > screen.height().saturating_sub(TASKBAR_H)
                    || !bg_valid;
                let taskbar_search_dirty = taskbar_surface.is_search_dirty();

                let launcher_changed = launcher_render_visible != prev_show_app_launcher;
                let launcher_needs_redraw =
                    launcher_changed || launcher_content_dirty || launcher_anim_phase != 0;
                let hud_dirty = display_state.hud_enabled && !taskbar_surface.is_valid();
                let ime_menu_changed = show_ime_menu != prev_show_ime_menu;
                let (ime_menu_x, ime_menu_y, ime_menu_w, ime_menu_h) = ime_menu_bounds(
                    screen.width(),
                    screen.height(),
                    battery_info.valid_percentage(),
                );
                let ime_menu_cache_dirty = show_ime_menu
                    && (ime_menu_changed
                        || !bg_valid
                        || (bx1 > bx0
                            && bx0 < (ime_menu_x + ime_menu_w + 54).max(0) as usize
                            && bx1 > (ime_menu_x - 54).max(0) as usize
                            && by0 < (ime_menu_y + ime_menu_h + 54).max(0) as usize
                            && by1 > (ime_menu_y - 54).max(0) as usize));
                if ime_menu_changed || ime_menu_cache_dirty {
                    cached_ime_menu_layer = None;
                }

                let taskbar_only = taskbar_dirty
                    && bx1 <= bx0
                    && !hud_dirty
                    && wm.count() == prev_window_count
                    && wm.focused_id == prev_focused_id
                    && prev_wallpaper_idx == display_state.wallpaper_index
                    && bg_cache.is_some()
                    && !launcher_render_visible
                    && !launcher_changed
                    && !ime_menu_changed;

                let launcher_only_redraw = (launcher_anim_phase != 0 || launcher_scroll_changed)
                    && launcher_needs_redraw
                    && cached_launcher_layer.is_some()
                    && bx1 <= bx0
                    && !taskbar_dirty
                    && !hud_dirty
                    && bg_valid;

                let launcher_cursor_separate = cursor_moved
                    && launcher_needs_redraw
                    && bx1 <= bx0
                    && !taskbar_dirty
                    && !hud_dirty
                    && bg_valid;

                if taskbar_only {
                    let w = screen.width();
                    let h = screen.height();
                    let pad = 32i32;
                    let prev_w = if prev_is_resizing {
                        cursor::CURSOR_BOX_SIZE_W
                    } else {
                        cursor::CURSOR_BOX_W
                    };
                    let prev_h = if prev_is_resizing {
                        cursor::CURSOR_BOX_SIZE_H
                    } else {
                        cursor::CURSOR_BOX_H
                    };
                    let x0 = (prev_cursor_x - pad).max(0) as usize;
                    let y0 = (prev_cursor_y - pad).max(0) as usize;
                    let x1 = (prev_cursor_x + prev_w as i32 + pad).min(w as i32) as usize;
                    let y1 = (prev_cursor_y + prev_h as i32 + pad).min(h as i32) as usize;
                    let buf = layer.buf_mut();
                    for y in y0..y1 {
                        let s = y * w + x0;
                        let e = y * w + x1;
                        buf[s..e].copy_from_slice(&cached_scene[s..e]);
                    }
                }

                let w = screen.width();
                let h = screen.height();
                let tb_y = h.saturating_sub(TASKBAR_H);
                let hud_y0 = tb_y.saturating_sub(44);
                let pad = 32i32;
                let cur_w = if is_resizing {
                    cursor::CURSOR_BOX_SIZE_W
                } else {
                    cursor::CURSOR_BOX_W
                };
                let cur_h = if is_resizing {
                    cursor::CURSOR_BOX_SIZE_H
                } else {
                    cursor::CURSOR_BOX_H
                };
                let prev_w = if prev_is_resizing {
                    cursor::CURSOR_BOX_SIZE_W
                } else {
                    cursor::CURSOR_BOX_W
                };
                let prev_h = if prev_is_resizing {
                    cursor::CURSOR_BOX_SIZE_H
                } else {
                    cursor::CURSOR_BOX_H
                };
                let cx0 = (prev_cursor_x.min(cursor_x) - pad).max(0) as usize;
                let cy0 = (prev_cursor_y.min(cursor_y) - pad).max(0) as usize;
                let cx1 = (prev_cursor_x.max(cursor_x) + cur_w.max(prev_w) as i32 + pad)
                    .min(w as i32) as usize;
                let cy1 = (prev_cursor_y.max(cursor_y) + cur_h.max(prev_h) as i32 + pad)
                    .min(h as i32) as usize;

                let (mut fx0, mut fy0, mut fx1, mut fy1) = if taskbar_only {
                    (0, tb_y, w, h)
                } else if cursor_moved && !launcher_cursor_separate {
                    (bx0.min(cx0), by0.min(cy0), bx1.max(cx1), by1.max(cy1))
                } else {
                    (bx0, by0, bx1, by1)
                };
                if taskbar_dirty && !taskbar_only {
                    fx0 = 0;
                    fy0 = fy0.min(tb_y);
                    fx1 = w;
                    fy1 = h;
                }
                if taskbar_search_dirty && !taskbar_dirty {
                    fx0 = fx0.min(0);
                    fy0 = fy0.min(tb_y);
                    fx1 = fx1.max(226.min(w));
                    fy1 = fy1.max(h);
                }
                if hud_dirty {
                    fx0 = 0;
                    fy0 = fy0.min(tb_y.saturating_sub(44));
                    fx1 = w;
                }
                if launcher_needs_redraw {
                    let grid_h = 3 * 88usize;
                    let grid_y = h.saturating_sub(TASKBAR_H + grid_h + 16);
                    let panel_x = 12usize;
                    let panel_y = grid_y.saturating_sub(8);
                    let panel_w = 4 * (52 + 16) + 16;
                    let panel_h = grid_h + 16;
                    let pad = 54usize;
                    fx0 = fx0.min(panel_x.saturating_sub(pad));
                    fy0 = fy0.min(panel_y.saturating_sub(pad));
                    fx1 = fx1.max((panel_x + panel_w + pad).min(w));
                    fy1 = fy1.max((panel_y + panel_h + pad).min(h));
                }
                if !bg_valid {
                    fx0 = 0;
                    fy0 = 0;
                    fx1 = w;
                    fy1 = h;
                }
                let ime_conversion_visible = japanese_ime.conversion.is_some();
                if ime_menu_changed || ime_menu_cache_dirty {
                    let (menu_x, menu_y, menu_w, menu_h) =
                        (ime_menu_x, ime_menu_y, ime_menu_w, ime_menu_h);
                    let pad = 28usize;
                    fx0 = fx0.min((menu_x.max(0) as usize).saturating_sub(pad));
                    fy0 = fy0.min((menu_y.max(0) as usize).saturating_sub(pad));
                    fx1 = fx1.max((menu_x + menu_w).max(0) as usize + pad).min(w);
                    fy1 = fy1.max((menu_y + menu_h).max(0) as usize + pad).min(h);
                }
                if ime_conversion_visible || prev_ime_conversion_visible {
                    fx0 = 0;
                    fy0 = fy0.min(tb_y.saturating_sub(76));
                    fx1 = w;
                    fy1 = fy1.max(tb_y);
                }
                let (ime_reading, ime_candidates, ime_selected) = japanese_ime
                    .conversion_view()
                    .map_or((None, &[][..], 0), |(reading, candidates, selected)| {
                        (Some(reading), candidates, selected)
                    });
                layer.push_clip(fx0, fy0, fx1, fy1);

                render_scene(
                    &mut layer,
                    &mut taskbar_surface,
                    &mut wm,
                    mouse_ev_count,
                    key_ev_count,
                    fps,
                    mouse_mode_label,
                    &ui_commands,
                    ui_win_id,
                    &mut warp_engines,
                    &mut html_engines,
                    cached_wallpaper.as_deref(),
                    &mut cached_launcher_layer,
                    &mut cached_ime_menu_layer,
                    taskbar_dirty,
                    tb_add_progress,
                    tb_remove_progress,
                    tb_shift_x,
                    display_state.hud_enabled,
                    &mut bg_cache,
                    bg_valid,
                    launcher_render_visible,
                    &app_list,
                    &app_icon_list,
                    hover_apps_icon,
                    app_search_focused,
                    &app_search_query,
                    app_launcher_scroll.position.max(0) as usize,
                    launcher_anim_phase,
                    launcher_anim_elapsed_ms,
                    launcher_scroll_changed,
                    launcher_only_redraw,
                    taskbar_only,
                    clock_hh,
                    clock_mm,
                    battery_info.valid_percentage(),
                    show_ime_menu,
                    ime_menu_selection(input_mode),
                    ime_reading,
                    ime_candidates,
                    ime_selected,
                );
                layer.pop_clip();

                let hud_redraw_separate =
                    hud_damage_pending && !(fx0 == 0 && fy0 <= hud_y0 && fx1 == w && fy1 >= tb_y);
                if hud_redraw_separate {
                    layer.push_clip(0, hud_y0, w, tb_y);
                    render_scene(
                        &mut layer,
                        &mut taskbar_surface,
                        &mut wm,
                        mouse_ev_count,
                        key_ev_count,
                        fps,
                        mouse_mode_label,
                        &ui_commands,
                        ui_win_id,
                        &mut warp_engines,
                        &mut html_engines,
                        cached_wallpaper.as_deref(),
                        &mut cached_launcher_layer,
                        &mut cached_ime_menu_layer,
                        false,
                        tb_add_progress,
                        tb_remove_progress,
                        tb_shift_x,
                        display_state.hud_enabled,
                        &mut bg_cache,
                        true,
                        launcher_render_visible,
                        &app_list,
                        &app_icon_list,
                        hover_apps_icon,
                        app_search_focused,
                        &app_search_query,
                        app_launcher_scroll.position.max(0) as usize,
                        launcher_anim_phase,
                        launcher_anim_elapsed_ms,
                        launcher_scroll_changed,
                        false,
                        false,
                        clock_hh,
                        clock_mm,
                        battery_info.valid_percentage(),
                        show_ime_menu,
                        ime_menu_selection(input_mode),
                        ime_reading,
                        ime_candidates,
                        ime_selected,
                    );
                    layer.pop_clip();
                }

                prev_window_count = wm.count();
                prev_focused_id = wm.focused_id;
                prev_show_ime_menu = show_ime_menu;
                prev_ime_conversion_visible = ime_conversion_visible;

                if tb_add_progress >= 1.0 {
                    tb_add_progress = -1.0;
                    tb_add_started_ms = None;
                    tb_shift_x = 0.0;
                }
                if tb_remove_progress >= 1.0 {
                    tb_remove_progress = -1.0;
                }

                for y in fy0..fy1 {
                    let s = y * w + fx0;
                    let e = y * w + fx1;
                    cached_scene[s..e].copy_from_slice(&layer.buf_ref()[s..e]);
                }
                if hud_redraw_separate {
                    for y in hud_y0..tb_y {
                        let s = y * w;
                        let e = s + w;
                        cached_scene[s..e].copy_from_slice(&layer.buf_ref()[s..e]);
                    }
                }
                hud_damage_pending = false;
                scene_dirty = false;
                launcher_content_dirty = false;
                wm.clear_pending_damage();

                if launcher_cursor_separate {
                    let buf = layer.buf_mut();
                    for y in cy0..cy1 {
                        let s = y * w + cx0;
                        let e = y * w + cx1;
                        buf[s..e].copy_from_slice(&cached_scene[s..e]);
                    }
                }

                cursor::draw_cursor_into_layer(
                    &mut layer,
                    cursor_x,
                    cursor_y,
                    is_resizing,
                    display_state.pointer_size,
                );
                prev_show_app_launcher = launcher_render_visible;
                let fw = fx1 - fx0;
                let fh = fy1 - fy0;
                let full_area = w * h;
                if taskbar_only {
                    layer.flush_rect(&mut screen, 0, tb_y, w, h);
                    layer.flush_rect(&mut screen, cx0, cy0, cx1, cy1);
                } else if !bg_valid || fw * fh >= full_area * 3 / 4 {
                    layer.flush(&mut screen);
                } else {
                    layer.flush_rect(&mut screen, fx0, fy0, fx1, fy1);
                    if launcher_cursor_separate {
                        layer.flush_rect(&mut screen, cx0, cy0, cx1, cy1);
                    }
                    if hud_redraw_separate {
                        layer.flush_rect(&mut screen, 0, hud_y0, w, tb_y);
                    }
                }

                prev_cursor_x = cursor_x;
                prev_cursor_y = cursor_y;
                prev_is_resizing = is_resizing;
            } else {
                let w = screen.width();
                let h = screen.height();
                let pad = 32i32;
                let cur_w = if is_resizing {
                    cursor::CURSOR_BOX_SIZE_W
                } else {
                    cursor::CURSOR_BOX_W
                };
                let cur_h = if is_resizing {
                    cursor::CURSOR_BOX_SIZE_H
                } else {
                    cursor::CURSOR_BOX_H
                };
                let prev_w = if prev_is_resizing {
                    cursor::CURSOR_BOX_SIZE_W
                } else {
                    cursor::CURSOR_BOX_W
                };
                let prev_h = if prev_is_resizing {
                    cursor::CURSOR_BOX_SIZE_H
                } else {
                    cursor::CURSOR_BOX_H
                };
                let x0 = (prev_cursor_x.min(cursor_x) - pad).max(0) as usize;
                let y0 = (prev_cursor_y.min(cursor_y) - pad).max(0) as usize;
                let x1 = (prev_cursor_x.max(cursor_x) + cur_w.max(prev_w) as i32 + pad)
                    .min(w as i32) as usize;
                let y1 = (prev_cursor_y.max(cursor_y) + cur_h.max(prev_h) as i32 + pad)
                    .min(h as i32) as usize;

                {
                    let buf = layer.buf_mut();
                    for y in y0..y1 {
                        let s = y * w + x0;
                        let e = y * w + x1;
                        buf[s..e].copy_from_slice(&cached_scene[s..e]);
                    }
                }

                cursor::draw_cursor_into_layer(
                    &mut layer,
                    cursor_x,
                    cursor_y,
                    is_resizing,
                    display_state.pointer_size,
                );
                layer.flush_rect(&mut screen, x0, y0, x1, y1);

                prev_cursor_x = cursor_x;
                prev_cursor_y = cursor_y;
                prev_is_resizing = is_resizing;
            }
        }
    }
}

nano_system::nano_entry!(baram_kernel_main);

fn draw_boot_logo(screen: &mut Screen) {
    const LOGO_PNG: &[u8] = include_bytes!("../../../data/logo.png");
    let Ok((header, pixels)) = png_decoder::decode(LOGO_PNG) else {
        screen.clear(Color::BLACK);
        return;
    };
    let img_w = header.width as usize;
    let img_h = header.height as usize;
    let screen_w = screen.width();
    let screen_h = screen.height();
    let mut logo_layer = LayerSystem::new(screen_w, screen_h);
    logo_layer.clear(Color::BLACK);
    let origin_x = screen_w.saturating_sub(img_w) / 2;
    let origin_y = screen_h.saturating_sub(img_h) / 2;
    let draw_w = img_w.min(screen_w);
    let draw_h = img_h.min(screen_h);
    let source_x = img_w.saturating_sub(screen_w) / 2;
    let source_y = img_h.saturating_sub(screen_h) / 2;
    let buffer = logo_layer.buf_mut();
    for y in 0..draw_h {
        let dst_row = (origin_y + y) * screen_w + origin_x;
        let src_row = (source_y + y) * img_w + source_x;
        for x in 0..draw_w {
            let pixel = pixels[src_row + x];
            buffer[dst_row + x] = Color::rgb(pixel[0], pixel[1], pixel[2]).0;
        }
    }
    logo_layer.flush(screen);
}

#[derive(Clone, Copy, PartialEq)]
enum NavigationEffect {
    None,
    SystemChanged,
    AppOpened,
}

struct PendingOsPermission {
    command: alloc::string::String,
    app_hash: alloc::string::String,
    dialog_win_id: WinId,
    source_win_id: Option<WinId>,
}

fn rebuild_filtered_apps(
    entries: &[AppEntry],
    query: &str,
    titles: &mut alloc::vec::Vec<alloc::string::String>,
    names: &mut alloc::vec::Vec<alloc::string::String>,
    icons: &mut alloc::vec::Vec<alloc::string::String>,
) {
    titles.clear();
    names.clear();
    icons.clear();
    let needle = query.trim().to_ascii_lowercase();
    for entry in entries {
        let matches = needle.is_empty()
            || entry.title.to_ascii_lowercase().contains(&needle)
            || entry.name.to_ascii_lowercase().contains(&needle)
            || entry
                .tags
                .iter()
                .any(|tag| tag.to_ascii_lowercase().contains(&needle));
        if matches {
            titles.push(entry.title.clone());
            names.push(entry.name.clone());
            icons.push(entry.icon.clone());
        }
    }
}

fn app_launcher_scroll_max(app_count: usize) -> i32 {
    const COLS: usize = 4;
    const VISIBLE_ROWS: usize = 3;
    const CELL_H: usize = 88;
    let rows = (app_count + COLS - 1) / COLS;
    rows.saturating_sub(VISIBLE_ROWS).saturating_mul(CELL_H) as i32
}

fn open_app(
    name: &str,
    app_entries: &[AppEntry],
    wm: &mut WindowManager,
    warp_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::warp::WarpEngine)>,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    ui_commands: &mut alloc::vec::Vec<baram_graphics::uiscript::Command>,
    ui_win_id: &mut Option<WinId>,
    x: i32,
    y: i32,
    w: usize,
    h: usize,
) -> Option<WinId> {
    let entry = app_entries.iter().find(|entry| entry.name == name)?;
    let win_id = wm.add(&entry.title, x, y, w, h);
    wm.set_icon(win_id, &entry.icon);
    let content_h = h.saturating_sub(baram_windowserver::window::title_bar_h());

    if entry.app_type.starts_with("warp-3") {
        let mut engine = baram_windowserver::html::HtmlEngine::new_warp3(&entry.name);
        engine.update(w as i32, content_h as i32);
        html_engines.push((win_id, engine));
    } else if entry.app_type.starts_with("html") {
        let (html, css) = baram_bsd::app::load_html_document(&entry.name);
        let mut engine = baram_windowserver::html::HtmlEngine::new(&html, &css);
        engine.set_origin(&entry.name);
        engine.update(w as i32, content_h as i32);
        html_engines.push((win_id, engine));
    } else if entry.app_type.starts_with("uiscript") {
        let source = baram_bsd::app::load_app_source(&entry.name);
        *ui_commands = baram_graphics::uiscript::parse(&source);
        *ui_win_id = Some(win_id);
    } else {
        let source = baram_bsd::app::load_app_source(&entry.name);
        let mut engine = baram_windowserver::warp::WarpEngine::new(&source);
        engine.set_origin(&entry.name);
        engine.update(w as i32, content_h as i32);
        warp_engines.push((win_id, engine));
    }
    Some(win_id)
}

fn handle_navigation(
    command: &str,
    app_entries: &[AppEntry],
    wm: &mut WindowManager,
    warp_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::warp::WarpEngine)>,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    ui_commands: &mut alloc::vec::Vec<baram_graphics::uiscript::Command>,
    ui_win_id: &mut Option<WinId>,
    display_state: &mut baram_bsd::uri::DisplayState,
    origin: &str,
    source_win_id: WinId,
    pending_permission: &mut Option<PendingOsPermission>,
    x: i32,
    y: i32,
) -> NavigationEffect {
    if let Some(decision) = command.strip_prefix("security://") {
        let Some(pending) = pending_permission.take() else {
            return NavigationEffect::None;
        };
        if source_win_id != pending.dialog_win_id || origin != "ospermission.w3a" {
            *pending_permission = Some(pending);
            return NavigationEffect::None;
        }
        wm.remove(pending.dialog_win_id);
        html_engines.retain(|(wid, _)| *wid != pending.dialog_win_id);
        wm.set_interaction_blocked(None);
        if decision == "always" {
            baram_bsd::security::allow_always(&pending.app_hash);
        }
        let effect = if decision == "once" || decision == "always" {
            execute_os_setting(&pending.command, wm, html_engines, display_state)
        } else {
            NavigationEffect::None
        };
        if let Some(source_win_id) = pending.source_win_id {
            if let Some((_, engine)) = html_engines
                .iter_mut()
                .find(|(wid, _)| *wid == source_win_id)
            {
                engine.complete_warp3_command();
            }
        }
        return effect;
    }

    if let Some(name) = baram_bsd::app::parse_app_uri(command) {
        if open_app(
            name,
            app_entries,
            wm,
            warp_engines,
            html_engines,
            ui_commands,
            ui_win_id,
            x,
            y,
            400,
            450,
        )
        .is_some()
        {
            return NavigationEffect::AppOpened;
        }
        return NavigationEffect::None;
    }

    if baram_bsd::security::is_settings_write(command) {
        let had_pending = pending_permission.is_some();
        if authorize_os_setting(
            command,
            origin,
            wm,
            html_engines,
            pending_permission,
            Some(source_win_id),
            x,
            y,
        ) {
            return execute_os_setting(command, wm, html_engines, display_state);
        }
        return if !had_pending && pending_permission.is_some() {
            NavigationEffect::AppOpened
        } else {
            NavigationEffect::None
        };
    }

    NavigationEffect::None
}

fn execute_os_setting(
    command: &str,
    wm: &mut WindowManager,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    display_state: &mut baram_bsd::uri::DisplayState,
) -> NavigationEffect {
    if !baram_bsd::uri::execute(command, display_state) {
        return NavigationEffect::None;
    }
    for (_, engine) in html_engines.iter_mut() {
        engine.refresh_config();
    }
    wm.set_all_dirty();
    NavigationEffect::SystemChanged
}

fn authorize_os_setting(
    command: &str,
    origin: &str,
    wm: &mut WindowManager,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    pending_permission: &mut Option<PendingOsPermission>,
    source_win_id: Option<WinId>,
    x: i32,
    y: i32,
) -> bool {
    if !baram_bsd::security::is_settings_write(command) {
        return false;
    }
    let Some(hash) = baram_bsd::security::app_hash(origin) else {
        return false;
    };
    if baram_bsd::security::is_always_allowed(&hash) {
        return true;
    }
    if pending_permission.is_some() {
        return false;
    }

    let dialog_win_id = wm.add("操作体系設定の変更", x, y, 520, 360);
    wm.set_icon(dialog_win_id, "redstar.png");
    let mut dialog = baram_windowserver::html::HtmlEngine::new_warp3("ospermission.w3a");
    dialog.set_warp3_text("app-name", &alloc::format!("アプリ: {origin}"));
    dialog.set_warp3_text("request-path", command);
    dialog.update(
        520,
        330usize.saturating_sub(baram_windowserver::window::title_bar_h()) as i32,
    );
    html_engines.push((dialog_win_id, dialog));
    if let Some(source_win_id) = source_win_id {
        wm.set_interaction_blocked(Some(source_win_id));
        if let Some((_, engine)) = html_engines
            .iter_mut()
            .find(|(wid, _)| *wid == source_win_id)
        {
            engine.hold_warp3_command();
        }
    }
    *pending_permission = Some(PendingOsPermission {
        command: command.into(),
        app_hash: hash,
        dialog_win_id,
        source_win_id,
    });
    false
}

fn cancel_permission_for_closed_window(
    closed_win_id: WinId,
    wm: &mut WindowManager,
    html_engines: &mut alloc::vec::Vec<(WinId, baram_windowserver::html::HtmlEngine)>,
    pending_permission: &mut Option<PendingOsPermission>,
) {
    let should_cancel = pending_permission.as_ref().is_some_and(|pending| {
        pending.dialog_win_id == closed_win_id || pending.source_win_id == Some(closed_win_id)
    });
    if !should_cancel {
        return;
    }
    let Some(pending) = pending_permission.take() else {
        return;
    };
    if pending.dialog_win_id != closed_win_id {
        wm.remove(pending.dialog_win_id);
        html_engines.retain(|(wid, _)| *wid != pending.dialog_win_id);
    }
    wm.set_interaction_blocked(None);
    if let Some(source_win_id) = pending.source_win_id {
        if source_win_id != closed_win_id {
            if let Some((_, engine)) = html_engines
                .iter_mut()
                .find(|(wid, _)| *wid == source_win_id)
            {
                engine.complete_warp3_command();
            }
        }
    }
}

fn wallpaper_for_state(
    state: &baram_bsd::uri::DisplayState,
    screen_w: usize,
    screen_h: usize,
) -> Option<Vec<u32>> {
    if state.wallpaper_mode == baram_bsd::uri::WallpaperMode::Color {
        state
            .wallpaper_color
            .map(|color| make_solid_wallpaper(color, screen_w, screen_h))
    } else {
        WALLPAPERS
            .get(state.wallpaper_index)
            .and_then(|bytes| decode_wallpaper(bytes, screen_w, screen_h))
    }
}
