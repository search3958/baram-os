#!/bin/bash

# --- 進捗表示関数 ---
TOTAL_STEPS=12
SPINNER_CHARS=("/" "-" "\\" "|")
SPINNER_INDEX=0

show_progress() {
    local step=$1
    local percent=$((step * 100 / TOTAL_STEPS))
    local filled=$((percent / 10))
    local empty=$((10 - filled))
    local bar=""
    for ((i=0; i<filled; i++)); do bar+="="; done
    for ((i=0; i<empty; i++)); do bar+="-"; done
    local spinner=${SPINNER_CHARS[$SPINNER_INDEX]}
    SPINNER_INDEX=$(( (SPINNER_INDEX + 1) % 4 ))
    printf "\r  ${spinner} [${bar}] %2d%% 完了" "$percent"
}

# --- 管理変数 ---
CURRENT_PID=0
PERF_MODE="default"
if [ "$1" = "max" ]; then
    PERF_MODE="max"
fi

# 現在実行中のビルド/QEMUプロセスを停止する関数
stop_current() {
    if [ "$CURRENT_PID" -ne 0 ]; then
        # 子プロセスグループ全体を終了
        pkill -P $CURRENT_PID 2>/dev/null
        kill $CURRENT_PID 2>/dev/null
        wait $CURRENT_PID 2>/dev/null
        CURRENT_PID=0
        echo -e "\n  🛑 停止しました"
    fi
}

# ビルドと実行の本体
do_build_and_run() {
    # 1. ビルド番号の自動更新
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
    echo "  🚀 BaramOS Build #$CURRENT_BN 開始"

    # 2. 出力ディレクトリの準備
    rm -rf output/isodir
    mkdir -p output/isodir/boot/grub
    show_progress 1

    # 3. アセンブラのコンパイル
    nasm -f elf32 arch/boot.s -o output/boot.o
    show_progress 2
    nasm -f elf32 arch/isr.s -o output/isr.o
    show_progress 3

    # 4. C 言語のコンパイル
    CFLAGS="-I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m32 -march=pentium4 -mno-sse -mno-sse2 -mstackrealign -DBUILD_NUMBER=$CURRENT_BN"

    i686-elf-gcc $CFLAGS -c kernel.c -o output/kernel.o || return 1
    show_progress 4
    i686-elf-gcc $CFLAGS -c drivers.c -o output/drivers.o || return 1
    show_progress 5
    i686-elf-gcc $CFLAGS -c ui/warp_engine.c -o output/warp_engine.o || return 1
    show_progress 6
    i686-elf-gcc $CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o || return 1
    show_progress 7

    # 5. カーネルのリンク
    i686-elf-gcc -T link.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/kernel.o output/drivers.o output/warp_engine.o output/warp1_engine.o \
        -ffreestanding -O2 -m32 -nostdlib -static-libgcc -lgcc || return 1
    show_progress 8

    # 6. ISO ディレクトリへのファイル配置
    cp output/kernel.bin output/isodir/boot/
    show_progress 9
    cp grub.cfg output/isodir/boot/grub/
    cp font/MPLUS2-Regular.ttf output/isodir/boot/
    
    # 拡張子変更に対応 (.warpc と .warp)
    cp ui/main.warpc output/isodir/boot/
    cp ui/new.warp output/isodir/boot/
    cp ui/terminal.warp output/isodir/boot/
    cp ui/menubar.warp output/isodir/boot/

    if [ -f "bootlogo.svg" ]; then
        cp bootlogo.svg output/isodir/boot/
    elif [ -f "ui/bootlogo.svg" ]; then
        cp ui/bootlogo.svg output/isodir/boot/
    else
        echo '<svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg"><circle cx="100" cy="100" r="80" fill="#00a5ff" /><text x="100" y="115" fill="white" font-family="sans-serif" font-size="40" text-anchor="middle">B</text></svg>' > output/isodir/boot/bootlogo.svg
    fi
    show_progress 10

    if [ -f "ui/wallpaper_1.svg" ]; then
        cp ui/wallpaper_1.svg output/isodir/boot/
    fi

    # 7. ISO イメージ作成
    i686-elf-grub-mkrescue -o output/os.iso output/isodir || return 1
    show_progress 11

    echo -e "\n  ✅ Build #$CURRENT_BN Success"

    # 8. QEMU 起動
    Q_CPU="coreduo"
    Q_ACCEL="-accel tcg,thread=multi"

    if [ "$PERF_MODE" = "max" ]; then
        Q_CPU="max"
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
}

# --- メインループ ---
clear
echo "========================================"
echo "  🚀 BaramOS Interactive Build System"
echo "  [r]: Build & Run (ビルド・再起動)"
echo "  [c]: Stop (停止)"
echo "  [q]: Quit (終了)"
echo "========================================"
if [ "$PERF_MODE" = "max" ]; then
    echo "  🔥 Performance Mode: MAX"
fi

# 終了時のクリーンアップ
trap "stop_current; exit" SIGINT SIGTERM

while true; do
    # 1文字の入力を待機 (非表示・サイレント)
    read -n 1 -s cmd
    case "$cmd" in
        r)
            stop_current
            do_build_and_run &
            CURRENT_PID=$!
            ;;
        c)
            stop_current
            ;;
        q)
            stop_current
            echo "  👋 終了します"
            exit 0
            ;;
    esac
done
