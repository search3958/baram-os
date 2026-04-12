#!/bin/bash
# ============================================================
# BaramOS i686 Build Test Script (macOS / Linux 両対応)
# ============================================================
set -e

# OS 判定
detect_os() {
    case "$(uname -s)" in
        Darwin*)  OS="macos" ;;
        Linux*)   OS="linux" ;;
        *)        OS="unknown" ;;
    esac
}
detect_os

if [ "$OS" = "unknown" ]; then
    echo "❌ サポートされていないOSです"
    exit 1
fi

echo "📱 OS: $OS ($(uname -m))"

mkdir -p output
BN_FILE=".build_no"
if [ ! -f "$BN_FILE" ]; then echo "0" > "$BN_FILE"; fi
CURRENT_BN=$(cat "$BN_FILE")
echo "#define BUILD_NUMBER $CURRENT_BN" > build_no.h

# ツールチェーン自動選択
CC=""
if command -v x86_64-elf-gcc >/dev/null 2>&1; then
    CC="x86_64-elf-gcc -m32"
elif command -v i686-elf-gcc >/dev/null 2>&1; then
    CC="i686-elf-gcc"
elif command -v clang >/dev/null 2>&1; then
    CC="clang --target=i686-elf"
else
    echo "❌ コンパイラが見つかりません"
    exit 1
fi

echo "🔧 コンパイラ: $CC"

AS="nasm -f elf32"
LD="$CC"

echo "Assembling arch files..."
$AS arch/boot.s -o output/boot.o
$AS arch/isr.s -o output/isr.o

CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m32 -march=pentium4 -mno-sse -mno-sse2 -mstackrealign -DBUILD_NUMBER=$CURRENT_BN"
KERNEL_CFLAGS="$CFLAGS -msse2"
LUA_CFLAGS="$CFLAGS -DLUA_USE_C89 -Ilua-master"

echo "Compiling C files..."
$CC $KERNEL_CFLAGS -c kernel.c -o output/kernel.o
$CC $CFLAGS -c drivers.c -o output/drivers.o
$CC $CFLAGS -c storage.c -o output/storage.o
$CC $CFLAGS -c fs.c -o output/fs.o
$CC $CFLAGS -c ui/warp_engine.c -o output/warp_engine.o
$CC $CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o
$CC $LUA_CFLAGS -c lua_impl.c -o output/lua.o
$CC $LUA_CFLAGS -c lua_glue.c -o output/lua_glue.o

echo "Linking..."
$LD -T link.ld -o output/kernel.bin \
    output/boot.o output/isr.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/lua.o output/lua_glue.o \
    -ffreestanding -O2 -m32 -nostdlib -static-libgcc -lgcc

echo "✅ i686 Build Success: output/kernel.bin created."
ls -l output/kernel.bin
