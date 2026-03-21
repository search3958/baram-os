#!/bin/bash

# --- 進捗表示関数 (垂れ流し版) ---
TOTAL_STEPS=13
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
    # \r を削除し、改行でログとして残るように変更
    printf "  ${spinner} [${bar}] %2d%% 完了\n" "$percent"
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
        echo "  🛑 停止しました"
    fi
}

# ビルドと実行の本体
do_build_and_run() {
    if [ -f "$HOME/.cargo/env" ]; then
        . "$HOME/.cargo/env"
    fi

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

    # 4. Rust staticlib のビルド
    cargo build --manifest-path rust_kernel/Cargo.toml --target i686-unknown-linux-gnu --release || return 1
    show_progress 4

    # 5. C 言語のコンパイル
    CFLAGS="-I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m32 -march=pentium4 -mno-sse -mno-sse2 -mstackrealign -DBUILD_NUMBER=$CURRENT_BN"

    i686-elf-gcc $CFLAGS -c kernel.c -o output/kernel.o || return 1
    show_progress 5
    i686-elf-gcc $CFLAGS -c drivers.c -o output/drivers.o || return 1
    show_progress 6
    i686-elf-gcc $CFLAGS -c storage.c -o output/storage.o || return 1
    show_progress 7
    i686-elf-gcc $CFLAGS -c fs.c -o output/fs.o || return 1
    show_progress 8
    i686-elf-gcc $CFLAGS -c ui/warp_engine.c -o output/warp_engine.o || return 1
    show_progress 9
    i686-elf-gcc $CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o || return 1
    show_progress 10

    # 6. カーネルのリンク
    i686-elf-gcc -T link.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o \
        rust_kernel/target/i686-unknown-linux-gnu/release/librust_kernel.a \
        -ffreestanding -O2 -m32 -nostdlib -static-libgcc -lgcc || return 1
    show_progress 11

    # 7. ISO ディレクトリへのファイル配置
    mkdir -p output/isodir/boot/grub
    cp output/kernel.bin output/isodir/boot/
    
    # --- Create initrd.tar ---
    INITRD_DIR="output/initrd_tmp"
    rm -rf "$INITRD_DIR"
    mkdir -p "$INITRD_DIR"
    
    cp ui/*.warp ui/*.warpc ui/*.svg "$INITRD_DIR/" 2>/dev/null
    [ -f "bootlogo.svg" ] && cp bootlogo.svg "$INITRD_DIR/"
    [ -f "os_settings.json" ] && cp os_settings.json "$INITRD_DIR/"
    [ -f ".os_settings.json" ] && cp .os_settings.json "$INITRD_DIR/os_settings.json"
    
    (cd "$INITRD_DIR" && tar -cf ../isodir/boot/initrd.tar *)
    rm -rf "$INITRD_DIR"

    # Generate grub.cfg
    GRUB_CFG="output/isodir/boot/grub/grub.cfg"
    cat > "$GRUB_CFG" <<EOF
set timeout=0
set default=0
set quiet=1
set gfxmode=1280x720x32,auto
set gfxpayload=keep
terminal_output gfxterm
menuentry "baram-os" {
    multiboot /boot/kernel.bin
    module /boot/MPLUS2-Regular.ttf
    module /boot/initrd.tar initrd
    boot
}
EOF

    if [ -f "font/MPLUS2-Regular.ttf" ]; then
        cp font/MPLUS2-Regular.ttf output/isodir/boot/
    fi
    
    show_progress 12
    
    # 8. ISO イメージ作成
    i686-elf-grub-mkrescue -o output/os.iso output/isodir || return 1
    show_progress 13

    # Disk image for storage
    if [ ! -f "output/os.img" ]; then
        dd if=/dev/zero of=output/os.img bs=1M count=64 2>/dev/null
    fi

    echo "  ✅ Build #$CURRENT_BN Success"

    # 8. QEMU 起動
    Q_CPU="coreduo"
    Q_ACCEL="-accel tcg,thread=multi"

    if [ "$PERF_MODE" = "max" ]; then
        Q_CPU="max"
        Q_ACCEL="-accel tcg,thread=multi,tb-size=1024"
    fi

    qemu-system-i386 -cdrom output/os.iso \
        -hda output/os.img \
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
# clear を削除 (過去のログを残すため)

# Sync warp symlinks on startup
if [ -f "warp_launcher.sh" ]; then
    ./warp_launcher.sh > /dev/null
fi

# Determine initial action
INITIAL_CMD=""
if [ "$1" = "r" ] || [ "$2" = "r" ]; then
    INITIAL_CMD="r"
fi

echo "========================================"
echo "  🚀 BaramOS Interactive Build System (Streaming Mode)"
echo "  [r]: Build & Run"
echo "  [c]: Stop"
echo "  [q]: Quit"
echo "========================================"
if [ "$PERF_MODE" = "max" ]; then
    echo "  🔥 Performance Mode: MAX"
fi

# 終了時のクリーンアップ
trap "stop_current; exit" SIGINT SIGTERM

# Initial action
if [ "$INITIAL_CMD" = "r" ]; then
    do_build_and_run &
    CURRENT_PID=$!
fi

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
