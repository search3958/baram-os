use crate::constants::{HALFWIDTH_PROLONGED_SOUND_MARK, PROLONGED_SOUND_MARK};

/// Returns true if char is the prolonged sound mark (fullwidth 'ー' or halfwidth 'ｰ').
pub fn is_prolonged_sound(char: char) -> bool {
    let code = char as u32;
    code == PROLONGED_SOUND_MARK || code == HALFWIDTH_PROLONGED_SOUND_MARK
}

#[test]
fn is_prolonged_sound_test() {
    assert!(is_prolonged_sound('ー'));
    assert!(is_prolonged_sound('ｰ'));
    assert!(!is_prolonged_sound('-'));
    assert!(!is_prolonged_sound('f'));
    assert!(!is_prolonged_sound('ふ'));
}
