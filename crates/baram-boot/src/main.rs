#![no_std]
#![no_main]

extern crate alloc;

use alloc::format;
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
use baram_windowserver::soft_keyboard::{Key as SoftKey, KeyboardLanguage, SoftKeyboard};
use baram_windowserver::window::{NativeFileDialogAction, SmoothScroll, WinId, WindowManager};
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
// Generated from AOSP PinyinIME's Apache-2.0 raw dictionary. The source
// spellings are joined so both `zhong guo` and `zhongguo` input resolve alike.
const PINYIN_DICTIONARY: &str = include_str!("pinyin_dictionary.tsv");


include!("ime.rs");
include!("clock.rs");
include!("runtime.rs");
include!("navigation.rs");

nano_system::nano_entry!(baram_kernel_main);
