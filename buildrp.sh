#!/usr/bin/env bash
# =============================================================================
#  buildrp.sh — build BaramOS + pftf UEFI firmware for Raspberry Pi 4B
# =============================================================================
#
#  What this script does:
#    1. Builds BaramOS as bootaa64.efi (aarch64 UEFI).
#    2. Downloads pftf Raspberry Pi 4 UEFI firmware (cached).
#    3. Creates a FAT32 disk image with pftf + BaramOS EFI.
#    4. Optionally writes the image to a USB drive.
#
#  Usage:
#    ./buildrp.sh                 # build image only
#    ./buildrp.sh write           # build + write to USB (auto-detect)
#    ./buildrp.sh write /dev/diskN  # build + write to specific disk
#    ./buildrp.sh clean           # remove build artifacts
#    ./buildrp.sh help
#
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# ---------- configuration ----------
PFTF_VERSION="${PFTF_VERSION:-v1.52}"
PFTF_URL="https://github.com/pftf/RPi4/releases/download/${PFTF_VERSION}/RPi4_UEFI_Firmware_${PFTF_VERSION}.zip"
IMAGE_NAME="baramos-rpi4.img"
IMAGE_SIZE_MB=128
RUNTIME_DIR="$SCRIPT_DIR/runtime"
TARGET_DIR="$SCRIPT_DIR/target/aarch64-unknown-uefi/release"
CACHE_DIR="$RUNTIME_DIR/pftf-cache"

# ---------- pretty logging ----------
log()  { printf "\033[1;34m[build]\033[0m %s\n" "$*"; }
warn() { printf "\033[1;33m[warn]\033[0m %s\n" "$*" >&2; }
err()  { printf "\033[1;31m[err ]\033[0m %s\n" "$*" >&2; }
die()  { err "$*"; exit 1; }

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "Required command '$1' not found. $2"
}

# ---------- step 1: build BaramOS ----------
SUBSYSTEM_NAMES=("windowserver" "font" "graphics" "iokit" "bsd")

build_efi() {
    log "Building BaramOS (aarch64-unknown-uefi) ..."
    cargo +nightly build --release --target aarch64-unknown-uefi
    local efi="$TARGET_DIR/bootaa64.efi"
    [ -f "$efi" ] || die "Build did not produce $efi"
    log "  -> $efi ($(stat -f %z "$efi") bytes)"

    log "Building subsystem binaries..."
    for name in "${SUBSYSTEM_NAMES[@]}"; do
        cargo +nightly build --release --target aarch64-unknown-uefi --bin "$name"
        local src="$TARGET_DIR/$name"
        local dst="$TARGET_DIR/$name.efi"
        if [ -f "$src" ] && [ ! -f "$dst" ]; then
            cp "$src" "$dst"
        fi
        if [ -f "$dst" ]; then
            log "  -> $dst ($(stat -f %z "$dst") bytes)"
        fi
    done
}

# ---------- step 2: download pftf ----------
ensure_pftf() {
    local zip="$CACHE_DIR/RPi4_UEFI_Firmware_${PFTF_VERSION}.zip"
    local marker="$CACHE_DIR/.extracted"

    if [ -f "$marker" ]; then
        log "pftf $PFTF_VERSION already cached"
        return 0
    fi

    mkdir -p "$CACHE_DIR"

    if [ ! -f "$zip" ]; then
        log "Downloading pftf $PFTF_VERSION ..."
        curl -L --fail -o "$zip" "$PFTF_URL" \
            || die "Failed to download pftf. Check network or PFTF_VERSION=$PFTF_VERSION"
    fi

    log "Extracting pftf ..."
    unzip -qo "$zip" -d "$CACHE_DIR/"
    touch "$marker"
    log "  -> $CACHE_DIR"
}

# ---------- step 3: create FAT image ----------
make_image() {
    local img="$RUNTIME_DIR/$IMAGE_NAME"
    local efi="$TARGET_DIR/bootaa64.efi"
    local pftf_dir="$CACHE_DIR"

    [ -f "$efi" ] || die "EFI binary not found. Run build first."
    [ -f "$pftf_dir/RPI_EFI.fd" ] || die "pftf not found. Run ensure_pftf first."

    mkdir -p "$RUNTIME_DIR"
    rm -f "$img"

    log "Creating RPi4 disk image ($IMAGE_SIZE_MB MiB) at $img ..."

    # Strategy A: mtools (cross-platform)
    if command -v mformat >/dev/null 2>&1 && command -v mcopy >/dev/null 2>&1; then
        log "  using mtools"
        truncate -s "${IMAGE_SIZE_MB}M" "$img" 2>/dev/null || \
            dd if=/dev/zero of="$img" bs=1m count="$IMAGE_SIZE_MB" 2>/dev/null
        mformat -i "$img" -F -T $((IMAGE_SIZE_MB * 1024 * 2)) ::

        # pftf root files
        mmd   -i "$img" ::/overlays
        mcopy -i "$img" "$pftf_dir/RPI_EFI.fd"            ::/
        mcopy -i "$img" "$pftf_dir/start4.elf"            ::/
        mcopy -i "$img" "$pftf_dir/fixup4.dat"            ::/
        mcopy -i "$img" "$pftf_dir/config.txt"            ::/
        mcopy -i "$img" "$pftf_dir/bcm2711-rpi-4-b.dtb"   ::/
        mcopy -i "$img" "$pftf_dir/bcm2711-rpi-400.dtb"   ::/
        mcopy -i "$img" "$pftf_dir/bcm2711-rpi-cm4.dtb"   ::/
        mcopy -i "$img" "$pftf_dir/overlays/"*.dtbo        ::/overlays/

        # BaramOS EFI
        mmd   -i "$img" ::/EFI
        mmd   -i "$img" ::/EFI/BOOT
        mcopy -i "$img" "$efi" ::/EFI/BOOT/BOOTAA64.EFI

        # Subsystem binaries
        mmd   -i "$img" ::/EFI/BOOT/bin 2>/dev/null || true
        for name in "${SUBSYSTEM_NAMES[@]}"; do
            local sub_bin="$TARGET_DIR/$name.efi"
            if [ -f "$sub_bin" ]; then
                mcopy -i "$img" "$sub_bin" ::/EFI/BOOT/bin/
                log "  copied $name.efi to /EFI/BOOT/bin/"
            fi
        done

        # BaramOS config
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            mcopy -i "$img" "$SCRIPT_DIR/config.xml" ::/EFI/BOOT/config.xml
            log "  copied config.xml to /EFI/BOOT/"
        fi

        # App files
        local app_src="$SCRIPT_DIR/app"
        if [ -d "$app_src" ]; then
            mmd   -i "$img" ::/apps 2>/dev/null || true
            for f in "$app_src"/*.warp "$app_src"/*.u1 "$app_src"/*.html "$app_src"/*.css "$app_src"/*.ini "$app_src"/*.w3u "$app_src"/*.w3s "$app_src"/index.yaml; do
                [ -f "$f" ] && mcopy -i "$img" "$f" ::/apps/
            done
            if [ -d "$app_src/icon" ]; then
                mmd   -i "$img" ::/apps/icon 2>/dev/null || true
                for f in "$app_src/icon"/*.png; do
                    [ -f "$f" ] && mcopy -i "$img" "$f" ::/apps/icon/
                done
            fi
            log "  copied app files to /apps/"
        fi

        # UEFI shell auto-boot
        printf 'fs0:\nEFI\\BOOT\\BOOTAA64.EFI\n' | mcopy -i "$img" - ::/startup.nsh

        log "  -> $img"
        return 0
    fi

    # Strategy B: macOS hdiutil
    if [ "$(uname -s)" = "Darwin" ]; then
        log "  using macOS hdiutil"
        local tmp_mount
        tmp_mount="$(mktemp -d /tmp/baramos_mount.XXXXXX)"

        hdiutil create -size "${IMAGE_SIZE_MB}m" -fs "MS-DOS FAT32" -volname "EFI" \
            -ov "$img" >/dev/null
        hdiutil attach -nobrowse -mountpoint "$tmp_mount" "$img" >/dev/null

        # pftf root files
        mkdir -p "$tmp_mount/overlays"
        cp "$pftf_dir/RPI_EFI.fd"           "$tmp_mount/"
        cp "$pftf_dir/start4.elf"           "$tmp_mount/"
        cp "$pftf_dir/fixup4.dat"           "$tmp_mount/"
        cp "$pftf_dir/config.txt"           "$tmp_mount/"
        cp "$pftf_dir/bcm2711-rpi-4-b.dtb"  "$tmp_mount/"
        cp "$pftf_dir/bcm2711-rpi-400.dtb"  "$tmp_mount/"
        cp "$pftf_dir/bcm2711-rpi-cm4.dtb"  "$tmp_mount/"
        cp "$pftf_dir/overlays/"*.dtbo       "$tmp_mount/overlays/"

        # BaramOS EFI
        mkdir -p "$tmp_mount/EFI/BOOT"
        cp "$efi" "$tmp_mount/EFI/BOOT/BOOTAA64.EFI"

        # Subsystem binaries
        mkdir -p "$tmp_mount/EFI/BOOT/bin"
        for name in "${SUBSYSTEM_NAMES[@]}"; do
            local sub_bin="$TARGET_DIR/$name.efi"
            if [ -f "$sub_bin" ]; then
                cp "$sub_bin" "$tmp_mount/EFI/BOOT/bin/"
                log "  copied $name.efi to /EFI/BOOT/bin/"
            fi
        done

        # BaramOS config
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            cp "$SCRIPT_DIR/config.xml" "$tmp_mount/EFI/BOOT/config.xml"
            log "  copied config.xml to /EFI/BOOT/"
        fi

        # App files
        local app_src="$SCRIPT_DIR/app"
        if [ -d "$app_src" ]; then
            mkdir -p "$tmp_mount/apps"
            for f in "$app_src"/*.warp "$app_src"/*.u1 "$app_src"/*.html "$app_src"/*.css "$app_src"/*.ini "$app_src"/*.w3u "$app_src"/*.w3s "$app_src"/index.yaml; do
                [ -f "$f" ] && cp "$f" "$tmp_mount/apps/"
            done
            if [ -d "$app_src/icon" ]; then
                mkdir -p "$tmp_mount/apps/icon"
                cp "$app_src/icon"/*.png "$tmp_mount/apps/icon/" 2>/dev/null || true
            fi
            log "  copied app files to /apps/"
        fi

        # UEFI shell auto-boot
        printf 'fs0:\nEFI\\BOOT\\BOOTAA64.EFI\n' > "$tmp_mount/startup.nsh"

        sync
        hdiutil detach "$tmp_mount" >/dev/null || true
        rmdir "$tmp_mount" 2>/dev/null || true
        log "  -> $img"
        return 0
    fi

    die "No FAT image creation tool found. Install 'mtools' (brew install mtools / apt install mtools)."
}

# ---------- step 4: write to USB ----------
find_usb() {
    if [ "$(uname -s)" != "Darwin" ]; then
        die "USB auto-detect is only supported on macOS. Specify the disk manually."
    fi

    local disks
    disks="$(diskutil list external -plist 2>/dev/null || true)"

    # Find external physical disks
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

    # Multiple disks — let user choose
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
}

write_to_usb() {
    local target="${1:-}"
    local img="$RUNTIME_DIR/$IMAGE_NAME"

    [ -f "$img" ] || die "Image not found. Run './buildrp.sh' first."

    if [ -z "$target" ]; then
        log "Auto-detecting USB drive ..."
        target="$(find_usb)"
    fi

    [ -b "$target" ] || die "Disk '$target' not found or not a block device."

    local size
    size="$(diskutil info "$target" 2>/dev/null | grep "Disk Size" | awk '{print $3, $4}' || echo "?")"
    log "Target: $target ($size)"

    warn "WARNING: This will ERASE all data on $target!"
    read -rp "Continue? [y/N] " confirm
    [ "$confirm" = "y" ] || [ "$confirm" = "Y" ] || die "Aborted."

    log "Writing $img to $target ..."
    sudo diskutil unmountDisk "$target" 2>/dev/null || true
    sudo dd if="$img" of="$target" bs=4M status=progress oflag=sync
    sync

    log "Done! Remove $target and plug it into your Raspberry Pi 4B."
}

# ---------- subcommands ----------
case "${1:-build}" in
    build)
        build_efi
        ensure_pftf
        make_image
        ;;
    write)
        build_efi
        ensure_pftf
        make_image
        write_to_usb "${2:-}"
        ;;
    pftf)
        ensure_pftf
        ;;
    clean)
        log "Cleaning ..."
        cargo clean
        rm -rf "$RUNTIME_DIR/$IMAGE_NAME" "$CACHE_DIR"
        ;;
    help|-h|--help)
        sed -n '2,/^# =\+/p' "$0" | sed 's/^# \?//'
        ;;
    *)
        err "Unknown command: $1"
        echo "Usage: $0 [build|write|pftf|clean|help]"
        exit 1
        ;;
esac
