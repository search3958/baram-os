#!/bin/bash

# Baram OS Lazward Build Script
# Usage: ./bld.sh [arch]
#   arch: 64, 32, arm, arm64 (default: 64)

set -e

ARCH="${1:-64}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_DIR="$SCRIPT_DIR/src"
BUILD_DIR="$SCRIPT_DIR/build"
OUTPUT_NAME="baram_os_$ARCH.bin"

echo "=========================================="
echo "Baram OS Lazward Build System"
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
# For x86_64, we need to ensure the multiboot header is at the beginning of the binary
if [[ "$ARCH" == "64" || "$ARCH" == "x86_64" ]]; then
    # Create a linker script that places .multiboot_header section first
    cat > "$BUILD_DIR/linker.ld" << 'EOF'
ENTRY(_start)

SECTIONS
{
    . = 0x100000;
    
    .multiboot_header ALIGN(8) :
    {
        *(.multiboot_header)
    }
    
    .text ALIGN(4K) :
    {
        *(.text)
    }
    
    .rodata ALIGN(4K) :
    {
        *(.rodata)
    }
    
    .data ALIGN(4K) :
    {
        *(.data)
    }
    
    .bss ALIGN(4K) :
    {
        *(.bss)
        *(COMMON)
    }
}
EOF
    LDFLAGS="-T $BUILD_DIR/linker.ld -m elf_x86_64"
fi

$LD $LDFLAGS \
    "$BUILD_DIR/boot.o" \
    "$BUILD_DIR/hal.o" \
    "$BUILD_DIR/arch_hal.o" \
    "$BUILD_DIR/kernel.o" \
    -o "$BUILD_DIR/baram_os_$ARCH.elf"

# Create binary
echo "Creating binary image..."
# Use architecture-specific objcopy if available
if [[ "$ARCH" == "arm64" || "$ARCH" == "aarch64" ]]; then
    OBJCOPY="aarch64-linux-gnu-objcopy"
elif [[ "$ARCH" == "arm" || "$ARCH" == "arm32" ]]; then
    OBJCOPY="arm-linux-gnueabihf-objcopy"
elif [[ "$ARCH" == "64" || "$ARCH" == "x86_64" ]]; then
    OBJCOPY="x86_64-elf-objcopy"
    if ! command -v "$OBJCOPY" &> /dev/null; then
        OBJCOPY="objcopy"
    fi
elif [[ "$ARCH" == "32" || "$ARCH" == "i386" || "$ARCH" == "i686" ]]; then
    OBJCOPY="i686-elf-objcopy"
    if ! command -v "$OBJCOPY" &> /dev/null; then
        OBJCOPY="objcopy"
    fi
else
    OBJCOPY="objcopy"
fi

if command -v "$OBJCOPY" &> /dev/null; then
    $OBJCOPY -O binary "$BUILD_DIR/baram_os_$ARCH.elf" "$BUILD_DIR/$OUTPUT_NAME"
else
    objcopy -O binary "$BUILD_DIR/baram_os_$ARCH.elf" "$BUILD_DIR/$OUTPUT_NAME"
fi

# Create ISO image for x86 architectures (needed for Multiboot2)
if [[ "$ARCH" == "64" || "$ARCH" == "32" ]]; then
    echo "Creating bootable ISO..."
    # Create a simple GRUB configuration
    mkdir -p "$BUILD_DIR/isoboot/boot/grub"
    
    cat > "$BUILD_DIR/isoboot/boot/grub/grub.cfg" << EOF
set timeout=0
menuentry "Baram OS Lazward" {
    multiboot2 /boot/baram_os.bin
    boot
}
EOF
    
    cp "$BUILD_DIR/baram_os_$ARCH.elf" "$BUILD_DIR/isoboot/boot/baram_os.bin"
    
    # Create ISO using grub-mkrescue (preferred method for Multiboot2)
    if command -v grub-mkrescue &> /dev/null; then
        grub-mkrescue -o "$BUILD_DIR/baram_os_${ARCH}.iso" "$BUILD_DIR/isoboot" 2>/dev/null || true
    elif command -v xorriso &> /dev/null; then
        xorriso -as mkisofs -o "$BUILD_DIR/baram_os_${ARCH}.iso" \
            -isohybrid-mbr /usr/lib/GRUB/i386-pc/eltorito.img \
            -c boot.catalog -b boot/grub/grub.bin \
            -no-emul-boot -boot-load-size 4 -boot-info-table \
            -V "BARAM_OS" "$BUILD_DIR/isoboot" 2>/dev/null || \
        genisoimage -o "$BUILD_DIR/baram_os_${ARCH}.iso" \
            -b boot/grub/grub.bin -c boot.catalog -no-emul-boot \
            -boot-load-size 4 -boot-info-table -V "BARAM_OS" \
            "$BUILD_DIR/isoboot" 2>/dev/null || true
    elif command -v genisoimage &> /dev/null; then
        genisoimage -o "$BUILD_DIR/baram_os_${ARCH}.iso" \
            -b boot/grub/grub.bin -c boot.catalog -no-emul-boot \
            -boot-load-size 4 -boot-info-table -V "BARAM_OS" \
            "$BUILD_DIR/isoboot" 2>/dev/null || true
    fi
fi

echo ""
echo "=========================================="
echo "Build complete!"
echo "Output: $BUILD_DIR/$OUTPUT_NAME"
echo "ELF file: $BUILD_DIR/baram_os_$ARCH.elf"
echo "=========================================="
echo ""

# Auto-launch QEMU
echo "Launching QEMU..."
case "$ARCH" in
    64|x86_64)
        # For x86_64, use ISO with GRUB for Multiboot2 support
        if [ -f "$BUILD_DIR/baram_os_${ARCH}.iso" ]; then
            qemu-system-x86_64 -cdrom "$BUILD_DIR/baram_os_${ARCH}.iso" -m 512M
        else
            echo "Error: ISO file not created. Check if grub-mkrescue is installed."
            exit 1
        fi
        ;;
    32|i386|i686)
        if [ -f "$BUILD_DIR/baram_os_${ARCH}.iso" ]; then
            qemu-system-i386 -cdrom "$BUILD_DIR/baram_os_${ARCH}.iso" -m 512M
        else
            echo "Error: ISO file not created. Check if grub-mkrescue is installed."
            exit 1
        fi
        ;;
    arm|arm32)
        qemu-system-arm -kernel "$BUILD_DIR/$OUTPUT_NAME" -M virt -m 512M
        ;;
    arm64|aarch64)
        qemu-system-aarch64 -kernel "$BUILD_DIR/$OUTPUT_NAME" -M virt -cpu cortex-a57 -m 512M
        ;;
esac
