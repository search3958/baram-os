#!/bin/bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <target-triple> <output-dir>" >&2
    exit 1
fi

TARGET="$1"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTDIR="$2"
mkdir -p "$OUTDIR"
OUTDIR="$(cd "$OUTDIR" && pwd)"
BUILD_DIR="$OUTDIR/warp_draw_service_build"
PKG_DIR="$OUTDIR/pkg_warp_draw_service"
PKG_FILE="$OUTDIR/warp_draw_service.pkg"
MODULE_FILE="$BUILD_DIR/warp_draw_service.ko"

rm -rf "$BUILD_DIR" "$PKG_DIR" "$PKG_FILE"
mkdir -p "$BUILD_DIR"

case "$TARGET" in
    x86_64-elf)
        CC_TOOL="$(command -v x86_64-elf-gcc || true)"
        LD_TOOL="$(command -v x86_64-elf-ld || true)"
        CFLAGS="-I$ROOT/include -I$ROOT -I$ROOT/ui -ffreestanding -O2 -Wall -Wno-unused-function -m64 -mno-red-zone -mcmodel=kernel -fno-pic -fno-pie"
        ;;
    i686-elf)
        CC_TOOL="$(command -v i686-elf-gcc || true)"
        LD_TOOL="$(command -v i686-elf-ld || true)"
        CFLAGS="-I$ROOT/include -I$ROOT -I$ROOT/ui -ffreestanding -O2 -Wall -Wno-unused-function -m32 -march=pentium4 -mno-sse -mno-sse2 -mstackrealign"
        ;;
    aarch64-elf)
        CC_TOOL="$(command -v aarch64-elf-gcc || true)"
        LD_TOOL="$(command -v aarch64-elf-ld || true)"
        CFLAGS="-I$ROOT/include -I$ROOT -I$ROOT/ui -ffreestanding -O2 -Wall -Wno-unused-function"
        ;;
    *)
        echo "warp_draw_service: unsupported target $TARGET" >&2
        exit 1
        ;;
esac

if [ -z "$CC_TOOL" ] && command -v clang >/dev/null 2>&1; then
    CC_TOOL="$(command -v clang) --target=$TARGET"
fi
if [ -z "$LD_TOOL" ]; then
    if command -v ld.lld >/dev/null 2>&1; then
        LD_TOOL="$(command -v ld.lld)"
    elif [ -x "/opt/homebrew/bin/ld.lld" ]; then
        LD_TOOL="/opt/homebrew/bin/ld.lld"
    else
        echo "warp_draw_service: no relocatable linker for $TARGET" >&2
        exit 1
    fi
fi
if [ -z "$CC_TOOL" ]; then
    echo "warp_draw_service: no compiler for $TARGET" >&2
    exit 1
fi

$CC_TOOL $CFLAGS -c "$ROOT/ui/warp_draw.c" -o "$BUILD_DIR/warp_draw.o"
"$LD_TOOL" -r -o "$MODULE_FILE" "$BUILD_DIR/warp_draw.o"

mkdir -p "$PKG_DIR/module"
cp "$MODULE_FILE" "$PKG_DIR/module/warp_draw_service.ko"

cat > "$PKG_DIR/manifest.json" <<EOF
{
  "name": "warp_draw_service",
  "type": "service",
  "abi": 1,
  "entry": "module/warp_draw_service.ko",
  "optional": true,
  "provides": ["warp.draw.native"],
  "payload": ["module/warp_draw_service.ko"]
}
EOF

cat > "$PKG_DIR/README.txt" <<EOF
warp_draw_service package payload.
This package provides the native Warp non-text shape rasterizer.
The kernel can select it with os_settings.json: "warpRenderer": "native-package".
EOF

(cd "$PKG_DIR" && tar -cf "$PKG_FILE" .)
rm -rf "$BUILD_DIR" "$PKG_DIR"
