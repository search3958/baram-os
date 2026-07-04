#![no_main]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, BltOp, BltRegion, BltPixel};
use uefi::proto::console::pointer::{Pointer, PointerState};
use uefi::proto::console::text::Input; // TextInputからInputに変更
use uefi::table::boot::ScopedProtocol;
use uefi::Status;

#[entry]
fn efi_main(_image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    // uefi_helpers::initを使用 (uefi_services::initの後継)
    uefi::helpers::init(&mut system_table).unwrap();
    let bs = system_table.boot_services();

    // 1. グラフィックモード (GOP) の取得と設定
    // locate_protocolの代わりにget_handle_for_protocolとopen_protocol_exclusiveを使用
    let gop_handle = bs.get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = bs.open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap();

    // 800x600以上の解像度を探す
    let mut target_mode = None;
    for m in gop.modes(bs) {
        if m.info().resolution().0 >= 800 {
            target_mode = Some(m);
            break;
        }
    }
    let mode = target_mode.unwrap_or_else(|| gop.modes(bs).next().unwrap());
    gop.set_mode(&mode).unwrap();

    // 2. マウス (Simple Pointer Protocol) の取得
    let ptr_handle = bs.get_handle_for_protocol::<Pointer>().unwrap();
    let mut ptr = bs.open_protocol_exclusive::<Pointer>(ptr_handle).unwrap();

    // 3. キーボード (Simple Text Input Protocol) の取得
    let tin_handle = bs.get_handle_for_protocol::<Input>().unwrap();
    let mut tin = bs.open_protocol_exclusive::<Input>(tin_handle).unwrap();

    let mut cursor_x: usize = 400;
    let mut cursor_y: usize = 300;
    
    // 16x16 の赤いマウスポインター用バッファ
    let pixel = BltPixel::new(255, 0, 0); // struct literalではなくnew()を使用
    let cursor_buffer: Vec<BltPixel> = alloc::vec![pixel; 16 * 16];

    // メインループ
    loop {
        // マウス入力の取得 (get_stateからread_stateに変更)
        if let Ok(Some(state)) = ptr.read_state() {
            let nx = cursor_x as i32 + state.relative_movement[0];
            let ny = cursor_y as i32 + state.relative_movement[1];
            // 画面端を超えないように調整 (1920x1080と仮定)
            if nx >= 0 && nx < 1920 { cursor_x = nx as usize; }
            if ny >= 0 && ny < 1080 { cursor_y = ny as usize; }
        }

        // キーボード入力の取得
        if let Ok(Some(_key)) = tin.read_key() {
            // キー入力に応じた処理をここに追加可能
        }

        // マウスポインターの描画 (Block Transfer)
        // dimsフィールドの追加
        let op = BltOp::BufferToVideo {
            buffer: &cursor_buffer,
            src: BltRegion::Full,
            dest: (cursor_x, cursor_y),
            dims: (16, 16),
        };
        let _ = gop.blt(op);

        bs.stall(10000); // 10ms Wait
    }
}
