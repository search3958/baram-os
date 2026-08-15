#!/usr/bin/env bash
# =============================================================================
#  build64.sh — one-shot build + run script for BaramOS (UEFI x86_64)
# =============================================================================
#
#  Usage:
#    ./build64.sh           # build + run in QEMU
#    ./build64.sh build     # only build (no QEMU launch)
#    ./build64.sh image     # only build + create the FAT image
#    ./build64.sh write           # build + write to USB (auto-detect)
#    ./build64.sh write /dev/sdX  # build + write to specific disk
#    ./build64.sh run       # only run (assumes image + firmware already exist)
#    ./build64.sh clean     # cargo clean
#    ./build64.sh help
#
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
source "$SCRIPT_DIR/scripts/nano_targets.sh"

PROJECT_NAME="baramos"
EFI_NAME="bootx64.efi"
IMAGE_NAME="osdisk-x64.img"
IMAGE_SIZE_MB=64
RUNTIME_DIR="$SCRIPT_DIR/runtime"
TARGET_DIR="$SCRIPT_DIR/target/x86_64-unknown-uefi/release"

QEMU_RAM="${QEMU_RAM:-0.25G}"
QEMU_DISPLAY="${QEMU_DISPLAY:-default}"
QEMU_SERIAL="${QEMU_SERIAL:-stdio}"
QEMU_MONITOR="${QEMU_MONITOR:-none}"

NANO_APP_NAMES=()
while IFS= read -r name; do
    [ -n "$name" ] && NANO_APP_NAMES+=("$name")
done < <(nano_app_bins)

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

if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
    log "Installing Rust nightly toolchain (required for build-std) ..."
    rustup toolchain install nightly --component rust-src || die "Failed to install nightly"
fi
if ! rustup target list --toolchain nightly --installed 2>/dev/null | grep -q '^x86_64-unknown-uefi'; then
    log "Installing Rust target x86_64-unknown-uefi for nightly ..."
    rustup target add x86_64-unknown-uefi --toolchain nightly || die "Failed to add UEFI target"
fi

build_efi() {
    local primary_bin
    primary_bin="$(nano_primary_bin)"
    log "Building $EFI_NAME ..."
    rm -f "$TARGET_DIR/$EFI_NAME"
    cargo +nightly build --release --target x86_64-unknown-uefi --bin "$primary_bin"
    if [ -f "$TARGET_DIR/$primary_bin.efi" ]; then
        cp "$TARGET_DIR/$primary_bin.efi" "$TARGET_DIR/$EFI_NAME"
    fi
    test -f "$TARGET_DIR/$EFI_NAME" || die "Build did not produce $EFI_NAME"
    log "  -> $TARGET_DIR/$EFI_NAME ($(stat -c %s "$TARGET_DIR/$EFI_NAME" 2>/dev/null || stat -f %z "$TARGET_DIR/$EFI_NAME") bytes)"

    log "Building Nano System application binaries..."
    for name in "${NANO_APP_NAMES[@]}"; do
        cargo +nightly build --release --target x86_64-unknown-uefi --bin "$name"
    done
}

make_fat_image() {
    local out="$RUNTIME_DIR/$IMAGE_NAME"
    local efi="$TARGET_DIR/$EFI_NAME"
    local w3a_dir="$RUNTIME_DIR/w3a"
    mkdir -p "$RUNTIME_DIR"
    mkdir -p "$w3a_dir"
    if [ -x "$SCRIPT_DIR/scripts/package_w3a.sh" ] && [ -d "$SCRIPT_DIR/app" ]; then
        "$SCRIPT_DIR/scripts/package_w3a.sh" "$SCRIPT_DIR/app" "$w3a_dir"
    fi
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
        mmd   -i "$out" ::/EFI/BOOT/bin 2>/dev/null || true
        for name in "${NANO_APP_NAMES[@]}"; do
            [ -f "$TARGET_DIR/$name.efi" ] && \
                mcopy -i "$out" "$TARGET_DIR/$name.efi" ::/EFI/BOOT/bin/
        done
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            mcopy -i "$out" "$SCRIPT_DIR/config.xml" ::/EFI/BOOT/config.xml
            log "  copied config.xml to /EFI/BOOT/"
        fi
        local app_src="$SCRIPT_DIR/app"
        if [ -d "$app_src" ]; then
            mmd -i "$out" ::/apps 2>/dev/null || true
            for f in "$app_src"/*.warp "$app_src"/*.u1 "$app_src"/*.html "$app_src"/*.css "$app_src"/index.yaml "$w3a_dir"/*.w3a "$w3a_dir"/*.w4a "$w3a_dir"/*.s4a; do
                [ -f "$f" ] && mcopy -i "$out" "$f" ::/apps/
            done
            if [ -d "$app_src/icon" ]; then
                mmd -i "$out" ::/apps/icon 2>/dev/null || true
                for f in "$app_src/icon"/*.png; do
                    [ -f "$f" ] && mcopy -i "$out" "$f" ::/apps/icon/
                done
            fi
            log "  copied app files to /apps/"
        fi
        printf 'fs0:\nEFI\\BOOT\\BOOTX64.EFI\n' | mcopy -i "$out" - ::/startup.nsh
        log "  -> $out"
        return 0
    fi

    if [ "$OS" = "Darwin" ]; then
        log "  using macOS hdiutil"
        local tmp_mount
        tmp_mount="$(mktemp -d /tmp/baramos_mount.XXXXXX)"
        hdiutil create -size "${IMAGE_SIZE_MB}m" -fs "MS-DOS FAT32" -volname "EFI" \
            -ov "$out" >/dev/null
        hdiutil attach -nobrowse -mountpoint "$tmp_mount" "$out" >/dev/null
        mkdir -p "$tmp_mount/EFI/BOOT"
        cp "$efi" "$tmp_mount/EFI/BOOT/BOOTX64.EFI"
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            cp "$SCRIPT_DIR/config.xml" "$tmp_mount/EFI/BOOT/config.xml"
            log "  copied config.xml to /EFI/BOOT/"
        fi
        local app_src="$SCRIPT_DIR/app"
        if [ -d "$app_src" ]; then
            mkdir -p "$tmp_mount/apps"
            for f in "$app_src"/*.warp "$app_src"/*.u1 "$app_src"/*.html "$app_src"/*.css "$app_src"/index.yaml "$w3a_dir"/*.w3a "$w3a_dir"/*.w4a "$w3a_dir"/*.s4a; do
                [ -f "$f" ] && cp "$f" "$tmp_mount/apps/"
            done
            if [ -d "$app_src/icon" ]; then
                mkdir -p "$tmp_mount/apps/icon"
                cp "$app_src/icon"/*.png "$tmp_mount/apps/icon/" 2>/dev/null || true
            fi
            log "  copied app files to /apps/"
        fi
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
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            mcopy -i "$out" "$SCRIPT_DIR/config.xml" ::/EFI/BOOT/config.xml
            log "  copied config.xml to /EFI/BOOT/"
        fi
        local app_src="$SCRIPT_DIR/app"
        if [ -d "$app_src" ]; then
            mmd -i "$out" ::/apps 2>/dev/null || true
            for f in "$app_src"/*.warp "$app_src"/*.u1 "$app_src"/*.html "$app_src"/*.css "$app_src"/index.yaml "$w3a_dir"/*.w3a "$w3a_dir"/*.w4a "$w3a_dir"/*.s4a; do
                [ -f "$f" ] && mcopy -i "$out" "$f" ::/apps/
            done
            if [ -d "$app_src/icon" ]; then
                mmd -i "$out" ::/apps/icon 2>/dev/null || true
                for f in "$app_src/icon"/*.png; do
                    [ -f "$f" ] && mcopy -i "$out" "$f" ::/apps/icon/
                done
            fi
            log "  copied app files to /apps/"
        fi
        printf 'fs0:\nEFI\\BOOT\\BOOTX64.EFI\n' | mcopy -i "$out" - ::/startup.nsh
        log "  -> $out"
        return 0
    fi

    die "No FAT image creation tool found. Install 'mtools' (brew install mtools / apt install mtools)."
}

ensure_firmware() {
    local fw_code="$RUNTIME_DIR/edk2-x86_64-code.fd"
    local fw_vars="$RUNTIME_DIR/edk2-x86_64-vars.fd"

    if [ -f "$fw_code" ] && [ -f "$fw_vars" ]; then
        log "Firmware present (split): $fw_code + $fw_vars"
        return 0
    fi

    log "Looking for x86_64 UEFI firmware (OVMF) ..."

    local code_path vars_path
    code_path=$(find /opt/homebrew /usr/local -name "edk2-x86_64-code.fd" 2>/dev/null | head -n 1)
    vars_path=$(find /opt/homebrew /usr/local -name "edk2-i386-vars.fd" 2>/dev/null | head -n 1)

    if [ -n "$code_path" ]; then
        log "  found code: $code_path"
        cp "$code_path" "$fw_code"
        if [ -n "$vars_path" ]; then
            log "  found vars: $vars_path"
            cp "$vars_path" "$fw_vars"
        else
            log "  vars not found, creating empty vars (64 MiB)"
            dd if=/dev/zero of="$fw_vars" bs=1m count=64 2>/dev/null
        fi
        log "  -> $fw_code + $fw_vars"
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
    local fw_code="$RUNTIME_DIR/edk2-x86_64-code.fd"
    local fw_vars="$RUNTIME_DIR/edk2-x86_64-vars.fd"
    local img="$RUNTIME_DIR/$IMAGE_NAME"
    [ -f "$img" ] || die "Disk image missing. Run './build64.sh' first."
    [ -f "$fw_code" ] && [ -f "$fw_vars" ] || die "Firmware missing. Run './build64.sh' first."

    log "Launching QEMU ..."
    log "  cpu     : qemu64"
    log "  ram     : $QEMU_RAM"
    log "  firmware: $fw_code + $fw_vars (OVMF pflash)"
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
        -drive "if=pflash,format=raw,readonly=on,file=$fw_code" \
        -drive "if=pflash,format=raw,file=$fw_vars" \
        -drive "if=none,file=$img,format=raw,id=hd0" \
        -device "virtio-blk-pci,drive=hd0" \
        -device "virtio-vga,edid=on,xres=1280,yres=720" \
        -device "qemu-xhci" \
        -device "usb-tablet" \
        -device "usb-mouse" \
        -device "usb-kbd" \
        -display "$QEMU_DISPLAY" \
        -serial "$QEMU_SERIAL" \
        -monitor "$QEMU_MONITOR"
}

find_usb() {
    if [ "$(uname -s)" = "Darwin" ]; then
        local candidates=()
        while IFS= read -r line; do
            local dev
            dev="$(echo "$line" | xargs)"
            if [ -n "$dev" ] && [ -b "$dev" ]; then
                candidates+=("$dev")
            fi
        done < <(diskutil list external 2>/dev/null | grep "\/dev\/disk[0-9]" | awk '{print $1}' | sort -u)

        if [ ${#candidates[@]} -eq 0 ]; then
            die "No external USB drives found. Connect a USB drive and try again."
        fi

        if [ ${#candidates[@]} -eq 1 ]; then
            echo "${candidates[0]}"
            return 0
        fi

        warn "Multiple external drives found:"
        for i in "${!candidates[@]}"; do
            local size
            size="$(diskutil info "${candidates[$i]}" 2>/dev/null | grep "Disk Size" | awk '{print $3, $4}' || echo "?")"
            printf "  [%d] %s (%s)\n" "$((i+1))" "${candidates[$i]}" "$size" >&2
        done
        read -rp "Select disk number [1-${#candidates[@]}]: " choice
        local idx=$((choice - 1))
        [ "$idx" -ge 0 ] && [ "$idx" -lt "${#candidates[@]}" ] || die "Invalid selection"
        echo "${candidates[$idx]}"
        return 0
    fi

    if [ "$(uname -s)" = "Linux" ]; then
        local candidates=()
        while IFS= read -r dev; do
            [ -b "$dev" ] && candidates+=("$dev")
        done < <(lsblk -dpno NAME,TYPE 2>/dev/null | awk '$2 == "disk" {print $1}')

        if [ ${#candidates[@]} -eq 0 ]; then
            die "No block devices found. Connect a USB drive and try again."
        fi

        if [ ${#candidates[@]} -eq 1 ]; then
            echo "${candidates[0]}"
            return 0
        fi

        warn "Multiple block devices found:"
        for i in "${!candidates[@]}"; do
            local size
            size="$(lsblk -dno SIZE "${candidates[$i]}" 2>/dev/null | xargs || echo "?")"
            printf "  [%d] %s (%s)\n" "$((i+1))" "${candidates[$i]}" "$size" >&2
        done
        read -rp "Select disk number [1-${#candidates[@]}]: " choice
        local idx=$((choice - 1))
        [ "$idx" -ge 0 ] && [ "$idx" -lt "${#candidates[@]}" ] || die "Invalid selection"
        echo "${candidates[$idx]}"
        return 0
    fi

    die "USB auto-detect is only supported on macOS/Linux. Specify the disk manually."
}

write_to_usb() {
    local target="${1:-}"
    local img="$RUNTIME_DIR/$IMAGE_NAME"

    [ -f "$img" ] || die "Image not found. Run './build64.sh image' first."

    if [ -z "$target" ]; then
        log "Auto-detecting USB drive ..."
        target="$(find_usb)"
    fi

    [ -b "$target" ] || die "Disk '$target' not found or not a block device."

    local size
    if [ "$(uname -s)" = "Darwin" ]; then
        size="$(diskutil info "$target" 2>/dev/null | grep "Disk Size" | awk '{print $3, $4}' || echo "?")"
    else
        size="$(lsblk -dno SIZE "$target" 2>/dev/null | xargs || echo "?")"
    fi
    log "Target: $target ($size)"
    log "Image:  $img"

    warn "WARNING: This will ERASE all data on $target!"
    read -rp "Continue? [y/N] " confirm
    [ "$confirm" = "y" ] || [ "$confirm" = "Y" ] || die "Aborted."

    log "Writing $img to $target ..."
    if [ "$(uname -s)" = "Darwin" ]; then
        sudo diskutil unmountDisk "$target" 2>/dev/null || true
    else
        sudo umount "${target}"* 2>/dev/null || true
    fi
    sudo dd if="$img" of="$target" bs=4M status=progress oflag=sync
    sync

    log "Done! Remove $target and plug it into your x86_64 PC."
    log "Boot from USB (press F12/F2/Del during POST to select boot device)."
}

case "${1:-build-run}" in
    build)
        build_efi
        ;;
    image)
        build_efi
        make_fat_image
        ;;
    write)
        build_efi
        make_fat_image
        ensure_firmware
        write_to_usb "${2:-}"
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
        rm -rf "$RUNTIME_DIR/$IMAGE_NAME" \
               "$RUNTIME_DIR/edk2-x86_64-code.fd" "$RUNTIME_DIR/edk2-x86_64-vars.fd"
        ;;
    help|-h|--help)
        sed -n '2,/^# =\+/p' "$0" | sed 's/^# \?//'
        ;;
    *)
        err "Unknown command: $1"
        echo "Usage: $0 [build|image|write|run|firmware|clean|help]"
        exit 1
        ;;
esac
