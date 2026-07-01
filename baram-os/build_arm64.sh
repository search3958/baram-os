#!/bin/bash
# Build script for ARM64 UEFI application
# License: MIT License

set -e

ARCH="arm64"
TARGET="aarch64-linux-gnu"
BUILD_DIR="build"
OUTPUT_NAME="BOOTAA64.EFI"

echo "Building Baram OS for $ARCH..."

mkdir -p $BUILD_DIR/$ARCH

if command -v ${TARGET}-gcc &> /dev/null; then
    CC=${TARGET}-gcc
    LD=${TARGET}-ld
    OBJCOPY=${TARGET}-objcopy
else
    if command -v clang &> /dev/null; then
        CC="clang --target=aarch64-linux-gnu"
        LD="ld.lld"
        OBJCOPY="llvm-objcopy"
    else
        echo "Error: No suitable cross-compiler found."
        echo "Please install gcc-aarch64-linux-gnu or clang."
        echo "On macOS: brew install aarch64-linux-gnu"
        exit 1
    fi
fi

echo "Using compiler: $CC"

echo "Compiling kernel..."
$CC -c -ffreestanding -fno-stack-protector -fno-pic -mgeneral-regs-only \
    -I include -I arch/$ARCH \
    src/kernel.c -o $BUILD_DIR/$ARCH/kernel.o

echo "Compiling architecture-specific code..."
$CC -c -ffreestanding -fno-stack-protector -fno-pic -mgeneral-regs-only \
    -I include -I arch/$ARCH \
    arch/$ARCH/graphics.c -o $BUILD_DIR/$ARCH/graphics.o

$CC -c -ffreestanding -fno-stack-protector -fno-pic -mgeneral-regs-only \
    -I include -I arch/$ARCH \
    arch/$ARCH/keyboard.c -o $BUILD_DIR/$ARCH/keyboard.o

$CC -c -ffreestanding -fno-stack-protector -fno-pic -mgeneral-regs-only \
    -I include -I arch/$ARCH \
    arch/$ARCH/mouse.c -o $BUILD_DIR/$ARCH/mouse.o

echo "Compiling EFI application..."
$CC -c -ffreestanding -fno-stack-protector -fno-pic -mgeneral-regs-only \
    -I include -I arch/$ARCH \
    efi/boot_arm64.c -o $BUILD_DIR/$ARCH/boot.o

echo "Linking..."
$LD -T arch/$ARCH/linker.ld -nostdlib -o $BUILD_DIR/$ARCH/kernel.elf \
    $BUILD_DIR/$ARCH/boot.o \
    $BUILD_DIR/$ARCH/kernel.o \
    $BUILD_DIR/$ARCH/graphics.o \
    $BUILD_DIR/$ARCH/keyboard.o \
    $BUILD_DIR/$ARCH/mouse.o

echo "Creating EFI executable..."
$objcopy -O pei-aarch64-little-efi-app $BUILD_DIR/$ARCH/kernel.elf $BUILD_DIR/$ARCH/$OUTPUT_NAME

echo "Build complete!"
echo "Output: $BUILD_DIR/$ARCH/$OUTPUT_NAME"

if command -v qemu-system-aarch64 &> /dev/null; then
    echo "Launching QEMU..."
    qemu-system-aarch64 \
        -bios QEMU_EFI.fd \
        -drive format=raw,file=fat:rw:$BUILD_DIR \
        -m 512M \
        -cpu cortex-a57
else
    echo "QEMU not found. Install it to auto-launch the emulator."
    echo "On macOS: brew install qemu"
fi
