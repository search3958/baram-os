#!/bin/bash
set -e

echo "=== ARM64 UEFI OS Builder ==="

# 依存関係のチェック
command -v rustup >/dev/null 2>&1 || { echo "Rustがインストールされていません"; exit 1; }
command -v qemu-system-aarch64 >/dev/null 2>&1 || { echo "QEMUがインストールされていません (brew install qemu)"; exit 1; }

# UEFIターゲットの追加
rustup target add aarch64-unknown-uefi

# ビルドの実行
echo "ビルド中..."
cargo build --target aarch64-unknown-uefi --release

# EFIファイルのコピー
mkdir -p build/esp/EFI/BOOT
cp target/aarch64-unknown-uefi/release/mini_os.efi build/esp/EFI/BOOT/BOOTAA64.EFI

# ディスクイメージの作成 (macOSネイティブのhdiutilを使用)
echo "ディスクイメージを作成中..."
rm -f build/disk.img
hdiutil create -size 64m -fs MS-DOS -volname EFI build/disk.img
hdiutil attach build/disk.img -mountpoint build/mnt
mkdir -p build/mnt/EFI/BOOT
cp build/esp/EFI/BOOT/BOOTAA64.EFI build/mnt/EFI/BOOT/BOOTAA64.EFI
hdiutil detach build/mnt -force

# UEFIファームウェアの探索 (HomebrewのQEMUに同梱されているもの)
EFI_CODE=""
if [ -f "/opt/homebrew/share/qemu/edk2-aarch64-code.fd" ]; then
    EFI_CODE="/opt/homebrew/share/qemu/edk2-aarch64-code.fd"
elif [ -f "/usr/local/share/qemu/edk2-aarch64-code.fd" ]; then
    EFI_CODE="/usr/local/share/qemu/edk2-aarch64-code.fd"
else
    echo "エラー: QEMUのUEFIファームウェア (edk2-aarch64-code.fd) が見つかりません。"
    exit 1
fi

# QEMUの起動
echo "QEMUを起動します（マウスとキーボードをキャプチャします。終了はQEMUウィンドウを閉じてください）..."
qemu-system-aarch64 \
    -M virt \
    -cpu cortex-a57 \
    -m 256M \
    -drive if=pflash,format=raw,file=$EFI_CODE,readonly=on \
    -drive if=virt,format=raw,file=build/disk.img \
    -device virtio-gpu-pci \
    -device qemu-xhci \
    -device usb-kbd \
    -device usb-mouse \
    -serial stdio
