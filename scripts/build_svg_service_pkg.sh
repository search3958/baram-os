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
BUILD_DIR="$OUTDIR/svg_service_build"
PKG_DIR="$OUTDIR/pkg_svg_service"
PKG_FILE="$OUTDIR/svg_service.pkg"
MODULE_FILE="$BUILD_DIR/svg_service.ko"

rm -rf "$BUILD_DIR" "$PKG_DIR" "$PKG_FILE"
mkdir -p "$BUILD_DIR"

bash "$ROOT/scripts/build_lunasvg.sh" "$TARGET" "$BUILD_DIR"

case "$TARGET" in
    x86_64-elf) LD_TOOL="$(command -v x86_64-elf-ld || true)" ;;
    i686-elf) LD_TOOL="$(command -v i686-elf-ld || true)" ;;
    aarch64-elf) LD_TOOL="$(command -v aarch64-elf-ld || true)" ;;
    *) LD_TOOL="" ;;
esac
if [ -z "$LD_TOOL" ]; then
    if command -v ld.lld >/dev/null 2>&1; then
        LD_TOOL="$(command -v ld.lld)"
    elif [ -x "/opt/homebrew/bin/ld.lld" ]; then
        LD_TOOL="/opt/homebrew/bin/ld.lld"
    else
        echo "svg_service: no relocatable linker for $TARGET" >&2
        exit 1
    fi
fi

"$LD_TOOL" -r -o "$MODULE_FILE" "$BUILD_DIR"/lunasvg_*.o

mkdir -p "$PKG_DIR/module"
cp "$MODULE_FILE" "$PKG_DIR/module/svg_service.ko"

cat > "$PKG_DIR/manifest.json" <<EOF
{
  "name": "svg_service",
  "type": "service",
  "abi": 1,
  "entry": "module/svg_service.ko",
  "optional": true,
  "provides": ["svg.decode", "svg.rasterize"],
  "payload": ["module/svg_service.ko"]
}
EOF

cat > "$PKG_DIR/README.txt" <<EOF
svg_service package payload.
lunaSVG and plutovg objects live only inside this pkg in shipped images.
The kernel loads module/svg_service.ko from this package at boot.
EOF

(cd "$PKG_DIR" && tar -cf "$PKG_FILE" .)
rm -rf "$BUILD_DIR" "$PKG_DIR"
