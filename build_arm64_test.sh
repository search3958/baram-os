#!/bin/bash
set -e
mkdir -p output
LUNASVG_OBJECTS=""
BN_FILE=".build_no"
if [ ! -f "$BN_FILE" ]; then echo "0" > "$BN_FILE"; fi
CURRENT_BN=$(cat "$BN_FILE")
echo "#define BUILD_NUMBER $CURRENT_BN" > build_no.h

if [ -x "/opt/homebrew/bin/aarch64-elf-gcc" ] && [ -x "/opt/homebrew/bin/aarch64-elf-as" ] && [ -x "/opt/homebrew/bin/aarch64-elf-ld" ]; then
    CC="/opt/homebrew/bin/aarch64-elf-gcc"
    AS="/opt/homebrew/bin/aarch64-elf-as"
    LD="/opt/homebrew/bin/aarch64-elf-ld"
else
    CC="clang --target=aarch64-elf"
    AS="clang --target=aarch64-elf"
    LD="ld.lld"
fi

echo "Assembling arch files..."
$AS -c arch/boot_arm64.s -o output/boot.o
$AS -c arch/isr_arm64.s -o output/isr.o
$AS -c arch/setjmp_arm64.s -o output/setjmp.o

COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -nostdlib -fno-stack-protector -fno-pic -DBUILD_NUMBER=$CURRENT_BN"
LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89   -Ilua-master"

echo "Compiling C files..."
$CC $COMMON_CFLAGS -c kernel.c -o output/kernel.o
$CC $COMMON_CFLAGS -c drivers.c -o output/drivers.o
$CC $COMMON_CFLAGS -c storage.c -o output/storage.o
$CC $COMMON_CFLAGS -c fs.c -o output/fs.o
$CC $COMMON_CFLAGS -c ui/warp_engine.c -o output/warp_engine.o
$CC $COMMON_CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o
$CC $COMMON_CFLAGS -c gpu/gpu_driver.c -o output/gpu_driver.o
$CC $COMMON_CFLAGS -c gpu/gpu_blur.c -o output/gpu_blur.o
$CC $LUA_CFLAGS -c lua_impl.c -o output/lua.o
$CC $LUA_CFLAGS -c lua_glue.c -o output/lua_glue.o
bash scripts/build_lunasvg.sh aarch64-elf output
LUNASVG_OBJECTS="$(cat output/lunasvg_objects.list)"

echo "Preparing initrd..."
INITRD_DIR="output/initrd_tmp"
rm -rf "$INITRD_DIR"
mkdir -p "$INITRD_DIR"
cp ui/*.warp ui/*.warpc ui/*.svg ui/*.lua "$INITRD_DIR/" 2>/dev/null || true
[ -f "font/HarmonyOS_Sans_Regular.ttf" ] && cp font/HarmonyOS_Sans_Regular.ttf "$INITRD_DIR/"
[ -f "bootlogo.svg" ] && cp bootlogo.svg "$INITRD_DIR/"
[ -f "os_settings.json" ] && cp os_settings.json "$INITRD_DIR/"
(cd "$INITRD_DIR" && tar -cf ../initrd.tar *)
rm -rf "$INITRD_DIR"

cat <<EOF > output/initrd.s
.section .rodata
.global _binary_initrd_bin_start
.global _binary_initrd_bin_size
.align 8
_binary_initrd_bin_start:
    .incbin "output/initrd.tar"
_binary_initrd_bin_end:
_binary_initrd_bin_size:
    .quad _binary_initrd_bin_end - _binary_initrd_bin_start
EOF
$AS -c output/initrd.s -o output/initrd.o

echo "Linking..."
$LD -T link_arm64.ld -o output/kernel.bin \
    output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/gpu_driver.o output/gpu_blur.o output/lua.o output/lua_glue.o output/initrd.o $LUNASVG_OBJECTS

echo "ARM64 Build Success: output/kernel.bin created."
ls -l output/kernel.bin
