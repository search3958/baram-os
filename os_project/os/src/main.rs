#![no_main]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, BltOp, BltRegion, BltPixel};
use uefi::proto::console::pointer::Pointer;
use uefi::proto::console::text::TextInput;
use uefi::table::boot::ScopedProtocol;
use uefi::Status;

#[entry]
fn efi_main(_image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    // UEFIサービスの初期化
    uefi_services::init(&mut system_table).unwrap();
    let bs = system_table.boot_services();

    // 1. グラフィックモード (GOP) の取得と設定
    let gop_handle = bs.locate_protocol::<GraphicsOutput>().unwrap().expect("GOP not found");
    let mut gop = unsafe { ScopedProtocol::<GraphicsOutput>::open_protocol(gop_handle, bs).unwrap() };

    // 800x600以上の解像度を探す
    let modes: Vec<_> = gop.modes().collect();
    let mode = modes.iter().find(|m| m.info().resolution.0 >= 800).unwrap_or(&modes[0]);
    gop.set_mode(*mode).unwrap();

    // 2. マウス (Simple Pointer Protocol) の取得
    let ptr_handle = bs.locate_protocol::<Pointer>().unwrap().expect("Pointer not found");
    let mut ptr = unsafe { ScopedProtocol::<Pointer>::open_protocol(ptr_handle, bs).unwrap() };

    // 3. キーボード (Simple Text Input Protocol) の取得
    let tin_handle = bs.locate_protocol::<TextInput>().unwrap().expect("TextInput not found");
    let mut tin = unsafe { ScopedProtocol::<TextInput>::open_protocol(tin_handle, bs).unwrap() };

    let mut cursor_x: usize = 400;
    let mut cursor_y: usize = 300;
    
    // 16x16 の赤いマウスポインター用バッファ
    let pixel = BltPixel { red: 255, green: 0, blue: 0 };
    let mut cursor_buffer = [pixel; 16 * 16];

    // メインループ
    loop {
        // マウス入力の取得
        let mut state = uefi::proto::console::pointer::State::default();
        if ptr.get_state(&mut state).is_ok() {
            let nx = cursor_x as i32 + state.relative_movement[0];
            let ny = cursor_y as i32 + state.relative_movement[1];
            if nx >= 0 && nx < 1920 { cursor_x = nx as usize; }
            if ny >= 0 && ny < 1080 { cursor_y = ny as usize; }
        }

        // キーボード入力の取得
        let mut key = uefi::proto::console::text::Key::default();
        if tin.read_key_stroke(&mut key).is_ok() {
            // キー入力に応じた処理をここに追加可能
        }

        // マウスポインターの描画 (Block Transfer)
        let op = BltOp::BufferToVideo {
            buffer: &cursor_buffer,
            src: BltRegion::Full,
            dest: (cursor_x, cursor_y),
        };
        let _ = gop.blt(op);

        bs.stall(10000); // 10ms Wait
    }
}
