#!/usr/bin/env bash
# =============================================================================
#  build64.sh — one-shot build + run script for MyOS (UEFI x86_64)
# =============================================================================
#
#  Usage:
#    ./build64.sh           # build + run in QEMU
#    ./build64.sh build     # only build (no QEMU launch)
#    ./build64.sh image     # only build + create the FAT image
#    ./build64.sh run       # only run (assumes image + firmware already exist)
#    ./build64.sh clean     # cargo clean
#    ./build64.sh help
#
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

PROJECT_NAME="myos"
EFI_NAME="bootx64.efi"
IMAGE_NAME="osdisk.img"
IMAGE_SIZE_MB=64
FIRMWARE_NAME="edk2-x86_64-code.fd"
RUNTIME_DIR="$SCRIPT_DIR/runtime"
TARGET_DIR="$SCRIPT_DIR/target/x86_64-unknown-uefi/release"

QEMU_RAM="${QEMU_RAM:-0.25G}"
QEMU_DISPLAY="${QEMU_DISPLAY:-default}"
QEMU_SERIAL="${QEMU_SERIAL:-stdio}"
QEMU_MONITOR="${QEMU_MONITOR:-none}"

log()  { printf "\033[1;34m[build]\033[0m %s\n" "$*"; }
warn() { printf "\033[1;33m[warn]\033[0m %s\n"  "$*" >&2; }
err()  { printf "\033[1;31m[err ]\033[0m %s\n"  "$*" >&2; }
die()  { err "$*"; exit 1; }

OS="$(uname -s)"
ARCH="$(uname -m)"
log "Detected OS=$OS ARCH=$ARCH"

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "Required command '$1' not found. $2"
}

if ! command -v cargo >/dev/null 2>&1; then
    die "cargo not found. Install Rust via: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi
if ! command -v rustup >/dev/null 2>&1; then
    die "rustup not found. Install Rust via https://rustup.rs"
fi

if ! rustup target list --installed 2>/dev/null | grep -q '^x86_64-unknown-uefi'; then
    log "Installing Rust target x86_64-unknown-uefi ..."
    rustup target add x86_64-unknown-uefi || die "Failed to add UEFI target"
fi

build_efi() {
    log "Building $EFI_NAME ..."
    cargo build --release --target x86_64-unknown-uefi
    # The Cargo.toml names the binary "bootaa64"; copy to BOOTX64.EFI for x86_64.
    if [ -f "$TARGET_DIR/bootaa64.efi" ] && [ ! -f "$TARGET_DIR/$EFI_NAME" ]; then
        cp "$TARGET_DIR/bootaa64.efi" "$TARGET_DIR/$EFI_NAME"
    fi
    test -f "$TARGET_DIR/$EFI_NAME" || die "Build did not produce $EFI_NAME"
    log "  -> $TARGET_DIR/$EFI_NAME ($(stat -c %s "$TARGET_DIR/$EFI_NAME" 2>/dev/null || stat -f %z "$TARGET_DIR/$EFI_NAME") bytes)"
}

make_fat_image() {
    local out="$RUNTIME_DIR/$IMAGE_NAME"
    local efi="$TARGET_DIR/$EFI_NAME"
    mkdir -p "$RUNTIME_DIR"
    rm -f "$out"

    log "Creating FAT disk image ($IMAGE_SIZE_MB MiB) at $out ..."

    if command -v mformat >/dev/null 2>&1 && command -v mcopy >/dev/null 2>&1; then
        log "  using mtools"
        truncate -s "${IMAGE_SIZE_MB}M" "$out" 2>/dev/null || \
            dd if=/dev/zero of="$out" bs=1m count="$IMAGE_SIZE_MB" 2>/dev/null || \
            dd if=/dev/zero of="$out" bs=1M   count="$IMAGE_SIZE_MB" 2>/dev/null
        mformat -i "$out" -F -T $((IMAGE_SIZE_MB * 1024 * 2)) ::
        mmd   -i "$out" ::/EFI
        mmd   -i "$out" ::/EFI/BOOT
        mcopy -i "$out" "$efi" ::/EFI/BOOT/BOOTX64.EFI
        printf 'fs0:\nEFI\\BOOT\\BOOTX64.EFI\n' | mcopy -i "$out" - ::/startup.nsh
        log "  -> $out"
        return 0
    fi

    if [ "$OS" = "Darwin" ]; then
        log "  using macOS hdiutil"
        local tmp_mount
        tmp_mount="$(mktemp -d /tmp/myos_mount.XXXXXX)"
        hdiutil create -size "${IMAGE_SIZE_MB}m" -fs "MS-DOS FAT32" -volname "EFI" \
            -ov "$out" >/dev/null
        hdiutil attach -nobrowse -mountpoint "$tmp_mount" "$out" >/dev/null
        mkdir -p "$tmp_mount/EFI/BOOT"
        cp "$efi" "$tmp_mount/EFI/BOOT/BOOTX64.EFI"
        printf 'fs0:\nEFI\\BOOT\\BOOTX64.EFI\n' > "$tmp_mount/startup.nsh"
        sync
        hdiutil detach "$tmp_mount" >/dev/null || true
        rmdir "$tmp_mount" 2>/dev/null || true
        log "  -> $out"
        return 0
    fi

    if command -v mkfs.vfat >/dev/null 2>&1 && command -v mtools >/dev/null 2>&1; then
        log "  using mkfs.vfat + mtools"
        truncate -s "${IMAGE_SIZE_MB}M" "$out"
        mkfs.vfat -F 32 -n EFI "$out" >/dev/null
        mmd   -i "$out" ::/EFI
        mmd   -i "$out" ::/EFI/BOOT
        mcopy -i "$out" "$efi" ::/EFI/BOOT/BOOTX64.EFI
        printf 'fs0:\nEFI\\BOOT\\BOOTX64.EFI\n' | mcopy -i "$out" - ::/startup.nsh
        log "  -> $out"
        return 0
    fi

    die "No FAT image creation tool found. Install 'mtools' (brew install mtools / apt install mtools)."
}

ensure_firmware() {
    local fw="$RUNTIME_DIR/$FIRMWARE_NAME"
    if [ -f "$fw" ]; then
        log "Firmware present: $fw"
        return 0
    fi

    log "Looking for x86_64 UEFI firmware (OVMF) ..."

    local fw_path
    fw_path=$(find /opt/homebrew /usr/local -name "edk2-x86_64-code.fd" 2>/dev/null | head -n 1)
    if [ -n "$fw_path" ]; then
        log "  found: $fw_path"
        cp "$fw_path" "$fw"
        log "  -> $fw"
        return 0
    fi

    die "UEFI firmware (edk2-x86_64-code.fd) not found. Install QEMU via: brew install qemu"
}

find_qemu() {
    local candidates=(
        "qemu-system-x86_64"
        "/opt/homebrew/bin/qemu-system-x86_64"
        "/usr/local/bin/qemu-system-x86_64"
        "/usr/bin/qemu-system-x86_64"
    )
    for c in "${candidates[@]}"; do
        if command -v "$c" >/dev/null 2>&1 || [ -x "$c" ]; then
            echo "$c"
            return 0
        fi
    done
    return 1
}

run_qemu() {
    local qemu
    qemu="$(find_qemu)" || die "qemu-system-x86_64 not found.

Install QEMU:
  macOS  : brew install qemu
  Debian : sudo apt install qemu-system-x86
  Ubuntu : sudo apt install qemu-system-x86
  Arch   : sudo pacman -S qemu-full
"
    local fw="$RUNTIME_DIR/$FIRMWARE_NAME"
    local img="$RUNTIME_DIR/$IMAGE_NAME"
    [ -f "$img" ] || die "Disk image missing. Run './build64.sh' first."
    [ -f "$fw" ]  || die "Firmware missing. Run './build64.sh' first."

    log "Launching QEMU ..."
    log "  cpu     : qemu64"
    log "  ram     : $QEMU_RAM"
    log "  firmware: $fw (OVMF pflash)"
    log "  disk    : $img"
    echo

    local extra_args=()
    if [ -n "${QEMU_DATADIR:-}" ]; then
        extra_args+=(-L "$QEMU_DATADIR")
    fi
    if [ -n "${QEMU_EXTRA_ARGS:-}" ]; then
        extra_args+=($QEMU_EXTRA_ARGS)
    fi

    exec "$qemu" \
        "${extra_args[@]}" \
        -cpu qemu64 \
        -m "$QEMU_RAM" \
        -drive "if=pflash,format=raw,readonly=on,file=$fw" \
        -drive "if=none,file=$img,format=raw,id=hd0" \
        -device "virtio-blk-pci,drive=hd0" \
        -device "virtio-vga" \
        -device "qemu-xhci" \
        -device "usb-tablet" \
        -device "usb-mouse" \
        -device "usb-kbd" \
        -display "$QEMU_DISPLAY" \
        -serial "$QEMU_SERIAL" \
        -monitor "$QEMU_MONITOR"
}

case "${1:-build-run}" in
    build)
        build_efi
        ;;
    image)
        build_efi
        make_fat_image
        ;;
    firmware)
        ensure_firmware
        ;;
    run)
        run_qemu
        ;;
    build-run|"")
        build_efi
        make_fat_image
        ensure_firmware
        run_qemu
        ;;
    clean)
        log "cargo clean"
        cargo clean
        rm -rf "$RUNTIME_DIR/$IMAGE_NAME" "$RUNTIME_DIR/$FIRMWARE_NAME"
        ;;
    help|-h|--help)
        sed -n '2,/^# =\+/p' "$0" | sed 's/^# \?//'
        ;;
    *)
        err "Unknown command: $1"
        echo "Usage: $0 [build|image|run|firmware|clean|help]"
        exit 1
        ;;
esac
