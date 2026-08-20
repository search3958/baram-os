#[derive(Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Latin,
    Hiragana,
    Korean(KoreanLayout),
    Pinyin,
}

struct PinyinIme {
    raw: alloc::string::String,
    visible_chars: usize,
    conversion: Option<PinyinConversion>,
    predictions: alloc::vec::Vec<alloc::string::String>,
}

struct PinyinConversion {
    pinyin: alloc::string::String,
    candidates: alloc::vec::Vec<alloc::string::String>,
    selected: usize,
}

impl PinyinIme {
    fn new() -> Self {
        Self {
            raw: alloc::string::String::new(),
            visible_chars: 0,
            conversion: None,
            predictions: alloc::vec::Vec::new(),
        }
    }
    fn reset(&mut self) {
        self.raw.clear();
        self.visible_chars = 0;
        self.conversion = None;
        self.predictions.clear();
    }
    fn edit(&mut self, key: u8) -> (alloc::string::String, usize) {
        if key != 0x08 && key != 0x7f && self.conversion.is_some() {
            self.reset();
        }
        if key == 0x08 || key == 0x7f {
            if self.raw.pop().is_some() {
                let replace = self.visible_chars;
                self.visible_chars = self.raw.chars().count();
                return (self.raw.clone(), replace);
            }
            return (alloc::string::String::new(), 1);
        }
        let ch = key as char;
        if ch.is_ascii_alphabetic() || ch == '\'' {
            self.raw.push(ch.to_ascii_lowercase());
            self.predictions = pinyin_candidates(&self.raw.replace('\'', "")).unwrap_or_default();
            let replace = self.visible_chars;
            self.visible_chars = self.raw.chars().count();
            return (self.raw.clone(), replace);
        }
        let mut text = self.raw.clone();
        text.push(ch);
        let replace = self.visible_chars;
        self.reset();
        (text, replace)
    }
    fn convert(&mut self) -> Option<(alloc::string::String, usize)> {
        if let Some(conversion) = self.conversion.as_mut() {
            conversion.selected = (conversion.selected + 1) % conversion.candidates.len();
            let text = conversion.candidates[conversion.selected].clone();
            let replace = self.visible_chars;
            self.visible_chars = text.chars().count();
            return Some((text, replace));
        }
        let key = self.raw.replace('\'', "");
        let candidates = pinyin_candidates(&key)?;
        let text = candidates[0].clone();
        let replace = self.visible_chars;
        self.visible_chars = text.chars().count();
        self.conversion = Some(PinyinConversion {
            pinyin: self.raw.clone(),
            candidates,
            selected: 0,
        });
        Some((text, replace))
    }
    fn edit_for_key(&mut self, key: u8) -> (alloc::string::String, usize) {
        if key == b' ' {
            return self.convert().unwrap_or_else(|| self.edit(key));
        }
        if key == b'\n' || key == b'\r' {
            self.reset();
            return (alloc::string::String::new(), 0);
        }
        self.edit(key)
    }
    fn conversion_view(&self) -> Option<(&str, &[alloc::string::String], usize)> {
        self.conversion.as_ref().map(|conversion| {
            (
                conversion.pinyin.as_str(),
                conversion.candidates.as_slice(),
                conversion.selected,
            )
        })
    }

    fn prediction_view(&self) -> Option<(&str, &[alloc::string::String])> {
        (!self.raw.is_empty() && !self.predictions.is_empty())
            .then_some((self.raw.as_str(), self.predictions.as_slice()))
    }

    fn commit_candidate(&mut self, index: usize) -> Option<(alloc::string::String, usize)> {
        let text = self
            .conversion
            .as_ref()
            .and_then(|conversion| conversion.candidates.get(index))
            .or_else(|| self.predictions.get(index))?
            .clone();
        let replace = self.visible_chars;
        self.reset();
        Some((text, replace))
    }
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
    predictions: alloc::vec::Vec<alloc::string::String>,
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
            predictions: alloc::vec::Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.romaji.clear();
        self.visible_chars = 0;
        self.conversion = None;
        self.predictions.clear();
    }

    fn edit(&mut self, key: u8) -> (alloc::string::String, usize) {
        // Starting another romaji run commits the selected candidate already
        // visible in the target field.
        if key != 0x08 && key != 0x7f && self.conversion.is_some() {
            self.conversion = None;
            self.romaji.clear();
            self.visible_chars = 0;
            self.predictions.clear();
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
            self.predictions = mozc_candidates(&text).unwrap_or_default();
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
        self.conversion.as_ref().map(|conversion| {
            (
                conversion.kana.as_str(),
                conversion.candidates.as_slice(),
                conversion.selected,
            )
        })
    }

    fn prediction_view(&self) -> Option<(&str, &[alloc::string::String])> {
        let kana = self.romaji.as_str().to_hiragana();
        (!kana.is_empty() && !self.predictions.is_empty())
            .then_some((self.romaji.as_str(), self.predictions.as_slice()))
    }

    fn commit_candidate(&mut self, index: usize) -> Option<(alloc::string::String, usize)> {
        let text = self
            .conversion
            .as_ref()
            .and_then(|conversion| conversion.candidates.get(index))
            .or_else(|| self.predictions.get(index))?
            .clone();
        let replace = self.visible_chars;
        self.reset();
        Some((text, replace))
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

fn pinyin_candidates(pinyin: &str) -> Option<alloc::vec::Vec<alloc::string::String>> {
    for line in PINYIN_DICTIONARY.lines().skip(1) {
        let mut fields = line.split('\t');
        if fields.next()? == pinyin {
            return Some(fields.map(alloc::string::String::from).collect());
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
        Self {
            raw: alloc::string::String::new(),
            visible_chars: 0,
        }
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
        'q' => 'ㅂ',
        'Q' => 'ㅃ',
        'w' => 'ㅈ',
        'W' => 'ㅉ',
        'e' => 'ㄷ',
        'E' => 'ㄸ',
        'r' => 'ㄱ',
        'R' => 'ㄲ',
        't' => 'ㅅ',
        'T' => 'ㅆ',
        'y' => 'ㅛ',
        'u' => 'ㅕ',
        'i' => 'ㅑ',
        'o' => 'ㅐ',
        'O' => 'ㅒ',
        'p' => 'ㅔ',
        'P' => 'ㅖ',
        'a' => 'ㅁ',
        's' => 'ㄴ',
        'd' => 'ㅇ',
        'f' => 'ㄹ',
        'g' => 'ㅎ',
        'h' => 'ㅗ',
        'j' => 'ㅓ',
        'k' => 'ㅏ',
        'l' => 'ㅣ',
        'z' => 'ㅋ',
        'x' => 'ㅌ',
        'c' => 'ㅊ',
        'v' => 'ㅍ',
        'b' => 'ㅠ',
        'n' => 'ㅜ',
        'm' => 'ㅡ',
        _ => return None,
    })
}

/// 조선 두벌식, exactly following the layout supplied in the request.
fn chosun_dubeolsik_jamo(key: char) -> Option<char> {
    Some(match key {
        'q' => 'ㅂ',
        'Q' => 'ㅃ',
        'w' | 'W' => 'ㅁ',
        'e' => 'ㄷ',
        'E' => 'ㄸ',
        'r' | 'R' => 'ㄹ',
        't' | 'T' => 'ㄱ',
        'y' | 'Y' => 'ㅕ',
        'u' | 'U' => 'ㅜ',
        'i' | 'I' => 'ㅓ',
        'o' => 'ㅐ',
        'O' => 'ㅒ',
        'p' => 'ㅔ',
        'P' => 'ㅖ',
        'a' => 'ㅈ',
        'A' => 'ㅉ',
        's' => 'ㄱ',
        'S' => 'ㄲ',
        'd' | 'D' => 'ㅇ',
        'f' | 'F' => 'ㄴ',
        'g' => 'ㅅ',
        'G' => 'ㅆ',
        'h' | 'H' => 'ㅗ',
        'j' | 'J' => 'ㅏ',
        'k' | 'K' => 'ㅣ',
        'l' | 'L' => 'ㅡ',
        'z' | 'Z' => 'ㅋ',
        'x' | 'X' => 'ㅌ',
        'c' | 'C' => 'ㅊ',
        'v' | 'V' => 'ㅍ',
        'b' | 'B' => 'ㅠ',
        'n' | 'N' => 'ㅛ',
        'm' | 'M' => 'ㅑ',
        _ => return None,
    })
}

fn hancom_roman_jamo(raw: &str) -> alloc::vec::Vec<char> {
    let lower = raw.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut jamo = alloc::vec::Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let tail = &lower[i..];
        let (len, ch) = if tail.starts_with("yei") || tail.starts_with("iei") {
            (3, 'ㅖ')
        } else if tail.starts_with("yai") || tail.starts_with("iai") {
            (3, 'ㅒ')
        } else if tail.starts_with("ya") || tail.starts_with("ia") {
            (2, 'ㅑ')
        } else if tail.starts_with("yu") || tail.starts_with("iu") {
            (2, 'ㅠ')
        } else if tail.starts_with("yo") || tail.starts_with("io") {
            (2, 'ㅛ')
        } else if tail.starts_with("ye") || tail.starts_with("ie") {
            (2, 'ㅕ')
        } else if tail.starts_with("ai") {
            (2, 'ㅐ')
        } else if tail.starts_with("ei") {
            (2, 'ㅔ')
        } else if tail.starts_with("oi") {
            (2, 'ㅚ')
        } else if tail.starts_with("ui") {
            (2, 'ㅟ')
        } else if tail.starts_with("wi") {
            (2, 'ㅢ')
        } else {
            let original = raw.as_bytes()[i] as char;
            let ch = match original {
                // Shift produces the five modern tense consonants.
                'G' => 'ㄲ',
                'D' => 'ㄸ',
                'B' => 'ㅃ',
                'S' => 'ㅆ',
                'J' => 'ㅉ',
                'a' | 'A' => 'ㅏ',
                'e' | 'E' => 'ㅓ',
                'i' | 'I' | 'y' | 'Y' => 'ㅣ',
                'o' | 'O' => 'ㅗ',
                'u' | 'U' => 'ㅜ',
                'w' | 'W' => 'ㅡ',
                'g' => 'ㄱ',
                'n' | 'N' => 'ㄴ',
                'd' => 'ㄷ',
                'r' | 'l' | 'R' | 'L' => 'ㄹ',
                'm' | 'M' => 'ㅁ',
                'b' => 'ㅂ',
                's' => 'ㅅ',
                'j' => 'ㅈ',
                'h' | 'H' => 'ㅎ',
                'f' | 'F' | 'p' | 'P' => 'ㅍ',
                't' | 'T' => 'ㅌ',
                'k' | 'K' => 'ㅋ',
                'c' | 'C' => 'ㅊ',
                'x' | 'X' => 'ㅇ',
                // V is intentionally unmapped in this layout.
                'v' | 'V' => {
                    i += 1;
                    continue;
                }
                _ => {
                    i += 1;
                    continue;
                }
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
        InputMode::Pinyin => 5,
    }
}

fn input_mode_for_menu_selection(selection: usize) -> InputMode {
    match selection {
        1 => InputMode::Hiragana,
        2 => InputMode::Korean(KoreanLayout::Dubeolsik),
        3 => InputMode::Korean(KoreanLayout::HancomRoman),
        4 => InputMode::Korean(KoreanLayout::ChosunDubeolsik),
        5 => InputMode::Pinyin,
        _ => InputMode::Latin,
    }
}

fn keyboard_language(mode: InputMode) -> KeyboardLanguage {
    match mode {
        InputMode::Latin => KeyboardLanguage::Latin,
        InputMode::Hiragana => KeyboardLanguage::Japanese,
        InputMode::Korean(KoreanLayout::Dubeolsik) => KeyboardLanguage::KoreanDubeolsik,
        InputMode::Korean(KoreanLayout::HancomRoman) => KeyboardLanguage::KoreanHancomRoman,
        InputMode::Korean(KoreanLayout::ChosunDubeolsik) => KeyboardLanguage::KoreanChosunDubeolsik,
        InputMode::Pinyin => KeyboardLanguage::ChinesePinyin,
    }
}

fn ime_edit_for_key(
    mode: InputMode,
    japanese: &mut JapaneseIme,
    hangul: &mut HangulIme,
    pinyin: &mut PinyinIme,
    key: u8,
) -> Option<(alloc::string::String, usize)> {
    match mode {
        InputMode::Latin => None,
        InputMode::Hiragana => Some(japanese.edit_for_key(key)),
        InputMode::Korean(layout) => Some(hangul.edit_for_key(key, layout)),
        InputMode::Pinyin => Some(pinyin.edit_for_key(key)),
    }
}

fn initial_index(ch: char) -> Option<u32> {
    "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ"
        .chars()
        .position(|c| c == ch)
        .map(|i| i as u32)
}
fn vowel_index(ch: char) -> Option<u32> {
    "ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ"
        .chars()
        .position(|c| c == ch)
        .map(|i| i as u32)
}
fn final_index(ch: char) -> Option<u32> {
    "\0ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ"
        .chars()
        .position(|c| c == ch)
        .map(|i| i as u32)
}
fn combined_vowel(a: char, b: char) -> Option<char> {
    match (a, b) {
        ('ㅗ', 'ㅏ') => Some('ㅘ'),
        ('ㅗ', 'ㅐ') => Some('ㅙ'),
        ('ㅗ', 'ㅣ') => Some('ㅚ'),
        ('ㅜ', 'ㅓ') => Some('ㅝ'),
        ('ㅜ', 'ㅔ') => Some('ㅞ'),
        ('ㅜ', 'ㅣ') => Some('ㅟ'),
        ('ㅡ', 'ㅣ') => Some('ㅢ'),
        _ => None,
    }
}
fn combined_final(a: char, b: char) -> Option<char> {
    match (a, b) {
        ('ㄱ', 'ㅅ') => Some('ㄳ'),
        ('ㄴ', 'ㅈ') => Some('ㄵ'),
        ('ㄴ', 'ㅎ') => Some('ㄶ'),
        ('ㄹ', 'ㄱ') => Some('ㄺ'),
        ('ㄹ', 'ㅁ') => Some('ㄻ'),
        ('ㄹ', 'ㅂ') => Some('ㄼ'),
        ('ㄹ', 'ㅅ') => Some('ㄽ'),
        ('ㄹ', 'ㅌ') => Some('ㄾ'),
        ('ㄹ', 'ㅍ') => Some('ㄿ'),
        ('ㄹ', 'ㅎ') => Some('ㅀ'),
        ('ㅂ', 'ㅅ') => Some('ㅄ'),
        _ => None,
    }
}
fn compose_hangul(jamo: &[char]) -> alloc::string::String {
    let mut out = alloc::string::String::new();
    let mut i = 0;
    while i < jamo.len() {
        let Some(l) = initial_index(jamo[i]) else {
            out.push(jamo[i]);
            i += 1;
            continue;
        };
        if i + 1 >= jamo.len() {
            out.push(jamo[i]);
            break;
        }
        let Some(mut v) = vowel_index(jamo[i + 1]) else {
            out.push(jamo[i]);
            i += 1;
            continue;
        };
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
                if i + 1 < jamo.len()
                    && initial_index(jamo[i + 1]).is_some()
                    && !(i + 2 < jamo.len() && vowel_index(jamo[i + 2]).is_some())
                {
                    if let Some(cluster) = combined_final(c, jamo[i + 1]) {
                        t = final_index(cluster).unwrap_or(0);
                        i += 2;
                    }
                }
                if t == 0 {
                    if let Some(final_jamo) = final_index(c) {
                        t = final_jamo;
                        i += 1;
                    }
                }
            }
        }
        if let Some(syllable) = char::from_u32(0xac00 + (l * 21 + v) * 28 + t) {
            out.push(syllable);
        }
    }
    out
}


