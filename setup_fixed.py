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
    # mtools (mcopy) のインストールを追加
    if ! command -v mcopy &> /dev/null; then
        echo "Installing mtools..."
        brew install mtools
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

# macOSとLinuxでFAT32フォーマットコマンドを分岐
if [[ "$OSTYPE" == "darwin"* ]]; then
    newfs_msdos -F 32 disk.img
else
    mkfs.fat -F 32 disk.img
fi
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
# uefi-servicesは廃止されたため、uefiクレートのhelpers機能を有効化
uefi = { version = "0.28.0", features = ["alloc", "global_allocator", "logger", "panic_handler"] }

[profile.release]
opt-level = "s"
""",
    "os/src/main.rs": """#![no_main]
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
""",
    "README.md": """# Custom AArch64 UEFI OS

This is a minimal OS that boots via UEFI on ARM64 (Raspberry Pi 3/4 and QEMU).
It features mouse pointer drawing and keyboard input handling using Rust and the `uefi` crate.

## How to run
1. `chmod +x build.sh`
2. `./build.sh`
"""
}

with zipfile.ZipFile("os_project_fixed.zip", "w", zipfile.ZIP_DEFLATED) as zf:
    for name, content in files.items():
        zf.writestr(name, content)

print("✅ os_project_fixed.zip を生成しました。解凍して ./build.sh を実行してください。")