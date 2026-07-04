#![no_main]
#![no_std]

extern crate alloc;

use uefi::prelude::*;
use uefi::proto::console::gop::{BltOp, BltPixel, BltRect, GraphicsOutput};
use uefi::proto::console::pointer::Pointer;
use uefi::table::boot::{OpenProtocolAttributes, OpenProtocolParams};

const SCREEN_WIDTH: usize = 640;
const SCREEN_HEIGHT: usize = 480;
const CURSOR_SIZE: usize = 16;

#[entry]
fn main(image: Handle, mut st: SystemTable<Boot>) -> Status {
    // UEFIの初期化 (非推奨警告を出さない新しいヘルパー関数)
    uefi::helpers::init(&mut st).unwrap();

    let bt = st.boot_services();

    // 1. グラフィックモードの初期化 (GOP)
    let gop_handle = bt.get_handle_for_protocol::<GraphicsOutput>().expect("GOP handle not found");
    let gop = unsafe {
        bt.open_protocol::<GraphicsOutput>(
            OpenProtocolParams {
                handle: gop_handle,
                agent: image,
                controller: None,
            },
            OpenProtocolAttributes::Exclusive,
        )
    }.expect("Failed to open GOP protocol").unwrap();

    // 640x480の解像度を探して設定
    let mode = gop.modes()
        .find(|m| m.info().resolution() == (SCREEN_WIDTH, SCREEN_HEIGHT))
        .expect("640x480 mode not found");
    gop.set_mode(&mode).expect("Failed to set graphics mode");

    // 2. マウスポインタの初期化
    let pointer_handle = bt.get_handle_for_protocol::<Pointer>().expect("Pointer handle not found");
    let pointer = unsafe {
        bt.open_protocol::<Pointer>(
            OpenProtocolParams {
                handle: pointer_handle,
                agent: image,
                controller: None,
            },
            OpenProtocolAttributes::Exclusive,
        )
    }.expect("Failed to open Pointer protocol").unwrap();
    pointer.reset(false).ok();

    // 3. キーボード入力の取得
    let stdin = st.stdin();
    stdin.reset(false).ok();

    let mut x: usize = SCREEN_WIDTH / 2;
    let mut y: usize = SCREEN_HEIGHT / 2;
    // BltPixelの順序は Blue, Green, Red (BGR)
    let mut bg_color = BltPixel { blue: 139, green: 0, red: 0 }; // 初期色: ダークブルー

    // メインループ
    loop {
        // キーボード入力の取得（何か押されたら背景色を変える）
        if let Ok(Some(_key)) = stdin.read_key() {
            let r = bg_color.red.wrapping_add(20);
            let g = bg_color.green.wrapping_add(20);
            let b = bg_color.blue.wrapping_add(20);
            bg_color = BltPixel { blue: b, green: g, red: r };
        }

        // マウス入力の取得（相対移動を加算）
        if let Ok(Some(state)) = pointer.read_state() {
            let dx = (state.relative_movement_x >> 14) as i64;
            let dy = (state.relative_movement_y >> 14) as i64;

            let new_x = x as i64 + dx;
            let new_y = y as i64 - dy; // Y軸は反転

            if new_x >= 0 && (new_x as usize) < SCREEN_WIDTH - CURSOR_SIZE {
                x = new_x as usize;
            }
            if new_y >= 0 && (new_y as usize) < SCREEN_HEIGHT - CURSOR_SIZE {
                y = new_y as usize;
            }
        }

        // 画面の描画（背景の塗りつぶし）
        gop.blt(BltOp::VideoFill {
            color: bg_color,
            dest: BltRect { x: 0, y: 0, width: SCREEN_WIDTH, height: SCREEN_HEIGHT },
        }).unwrap();

        // マウスカーソルの描画（16x16の白い四角形）
        gop.blt(BltOp::VideoFill {
            color: BltPixel { blue: 255, green: 255, red: 255 },
            dest: BltRect { x: x, y: y, width: CURSOR_SIZE, height: CURSOR_SIZE },
        }).unwrap();

        // CPUの負荷を下げるためのディレイ（約60FPS）
        bt.stall(16_000);
    }
}
