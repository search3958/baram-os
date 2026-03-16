#!/bin/bash

# --- 進捗表示関数 ---
TOTAL_STEPS=12
CURRENT_STEP=0
SPINNER_CHARS=("/" "-" "\\" "|")
SPINNER_INDEX=0

show_progress() {
    local percent=$((CURRENT_STEP * 100 / TOTAL_STEPS))
    local filled=$((percent / 10))
    local empty=$((10 - filled))
    local bar=""
    for ((i=0; i<filled; i++)); do bar+="="; done
    for ((i=0; i<empty; i++)); do bar+="-"; done
    local spinner=${SPINNER_CHARS[$SPINNER_INDEX]}
    SPINNER_INDEX=$(( (SPINNER_INDEX + 1) % 4 ))
    printf "\r  ${spinner} [${bar}] %2d%% 完了" "$percent"
}

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

echo ""
echo "  🚀 BaramOS Build #$CURRENT_BN"
echo ""

# --- 2. 出力ディレクトリの準備 ---
mkdir -p output
mkdir -p output/isodir/boot/grub
CURRENT_STEP=1
show_progress

# --- 3. アセンブラのコンパイル ---
nasm -f elf32 arch/boot.s -o output/boot.o
CURRENT_STEP=2
show_progress
nasm -f elf32 arch/isr.s -o output/isr.o
CURRENT_STEP=3
show_progress

# --- 4. C 言語のコンパイル ---
CFLAGS="-I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m32 -march=pentium4 -mno-sse -mno-sse2 -mstackrealign -DBUILD_NUMBER=$CURRENT_BN"

i686-elf-gcc $CFLAGS -c kernel.c -o output/kernel.o || exit 1
CURRENT_STEP=4
show_progress
i686-elf-gcc $CFLAGS -c drivers.c -o output/drivers.o || exit 1
CURRENT_STEP=5
show_progress
i686-elf-gcc $CFLAGS -c ui/warp_engine.c -o output/warp_engine.o || exit 1
CURRENT_STEP=6
show_progress
i686-elf-gcc $CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o || exit 1
CURRENT_STEP=7
show_progress

# --- 5. カーネルのリンク ---
i686-elf-gcc -T link.ld -o output/kernel.bin \
    output/boot.o output/isr.o output/kernel.o output/drivers.o output/warp_engine.o output/warp1_engine.o \
    -ffreestanding -O2 -m32 -nostdlib -static-libgcc -lgcc || exit 1
CURRENT_STEP=8
show_progress

# --- 6. ISO ディレクトリへのファイル配置 ---
cp output/kernel.bin output/isodir/boot/
CURRENT_STEP=9
show_progress
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
CURRENT_STEP=10
show_progress

# wallpaper_1.svg を配置
if [ -f "ui/wallpaper_1.svg" ]; then
    cp ui/wallpaper_1.svg output/isodir/boot/
fi

# --- 7. ISO イメージ作成 ---
i686-elf-grub-mkrescue -o output/os.iso output/isodir || exit 1
CURRENT_STEP=11
show_progress

echo ""
echo "  ✅ Build #$CURRENT_BN Success"
echo ""

# --- 8. QEMU 起動設定 (究極高速化) ---
# デフォルト
Q_CPU="coreduo"
Q_ACCEL="-accel tcg,thread=multi"

if [ "$1" = "max" ]; then
    echo "  🔥 MAX Performance Mode: Tuning TCG for Apple Silicon..."
    # 1. CPU を 'max' に設定：QEMU がエミュレートできる全機能を解放
    Q_CPU="max"
    # 2. tb-size を 1GB に拡大：翻訳済みコードをすべてメモリに保持し、再翻訳を排除
    # 3. thread=multi: ホストのマルチコアを最大限活用
    Q_ACCEL="-accel tcg,thread=multi,tb-size=1024"
fi

qemu-system-i386 -cdrom output/os.iso \
    -vga virtio \
    -m 2G \
    -smp 4 \
    $Q_ACCEL \
    -cpu $Q_CPU \
    -rtc base=localtime \
    -net none \
    -display cocoa
