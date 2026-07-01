#!/bin/bash

set -e

ARCH="x86_64"
CC="gcc"
LD="ld"
CFLAGS="-ffreestanding -fno-stack-protector -fno-pie -nostdlib -m64 -I./kernel -O2"
LDFLAGS="-T linker.ld -n -o kernel.bin"

echo "Building kernel for $ARCH..."

# カーネルソースのコンパイル
$CC $CFLAGS -c kernel/main.c -o main.o
$CC $CFLAGS -c kernel/framebuffer.c -o framebuffer.o
$CC $CFLAGS -c kernel/mouse.c -o mouse.o
$CC $CFLAGS -c kernel/keyboard.c -o keyboard.o
$CC $CFLAGS -c kernel/interrupt.c -o interrupt.o

# リンク
$LD $LDFLAGS main.o framebuffer.o mouse.o keyboard.o interrupt.o

echo "Build complete: kernel.bin"

# クリーンアップ
rm -f *.o
