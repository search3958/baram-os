#!/bin/bash
# Build script for x86_64 UEFI application
# License: MIT License

set -e

ARCH="x86_64"
TARGET="x86_64-w64-mingw32"
BUILD_DIR="build"
OUTPUT_NAME="BOOTX64.EFI"

echo "Building Baram OS for $ARCH..."

# Create build directory
mkdir -p $BUILD_DIR/$ARCH

# Use clang with lld for macOS compatibility
if command -v clang &> /dev/null; then
    CC="clang --target=${TARGET}"
    LD="lld-link"
    OBJCOPY="llvm-objcopy"
elif command -v ${TARGET}-gcc &> /dev/null; then
    CC=${TARGET}-gcc
    LD=${TARGET}-ld
    OBJCOPY=${TARGET}-objcopy
else
    echo "Error: No suitable cross-compiler found."
    echo "Please install mingw-w64 or clang."
    echo "On macOS: brew install llvm mingw-w64"
    exit 1
fi

echo "Using compiler: $CC"

# Compile kernel sources
echo "Compiling kernel..."
$CC -c -ffreestanding -fno-stack-protector -fno-pic -mno-red-zone \
    -I include -I arch/$ARCH \
    src/kernel.c -o $BUILD_DIR/$ARCH/kernel.o

# Compile architecture-specific sources
echo "Compiling architecture-specific code..."
$CC -c -ffreestanding -fno-stack-protector -fno-pic -mno-red-zone \
    -I include -I arch/$ARCH \
    arch/$ARCH/graphics.c -o $BUILD_DIR/$ARCH/graphics.o

$CC -c -ffreestanding -fno-stack-protector -fno-pic -mno-red-zone \
    -I include -I arch/$ARCH \
    arch/$ARCH/keyboard.c -o $BUILD_DIR/$ARCH/keyboard.o

$CC -c -ffreestanding -fno-stack-protector -fno-pic -mno-red-zone \
    -I include -I arch/$ARCH \
    arch/$ARCH/mouse.c -o $BUILD_DIR/$ARCH/mouse.o

# Compile EFI application
echo "Compiling EFI application..."
$CC -c -ffreestanding -fno-stack-protector -fno-pic -mno-red-zone \
    -I include -I arch/$ARCH \
    efi/boot_x86_64.c -o $BUILD_DIR/$ARCH/boot.o

# Link all objects using lld-link (PE/COFF format directly)
echo "Linking..."
if command -v lld-link &> /dev/null; then
    lld-link /machine:x64 /subsystem:efi_application \
        /out:$BUILD_DIR/$ARCH/$OUTPUT_NAME \
        $BUILD_DIR/$ARCH/boot.o \
        $BUILD_DIR/$ARCH/kernel.o \
        $BUILD_DIR/$ARCH/graphics.o \
        $BUILD_DIR/$ARCH/keyboard.o \
        $BUILD_DIR/$ARCH/mouse.o \
        /entry:efi_main /nodefaultlib
else
    # Fallback to traditional linking
    $LD -T arch/$ARCH/linker.ld -nostdlib -o $BUILD_DIR/$ARCH/kernel.elf \
        $BUILD_DIR/$ARCH/boot.o \
        $BUILD_DIR/$ARCH/kernel.o \
        $BUILD_DIR/$ARCH/graphics.o \
        $BUILD_DIR/$ARCH/keyboard.o \
        $BUILD_DIR/$ARCH/mouse.o
    
    # Convert to PE/COFF format for UEFI
    echo "Creating EFI executable..."
    $OBJCOPY -O pei-x86-64 $BUILD_DIR/$ARCH/kernel.elf $BUILD_DIR/$ARCH/$OUTPUT_NAME
fi

echo "Build complete!"
echo "Output: $BUILD_DIR/$ARCH/$OUTPUT_NAME"

# Auto-launch QEMU if available
if command -v qemu-system-x86_64 &> /dev/null; then
    echo "Launching QEMU..."
    # Check for OVMF firmware
    if [ -f "/usr/share/OVMF/OVMF.fd" ]; then
        OVMF_PATH="/usr/share/OVMF/OVMF.fd"
    elif [ -f "/usr/share/qemu/OVMF.fd" ]; then
        OVMF_PATH="/usr/share/qemu/OVMF.fd"
    elif [ -f "OVMF.fd" ]; then
        OVMF_PATH="OVMF.fd"
    else
        echo "Warning: OVMF.fd not found. Please install OVMF or download it."
        echo "On macOS: brew install ovmf"
        OVMF_PATH=""
    fi
    
    if [ -n "$OVMF_PATH" ]; then
        qemu-system-x86_64 \
            -bios "$OVMF_PATH" \
            -drive format=raw,file=fat:rw:$BUILD_DIR \
            -m 512M
    else
        qemu-system-x86_64 \
            -drive format=raw,file=fat:rw:$BUILD_DIR \
            -m 512M
    fi
else
    echo "QEMU not found. Install it to auto-launch the emulator."
    echo "On macOS: brew install qemu"
fi
