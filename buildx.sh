#!/usr/bin/env bash
# =============================================================================
#  buildx.sh — build + run script for BaramOS
# =============================================================================
#
#  What this script does:
#    1. Verifies required tools are installed (cargo / rustup / espup / espflash / qemu).
#    2. Adds the `xtensa-esp32s3-none-elf` Rust target if missing.
#    3. Builds BaramOS for ESP32-S3 or UEFI.
#    4. Flashes the binary to an ESP32-S3 board via espflash,
#       or runs it in qemu-system-xtensa emulation.
#
#  Usage:
#    ./buildx.sh           # build for ESP32-S3 (default)
#    ./buildx.sh build     # build for ESP32-S3
#    ./buildx.sh build-uefi # build for UEFI
#    ./buildx.sh flash     # build and flash to ESP32-S3
#    ./buildx.sh qemu      # build and run in QEMU
#    ./buildx.sh clean     # cargo clean
#    ./buildx.sh help
#
#  ESP32-S3 specific:
#    - Target: xtensa-esp32s3-none-elf
#    - Flash:  espflash
#    - QEMU:   qemu-system-xtensa -machine virt
#    - RAM:    8MiB (ESP32-S3 default)
#    - CPU:    dc232b (Xtensa LX7 compatible)
#
#  UEFI specific:
#    - Target: x86_64-unknown-uefi
#    - Build:  cargo +nightly build --features uefi
#    - QEMU:   qemu-system-x86_64
#
# =============================================================================

set -euo pipefail

# ---------- script metadata ----------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
source "$SCRIPT_DIR/scripts/nano_targets.sh"

PROJECT_NAME="baramos"
XIAO_IMAGE_NAME="xiao-esp32s3.img"
IMAGE_SIZE_MB=16
RUNTIME_DIR="$SCRIPT_DIR/runtime"
TARGET_DIR="$SCRIPT_DIR/target/xtensa-esp32s3-none-elf/release"
UEFI_TARGET_DIR="$SCRIPT_DIR/target/x86_64-unknown-uefi/release"
ESP32S3_TARGET="xtensa-esp32s3-none-elf"
UEFI_TARGET="x86_64-unknown-uefi"

# ESP32-S3 defaults
ESP32S3_CPU="${ESP32S3_CPU:-dc232b}"
ESP32S3_RAM="${ESP32S3_RAM:-8M}"
ESP32S3_FLASH="${ESP32S3_FLASH:-0x10000}"
ESP32S3_PORT="${ESP32S3_PORT:-/dev/ttyUSB0}"
ESP32S3_BAUD="${ESP32S3_BAUD:-921600}"
QEMU_MACHINE="${QEMU_MACHINE:-virt}"
QEMU_DISPLAY="${QEMU_DISPLAY:-none}"
QEMU_SERIAL="${QEMU_SERIAL:-stdio}"
QEMU_MONITOR="${QEMU_MONITOR:-none}"
XIAO_MODE=1

# ---------- pretty logging ----------
log()  { printf "\033[1;34m[build]\033[0m %s\n" "$*"; }
warn() { printf "\033[1;33m[warn]\033[0m %s\n"  "$*" >&2; }
err()  { printf "\033[1;31m[err ]\033[0m %s\n"  "$*" >&2; }
die()  { err "$*"; exit 1; }

# ---------- platform detection ----------
OS="$(uname -s)"
ARCH="$(uname -m)"
log "Detected OS=$OS ARCH=$ARCH"

# ---------- step 1: Rust toolchain ----------
# ESP32-S3 uses the esp-rs Xtensa toolchain, installed via espup.
ensure_toolchain() {
    if ! rustup target list --installed 2>/dev/null | grep -q "^${ESP32S3_TARGET}$"; then
        log "Installing ESP32-S3 Rust toolchain via espup ..."
        if ! command -v espup >/dev/null 2>&1; then
            log "  Installing espup ..."
            cargo install espup --locked || die "Failed to install espup"
        fi
        espup install --targets esp32s3 || die "Failed to install ESP32-S3 toolchain via espup"
        log "  -> Using esp toolchain"
    fi
}

ensure_uefi_toolchain() {
    if ! rustup target list --installed 2>/dev/null | grep -q "^${UEFI_TARGET}$"; then
        log "Installing UEFI target via rustup ..."
        rustup target add "${UEFI_TARGET}" || die "Failed to add UEFI target"
        log "  -> Using nightly toolchain for UEFI"
    fi
}

# ---------- step 2: check tools ----------
ensure_espflash() {
    if ! command -v espflash >/dev/null 2>&1; then
        log "espflash not found. Installing ..."
        cargo install espflash --locked || warn "Could not install espflash"
    fi
}

ensure_esptool() {
    if command -v esptool.py >/dev/null 2>&1; then
        return 0
    fi
    if command -v esptool >/dev/null 2>&1; then
        return 0
    fi
    warn "esptool not found. Install with:

  pip3 install esptool

Or:
  brew install esptool
"
}

ensure_qemu_xtensa() {
    local qemu
    qemu="$(find_qemu_xtensa)" || return 0
    log "QEMU xtensa found: $qemu"
}

# ---------- step 3: build for ESP32-S3 ----------
build_esp32s3() {
    ensure_toolchain
    log "Building ESP32-S3 BaramOS (${ESP32S3_TARGET}) with esp32s3 features ..."
    CARGO_PROFILE_RELEASE_LTO=fat \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
    cargo +esp build --release --features esp32s3 --target "${ESP32S3_TARGET}" \
        --manifest-path "$SCRIPT_DIR/crates/baram-xiao/Cargo.toml"
    local xiao_target="$TARGET_DIR/xiao"
    test -f "$xiao_target" || test -f "$xiao_target.elf" || die "ESP32-S3 build did not produce $TARGET_DIR/xiao"
    log "  -> $TARGET_DIR/xiao ($(stat -c %s "$xiao_target" 2>/dev/null || stat -f %z "$xiao_target") bytes)"
}

# ---------- step 4: build for UEFI ----------
build_uefi() {
    ensure_uefi_toolchain
    log "Building UEFI BaramOS (${UEFI_TARGET}) with uefi features ..."
    CARGO_PROFILE_RELEASE_LTO=fat \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
    cargo +nightly build --release --features uefi --target "${UEFI_TARGET}" \
        --manifest-path "$SCRIPT_DIR/crates/baram-xiao/Cargo.toml"
    local uefi_target="$UEFI_TARGET_DIR/baramos"
    test -f "$uefi_target" || test -f "$uefi_target.efi" || die "UEFI build did not produce $UEFI_TARGET_DIR/baramos"
    log "  -> $UEFI_TARGET_DIR/baramos ($(stat -c %s "$uefi_target" 2>/dev/null || stat -f %z "$uefi_target") bytes)"
}

# ---------- step 5: flash to ESP32-S3 ----------
flash_esp32s3() {
    local xiao_bin="$TARGET_DIR/xiao"
    [ -f "$xiao_bin" ] || die "Xiao binary not found. Run './buildx.sh build' first."

    ensure_espflash

    log "Flashing to ESP32-S3 on ${ESP32S3_PORT} at ${ESP32S3_BAUD} ..."
    log "  binary: $xiao_bin"
    log "  flash address: ${ESP32S3_FLASH}"
    log "  chip: esp32s3"
    echo

    espflash --chip esp32s3 --port "$ESP32S3_PORT" --baud "$ESP32S3_BAUD" \
        "$xiao_bin"

    log "Flash complete! Reset the ESP32-S3 to boot."
}

# ---------- step 6: QEMU emulation ----------
find_qemu_xtensa() {
    local candidates=(
        "qemu-system-xtensa"
        "/opt/homebrew/bin/qemu-system-xtensa"
        "/usr/local/bin/qemu-system-xtensa"
        "/usr/bin/qemu-system-xtensa"
    )
    for c in "${candidates[@]}"; do
        if command -v "$c" >/dev/null 2>&1 || [ -x "$c" ]; then
            echo "$c"
            return 0
        fi
    done
    return 1
}

find_qemu_uefi() {
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

run_qemu_esp32s3() {
    local qemu
    qemu="$(find_qemu_xtensa)" || die "qemu-system-xtensa not found.

Install QEMU with ESP32 support:
  macOS  : brew install qemu
  Debian : sudo apt install qemu-system-xtensa
  Ubuntu : sudo apt install qemu-system-xtensa
"
    local xiao_bin="$TARGET_DIR/xiao"
    [ -f "$xiao_bin" ] || die "Xiao binary not found. Run './buildx.sh build' first."

    log "Launching QEMU ESP32-S3 emulation ..."
    log "  machine : $QEMU_MACHINE"
    log "  cpu     : $ESP32S3_CPU"
    log "  ram     : $ESP32S3_RAM"
    log "  binary  : $xiao_bin"
    echo

    local extra_args=()
    if [ -n "${QEMU_DATADIR:-}" ]; then
        extra_args+=(-L "$QEMU_DATADIR")
    fi
    # shellcheck disable=SC2206
    if [ -n "${QEMU_EXTRA_ARGS:-}" ]; then
        extra_args+=($QEMU_EXTRA_ARGS)
    fi

    # ESP32-S3 QEMU args:
    #   -machine virt    : Xtensa virt machine (closest to ESP32-S3)
    #   -cpu dc232b      : Xtensa CPU (LX7-compatible for ESP32-S3)
    #   -m 8M            : ESP32-S3 default RAM
    #   -kernel          : ELF binary to load
    #   -serial          : serial console
    #   -display         : display backend
    #   -monitor         : HMP monitor
    #   -device usb-kbd  : USB keyboard (if available)
    #   -device usb-mouse: USB mouse (if available)
    exec "$qemu" \
        "${extra_args[@]}" \
        -machine "$QEMU_MACHINE" \
        -cpu "$ESP32S3_CPU" \
        -m "$ESP32S3_RAM" \
        -kernel "$xiao_bin" \
        -serial "$QEMU_SERIAL" \
        -monitor "$QEMU_MONITOR" \
        -display "$QEMU_DISPLAY" \
        -device usb-kbd \
        -device usb-mouse \
        -device qemu-xhci
}

run_qemu_uefi() {
    local qemu
    qemu="$(find_qemu_uefi)" || die "qemu-system-x86_64 not found.

Install QEMU for UEFI:
  macOS  : brew install qemu
  Debian : sudo apt install qemu-system-x86
  Ubuntu : sudo apt install qemu-system-x86
"
    local uefi_bin="$UEFI_TARGET_DIR/baramos"
    [ -f "$uefi_bin" ] || die "UEFI binary not found. Run './buildx.sh build-uefi' first."

    log "Launching QEMU UEFI emulation ..."
    log "  binary  : $uefi_bin"
    echo

    local extra_args=()
    if [ -n "${QEMU_DATADIR:-}" ]; then
        extra_args+=(-L "$QEMU_DATADIR")
    fi
    # shellcheck disable=SC2206
    if [ -n "${QEMU_EXTRA_ARGS:-}" ]; then
        extra_args+=($QEMU_EXTRA_ARGS)
    fi

    exec "$qemu" \
        "${extra_args[@]}" \
        -machine q35 \
        -m 256M \
        -kernel "$uefi_bin" \
        -serial "$QEMU_SERIAL" \
        -monitor "$QEMU_MONITOR" \
        -display "$QEMU_DISPLAY" \
        -device usb-kbd \
        -device usb-mouse
}

# ---------- subcommands ----------
case "${1:-build}" in
    build)
        build_esp32s3
        ;;
    build-uefi)
        build_uefi
        ;;
    flash)
        build_esp32s3
        ensure_espflash
        flash_esp32s3
        ;;
    qemu)
        build_esp32s3
        ensure_qemu_xtensa
        run_qemu_esp32s3
        ;;
    qemu-uefi)
        build_uefi
        find_qemu_uefi
        run_qemu_uefi
        ;;
    clean)
        log "cargo clean"
        cargo clean
        rm -rf "$RUNTIME_DIR/$XIAO_IMAGE_NAME"
        ;;
    help|-h|--help)
        sed -n '2,/^# =\+/p' "$0" | sed 's/^# \?//'
        ;;
    *)
        err "Unknown command: $1"
        echo "Usage: $0 [build|build-uefi|flash|qemu|qemu-uefi|clean|help]"
        exit 1
        ;;
esac
