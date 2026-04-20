#!/bin/bash

# --- 進捗表示関数 ---
TOTAL_STEPS=100
SPINNER_CHARS=("/" "-" "\\" "|")
SPINNER_INDEX=0
CURRENT_PERCENT=0

show_progress() {
    local target=$1
    while [ $CURRENT_PERCENT -lt $target ]; do
        CURRENT_PERCENT=$((CURRENT_PERCENT + 1))
        local filled=$((CURRENT_PERCENT / 10))
        local empty=$((10 - filled))
        local bar=""
        for ((i=0; i<filled; i++)); do bar+="="; done
        for ((i=0; i<empty; i++)); do bar+="-"; done
        local spinner=${SPINNER_CHARS[$SPINNER_INDEX]}
        SPINNER_INDEX=$(( (SPINNER_INDEX + 1) % 4 ))
        printf "\r\e[K  ${spinner} [${bar}] %3d%% 完了" "$CURRENT_PERCENT"
        sleep 0.002
    done
}

CURRENT_PID=0
PERF_MODE="default"
if [ "$1" = "max" ]; then
    PERF_MODE="max"
fi

stop_current() {
    if [ "$CURRENT_PID" -ne 0 ]; then
        pkill -P $CURRENT_PID 2>/dev/null
        kill $CURRENT_PID 2>/dev/null
        wait $CURRENT_PID 2>/dev/null
        CURRENT_PID=0
        echo "  🛑 停止しました"
    fi
}

do_build_and_run() {
    BN_FILE=".build_no"
    if [ ! -f "$BN_FILE" ]; then
        echo "0" > "$BN_FILE"
    fi

    PREV_BN=$(cat "$BN_FILE")
    CURRENT_BN=$((PREV_BN + 1))
    echo "$CURRENT_BN" > "$BN_FILE"
    echo "#define BUILD_NUMBER $CURRENT_BN" > build_no.h

    CURRENT_PERCENT=0

    echo ""
    echo "  🚀 BaramOS Build #$CURRENT_BN 開 始 (ARM64)"

    rm -rf output
    mkdir -p output
    show_progress 10

    CC="clang --target=aarch64-elf"
    AS="clang --target=aarch64-elf"
    LD="ld.lld"
    
    $AS -c arch/boot_arm64.s -o output/boot.o || return 1
    $AS -c arch/isr_arm64.s -o output/isr.o || return 1
    $AS -c arch/setjmp_arm64.s -o output/setjmp.o || return 1
    show_progress 20

    COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -nostdlib -fno-stack-protector -fno-pic -DBUILD_NUMBER=$CURRENT_BN"
    LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89   -Ilua-master"

    $CC $COMMON_CFLAGS -c kernel.c -o output/kernel.o || return 1
    show_progress 40
    $CC $COMMON_CFLAGS -c drivers.c -o output/drivers.o || return 1
    show_progress 50
    $CC $COMMON_CFLAGS -c storage.c -o output/storage.o || return 1
    show_progress 55
    $CC $COMMON_CFLAGS -c fs.c -o output/fs.o || return 1
    show_progress 60
    $CC $COMMON_CFLAGS -c ui/warp_engine.c -o output/warp_engine.o || return 1
    show_progress 70
    $CC $COMMON_CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o || return 1
    show_progress 75
    show_progress 78
    $CC $LUA_CFLAGS -c lua_impl.c -o output/lua.o || return 1
    show_progress 80
    $CC $LUA_CFLAGS -c lua_glue.c -o output/lua_glue.o || return 1
    show_progress 82

    # Prepare initrd (tar)
    INITRD_TAR="output/initrd.tar"
    INITRD_DIR="output/initrd_tmp"
    rm -rf "$INITRD_DIR"
    mkdir -p "$INITRD_DIR"
    cp ui/*.warp ui/*.warpc ui/*.svg ui/*.lua "$INITRD_DIR/" 2>/dev/null
    [ -f "font/IBMPlexSansJP-Regular.ttf" ] && cp font/IBMPlexSansJP-Regular.ttf "$INITRD_DIR/"
    [ -f "bootlogo.svg" ] && cp bootlogo.svg "$INITRD_DIR/"
    [ -f "os_settings.json" ] && cp os_settings.json "$INITRD_DIR/"
    (cd "$INITRD_DIR" && tar -cf ../initrd.tar *)
    rm -rf "$INITRD_DIR"

    # Convert tar to object file using assembly (most compatible way)
    cat <<EOF > output/initrd.s
.section .rodata
.global _binary_initrd_bin_start
.global _binary_initrd_bin_size
.align 8
_binary_initrd_bin_start:
    .incbin "output/initrd.tar"
_binary_initrd_bin_end:
_binary_initrd_bin_size:
    .quad _binary_initrd_bin_end - _binary_initrd_bin_start
EOF
    $AS -c output/initrd.s -o output/initrd.o || return 1

    $LD -T link_arm64.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/lua.o output/lua_glue.o output/initrd.o || return 1

    show_progress 100
    echo ""
    echo "  ✅ Build #$CURRENT_BN Success"

    Q_CPU="cortex-a53"
    Q_ACCEL="" # No KVM on macOS for ARM64 target easily without specific setup

    qemu-system-aarch64 -M virt -cpu $Q_CPU \
        -kernel output/kernel.bin \
        -m 2G \
        -smp 4 \
        -serial stdio \
        -device qemu-xhci \
        -device usb-kbd \
        -device usb-mouse \
        -device ramfb \
        -display cocoa
}

if [ -f "warp_launcher.sh" ]; then
    ./warp_launcher.sh > /dev/null
fi

INITIAL_CMD=""
if [ "$1" = "r" ] || [ "$2" = "r" ]; then
    INITIAL_CMD="r"
fi

echo "========================================"
echo "  🚀 BaramOS Interactive Build System (ARM64)"
echo "  [r]: Build & Run"
echo "  [c]: Stop"
echo "  [q]: Quit"
echo "========================================"

trap "stop_current; exit" SIGINT SIGTERM

if [ "$INITIAL_CMD" = "r" ]; then
    do_build_and_run &
    CURRENT_PID=$!
fi

do_build_only() {
    BN_FILE=".build_no"
    if [ ! -f "$BN_FILE" ]; then
        echo "0" > "$BN_FILE"
    fi

    PREV_BN=$(cat "$BN_FILE")
    CURRENT_BN=$((PREV_BN + 1))
    echo "$CURRENT_BN" > "$BN_FILE"
    echo "#define BUILD_NUMBER $CURRENT_BN" > build_no.h

    CURRENT_PERCENT=0

    echo ""
    echo "  🚀 BaramOS Build #$CURRENT_BN 開 始 (ARM64)"

    rm -rf output
    mkdir -p output
    show_progress 10

    CC="clang --target=aarch64-elf"
    AS="clang --target=aarch64-elf"
    LD="ld.lld"
    
    $AS -c arch/boot_arm64.s -o output/boot.o || return 1
    $AS -c arch/isr_arm64.s -o output/isr.o || return 1
    $AS -c arch/setjmp_arm64.s -o output/setjmp.o || return 1
    show_progress 20

    COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -nostdlib -fno-stack-protector -fno-pic -DBUILD_NUMBER=$CURRENT_BN"
    LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89   -Ilua-master"

    $CC $COMMON_CFLAGS -c kernel.c -o output/kernel.o || return 1
    show_progress 40
    $CC $COMMON_CFLAGS -c drivers.c -o output/drivers.o || return 1
    show_progress 50
    $CC $COMMON_CFLAGS -c storage.c -o output/storage.o || return 1
    show_progress 55
    $CC $COMMON_CFLAGS -c fs.c -o output/fs.o || return 1
    show_progress 60
    $CC $COMMON_CFLAGS -c ui/warp_engine.c -o output/warp_engine.o || return 1
    show_progress 70
    $CC $COMMON_CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o || return 1
    show_progress 75
    show_progress 78
    $CC $LUA_CFLAGS -c lua_impl.c -o output/lua.o || return 1
    show_progress 80
    $CC $LUA_CFLAGS -c lua_glue.c -o output/lua_glue.o || return 1
    show_progress 82

    # Prepare initrd (tar)
    INITRD_TAR="output/initrd.tar"
    INITRD_DIR="output/initrd_tmp"
    rm -rf "$INITRD_DIR"
    mkdir -p "$INITRD_DIR"
    cp ui/*.warp ui/*.warpc ui/*.svg ui/*.lua "$INITRD_DIR/" 2>/dev/null
    [ -f "font/IBMPlexSansJP-Regular.ttf" ] && cp font/IBMPlexSansJP-Regular.ttf "$INITRD_DIR/"
    [ -f "bootlogo.svg" ] && cp bootlogo.svg "$INITRD_DIR/"
    [ -f "os_settings.json" ] && cp os_settings.json "$INITRD_DIR/"
    (cd "$INITRD_DIR" && tar -cf ../initrd.tar *)
    rm -rf "$INITRD_DIR"

    # Convert tar to object file using assembly (most compatible way)
    cat <<EOF > output/initrd.s
.section .rodata
.global _binary_initrd_bin_start
.global _binary_initrd_bin_size
.align 8
_binary_initrd_bin_start:
    .incbin "output/initrd.tar"
_binary_initrd_bin_end:
_binary_initrd_bin_size:
    .quad _binary_initrd_bin_end - _binary_initrd_bin_start
EOF
    $AS -c output/initrd.s -o output/initrd.o || return 1

    $LD -T link_arm64.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/lua.o output/lua_glue.o output/initrd.o || return 1

    show_progress 100
    echo ""
    echo "  ✅ Build #$CURRENT_BN Success"
}

if [ "$1" = "b" ] || [ "$2" = "b" ]; then
    do_build_only
    exit 0
fi

while true; do
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
