#![no_main]
#![no_std]

extern crate alloc;

use uefi::prelude::*;
use uefi::proto::console::gop::{BltOp, BltPixel, Coordinate, GraphicsOutput, Rectangle};
use uefi::proto::console::pointer::Pointer;
use uefi::proto::console::text::input::Input;

const SCREEN_WIDTH: usize = 640;
const SCREEN_HEIGHT: usize = 480;
const CURSOR_SIZE: usize = 16;

#[entry]
fn main(_image: Handle, st: SystemTable<Boot>) -> Status {
    uefi_services::init(&st).unwrap();

    // 1. グラフィックモードの初期化 (GOP)
    let gop = st.boot_services().locate_protocol::<GraphicsOutput>().expect("GOP not found");
    let gop = unsafe { &mut *gop.get() };
    
    let mode = gop.modes()
        .find(|m| m.info().resolution() == (SCREEN_WIDTH, SCREEN_HEIGHT))
        .expect("640x480 mode not found");
    gop.set_mode(&mode).expect("Failed to set graphics mode");

    // 2. マウスポインタの初期化
    let pointer = st.boot_services().locate_protocol::<Pointer>().expect("Pointer not found");
    let pointer = unsafe { &mut *pointer.get() };
    pointer.reset(false).ok();

    // 3. キーボード入力の初期化
    let stdin = st.stdin();
    stdin.reset(false).ok();

    let mut x: usize = SCREEN_WIDTH / 2;
    let mut y: usize = SCREEN_HEIGHT / 2;
    let mut bg_color = BltPixel { red: 0, green: 0, blue: 139, reserved: 0 }; // 初期色: ダークブルー

    // メインループ
    loop {
        // キーボード入力の取得（何か押されたら背景色を変える）
        if let Ok(Some(_key)) = stdin.read_key() {
            let r = bg_color.red.wrapping_add(20);
            let g = bg_color.green.wrapping_add(20);
            let b = bg_color.blue.wrapping_add(20);
            bg_color = BltPixel { red: r, green: g, blue: b, reserved: 0 };
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
            dst: Rectangle::new(Coordinate::new(0, 0), Coordinate::new(SCREEN_WIDTH, SCREEN_HEIGHT)),
        }).unwrap();

        // マウスカーソルの描画（16x16の白い四角形）
        gop.blt(BltOp::VideoFill {
            color: BltPixel { red: 255, green: 255, blue: 255, reserved: 0 },
            dst: Rectangle::new(Coordinate::new(x, y), Coordinate::new(x + CURSOR_SIZE, y + CURSOR_SIZE)),
        }).unwrap();

        // CPUの負荷を下げるためのディレイ（約60FPS）
        st.boot_services().stall(16_000);
    }
}
