#!/bin/bash

set -e

ARCH="arm64"
CC="aarch64-linux-gnu-gcc"
LD="aarch64-linux-gnu-ld"
CFLAGS="-ffreestanding -fno-stack-protector -fno-pie -nostdlib -mgeneral-regs-only -I./kernel -O2"
LDFLAGS="-T linker_arm64.ld -n -o kernel_arm64.bin"

echo "Building kernel for $ARCH..."

# カーネルソースのコンパイル
$CC $CFLAGS -c kernel/main.c -o main.o
$CC $CFLAGS -c kernel/framebuffer.c -o framebuffer.o
$CC $CFLAGS -c kernel/mouse.c -o mouse.o
$CC $CFLAGS -c kernel/keyboard.c -o keyboard.o
$CC $CFLAGS -c kernel/interrupt.c -o interrupt.o

# リンク
$LD $LDFLAGS main.o framebuffer.o mouse.o keyboard.o interrupt.o

echo "Build complete: kernel_arm64.bin"

# クリーンアップ
rm -f *.o
