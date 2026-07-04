#!/bin/bash
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
qemu-system-aarch64 -M virt -cpu cortex-a53 -m 1024 \
    -bios "$FIRMWARE" \
    -hda disk.img \
    -serial stdio \
    -device usb-kbd \
    -device usb-mouse
