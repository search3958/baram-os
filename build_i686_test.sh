#!/bin/bash
set -e
mkdir -p output
BN_FILE=".build_no"
if [ ! -f "$BN_FILE" ]; then echo "0" > "$BN_FILE"; fi
CURRENT_BN=$(cat "$BN_FILE")
echo "#define BUILD_NUMBER $CURRENT_BN" > build_no.h

CC="i686-elf-gcc"
AS="nasm -f elf32"
LD="i686-elf-gcc"

if [ -x "/opt/homebrew/bin/nasm" ]; then
    AS="/opt/homebrew/bin/nasm -f elf32"
fi

if [ -x "/opt/homebrew/bin/i686-elf-gcc" ]; then
    CC="/opt/homebrew/bin/i686-elf-gcc"
    LD="/opt/homebrew/bin/i686-elf-gcc"
fi

echo "Assembling arch files..."
$AS arch/boot.s -o output/boot.o
$AS arch/isr.s -o output/isr.o
$AS arch/setjmp.s -o output/setjmp.o

CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m32 -march=pentium4 -mno-sse -mno-sse2 -mstackrealign -DBUILD_NUMBER=$CURRENT_BN"
KERNEL_CFLAGS="$CFLAGS -msse2"
LUA_CFLAGS="$CFLAGS -DLUA_USE_C89   -Ilua-master"

echo "Compiling C files..."
$CC $KERNEL_CFLAGS -c kernel.c -o output/kernel.o
$CC $CFLAGS -c drivers.c -o output/drivers.o
$CC $CFLAGS -c storage.c -o output/storage.o
$CC $CFLAGS -c fs.c -o output/fs.o
$CC $CFLAGS -c ui/warp_engine.c -o output/warp_engine.o
$CC $CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o
$CC $CFLAGS -c ui/warp_draw.c -o output/warp_draw.o
$CC $CFLAGS -c gpu/gpu_driver.c -o output/gpu_driver.o
$CC $CFLAGS -c gpu/gpu_blur.c -o output/gpu_blur.o
$CC $LUA_CFLAGS -c lua_impl.c -o output/lua.o
$CC $LUA_CFLAGS -c lua_glue.c -o output/lua_glue.o
bash scripts/build_svg_service_pkg.sh i686-elf output

echo "Linking..."
$LD -T link.ld -o output/kernel.bin \
    output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/warp_draw.o output/gpu_driver.o output/gpu_blur.o output/lua.o output/lua_glue.o \
    -ffreestanding -O2 -m32 -nostdlib -static-libgcc -lgcc
rm -f output/svg_service.pkg

echo "i686 Build Success: output/kernel.bin created."
ls -l output/kernel.bin
