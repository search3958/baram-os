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
#    5. Downloads QEMU_EFI.fd (AAVMF) firmware if it isn't already present.
#    6. Boots the OS in qemu-system-aarch64 using the QEMU `virt` machine
#       (Cortex-A72, 1 GiB RAM, USB mouse + keyboard, VGA display).
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
IMAGE_SIZE_MB=64
FIRMWARE_NAME="QEMU_EFI.fd"
RUNTIME_DIR="$SCRIPT_DIR/runtime"
TARGET_DIR="$SCRIPT_DIR/target/aarch64-unknown-uefi/release"

# QEMU defaults — override via env vars if desired.
QEMU_MACHINE="${QEMU_MACHINE:-virt}"
QEMU_CPU="${QEMU_CPU:-cortex-a72}"
QEMU_RAM="${QEMU_RAM:-0.25G}"
QEMU_DISPLAY="${QEMU_DISPLAY:-default}"
# Where the firmware/OS serial output goes.  Default is `stdio` so you can
# see boot logs in the terminal.  Use `null` to silence serial.
QEMU_SERIAL="${QEMU_SERIAL:-stdio}"
# Where the QEMU HMP monitor goes.  Default is `none` (no monitor).  Use
# `stdio` to control QEMU via the terminal (e.g. `screendump`, `quit`).
QEMU_MONITOR="${QEMU_MONITOR:-none}"

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

# ---------- step 3: FAT image creation ----------
# Try every strategy in order.  We prefer mtools (cross-platform, fast)
# then macOS hdiutil, then Linux loop-mount.
make_fat_image() {
    local out="$RUNTIME_DIR/$IMAGE_NAME"
    local efi="$TARGET_DIR/$EFI_NAME"
    local w3a_dir="$RUNTIME_DIR/w3a"
    mkdir -p "$RUNTIME_DIR"
    mkdir -p "$w3a_dir"
    if [ -x "$SCRIPT_DIR/scripts/package_w3a.sh" ] && [ -d "$SCRIPT_DIR/app" ]; then
        "$SCRIPT_DIR/scripts/package_w3a.sh" "$SCRIPT_DIR/app" "$w3a_dir"
    fi

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
        mcopy -i "$out" "$efi" ::/EFI/BOOT/BOOTAA64.EFI
        # Create bin directory for subsystems
        mmd   -i "$out" ::/EFI/BOOT/bin 2>/dev/null || true
        # Copy subsystem binaries
        for name in "${SUBSYSTEM_NAMES[@]}"; do
            local sub_bin="$TARGET_DIR/$name.efi"
            if [ -f "$sub_bin" ]; then
                mcopy -i "$out" "$sub_bin" ::/EFI/BOOT/bin/
                log "  copied $name.efi to /EFI/BOOT/bin/"
            fi
        done
        # Copy config file
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            mcopy -i "$out" "$SCRIPT_DIR/config.xml" ::/EFI/BOOT/config.xml
            log "  copied config.xml to /EFI/BOOT/"
        fi
        # Auto-boot script: tells the UEFI shell to run our EFI binary
        # without waiting for the 5-second startup.nsh countdown.
        printf 'fs0:\nEFI\\BOOT\\BOOTAA64.EFI\n' | mcopy -i "$out" - ::/startup.nsh
        # Copy app files to /apps/ directory
        local app_src="$SCRIPT_DIR/app"
        if [ -d "$app_src" ]; then
            mmd -i "$out" ::/apps 2>/dev/null || true
            for f in "$app_src"/*.warp "$app_src"/*.u1 "$app_src"/*.html "$app_src"/*.css "$app_src"/index.yaml "$w3a_dir"/*.w3a "$w3a_dir"/*.w4a "$w3a_dir"/*.s4a; do
                [ -f "$f" ] && mcopy -i "$out" "$f" ::/apps/
            done
            # Copy icon subdirectory
            if [ -d "$app_src/icon" ]; then
                mmd -i "$out" ::/apps/icon 2>/dev/null || true
                for f in "$app_src/icon"/*.png; do
                    [ -f "$f" ] && mcopy -i "$out" "$f" ::/apps/icon/
                done
            fi
            log "  copied app files to /apps/"
        fi
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
        mkdir -p "$tmp_mount/EFI/BOOT"
        cp "$efi" "$tmp_mount/EFI/BOOT/BOOTAA64.EFI"
        # Create bin directory for subsystems
        mkdir -p "$tmp_mount/EFI/BOOT/bin"
        # Copy subsystem binaries
        for name in "${SUBSYSTEM_NAMES[@]}"; do
            local sub_bin="$TARGET_DIR/$name.efi"
            if [ -f "$sub_bin" ]; then
                cp "$sub_bin" "$tmp_mount/EFI/BOOT/bin/"
                log "  copied $name.efi to /EFI/BOOT/bin/"
            fi
        done
        # Auto-boot script.
        printf 'fs0:\nEFI\\BOOT\\BOOTAA64.EFI\n' > "$tmp_mount/startup.nsh"
        # Copy config file
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            cp "$SCRIPT_DIR/config.xml" "$tmp_mount/EFI/BOOT/config.xml"
            log "  copied config.xml to /EFI/BOOT/"
        fi
        # Copy app files
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
        mcopy -i "$out" "$efi" ::/EFI/BOOT/BOOTAA64.EFI
        # Create bin directory for subsystems
        mmd   -i "$out" ::/EFI/BOOT/bin 2>/dev/null || true
        # Copy subsystem binaries
        for name in "${SUBSYSTEM_NAMES[@]}"; do
            local sub_bin="$TARGET_DIR/$name.efi"
            if [ -f "$sub_bin" ]; then
                mcopy -i "$out" "$sub_bin" ::/EFI/BOOT/bin/
                log "  copied $name.efi to /EFI/BOOT/bin/"
            fi
        done
        # Auto-boot script.
        printf 'fs0:\nEFI\\BOOT\\BOOTAA64.EFI\n' | mcopy -i "$out" - ::/startup.nsh
        # Copy config file
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            mcopy -i "$out" "$SCRIPT_DIR/config.xml" ::/EFI/BOOT/config.xml
            log "  copied config.xml to /EFI/BOOT/"
        fi
        # Copy app files to /apps/ directory
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
        mkdir -p "$tmp_mount/EFI/BOOT"
        cp "$efi" "$tmp_mount/EFI/BOOT/BOOTAA64.EFI"
        # Create bin directory for subsystems
        mkdir -p "$tmp_mount/EFI/BOOT/bin"
        # Copy subsystem binaries
        for name in "${SUBSYSTEM_NAMES[@]}"; do
            local sub_bin="$TARGET_DIR/$name.efi"
            if [ -f "$sub_bin" ]; then
                cp "$sub_bin" "$tmp_mount/EFI/BOOT/bin/"
                log "  copied $name.efi to /EFI/BOOT/bin/"
            fi
        done
        # Copy config file
        if [ -f "$SCRIPT_DIR/config.xml" ]; then
            cp "$SCRIPT_DIR/config.xml" "$tmp_mount/EFI/BOOT/config.xml"
            log "  copied config.xml to /EFI/BOOT/"
        fi
        # Copy app files
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
    local fw="$RUNTIME_DIR/$FIRMWARE_NAME"
    local fw_code="$RUNTIME_DIR/AAVMF_CODE.fd"
    local fw_vars="$RUNTIME_DIR/AAVMF_VARS.fd"

    # If the split firmware (AAVMF_CODE.fd + AAVMF_VARS.fd) is present, use it.
    if [ -f "$fw_code" ] && [ -f "$fw_vars" ]; then
        log "Firmware present (split AAVMF): $fw_code + $fw_vars"
        return 0
    fi
    # If a single QEMU_EFI.fd is present, we'll use that via -bios (handled below).
    if [ -f "$fw" ] && [ "$(stat -c %s "$fw" 2>/dev/null || stat -f %z "$fw")" -gt 1000000 ]; then
        log "Firmware present (single): $fw"
        return 0
    fi

    log "Downloading UEFI firmware ..."

    # Try a few sources in order.
    # Source 1: GitHub mirror of AAVMF (single QEMU_EFI.fd).
    local github_url="https://github.com/retroplasma/edk2-uefi-arm64/releases/download/r23/QEMU_EFI.fd"
    if curl -L --fail --silent --show-error -o "$fw.tmp" "$github_url"; then
        mv "$fw.tmp" "$fw"
        log "  -> $fw"
        return 0
    fi
    rm -f "$fw.tmp"

    # Source 2: Debian qemu-efi-aarch64 .deb (contains QEMU_EFI.fd).
    log "GitHub mirror failed; trying Debian qemu-efi-aarch64 .deb ..."
    local deb_url2
    deb_url2="$(apt-get download --print-uri qemu-efi-aarch64 2>/dev/null | head -1 || true)"
    if [ -z "$deb_url2" ]; then
        deb_url2="http://ftp.debian.org/debian/pool/main/e/edk2/qemu-efi-aarch64_2025.02-8+deb13u1_all.deb"
    fi
    log "  fetching $deb_url2"
    if curl -L --fail --silent --show-error -o "$RUNTIME_DIR/qemu-efi-aarch64.deb" "$deb_url2"; then
        (cd "$RUNTIME_DIR" && ar x qemu-efi-aarch64.deb data.tar.xz 2>/dev/null \
            && tar xf data.tar.xz ./usr/share/qemu-efi-aarch64/QEMU_EFI.fd 2>/dev/null \
            && mv ./usr/share/qemu-efi-aarch64/QEMU_EFI.fd "$FIRMWARE_NAME" \
            && rm -rf ./usr data.tar.xz qemu-efi-aarch64.deb)
        if [ -f "$fw" ]; then
            log "  -> $fw"
            return 0
        fi
    fi
    rm -f "$RUNTIME_DIR/qemu-efi-aarch64.deb" "$RUNTIME_DIR/data.tar.xz" "$fw.tmp"

    die "Could not download UEFI firmware. Manually place one of:
  $RUNTIME_DIR/$FIRMWARE_NAME            (single QEMU_EFI.fd, used with -bios)
  $RUNTIME_DIR/AAVMF_CODE.fd + AAVMF_VARS.fd  (split firmware, preferred)

You can install it via your package manager:
  Debian/Ubuntu : sudo apt install qemu-efi-aarch64
                  (file at /usr/share/qemu-efi-aarch64/QEMU_EFI.fd)
  macOS (brew)  : brew install qemu
                  (file at /opt/homebrew/share/qemu/edk2-aarch64-code.fd)
"
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
    local fw="$RUNTIME_DIR/$FIRMWARE_NAME"
    local fw_code="$RUNTIME_DIR/AAVMF_CODE.fd"
    local fw_vars="$RUNTIME_DIR/AAVMF_VARS.fd"
    local img="$RUNTIME_DIR/$IMAGE_NAME"
    [ -f "$img" ] || die "Disk image missing. Run './build.sh' first."

    # Determine which firmware layout to use.
    local use_split=0
    if [ -f "$fw_code" ] && [ -f "$fw_vars" ]; then
        use_split=1
    elif [ ! -f "$fw" ]; then
        die "Firmware missing. Run './build.sh' first."
    fi

    log "Launching QEMU ..."
    log "  machine : $QEMU_MACHINE"
    log "  cpu     : $QEMU_CPU"
    log "  ram     : $QEMU_RAM"
    if [ "$use_split" -eq 1 ]; then
        log "  firmware: $fw_code + $fw_vars (split AAVMF)"
    else
        log "  firmware: $fw (single QEMU_EFI.fd)"
    fi
    log "  disk    : $img"
    echo

    # Build the firmware args.
    local fw_args=()
    if [ "$use_split" -eq 1 ]; then
        fw_args+=(
            -drive "if=pflash,format=raw,readonly=on,file=$fw_code"
            -drive "if=pflash,format=raw,file=$fw_vars"
        )
    else
        fw_args+=(-bios "$fw")
    fi

    # QEMU args explanation:
    #   -machine virt            : ARM virt machine (matches Raspberry Pi UEFI class)
    #   -cpu cortex-a72          : 64-bit ARM core (same family as Pi 4)
    #   -m 1G                    : 1 GiB RAM
    #   -pflash CODE + VARS      : UEFI firmware (AAVMF) — modern split layout
    #                              (or -bios QEMU_EFI.fd for single-file firmware)
    #   -drive ...,format=raw    : FAT image as removable media (bootable)
    #   -device ramfb            : simple firmware framebuffer (works on every QEMU build)
    #                              alternative: -device virtio-gpu-device (better resolution,
    #                              needs virtio-gpu driver in firmware)
    #   -device qemu-xhci        : USB 3.0 host controller (required for usb-kbd / usb-mouse)
    #   -device usb-tablet       : USB absolute pointing device (exposed by AAVMF as the
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
        -machine "$QEMU_MACHINE" \
        -cpu "$QEMU_CPU" \
        -m "$QEMU_RAM" \
        "${fw_args[@]}" \
        -drive "if=none,file=$img,format=raw,id=hd0,cache=none" \
        -device "virtio-blk-device,drive=hd0" \
        -device "ramfb" \
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
