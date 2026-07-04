#!/bin/bash
set -e

echo "=== BARAM-OS Builder ==="

command -v rustup >/dev/null 2>&1 || { echo "Rustがインストールされていません"; exit 1; }
command -v qemu-system-aarch64 >/dev/null 2>&1 || { echo "QEMUがインストールされていません (brew install qemu)"; exit 1; }

rustup target add aarch64-unknown-uefi

echo "ビルド中..."
cargo build --target aarch64-unknown-uefi --release

EFI_FILE="target/aarch64-unknown-uefi/release/baram_os.efi"

# UEFIファームウェアの探索 (Homebrewのパスを自動検索)
EFI_CODE=$(find /opt/homebrew /usr/local -name "edk2-aarch64-code.fd" 2>/dev/null | head -n 1)

if [ -z "$EFI_CODE" ]; then
    echo "エラー: UEFIファームウェア (edk2-aarch64-code.fd) が見つかりません。"
    echo "HomebrewでQEMUをインストールしてください: brew install qemu"
    exit 1
fi

# QEMUの起動 (-kernel オプションで直接EFIバイナリをロード)
echo "QEMUを起動します（マウスとキーボードをキャプチャします。終了はQEMUウィンドウを閉じてください）..."
qemu-system-aarch64 \
    -M virt \
    -cpu cortex-a57 \
    -m 256M \
    -bios "$EFI_CODE" \
    -kernel "$EFI_FILE" \
    -device virtio-gpu-pci \
    -device qemu-xhci \
    -device usb-kbd \
    -device usb-mouse \
    -serial stdio
