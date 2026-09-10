#!/usr/bin/env bash
# =============================================================================
#  build.sh — one-shot build + run script for BaramOS (UEFI ARM64)
# =============================================================================
#
#  What this script does:
#    1. Verifies required tools are installed (cargo / rustup / qemu).
#    2. Adds the `aarch64-unknown-uefi` Rust target if missing.
#    3. Builds the OS as bootaa64.efi (PE32+ ARM64 UEFI application).
#    4. Prepares a FAT-formatted disk image with the EFI binary at
#       EFI/BOOT/BOOTAA64.EFI (the standard UEFI removable-media path).
#    5. Uses the bundled BaramOS AArch64 UEFI firmware, built without the
#       upstream AAVMF 128 MiB minimum-memory assertion.
#    6. Boots the OS in qemu-system-aarch64 using the QEMU `virt` machine
#       (Cortex-A72, normal memory or the Xiao 20 MiB profile, USB mouse +
#       keyboard, 128x64 Xiao display).
#
#  Tested on:
#    * macOS 13+  (Intel + Apple Silicon) with Homebrew qemu/rust
#    * Linux x86_64 with rustup + apt/conda qemu
#
#  Usage:
#    ./build.sh           # build + run in QEMU
#    ./build.sh build     # only build (no QEMU launch)
#    ./build.sh image     # only build + create the FAT image
#    ./build.sh run       # only run (assumes image + firmware already exist)
#    ./build.sh clean     # cargo clean
#    ./build.sh x       # ARM64 Xiao kiosk build + run
#    ./build.sh help
#
# =============================================================================

set -euo pipefail

# ---------- script metadata ----------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
source "$SCRIPT_DIR/scripts/nano_targets.sh"

PROJECT_NAME="baramos"
EFI_NAME="bootaa64.efi"
IMAGE_NAME="osdisk-arm64.img"
XIAO_IMAGE_NAME="osdisk-arm64-xiao.img"
IMAGE_SIZE_MB=64
FIRMWARE_NAME="baram-aarch64-uefi.fd"
FIRMWARE_PATH="$SCRIPT_DIR/firmware/$FIRMWARE_NAME"
XIAO_FIRMWARE_PATH="$SCRIPT_DIR/firmware/baram-aarch64-xiao-uefi.fd"
RUNTIME_DIR="$SCRIPT_DIR/runtime"
TARGET_DIR="$SCRIPT_DIR/target/aarch64-unknown-uefi/release"

# QEMU defaults — override via env vars if desired.
QEMU_MACHINE="${QEMU_MACHINE:-virt}"
QEMU_CPU="${QEMU_CPU:-cortex-a72}"
QEMU_RAM_WAS_SET=0
if [ "${QEMU_RAM+x}" = x ]; then
    QEMU_RAM_WAS_SET=1
fi
QEMU_RAM="${QEMU_RAM:-0.15G}"
QEMU_DISPLAY_WAS_SET=0
if [ "${QEMU_DISPLAY+x}" = x ]; then
    QEMU_DISPLAY_WAS_SET=1
fi
QEMU_DISPLAY="${QEMU_DISPLAY:-}"
# Where the firmware/OS serial output goes.  Default is `stdio` so you can
# see boot logs in the terminal.  Use `null` to silence serial.
QEMU_SERIAL="${QEMU_SERIAL:-stdio}"
# Where the QEMU HMP monitor goes.  Default is `none` (no monitor).  Use
# `stdio` to control QEMU via the terminal (e.g. `screendump`, `quit`).
QEMU_MONITOR="${QEMU_MONITOR:-none}"
XIAO_MODE=0

# ---------- pretty logging ----------
log()  { printf "\033[1;34m[build]\033[0m %s\n" "$*"; }
warn() { printf "\033[1;33m[warn]\033[0m %s\n"  "$*" >&2; }
err()  { printf "\033[1;31m[err ]\033[0m %s\n"  "$*" >&2; }
die()  { err "$*"; exit 1; }

# ---------- platform detection ----------
OS="$(uname -s)"
ARCH="$(uname -m)"
log "Detected OS=$OS ARCH=$ARCH"

# `default` is an alias for the host GUI backend, but QEMU does not accept
# backend options after that alias. Resolve the backend before launch so the
# requested zoom-to-fit setting is passed in a form QEMU actually supports.
if [ "$QEMU_DISPLAY_WAS_SET" -eq 0 ]; then
    case "$OS" in
        Darwin) QEMU_DISPLAY="cocoa,zoom-to-fit=on" ;;
        *) QEMU_DISPLAY="gtk,zoom-to-fit=on" ;;
    esac
fi

# ---------- step 1: Rust toolchain ----------
require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "Required command '$1' not found. $2"
}

if ! command -v cargo >/dev/null 2>&1; then
    die "cargo not found. Install Rust via:

  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
"
fi
if ! command -v rustup >/dev/null 2>&1; then
    die "rustup not found. Install Rust via https://rustup.rs"
fi

# Nightly is required for build-std (we need core/alloc for the UEFI target).
if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
    log "Installing Rust nightly toolchain (required for build-std) ..."
    rustup toolchain install nightly --component rust-src || die "Failed to install nightly"
fi
if ! rustup target list --toolchain nightly --installed 2>/dev/null | grep -q '^aarch64-unknown-uefi'; then
    log "Installing Rust target aarch64-unknown-uefi for nightly ..."
    rustup target add aarch64-unknown-uefi --toolchain nightly || die "Failed to add UEFI target"
fi

# ---------- step 2: build the EFI app ----------
PRIMARY_BIN="$(nano_primary_bin)"
SUBSYSTEM_NAMES=()
while IFS= read -r name; do
    [ -n "$name" ] && SUBSYSTEM_NAMES+=("$name")
done < <(nano_app_bins)

build_efi() {
    log "Building $EFI_NAME ..."
    cargo +nightly build --release --target aarch64-unknown-uefi --bin "$PRIMARY_BIN"
    if [ -f "$TARGET_DIR/$PRIMARY_BIN" ] && [ ! -f "$TARGET_DIR/$PRIMARY_BIN.efi" ]; then
        cp "$TARGET_DIR/$PRIMARY_BIN" "$TARGET_DIR/$PRIMARY_BIN.efi"
    fi
    if [ "$PRIMARY_BIN.efi" != "$EFI_NAME" ]; then
        cp "$TARGET_DIR/$PRIMARY_BIN.efi" "$TARGET_DIR/$EFI_NAME"
    fi
    test -f "$TARGET_DIR/$EFI_NAME" || die "Build did not produce $EFI_NAME"
    log "  -> $TARGET_DIR/$EFI_NAME ($(stat -c %s "$TARGET_DIR/$EFI_NAME" 2>/dev/null || stat -f %z "$TARGET_DIR/$EFI_NAME") bytes)"

    log "Building subsystem binaries..."
    for name in "${SUBSYSTEM_NAMES[@]}"; do
        cargo +nightly build --release --target aarch64-unknown-uefi --bin "$name"
        local src="$TARGET_DIR/$name"
        local dst="$TARGET_DIR/$name.efi"
        if [ -f "$src" ] && [ ! -f "$dst" ]; then
            cp "$src" "$dst"
        fi
        if [ -f "$dst" ]; then
            log "  -> $dst ($(stat -c %s "$dst" 2>/dev/null || stat -f %z "$dst") bytes)"
        fi
    done
}

# The Xiao system is a separate ARM64 image. It deliberately builds only the
# Nano/Warp4 kiosk entry point; the normal desktop and its subsystem binaries
# are not part of this image.
build_xiao() {
    XIAO_MODE=1
    FIRMWARE_PATH="$XIAO_FIRMWARE_PATH"
    # Xiao uses the smallest whole-MiB guest size verified to reach the kiosk
    # with the bundled 128x64 firmware. 19M does not reach Nano; 20M does.
    # Keep an explicit QEMU_RAM override available for diagnostics.
    if [ "$QEMU_RAM_WAS_SET" -eq 0 ]; then
        QEMU_RAM="20M"
    fi
    local xiao_target="$SCRIPT_DIR/target/aarch64-unknown-uefi/release/xiao.efi"
    local xiao_efi="$TARGET_DIR/bootaa64-xiao.efi"
    log "Building ARM64 Xiao kiosk system ..."
    # Xiao is the memory-constrained image. Use whole-program LTO and size
    # optimization only for this branch; the normal system keeps its normal
    # release profile and feature set.
    CARGO_PROFILE_RELEASE_LTO=fat \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
    cargo +nightly build --release --target aarch64-unknown-uefi \
        --manifest-path "$SCRIPT_DIR/crates/baram-xiao/Cargo.toml"
    test -f "$xiao_target" || die "Xiao build did not produce $xiao_target"
    cp "$xiao_target" "$xiao_efi"
    cp "$xiao_target" "$TARGET_DIR/$EFI_NAME"
    log "  -> $xiao_efi ($(stat -c %s "$xiao_efi" 2>/dev/null || stat -f %z "$xiao_efi") bytes)"
}

# ---------- step 3: FAT image creation ----------
# Try every strategy in order.  We prefer mtools (cross-platform, fast)
# then macOS hdiutil, then Linux loop-mount.
make_fat_image() {
    local image_name="$IMAGE_NAME"
    if [ "$XIAO_MODE" -eq 1 ]; then
        image_name="$XIAO_IMAGE_NAME"
    fi
    local out="$RUNTIME_DIR/$image_name"
    local efi="$TARGET_DIR/$EFI_NAME"
    local files_tree="$RUNTIME_DIR/files-tree"
    local xiao_bdf="$SCRIPT_DIR/crates/baram-xiao/src/misaki_gothic_2nd.bdf"
    if [ "$XIAO_MODE" -eq 1 ] && [ ! -f "$xiao_bdf" ]; then
        die "Xiao BDF missing: $xiao_bdf"
    fi
    mkdir -p "$RUNTIME_DIR"
    local package_profile="normal"
    if [ "$XIAO_MODE" -eq 1 ]; then
        package_profile="xiao"
    fi
    "$SCRIPT_DIR/scripts/package_files.sh" "$SCRIPT_DIR/files" "$files_tree" "$package_profile"

    if [ -f "$out" ]; then
        log "Removing existing disk image $out ..."
        rm -f "$out"
    fi

    log "Creating FAT disk image ($IMAGE_SIZE_MB MiB) at $out ..."

    # Strategy A: mtools (mformat + mcopy) — works the same on macOS/Linux.
    if command -v mformat >/dev/null 2>&1 && command -v mcopy >/dev/null 2>&1; then
        log "  using mtools"
        truncate -s "${IMAGE_SIZE_MB}M" "$out" 2>/dev/null || \
            dd if=/dev/zero of="$out" bs=1m count="$IMAGE_SIZE_MB" 2>/dev/null || \
            dd if=/dev/zero of="$out" bs=1M   count="$IMAGE_SIZE_MB" 2>/dev/null
        mformat -i "$out" -F -T $((IMAGE_SIZE_MB * 1024 * 2)) ::
        mmd   -i "$out" ::/EFI
        mmd   -i "$out" ::/EFI/BOOT
        mmd   -i "$out" ::/files
        if [ -d "$files_tree/app" ]; then
            mmd -i "$out" ::/files/app
        fi
        if [ -d "$files_tree/data" ]; then
            mmd -i "$out" ::/files/data
        fi
        mcopy -i "$out" "$efi" ::/EFI/BOOT/BOOTAA64.EFI
        if [ "$XIAO_MODE" -eq 1 ]; then
            mcopy -i "$out" "$xiao_bdf" ::/EFI/BOOT/MISAKI_GOTHIC_2ND.BDF
            log "  copied xiao BDF (streamed at runtime)"
        fi
        # Create bin directory for subsystems
        mmd   -i "$out" ::/EFI/BOOT/bin 2>/dev/null || true
        if [ "$XIAO_MODE" -eq 0 ]; then
            # Copy subsystem binaries for the normal desktop image only.
            for name in "${SUBSYSTEM_NAMES[@]}"; do
                local sub_bin="$TARGET_DIR/$name.efi"
                if [ -f "$sub_bin" ]; then
                    mcopy -i "$out" "$sub_bin" ::/EFI/BOOT/bin/
                    log "  copied $name.efi to /EFI/BOOT/bin/"
                fi
            done
        fi
        # Copy config file
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            mcopy -i "$out" "$SCRIPT_DIR/config.xml" ::/EFI/BOOT/config.xml
            log "  copied config.xml to /EFI/BOOT/"
        fi
        if [ -d "$files_tree/app" ]; then
            mcopy -s -i "$out" "$files_tree/app/." ::/files/app/
        fi
        if [ -d "$files_tree/data" ]; then
            mcopy -s -i "$out" "$files_tree/data/." ::/files/data/
        fi
        log "  copied files as regular FAT files"
        # Auto-boot script: tells the UEFI shell to run our EFI binary
        # without waiting for the 5-second startup.nsh countdown.
        printf 'fs0:\nEFI\\BOOT\\BOOTAA64.EFI\n' | mcopy -i "$out" - ::/startup.nsh
        log "  -> $out"
        return 0
    fi

    # Strategy B: macOS hdiutil (always available on macOS).
    if [ "$OS" = "Darwin" ]; then
        log "  using macOS hdiutil"
        local tmp_mount
        tmp_mount="$(mktemp -d /tmp/baramos_mount.XXXXXX)"
        hdiutil create -size "${IMAGE_SIZE_MB}m" -fs "MS-DOS FAT32" -volname "EFI" \
            -ov "$out" >/dev/null
        # hdiutil create appends .dmg unless we use -type UDIF; rename to be safe.
        hdiutil attach -nobrowse -mountpoint "$tmp_mount" "$out" >/dev/null
        mkdir -p "$tmp_mount/EFI/BOOT" "$tmp_mount/files"
        cp "$efi" "$tmp_mount/EFI/BOOT/BOOTAA64.EFI"
        if [ "$XIAO_MODE" -eq 1 ]; then
            cp "$xiao_bdf" "$tmp_mount/EFI/BOOT/MISAKI_GOTHIC_2ND.BDF"
            log "  copied xiao BDF (streamed at runtime)"
        fi
        # Create bin directory for subsystems
        mkdir -p "$tmp_mount/EFI/BOOT/bin"
        if [ "$XIAO_MODE" -eq 0 ]; then
            # Copy subsystem binaries for the normal desktop image only.
            for name in "${SUBSYSTEM_NAMES[@]}"; do
                local sub_bin="$TARGET_DIR/$name.efi"
                if [ -f "$sub_bin" ]; then
                    cp "$sub_bin" "$tmp_mount/EFI/BOOT/bin/"
                    log "  copied $name.efi to /EFI/BOOT/bin/"
                fi
            done
        fi
        # Auto-boot script.
        printf 'fs0:\nEFI\\BOOT\\BOOTAA64.EFI\n' > "$tmp_mount/startup.nsh"
        # Copy config file
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            cp "$SCRIPT_DIR/config.xml" "$tmp_mount/EFI/BOOT/config.xml"
            log "  copied config.xml to /EFI/BOOT/"
        fi
        if [ -d "$files_tree/app" ]; then
            mkdir -p "$tmp_mount/files/app"
            cp -R "$files_tree/app/." "$tmp_mount/files/app/"
        fi
        if [ -d "$files_tree/data" ]; then
            mkdir -p "$tmp_mount/files/data"
            cp -R "$files_tree/data/." "$tmp_mount/files/data/"
        fi
        log "  copied files as regular FAT files"
        sync
        hdiutil detach "$tmp_mount" >/dev/null || true
        rmdir "$tmp_mount" 2>/dev/null || true
        log "  -> $out"
        return 0
    fi

    # Strategy C: Linux mkfs.vfat + mtools-less (loop mount).
    if command -v mkfs.vfat >/dev/null 2>&1 && command -v mtools >/dev/null 2>&1; then
        log "  using mkfs.vfat + mtools"
        truncate -s "${IMAGE_SIZE_MB}M" "$out"
        mkfs.vfat -F 32 -n EFI "$out" >/dev/null
        mmd   -i "$out" ::/EFI
        mmd   -i "$out" ::/EFI/BOOT
        mmd   -i "$out" ::/files
        if [ -d "$files_tree/app" ]; then
            mmd -i "$out" ::/files/app
        fi
        if [ -d "$files_tree/data" ]; then
            mmd -i "$out" ::/files/data
        fi
        mcopy -i "$out" "$efi" ::/EFI/BOOT/BOOTAA64.EFI
        if [ "$XIAO_MODE" -eq 1 ]; then
            mcopy -i "$out" "$xiao_bdf" ::/EFI/BOOT/MISAKI_GOTHIC_2ND.BDF
            log "  copied xiao BDF (streamed at runtime)"
        fi
        # Create bin directory for subsystems
        mmd   -i "$out" ::/EFI/BOOT/bin 2>/dev/null || true
        if [ "$XIAO_MODE" -eq 0 ]; then
            # Copy subsystem binaries for the normal desktop image only.
            for name in "${SUBSYSTEM_NAMES[@]}"; do
                local sub_bin="$TARGET_DIR/$name.efi"
                if [ -f "$sub_bin" ]; then
                    mcopy -i "$out" "$sub_bin" ::/EFI/BOOT/bin/
                    log "  copied $name.efi to /EFI/BOOT/bin/"
                fi
            done
        fi
        # Auto-boot script.
        printf 'fs0:\nEFI\\BOOT\\BOOTAA64.EFI\n' | mcopy -i "$out" - ::/startup.nsh
        # Copy config file
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            mcopy -i "$out" "$SCRIPT_DIR/config.xml" ::/EFI/BOOT/config.xml
            log "  copied config.xml to /EFI/BOOT/"
        fi
        if [ -d "$files_tree/app" ]; then
            mcopy -s -i "$out" "$files_tree/app/." ::/files/app/
        fi
        if [ -d "$files_tree/data" ]; then
            mcopy -s -i "$out" "$files_tree/data/." ::/files/data/
        fi
        log "  copied files as regular FAT files"
        log "  -> $out"
        return 0
    fi

    # Strategy D: Linux loop mount (needs root or sudo).
    if [ "$OS" = "Linux" ] && command -v mkfs.vfat >/dev/null 2>&1; then
        warn "Falling back to loop-mount FAT image creation (may require sudo)."
        truncate -s "${IMAGE_SIZE_MB}M" "$out"
        mkfs.vfat -F 32 -n EFI "$out" >/dev/null
        local tmp_mount
        tmp_mount="$(mktemp -d /tmp/baramos_mount.XXXXXX)"
        sudo mount -o loop "$out" "$tmp_mount" 2>/dev/null || \
            mount -o loop "$out" "$tmp_mount" 2>/dev/null || {
                err "Could not mount loop device. Install 'mtools' for non-root image creation."
                rm -rf "$tmp_mount"
                return 1
            }
        mkdir -p "$tmp_mount/EFI/BOOT" "$tmp_mount/files"
        cp "$efi" "$tmp_mount/EFI/BOOT/BOOTAA64.EFI"
        if [ "$XIAO_MODE" -eq 1 ]; then
            cp "$xiao_bdf" "$tmp_mount/EFI/BOOT/MISAKI_GOTHIC_2ND.BDF"
            log "  copied xiao BDF (streamed at runtime)"
        fi
        # Create bin directory for subsystems
        mkdir -p "$tmp_mount/EFI/BOOT/bin"
        if [ "$XIAO_MODE" -eq 0 ]; then
            # Copy subsystem binaries for the normal desktop image only.
            for name in "${SUBSYSTEM_NAMES[@]}"; do
                local sub_bin="$TARGET_DIR/$name.efi"
                if [ -f "$sub_bin" ]; then
                    cp "$sub_bin" "$tmp_mount/EFI/BOOT/bin/"
                    log "  copied $name.efi to /EFI/BOOT/bin/"
                fi
            done
        fi
        # Copy config file
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            cp "$SCRIPT_DIR/config.xml" "$tmp_mount/EFI/BOOT/config.xml"
            log "  copied config.xml to /EFI/BOOT/"
        fi
        if [ -d "$files_tree/app" ]; then
            mkdir -p "$tmp_mount/files/app"
            cp -R "$files_tree/app/." "$tmp_mount/files/app/"
        fi
        if [ -d "$files_tree/data" ]; then
            mkdir -p "$tmp_mount/files/data"
            cp -R "$files_tree/data/." "$tmp_mount/files/data/"
        fi
        log "  copied files as regular FAT files"
        sync
        sudo umount "$tmp_mount" 2>/dev/null || umount "$tmp_mount" 2>/dev/null || true
        rmdir "$tmp_mount" 2>/dev/null || true
        log "  -> $out"
        return 0
    fi

    die "No FAT image creation tool found. Install 'mtools' (brew install mtools / apt install mtools)."
}

# ---------- step 4: UEFI firmware ----------
ensure_firmware() {
    if [ ! -f "$FIRMWARE_PATH" ]; then
        die "BaramOS AArch64 UEFI firmware is missing: $FIRMWARE_PATH

This repository requires its patched firmware for both BaramOS and Xiao;
do not substitute the stock AAVMF firmware because it rejects RAM below
128 MiB before GOP initialization."
    fi
    # Do not impose a firmware-size floor here. A valid UEFI image may be
    # intentionally minimal, and QEMU/UEFI should be the authority on
    # whether its contents are bootable. Only reject a missing/empty image.
    [ -s "$FIRMWARE_PATH" ] || die "UEFI firmware is empty: $FIRMWARE_PATH"
    log "Firmware present (BaramOS low-memory UEFI): $FIRMWARE_PATH"
}

# ---------- step 5: QEMU launch ----------
find_qemu() {
    # Common names + brew paths.
    local candidates=(
        "qemu-system-aarch64"
        "/opt/homebrew/bin/qemu-system-aarch64"
        "/usr/local/bin/qemu-system-aarch64"
        "/usr/bin/qemu-system-aarch64"
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
    qemu="$(find_qemu)" || die "qemu-system-aarch64 not found.

Install QEMU:
  macOS  : brew install qemu
  Debian : sudo apt install qemu-system-arm
  Ubuntu : sudo apt install qemu-system-arm
  Arch   : sudo pacman -S qemu-system-aarch64
"
    local image_name="$IMAGE_NAME"
    if [ "$XIAO_MODE" -eq 1 ]; then
        image_name="$XIAO_IMAGE_NAME"
    fi
    local img="$RUNTIME_DIR/$image_name"
    [ -f "$img" ] || die "Disk image missing. Run './build.sh' first."
    [ -f "$FIRMWARE_PATH" ] || die "Firmware missing: $FIRMWARE_PATH"

    log "Launching QEMU ..."
    log "  machine : $QEMU_MACHINE"
    log "  cpu     : $QEMU_CPU"
    log "  ram     : $QEMU_RAM"
    log "  firmware: $FIRMWARE_PATH (BaramOS low-memory UEFI)"
    log "  disk    : $img"
    echo

    # Build the firmware args.
    local fw_args=()
    fw_args+=(-bios "$FIRMWARE_PATH")

    # QEMU args explanation:
    #   -machine virt            : ARM virt machine (matches Raspberry Pi UEFI class)
    #   -cpu cortex-a72          : 64-bit ARM core (same family as Pi 4)
    #   -m <profile>             : normal desktop RAM or Xiao's 22.352 MiB target
    #   -bios                    : BaramOS UEFI firmware with no 128 MiB floor
    #   -drive ...,format=raw    : FAT image as removable media (bootable)
    #   -device ramfb            : BaramOS firmware framebuffer
    #   -device qemu-xhci        : USB 3.0 host controller (required for usb-kbd / usb-mouse)
    #   -device usb-tablet       : USB absolute pointing device (exposed by UEFI as the
    #                              EFI Absolute Pointer Protocol — best mouse support)
    #   -device usb-kbd          : USB keyboard (Simple Text Input)
    #   -display <disp>          : GUI window (use 'none' for headless)
    #   -serial <serial>         : serial console
    #   -monitor <monitor>       : HMP monitor (use 'none' to disable, 'stdio' to control
    #                              QEMU via the terminal — try `screendump`, `sendkey`, `quit`)
    #
    # Optional env vars:
    #   QEMU_DATADIR             : path to QEMU's data dir (auto-detected normally;
    #                              set this only if QEMU can't find its romfiles)
    #   QEMU_EXTRA_ARGS          : extra args appended verbatim to the QEMU command line
    #   QEMU_SERIAL              : where serial console goes ('stdio', 'null', 'file:PATH')
    #   QEMU_MONITOR             : where HMP monitor goes ('none', 'stdio')
    #   XIAO_ICOUNT=1            : opt into slow instruction-count timing for Xiao
    #                              (disabled by default so pointer input stays responsive)
    local extra_args=()
    if [ -n "${QEMU_DATADIR:-}" ]; then
        extra_args+=(-L "$QEMU_DATADIR")
    fi
    # shellcheck disable=SC2206
    if [ -n "${QEMU_EXTRA_ARGS:-}" ]; then
        extra_args+=($QEMU_EXTRA_ARGS)
    fi

    local display_device="ramfb"
    if [ "$XIAO_MODE" -eq 1 ] && [ "${XIAO_ICOUNT:-0}" = "1" ]; then
        # This is intentionally opt-in. Instruction-count timing with
        # real-time alignment can make TCG fall seconds behind while UEFI,
        # framebuffer copies, and Warp4 are active.
        extra_args+=(-icount "shift=1,align=on")
    fi

    log "  display : $QEMU_DISPLAY"

    exec "$qemu" \
        "${extra_args[@]}" \
        -machine "$QEMU_MACHINE" \
        -cpu "$QEMU_CPU" \
        -m "$QEMU_RAM" \
        "${fw_args[@]}" \
        -drive "if=none,file=$img,format=raw,id=hd0,cache=none" \
        -device "virtio-blk-device,drive=hd0" \
        -device "$display_device" \
        -device "qemu-xhci" \
        -device "usb-tablet" \
        -device "usb-mouse" \
        -device "usb-kbd" \
        -display "$QEMU_DISPLAY" \
        -serial "$QEMU_SERIAL" \
        -monitor "$QEMU_MONITOR"
}

# ---------- subcommands ----------
case "${1:-build-run}" in
    x)
        build_xiao
        make_fat_image
        ensure_firmware
        run_qemu
        ;;
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
        rm -rf "$RUNTIME_DIR/$IMAGE_NAME"
        rm -rf "$RUNTIME_DIR/$XIAO_IMAGE_NAME"
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
