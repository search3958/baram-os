#!/bin/bash

# HAL OS Build Script
# Usage: ./bld.sh [arch]
#   arch: 64, 32, arm, arm64 (default: 64)

set -e

ARCH="${1:-64}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_DIR="$SCRIPT_DIR/src"
BUILD_DIR="$SCRIPT_DIR/build"
OUTPUT_NAME="hal_os_$ARCH.bin"

echo "=========================================="
echo "HAL OS Build System"
echo "Architecture: $ARCH"
echo "=========================================="

# Create build directory
mkdir -p "$BUILD_DIR"

# Set compiler and flags based on architecture
case "$ARCH" in
    64|x86_64)
        CC="x86_64-elf-gcc"
        LD="x86_64-elf-ld"
        ASM="x86_64-elf-as"
        ARCH_DIR="x86_64"
        CFLAGS="-m64 -ffreestanding -fno-stack-protector -nostdlib -O2 -Wall -Wextra"
        LDFLAGS="-m elf_x86_64 -Ttext 0x100000"
        ;;
    32|i386|i686)
        CC="i686-elf-gcc"
        LD="i686-elf-ld"
        ASM="i686-elf-as"
        ARCH_DIR="i386"
        CFLAGS="-m32 -ffreestanding -fno-stack-protector -nostdlib -O2 -Wall -Wextra"
        LDFLAGS="-m elf_i386 -Ttext 0x100000"
        ;;
    arm|arm32)
        # Try multiple possible cross-compiler names
        if command -v arm-none-eabi-gcc &> /dev/null; then
            CC="arm-none-eabi-gcc"
            LD="arm-none-eabi-ld"
        elif command -v arm-linux-gnueabihf-gcc &> /dev/null; then
            CC="arm-linux-gnueabihf-gcc"
            LD="arm-linux-gnueabihf-ld"
        else
            CC="gcc"  # Fallback to native gcc
            LD="ld"
        fi
        ARCH_DIR="arm"
        CFLAGS="-march=armv7-a -mfpu=vfp -mfloat-abi=softfp -ffreestanding -fno-stack-protector -nostdlib -O2 -Wall -Wextra"
        LDFLAGS="-Ttext 0x8000"
        ;;
    arm64|aarch64)
        # Try multiple possible cross-compiler names
        if command -v aarch64-elf-gcc &> /dev/null; then
            CC="aarch64-elf-gcc"
            LD="aarch64-elf-ld"
        elif command -v aarch64-linux-gnu-gcc &> /dev/null; then
            CC="aarch64-linux-gnu-gcc"
            LD="aarch64-linux-gnu-ld"
        else
            CC="gcc"  # Fallback to native gcc (won't work for actual ARM but allows build)
            LD="ld"
        fi
        ARCH_DIR="arm64"
        CFLAGS="-ffreestanding -fno-stack-protector -nostdlib -O2 -Wall -Wextra"
        LDFLAGS="-Ttext 0x80000"
        ;;
    *)
        echo "Unknown architecture: $ARCH"
        echo "Supported architectures: 64, 32, arm, arm64"
        exit 1
        ;;
esac

echo "Using compiler: $CC"
echo "Build directory: $BUILD_DIR"
echo ""

# Check if cross-compiler exists
if ! command -v "$CC" &> /dev/null; then
    echo "Warning: $CC not found. Attempting to use system gcc with appropriate flags..."
    case "$ARCH" in
        64|x86_64)
            CC="gcc"
            LD="ld"
            CFLAGS="-m64 -ffreestanding -fno-stack-protector -nostdlib -O2 -Wall -Wextra"
            LDFLAGS="-m elf_x86_64 -Ttext 0x100000"
            ;;
        32|i386|i686)
            CC="gcc"
            LD="ld"
            CFLAGS="-m32 -ffreestanding -fno-stack-protector -nostdlib -O2 -Wall -Wextra"
            LDFLAGS="-m elf_i386 -Ttext 0x100000"
            ;;
        *)
            echo "Error: No suitable compiler found for $ARCH"
            exit 1
            ;;
    esac
fi

# Compile HAL
echo "[1/4] Compiling HAL..."
$CC $CFLAGS -c -I"$SRC_DIR/hal" -I"$SRC_DIR/arch/$ARCH_DIR" \
    "$SRC_DIR/hal/hal.c" \
    -o "$BUILD_DIR/hal.o"

# Compile architecture-specific HAL
echo "[2/4] Compiling $ARCH HAL..."
$CC $CFLAGS -c -I"$SRC_DIR/hal" -I"$SRC_DIR/arch/$ARCH_DIR" \
    "$SRC_DIR/arch/$ARCH_DIR/hal.c" \
    -o "$BUILD_DIR/arch_hal.o"

# Compile kernel
echo "[3/4] Compiling kernel..."
$CC $CFLAGS -c -I"$SRC_DIR/boot" -I"$SRC_DIR/hal" -I"$SRC_DIR/arch/$ARCH_DIR" \
    "$SRC_DIR/boot/kernel.c" \
    -o "$BUILD_DIR/kernel.o"

# Assemble bootloader
echo "[4/4] Assembling bootloader..."
if [[ "$ARCH" == "arm" || "$ARCH" == "arm64" ]]; then
    $CC $CFLAGS -c "$SRC_DIR/arch/$ARCH_DIR/boot.S" -o "$BUILD_DIR/boot.o"
else
    $CC $CFLAGS -c "$SRC_DIR/arch/$ARCH_DIR/boot.S" -o "$BUILD_DIR/boot.o"
fi

# Link everything
echo "Linking..."
$LD $LDFLAGS \
    "$BUILD_DIR/boot.o" \
    "$BUILD_DIR/hal.o" \
    "$BUILD_DIR/arch_hal.o" \
    "$BUILD_DIR/kernel.o" \
    -o "$BUILD_DIR/hal_os_$ARCH.elf"

# Create binary
echo "Creating binary image..."
objcopy -O binary "$BUILD_DIR/hal_os_$ARCH.elf" "$BUILD_DIR/$OUTPUT_NAME"

echo ""
echo "=========================================="
echo "Build complete!"
echo "Output: $BUILD_DIR/$OUTPUT_NAME"
echo "=========================================="
echo ""
echo "To run with QEMU:"
case "$ARCH" in
    64|x86_64)
        echo "  qemu-system-x86_64 -kernel $BUILD_DIR/$OUTPUT_NAME"
        ;;
    32|i386|i686)
        echo "  qemu-system-i386 -kernel $BUILD_DIR/$OUTPUT_NAME"
        ;;
    arm|arm32)
        echo "  qemu-system-arm -kernel $BUILD_DIR/$OUTPUT_NAME -M virt"
        ;;
    arm64|aarch64)
        echo "  qemu-system-aarch64 -kernel $BUILD_DIR/$OUTPUT_NAME -M virt"
        ;;
esac
