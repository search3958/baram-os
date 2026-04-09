#!/bin/bash
set -e
mkdir -p output
BN_FILE=".build_no"
if [ ! -f "$BN_FILE" ]; then echo "0" > "$BN_FILE"; fi
CURRENT_BN=$(cat "$BN_FILE")
echo "#define BUILD_NUMBER $CURRENT_BN" > build_no.h

CC="clang --target=x86_64-elf"
AS="nasm -f elf64"
LD="ld.lld"

echo "Assembling arch files..."
$AS arch/boot64.s -o output/boot.o
$AS arch/isr64.s -o output/isr.o
$AS arch/setjmp64.s -o output/setjmp.o

COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m64 -mno-red-zone -mcmodel=kernel -fno-pic -fno-pie -DBUILD_NUMBER=$CURRENT_BN"
KERNEL_CFLAGS="$COMMON_CFLAGS -msse2"
LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89   -Ilua-master"

echo "Compiling C files..."
$CC $KERNEL_CFLAGS -c kernel.c -o output/kernel.o
$CC $COMMON_CFLAGS -c drivers.c -o output/drivers.o
$CC $COMMON_CFLAGS -c storage.c -o output/storage.o
$CC $COMMON_CFLAGS -c fs.c -o output/fs.o
$CC $COMMON_CFLAGS -c ui/warp_engine.c -o output/warp_engine.o
$CC $COMMON_CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o
$CC $COMMON_CFLAGS -c gpu/gpu_driver.c -o output/gpu_driver.o
$CC $COMMON_CFLAGS -c gpu/gpu_blur.c -o output/gpu_blur.o
$CC $LUA_CFLAGS -c lua_impl.c -o output/lua.o
$CC $LUA_CFLAGS -c lua_glue.c -o output/lua_glue.o

echo "Linking..."
$LD -T link64.ld -o output/kernel.bin \
    output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/gpu_driver.o output/gpu_blur.o output/lua.o output/lua_glue.o

echo "x86_64 Build Success: output/kernel.bin created."
ls -l output/kernel.bin
