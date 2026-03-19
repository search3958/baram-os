#!/bin/bash

# --- 進捗表示・ターミナル管理変数 ---
TOTAL_STEPS=12
SPINNER_CHARS=("/" "-" "\\" "|")
SPINNER_INDEX=0

# --- ターミナル管理機能 ---
init_terminal() {
    local rows=$(tput lines)
    if [ -n "$rows" ] && [ "$rows" -gt 2 ]; then
        # 画面の最終行を除いた範囲をスクロール領域に設定
        # これにより、ビルドログは普通に上から下へ流れ、最下行は固定されます
        printf "\033[1;$(($rows - 1))r"
    fi
}

cleanup_terminal() {
    printf "\033[r" # スクロール範囲の設定を解除（通常の状態に戻す）
}

draw_footer() {
    local mode=$1
    local step=$2
    local rows=$(tput lines)
    [ -z "$rows" ] && return
    
    # カーソル位置を保存して最下行へ移動、行消去
    printf "\033[s"
    printf "\033[${rows};0H\033[K"
    
    if [ "$mode" = "progress" ]; then
        local percent=$((step * 100 / TOTAL_STEPS))
        local filled=$((percent / 10))
        local empty=$((10 - filled))
        local bar=""
        for ((i=0; i<filled; i++)); do bar+="="; done
        for ((i=0; i<empty; i++)); do bar+="-"; done
        local spinner=${SPINNER_CHARS[$SPINNER_INDEX]}
        SPINNER_INDEX=$(( (SPINNER_INDEX + 1) % 4 ))
        printf "  \e[1;36m${spinner} [${bar}] %2d%% 完了\e[0m" "$percent"
    else
        printf "  \e[1;33m[r] Build & Run  [c] Stop  [k] Clear  [q] Quit\e[0m"
        if [ "$PERF_MODE" = "max" ]; then
            printf "  \e[1;31m 🔥 MAX PERF\e[0m"
        fi
    fi
    # 保存したカーソル位置（ログ出力の末尾など）に戻る
    printf "\033[u"
}

show_progress() {
    draw_footer "progress" "$1"
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
        pkill -P $CURRENT_PID 2>/dev/null
        kill $CURRENT_PID 2>/dev/null
        wait $CURRENT_PID 2>/dev/null
        CURRENT_PID=0
        echo -e "\n  🛑 停止しました"
        draw_footer "menu"
    fi
}

# ビルドと実行の本体
do_build_and_run() {
    draw_footer "progress" 0
    # 1. ビルド番号の自動更新
    BN_FILE=".build_no"
    if [ ! -f "$BN_FILE" ]; then
        echo "0" > "$BN_FILE"
    fi

    PREV_BN=$(cat "$BN_FILE")
    CURRENT_BN=$((PREV_BN + 1))
    echo "$CURRENT_BN" > "$BN_FILE"

    echo "#define BUILD_NUMBER $CURRENT_BN" > build_no.h

    echo ""
    echo "  🚀 BaramOS Build #$CURRENT_BN 開始"

    # 2. 出力ディレクトリの準備
    rm -rf output
    mkdir -p output/isodir/boot/grub
    show_progress 1

    # 3. アセンブラのコンパイル
    nasm -f elf32 arch/boot.s -o output/boot.o
    show_progress 2
    nasm -f elf32 arch/isr.s -o output/isr.o
    show_progress 3

    # 4. C 言語のコンパイル
    CFLAGS="-I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m32 -march=pentium4 -mno-sse -mno-sse2 -mstackrealign -DBUILD_NUMBER=$CURRENT_BN"

    # ログは普通に出力される
    i686-elf-gcc $CFLAGS -c kernel.c -o output/kernel.o || return 1
    show_progress 4
    i686-elf-gcc $CFLAGS -c drivers.c -o output/drivers.o || return 1
    show_progress 5
    i686-elf-gcc $CFLAGS -c storage.c -o output/storage.o || return 1
    i686-elf-gcc $CFLAGS -c fs.c -o output/fs.o || return 1
    i686-elf-gcc $CFLAGS -c ui/warp_engine.c -o output/warp_engine.o || return 1
    show_progress 6
    i686-elf-gcc $CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o || return 1
    show_progress 7

    # 5. カーネルのリンク
    i686-elf-gcc -T link.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o \
        -ffreestanding -O2 -m32 -nostdlib -static-libgcc -lgcc || return 1
    show_progress 8

    # 6. ISO ディレクトリへのファイル配置
    mkdir -p output/isodir/boot/grub
    cp output/kernel.bin output/isodir/boot/
    show_progress 9
    
    # --- Create initrd.tar ---
    INITRD_DIR="output/initrd_tmp"
    rm -rf "$INITRD_DIR"
    mkdir -p "$INITRD_DIR"
    
    if [ -f ".app_files" ]; then
        while IFS= read -r line || [ -n "$line" ]; do
            [[ -z "$line" || "$line" =~ ^# ]] && continue
            if [ -f "ui/$line" ]; then
                cp "ui/$line" "$INITRD_DIR/"
            fi
        done < ".app_files"
    else
        cp ui/*.warp ui/*.warpc ui/*.svg "$INITRD_DIR/" 2>/dev/null
    fi

    cp ui/*.svg "$INITRD_DIR/" 2>/dev/null
    [ -f "bootlogo.svg" ] && cp bootlogo.svg "$INITRD_DIR/"
    [ -f "os_settings.json" ] && cp os_settings.json "$INITRD_DIR/"
    [ -f ".os_settings.json" ] && cp .os_settings.json "$INITRD_DIR/os_settings.json"
    
    (cd "$INITRD_DIR" && tar -cf ../isodir/boot/initrd.tar *)
    rm -rf "$INITRD_DIR"

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
EOF

    if [ -f "font/MPLUS2-Regular.ttf" ]; then
        cp font/MPLUS2-Regular.ttf output/isodir/boot/
    fi
    
    show_progress 10
    echo "    boot" >> "$GRUB_CFG"
    echo "}" >> "$GRUB_CFG"
    
    # 7. ISO イメージ作成
    i686-elf-grub-mkrescue -o output/os.iso output/isodir || return 1
    show_progress 11

    if [ ! -f "output/os.img" ]; then
        dd if=/dev/zero of=output/os.img bs=1M count=64
    fi

    echo -e "\n  ✅ Build #$CURRENT_BN Success"
    draw_footer "menu"

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
init_terminal
clear

if [ -f "warp_launcher.sh" ]; then
    ./warp_launcher.sh > /dev/null
fi

INITIAL_CMD=""
if [ "$1" = "r" ] || [ "$2" = "r" ]; then
    INITIAL_CMD="r"
fi

echo "========================================"
echo "  🚀 BaramOS Interactive Build System"
echo "  [r]: Build & Run (ビルド・再起動)"
echo "  [c]: Stop (停止)"
echo "  [k]: Clear (ログ消去)"
echo "  [q]: Quit (終了)"
echo "  💡 Hint: You can run apps directly via ./appname.warp"
echo "========================================"
if [ "$PERF_MODE" = "max" ]; then
    echo "  🔥 Performance Mode: MAX"
fi

trap "stop_current; cleanup_terminal; echo; exit" SIGINT SIGTERM

if [ "$INITIAL_CMD" = "r" ]; then
    do_build_and_run &
    CURRENT_PID=$!
fi

while true; do
    draw_footer "menu"
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
        k)
            # 画面を消去し、固定メニューを再描画
            clear
            init_terminal
            draw_footer "menu"
            ;;
        q)
            stop_current
            cleanup_terminal
            echo "  👋 終了します"
            exit 0
            ;;
    esac
done
