#!/bin/bash
# ============================================================
# BaramOS x86_64 Build Test Script (macOS / Linux 両対応)
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
LD_CMD=""

if command -v x86_64-elf-gcc >/dev/null 2>&1; then
    CC="x86_64-elf-gcc"
    LD_CMD="x86_64-elf-gcc"
elif command -v clang >/dev/null 2>&1; then
    CC="clang --target=x86_64-elf"
    if command -v ld.lld >/dev/null 2>&1; then
        LD_CMD="ld.lld"
    elif command -v lld >/dev/null 2>&1; then
        LD_CMD="lld"
    fi
elif command -v gcc >/dev/null 2>&1; then
    CC="gcc"
    LD_CMD="gcc"
else
    echo "❌ コンパイラが見つかりません"
    exit 1
fi

if [ -z "$LD_CMD" ]; then
    echo "❌ リンカが見つかりません"
    exit 1
fi

echo "🔧 コンパイラ: $CC"
echo "🔗 リンカ: $LD_CMD"

AS="nasm -f elf64"

echo "Assembling arch files..."
$AS arch/boot64.s -o output/boot.o
$AS arch/isr64.s -o output/isr.o
$AS arch/setjmp64.s -o output/setjmp.o

COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m64 -mno-red-zone -mcmodel=kernel -fno-pic -fno-pie -DBUILD_NUMBER=$CURRENT_BN"
KERNEL_CFLAGS="$COMMON_CFLAGS -msse2"
LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89 -Ilua-master"

echo "Compiling C files..."
$CC $KERNEL_CFLAGS -c kernel.c -o output/kernel.o
$CC $COMMON_CFLAGS -c drivers.c -o output/drivers.o
$CC $COMMON_CFLAGS -c storage.c -o output/storage.o
$CC $COMMON_CFLAGS -c fs.c -o output/fs.o
$CC $COMMON_CFLAGS -c ui/warp_engine.c -o output/warp_engine.o
$CC $COMMON_CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o
$CC $COMMON_CFLAGS -c gpu/gpu_driver.c -o output/gpu_driver.o
$CC $COMMON_CFLAGS -c gpu/gpu_blur.c -o output/gpu_blur.o
$CC $COMMON_CFLAGS -c gpu/gpu_svg.c -o output/gpu_svg.o
$CC $COMMON_CFLAGS -Igpu/libtess2/Include -c gpu/libtess2/Source/bucketalloc.c -o output/tess_bucketalloc.o
$CC $COMMON_CFLAGS -Igpu/libtess2/Include -c gpu/libtess2/Source/dict.c -o output/tess_dict.o
$CC $COMMON_CFLAGS -Igpu/libtess2/Include -c gpu/libtess2/Source/geom.c -o output/tess_geom.o
$CC $COMMON_CFLAGS -Igpu/libtess2/Include -c gpu/libtess2/Source/mesh.c -o output/tess_mesh.o
$CC $COMMON_CFLAGS -Igpu/libtess2/Include -c gpu/libtess2/Source/priorityq.c -o output/tess_priorityq.o
$CC $COMMON_CFLAGS -Igpu/libtess2/Include -c gpu/libtess2/Source/sweep.c -o output/tess_sweep.o
$CC $COMMON_CFLAGS -Igpu/libtess2/Include -c gpu/libtess2/Source/tess.c -o output/tess_tess.o
$CC $LUA_CFLAGS -c lua_impl.c -o output/lua.o
$CC $LUA_CFLAGS -c lua_glue.c -o output/lua_glue.o

echo "Linking..."
if [ "$LD_CMD" = "ld.lld" ] || [ "$LD_CMD" = "lld" ]; then
    $LD_CMD -T link64.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/gpu_driver.o output/gpu_blur.o output/gpu_svg.o output/tess_bucketalloc.o output/tess_dict.o output/tess_geom.o output/tess_mesh.o output/tess_priorityq.o output/tess_sweep.o output/tess_tess.o output/lua.o output/lua_glue.o
else
    $LD_CMD -T link64.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/gpu_driver.o output/gpu_blur.o output/gpu_svg.o output/tess_bucketalloc.o output/tess_dict.o output/tess_geom.o output/tess_mesh.o output/tess_priorityq.o output/tess_sweep.o output/tess_tess.o output/lua.o output/lua_glue.o \
        -ffreestanding -nostdlib -static-libgcc -lgcc
fi

echo "✅ x86_64 Build Success: output/kernel.bin created."
ls -l output/kernel.bin
