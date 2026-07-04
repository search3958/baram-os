import os
import zipfile

files = {
    "build.sh": """#!/bin/bash
set -e

echo "=== Custom AArch64 UEFI OS Builder ==="

# Rust の確認とインストール
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    source "$HOME/.cargo/env"
fi

# QEMU と依存パッケージのインストール
if [[ "$OSTYPE" == "darwin"* ]]; then
    if ! command -v brew &> /dev/null; then
        echo "Homebrew is required on macOS."
        exit 1
    fi
    if ! command -v qemu-system-aarch64 &> /dev/null; then
        echo "Installing QEMU..."
        brew install qemu
    fi
    
    # OVMF (UEFIファームウェア) のパスを探す
    QEMU_FW_DIR=$(brew --prefix qemu)/share/qemu
    FIRMWARE="${QEMU_FW_DIR}/edk2-aarch64-code.fd"
    if [ ! -f "$FIRMWARE" ]; then
        FIRMWARE="${QEMU_FW_DIR}/AAVMF_CODE.fd"
    fi
else
    # Linux 環境用
    sudo apt-get update
    sudo apt-get install -y qemu-system-arm qemu-efi-aarch64 mtools dosfstools
    FIRMWARE="/usr/share/AAVMF/AAVMF_CODE.fd"
    if [ ! -f "$FIRMWARE" ]; then
        FIRMWARE="/usr/share/qemu-efi-aarch64/QEMU_EFI.fd"
    fi
fi

# Rust の AArch64 UEFI ターゲットを追加
rustup target add aarch64-unknown-uefi

echo "Building OS Kernel..."
cd os
cargo build --target aarch64-unknown-uefi --release

echo "Creating Bootable EFI Disk Image..."
mkdir -p disk/EFI/BOOT
cp target/aarch64-unknown-uefi/release/os.efi disk/EFI/BOOT/BOOTAA64.EFI
dd if=/dev/zero of=disk.img bs=1M count=64
mkfs.fat -F 32 disk.img
mcopy -i disk.img -s disk/* ::

echo "Starting QEMU..."
qemu-system-aarch64 -M virt -cpu cortex-a53 -m 1024 \\
    -bios "$FIRMWARE" \\
    -hda disk.img \\
    -serial stdio \\
    -device usb-kbd \\
    -device usb-mouse
""",
    "os/Cargo.toml": """[package]
name = "os"
version = "0.1.0"
edition = "2021"

[dependencies]
uefi = "0.28.0"
uefi-services = "0.25.0"

[profile.release]
opt-level = "s"
""",
    "os/src/main.rs": """#![no_main]
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
""",
    "README.md": """# Custom AArch64 UEFI OS

This is a minimal OS that boots via UEFI on ARM64 (Raspberry Pi 3/4 and QEMU).
It features mouse pointer drawing and keyboard input handling using Rust and the `uefi` crate.

## How to run
1. `chmod +x build.sh`
2. `./build.sh`
"""
}

with zipfile.ZipFile("os_project.zip", "w", zipfile.ZIP_DEFLATED) as zf:
    for name, content in files.items():
        zf.writestr(name, content)

print("✅ os_project.zip を生成しました。解凍して ./build.sh を実行してください。")