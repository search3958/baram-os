#!/bin/bash

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

find_tool() {
    local tool="$1"
    if command -v "$tool" >/dev/null 2>&1; then
        command -v "$tool"
        return 0
    fi
    if [ -x "/opt/homebrew/bin/$tool" ]; then
        printf '%s\n' "/opt/homebrew/bin/$tool"
        return 0
    fi
    if [ -x "/opt/homebrew/opt/llvm/bin/$tool" ]; then
        printf '%s\n' "/opt/homebrew/opt/llvm/bin/$tool"
        return 0
    fi
    return 1
}

stop_current() {
    if [ "$CURRENT_PID" -ne 0 ]; then
        pkill -P $CURRENT_PID 2>/dev/null
        kill $CURRENT_PID 2>/dev/null
        wait $CURRENT_PID 2>/dev/null
        CURRENT_PID=0
        echo "  🛑 停止しました"
    fi
}

resolve_arm64_toolchain() {
    local found_cc found_ld found_as

    found_cc=$(find_tool aarch64-elf-gcc) || found_cc=""
    found_ld=$(find_tool aarch64-elf-ld) || found_ld=""
    found_as=$(find_tool aarch64-elf-as) || found_as=""
    if [ -n "$found_cc" ] && [ -n "$found_ld" ] && [ -n "$found_as" ]; then
        CC="$found_cc"
        LD="$found_ld"
        AS="$found_as"
        return 0
    fi

    found_cc=$(find_tool aarch64-linux-gnu-gcc) || found_cc=""
    found_ld=$(find_tool aarch64-linux-gnu-ld) || found_ld=""
    found_as=$(find_tool aarch64-linux-gnu-as) || found_as=""
    if [ -n "$found_cc" ] && [ -n "$found_ld" ] && [ -n "$found_as" ]; then
        CC="$found_cc"
        LD="$found_ld"
        AS="$found_as"
        return 0
    fi

    found_cc=$(find_tool clang) || found_cc=""
    found_ld=$(find_tool ld.lld) || found_ld=""
    found_as=$(find_tool aarch64-elf-as) || found_as=""
    if [ -n "$found_cc" ] && [ -n "$found_ld" ] && [ -n "$found_as" ]; then
        CC="$found_cc --target=aarch64-elf"
        LD="$found_ld"
        AS="$found_as"
        return 0
    fi

    return 1
}

compile_c() {
    local src=$1
    local out=$2
    local flags=$3

    $CC $flags -c "$src" -o "$out"
}

link_kernel() {
    # Create initrd as binary and embed it
    INITRD_DIR="output/initrd_tmp"
    rm -rf "$INITRD_DIR"
    mkdir -p "$INITRD_DIR/system/services"

    cp ui/*.warp ui/*.warpc ui/*.svg ui/*.lua "$INITRD_DIR/" 2>/dev/null
    cp ui/*.bmp ui/*.png ui/*.jpg ui/*.jpeg ui/*.tga ui/*.gif "$INITRD_DIR/" 2>/dev/null
    [ -f "bootlogo.svg" ] && cp bootlogo.svg "$INITRD_DIR/"
    [ -f "os_settings.json" ] && cp os_settings.json "$INITRD_DIR/"
    [ -f ".os_settings.json" ] && cp .os_settings.json "$INITRD_DIR/os_settings.json"
    [ -f "output/svg_service.pkg" ] && cp output/svg_service.pkg "$INITRD_DIR/system/services/svg_service.pkg"

    (cd "$INITRD_DIR" && tar -cf ../initrd.tar *)
    rm -rf "$INITRD_DIR"
    rm -f output/svg_service.pkg

    # Convert to binary object
    local OBJCOPY
    OBJCOPY=$(find_tool aarch64-elf-objcopy) || return 1
    "$OBJCOPY" -I binary -O elf64-littleaarch64 -B aarch64 output/initrd.tar output/initrd.o

    $CC -T link_arm64.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/warp_draw.o output/gpu_driver.o output/gpu_blur.o output/lua.o output/lua_glue.o output/initrd.o \
        -ffreestanding -O2 -nostdlib -static-libgcc -lgcc
}

do_build_and_run() {
    if ! resolve_arm64_toolchain; then
        echo "  ❌ arm64 ビルド用ツールチェーンが見つかりません"
        echo "     必要: aarch64-elf-gcc または aarch64-linux-gnu-gcc + as"
        echo "     代替: clang + ld.lld + aarch64-elf-as"
        return 1
    fi

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
    echo "  🚀 BaramOS Build #$CURRENT_BN 開始 (arm64)"

    rm -rf output
    mkdir -p output
    show_progress 10

    $AS -o output/boot.o arch/boot_arm64.s || return 1
    show_progress 15
    $AS -o output/isr.o arch/isr_arm64.s || return 1
    show_progress 20
    $AS -o output/setjmp.o arch/setjmp_arm64.s || return 1
    show_progress 25

    COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -DBUILD_NUMBER=$CURRENT_BN"
    KERNEL_CFLAGS="$COMMON_CFLAGS"
    LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89 -Wno-invalid-noreturn -Ilua-master"

    compile_c kernel.c output/kernel.o "$KERNEL_CFLAGS" || return 1
    show_progress 40
    compile_c drivers.c output/drivers.o "$COMMON_CFLAGS" || return 1
    show_progress 50
    compile_c storage.c output/storage.o "$COMMON_CFLAGS" || return 1
    show_progress 55
    compile_c fs.c output/fs.o "$COMMON_CFLAGS" || return 1
    show_progress 60
    compile_c ui/warp_engine.c output/warp_engine.o "$COMMON_CFLAGS" || return 1
    show_progress 70
    compile_c ui/warp1_engine.c output/warp1_engine.o "$COMMON_CFLAGS" || return 1
    show_progress 72
    compile_c ui/warp_draw.c output/warp_draw.o "$COMMON_CFLAGS" || return 1
    show_progress 73
    compile_c gpu/gpu_driver.c output/gpu_driver.o "$COMMON_CFLAGS" || return 1
    show_progress 74
    compile_c gpu/gpu_blur.c output/gpu_blur.o "$COMMON_CFLAGS" || return 1
    show_progress 75
    bash scripts/build_svg_service_pkg.sh aarch64-elf output || return 1
    show_progress 78
    compile_c lua_impl.c output/lua.o "$LUA_CFLAGS" || return 1
    show_progress 80
    compile_c lua_glue.c output/lua_glue.o "$LUA_CFLAGS" || return 1
    show_progress 82

    link_kernel || return 1
    show_progress 85

    echo ""
    echo "  ✅ Build #$CURRENT_BN Success"

    # For arm64, use QEMU with aarch64
    qemu-system-aarch64 -kernel output/kernel.bin \
        -m 512M \
        -machine virt \
        -cpu cortex-a72 \
        -device ramfb \
        -nographic \
        -semihosting \
        -semihosting-config enable=on,target=native
}

if [ -f "warp_launcher.sh" ]; then
    ./warp_launcher.sh > /dev/null
fi

INITIAL_CMD=""
if [ "$1" = "r" ] || [ "$2" = "r" ]; then
    INITIAL_CMD="r"
fi

echo "========================================"
echo "  🚀 BaramOS Interactive Build System (arm64)"
echo "  [r]: Build & Run"
echo "  [c]: Stop"
echo "  [q]: Quit"
echo "========================================"
if [ "$PERF_MODE" = "max" ]; then
    echo "  🔥 Performance Mode: MAX"
fi

trap "stop_current; exit" SIGINT SIGTERM

if [ "$INITIAL_CMD" = "r" ]; then
    do_build_and_run
fi

do_build_only() {
    if ! resolve_arm64_toolchain; then
        echo "  ❌ arm64 ビルド用ツールチェーンが見つかりません"
        return 1
    fi

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
    echo "  🚀 BaramOS Build #$CURRENT_BN 開始 (arm64)"

    rm -rf output
    mkdir -p output
    show_progress 10

    $AS -o output/boot.o arch/boot_arm64.s || return 1
    show_progress 15
    $AS -o output/isr.o arch/isr_arm64.s || return 1
    show_progress 20
    $AS -o output/setjmp.o arch/setjmp_arm64.s || return 1
    show_progress 25

    COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -DBUILD_NUMBER=$CURRENT_BN"
    KERNEL_CFLAGS="$COMMON_CFLAGS"
    LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89 -Wno-invalid-noreturn -Ilua-master"

    compile_c kernel.c output/kernel.o "$KERNEL_CFLAGS" || return 1
    show_progress 40
    compile_c drivers.c output/drivers.o "$COMMON_CFLAGS" || return 1
    show_progress 50
    compile_c storage.c output/storage.o "$COMMON_CFLAGS" || return 1
    show_progress 55
    compile_c fs.c output/fs.o "$COMMON_CFLAGS" || return 1
    show_progress 60
    compile_c ui/warp_engine.c output/warp_engine.o "$COMMON_CFLAGS" || return 1
    show_progress 70
    compile_c ui/warp1_engine.c output/warp1_engine.o "$COMMON_CFLAGS" || return 1
    show_progress 72
    compile_c ui/warp_draw.c output/warp_draw.o "$COMMON_CFLAGS" || return 1
    show_progress 73
    compile_c gpu/gpu_driver.c output/gpu_driver.o "$COMMON_CFLAGS" || return 1
    show_progress 74
    compile_c gpu/gpu_blur.c output/gpu_blur.o "$COMMON_CFLAGS" || return 1
    show_progress 75
    bash scripts/build_svg_service_pkg.sh aarch64-elf output || return 1
    show_progress 78
    compile_c lua_impl.c output/lua.o "$LUA_CFLAGS" || return 1
    show_progress 80
    compile_c lua_glue.c output/lua_glue.o "$LUA_CFLAGS" || return 1
    show_progress 82

    link_kernel || return 1
    rm -f output/svg_service.pkg
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
            do_build_and_run
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
