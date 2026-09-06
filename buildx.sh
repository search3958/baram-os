#!/usr/bin/env bash
# =============================================================================
#  buildx.sh — build + run script for BaramOS Xiao on ESP32-S3
# =============================================================================
#
#  What this script does:
#    1. Verifies required tools are installed (cargo / rustup / esptool / qemu).
#    2. Adds the `xtensa-esp32s3-none-elf` Rust target if missing.
#    3. Builds the Xiao kiosk as an ELF binary for ESP32-S3.
#    4. Flashes the binary to an ESP32-S3 board via esptool,
#       or runs it in qemu-system-xtensa emulation.
#
#  Usage:
#    ./buildx.sh           # build + flash to ESP32-S3
#    ./buildx.sh build     # only build (no flash)
#    ./buildx.sh qemu      # build + run in QEMU emulation
#    ./buildx.sh run       # only flash (assumes build already done)
#    ./buildx.sh clean     # cargo clean
#    ./buildx.sh help
#
#  ESP32-S3 specific:
#    - Target: xtensa-esp32s3-none-elf
#    - Flash:  esptool.py --chip esp32s3
#    - QEMU:   qemu-system-xtensa -machine virt
#    - RAM:    8MiB (ESP32-S3 default)
#    - CPU:    dc232b (Xtensa LX7 compatible)
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
ESP32S3_TARGET="xtensa-esp32s3-none-elf"

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

# ---------- step 2: check tools ----------
ensure_esptool() {
    if ! command -v esptool.py >/dev/null 2>&1 && ! command -v esptool >/dev/null 2>&1; then
        die "esptool not found. Install with:

  pip3 install esptool

Or:
  brew install esptool
"
    fi
    if ! command -v espflash >/dev/null 2>&1; then
        log "espflash not found. Installing ..."
        cargo install espflash --locked || warn "Could not install espflash"
    fi
}

ensure_qemu_xtensa() {
    local qemu
    qemu="$(find_qemu_xtensa)" || return 0
    log "QEMU xtensa found: $qemu"
}

# ---------- step 3: build the Xiao ELF ----------
build_xiao() {
    ensure_toolchain
    log "Building ESP32-S3 Xiao kiosk system (${ESP32S3_TARGET}) ..."
    CARGO_PROFILE_RELEASE_LTO=fat \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
    cargo +esp build --release --target "${ESP32S3_TARGET}" \
        --manifest-path "$SCRIPT_DIR/crates/baram-xiao/Cargo.toml"
    local xiao_target="$TARGET_DIR/xiao"
    test -f "$xiao_target" || test -f "$xiao_target.elf" || die "Xiao build did not produce $TARGET_DIR/xiao"
    log "  -> $TARGET_DIR/xiao ($(stat -c %s "$xiao_target" 2>/dev/null || stat -f %z "$xiao_target") bytes)"
}

# ---------- step 4: flash to ESP32-S3 ----------
flash_esp32s3() {
    local xiao_bin="$TARGET_DIR/xiao"
    [ -f "$xiao_bin" ] || die "Xiao binary not found. Run './buildx.sh build' first."

    local esptool_cmd
    if command -v esptool.py >/dev/null 2>&1; then
        esptool_cmd="esptool.py"
    else
        esptool_cmd="esptool"
    fi

    log "Flashing to ESP32-S3 on ${ESP32S3_PORT} at ${ESP32S3_BAUD} ..."
    log "  binary: $xiao_bin"
    log "  flash address: ${ESP32S3_FLASH}"
    log "  chip: esp32s3"
    echo

    "$esptool_cmd" --chip esp32s3 --port "$ESP32S3_PORT" --baud "$ESP32S3_BAUD" \
        write_flash "${ESP32S3_FLASH}" "$xiao_bin"

    log "Flash complete! Reset the ESP32-S3 to boot."
}

# ---------- step 5: QEMU emulation ----------
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

run_qemu() {
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

# ---------- subcommands ----------
case "${1:-build}" in
    build)
        build_xiao
        ;;
    flash|run)
        build_xiao
        ensure_esptool
        flash_esp32s3
        ;;
    qemu)
        build_xiao
        ensure_qemu_xtensa
        run_qemu
        ;;
    build-qemu)
        build_xiao
        ensure_qemu_xtensa
        run_qemu
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
        echo "Usage: $0 [build|flash|qemu|build-qemu|clean|help]"
        exit 1
        ;;
esac
