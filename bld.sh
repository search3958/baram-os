#!/bin/bash

# --- 1. ビルド番号の自動更新 ---
BN_FILE=".build_no"
if [ ! -f "$BN_FILE" ]; then
    echo "0" > "$BN_FILE"
fi

PREV_BN=$(cat "$BN_FILE")
CURRENT_BN=$((PREV_BN + 1))
echo "$CURRENT_BN" > "$BN_FILE"

# ヘッダファイル生成
echo "#define BUILD_NUMBER $CURRENT_BN" > build_no.h

echo "  🚀 BaramOS Build #$CURRENT_BN"

# --- 2. 出力ディレクトリの準備 ---
mkdir -p output
mkdir -p output/isodir/boot/grub

# --- 3. アセンブラのコンパイル ---
nasm -f elf32 arch/boot.s -o output/boot.o
nasm -f elf32 arch/isr.s -o output/isr.o

# --- 4. C言語のコンパイル ---
CFLAGS="-I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m32 -march=pentium4 -mno-sse -mno-sse2 -mstackrealign -DBUILD_NUMBER=$CURRENT_BN"

i686-elf-gcc $CFLAGS -c kernel.c -o output/kernel.o || exit 1
i686-elf-gcc $CFLAGS -c drivers.c -o output/drivers.o || exit 1
i686-elf-gcc $CFLAGS -c ui/warp_engine.c -o output/warp_engine.o || exit 1
i686-elf-gcc $CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o || exit 1

# --- 5. カーネルのリンク ---
i686-elf-gcc -T link.ld -o output/kernel.bin \
    output/boot.o output/isr.o output/kernel.o output/drivers.o output/warp_engine.o output/warp1_engine.o \
    -ffreestanding -O2 -m32 -nostdlib -static-libgcc -lgcc || exit 1

# --- 6. ISOディレクトリへのファイル配置 ---
cp output/kernel.bin output/isodir/boot/
cp grub.cfg output/isodir/boot/grub/
cp font/MPLUS2-Regular.ttf output/isodir/boot/
cp ui/main.warp output/isodir/boot/
cp ui/new.warp1 output/isodir/boot/

# bootlogo.svg を配置
if [ -f "bootlogo.svg" ]; then
    cp bootlogo.svg output/isodir/boot/
elif [ -f "ui/bootlogo.svg" ]; then
    cp ui/bootlogo.svg output/isodir/boot/
else
    echo '<svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg"><circle cx="100" cy="100" r="80" fill="#00a5ff" /><text x="100" y="115" fill="white" font-family="sans-serif" font-size="40" text-anchor="middle">B</text></svg>' > output/isodir/boot/bootlogo.svg
fi

# wallpaper_1.svg を配置
if [ -f "ui/wallpaper_1.svg" ]; then
    cp ui/wallpaper_1.svg output/isodir/boot/
fi

# --- 7. ISOイメージ作成 ---
i686-elf-grub-mkrescue -o output/os.iso output/isodir || exit 1

echo "  ✅ Build #$CURRENT_BN Success"

# --- 8. QEMU起動設定 (究極高速化) ---
# デフォルト
Q_CPU="coreduo"
Q_ACCEL="-accel tcg,thread=multi"

if [ "$1" = "max" ]; then
    echo "  🔥 MAX Performance Mode: Tuning TCG for Apple Silicon..."
    # 1. CPUを 'max' に設定: QEMUがエミュレートできる全機能を解放
    Q_CPU="max"
    # 2. tb-sizeを1GBに拡大: 翻訳済みコードをすべてメモリに保持し、再翻訳を排除
    # 3. thread=multi: ホストのマルチコアを最大限活用
    Q_ACCEL="-accel tcg,thread=multi,tb-size=1024"
fi

qemu-system-i386 -cdrom output/os.iso \
    -vga std \
    -m 2G \
    -smp 4 \
    $Q_ACCEL \
    -cpu $Q_CPU \
    -rtc base=localtime \
    -net none \
    -display cocoa