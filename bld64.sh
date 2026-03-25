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

stop_current() {
    if [ "$CURRENT_PID" -ne 0 ]; then
        pkill -P $CURRENT_PID 2>/dev/null
        kill $CURRENT_PID 2>/dev/null
        wait $CURRENT_PID 2>/dev/null
        CURRENT_PID=0
        echo "  🛑 停止しました"
    fi
}

resolve_x64_toolchain() {
    if command -v x86_64-elf-gcc >/dev/null 2>&1 && command -v x86_64-elf-grub-mkrescue >/dev/null 2>&1; then
        CC="x86_64-elf-gcc"
        GRUB_MKRESCUE="x86_64-elf-grub-mkrescue"
        LD_IS_CLANG=0
        return 0
    fi

    if command -v clang >/dev/null 2>&1 && command -v grub-mkrescue >/dev/null 2>&1; then
        CC="clang --target=x86_64-elf"
        GRUB_MKRESCUE="grub-mkrescue"
        LD_IS_CLANG=1
        return 0
    fi

    if command -v clang >/dev/null 2>&1 && command -v i686-elf-grub-mkrescue >/dev/null 2>&1; then
        CC="clang --target=x86_64-elf"
        GRUB_MKRESCUE="i686-elf-grub-mkrescue"
        LD_IS_CLANG=1
        return 0
    fi

    return 1
}

check_x64_linker() {
    if command -v x86_64-elf-gcc >/dev/null 2>&1; then
        return 0
    fi

    if command -v ld.lld >/dev/null 2>&1 || command -v x86_64-elf-ld >/dev/null 2>&1; then
        return 0
    fi

    return 1
}

compile_c() {
    local src=$1
    local out=$2
    local flags=$3

    if [ "$LD_IS_CLANG" -eq 1 ]; then
        clang --target=x86_64-elf $flags -c "$src" -o "$out"
    else
        $CC $flags -c "$src" -o "$out"
    fi
}

link_kernel() {
    local LLD_CMD=""
    if command -v ld.lld >/dev/null 2>&1; then
        LLD_CMD="ld.lld"
    elif command -v lld >/dev/null 2>&1; then
        LLD_CMD="lld"
    fi

    if [ "$LD_IS_CLANG" -eq 1 ]; then
        if [ -n "$LLD_CMD" ]; then
            # macOS の clang はデフォルトで Mach-O リンカを呼ぼうとするため、
            # 直接 lld を呼ぶか、-fuse-ld にフルパスまたは適切な指定が必要です。
            # ここでは直接 lld (ELF 用) を使用する設定を試みます。
            clang --target=x86_64-elf -fuse-ld="$LLD_CMD" -T link64.ld -o output/kernel.bin \
                output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/files.o output/lua.o output/lua_glue.o \
                -ffreestanding -O2 -nostdlib -static-libgcc -lgcc 2>/dev/null || \
            $LLD_CMD -T link64.ld -o output/kernel.bin \
                output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/files.o output/lua.o output/lua_glue.o
            return $?
        fi
        echo "  ❌ 64-bit linker が見つかりません (ld.lld または x86_64-elf ツールチェーンが必要です)"
        return 1
    fi

    $CC -T link64.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/lua.o output/lua_glue.o \
        -ffreestanding -O2 -m64 -mcmodel=kernel -mno-red-zone -nostdlib -static-libgcc -lgcc
}

do_build_and_run() {
    if ! resolve_x64_toolchain; then
        echo "  ❌ 64-bit ビルド用ツールチェーンが見つかりません"
        echo "     必要: x86_64-elf-gcc + x86_64-elf-grub-mkrescue"
        echo "     代替: clang + ld.lld + grub-mkrescue"
        return 1
    fi

    if ! check_x64_linker; then
        echo "  ❌ この環境では 64-bit ELF リンカが未導入です"
        echo "     32-bit は ./bld32.sh で引き続きビルドできます"
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
    echo "  🚀 BaramOS Build #$CURRENT_BN 開始 (64-bit)"

    rm -rf output
    mkdir -p output/isodir/boot/grub
    show_progress 10

    nasm -f elf64 arch/boot64.s -o output/boot.o || return 1
    show_progress 15
    nasm -f elf64 arch/isr64.s -o output/isr.o || return 1
    nasm -f elf64 arch/setjmp64.s -o output/setjmp.o || return 1
    show_progress 20

    COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m64 -mno-red-zone -mcmodel=kernel -fno-pic -fno-pie -DBUILD_NUMBER=$CURRENT_BN"
    KERNEL_CFLAGS="$COMMON_CFLAGS -msse2"
    LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89   -Ilua-master"

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
    show_progress 75
    compile_c files.c output/files.o "$COMMON_CFLAGS" || return 1
    show_progress 78
    compile_c lua_impl.c output/lua.o "$LUA_CFLAGS" || return 1
    show_progress 80
    compile_c lua_glue.c output/lua_glue.o "$LUA_CFLAGS" || return 1
    show_progress 82

    link_kernel || return 1
    show_progress 85

    mkdir -p output/isodir/boot/grub
    cp output/kernel.bin output/isodir/boot/

    INITRD_DIR="output/initrd_tmp"
    rm -rf "$INITRD_DIR"
    mkdir -p "$INITRD_DIR"

    cp ui/*.warp ui/*.warpc ui/*.svg ui/*.lua "$INITRD_DIR/" 2>/dev/null
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
menuentry "baram-os (64-bit)" {
    multiboot /boot/kernel.bin
    module /boot/MPLUS2-Regular.ttf
    module /boot/initrd.tar initrd
    boot
}
EOF

    if [ -f "font/MPLUS2-Regular.ttf" ]; then
        cp font/MPLUS2-Regular.ttf output/isodir/boot/
    fi

    show_progress 90

    $GRUB_MKRESCUE -o output/os.iso output/isodir || return 1
    show_progress 100
    echo ""

    if [ ! -f "output/os.img" ]; then
        dd if=/dev/zero of=output/os.img bs=1M count=64 2>/dev/null
    fi

    echo "  ✅ Build #$CURRENT_BN Success"

    Q_CPU="qemu64"
    Q_ACCEL="-accel tcg,thread=multi"

    if [ "$PERF_MODE" = "max" ]; then
        Q_CPU="max"
        Q_ACCEL="-accel tcg,thread=multi,tb-size=2048"
    fi

    qemu-system-x86_64 -cdrom output/os.iso \
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

if [ -f "warp_launcher.sh" ]; then
    ./warp_launcher.sh > /dev/null
fi

INITIAL_CMD=""
if [ "$1" = "r" ] || [ "$2" = "r" ]; then
    INITIAL_CMD="r"
fi

echo "========================================"
echo "  🚀 BaramOS Interactive Build System (64-bit)"
echo "  [r]: Build & Run"
echo "  [c]: Stop"
echo "  [q]: Quit"
echo "========================================"
echo "  ℹ️  32-bit build は ./bld32.sh を使用"
if [ "$PERF_MODE" = "max" ]; then
    echo "  🔥 Performance Mode: MAX"
fi

trap "stop_current; exit" SIGINT SIGTERM

if [ "$INITIAL_CMD" = "r" ]; then
    do_build_and_run &
    CURRENT_PID=$!
fi

do_build_only() {
    if ! resolve_x64_toolchain; then
        echo "  ❌ 64-bit ビルド用ツールチェ ーンが見つかりません"
        return 1
    fi

    if ! check_x64_linker; then
        echo "  ❌ この環境では 64-bit ELF リンカが未導入です"
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
    echo "  🚀 BaramOS Build #$CURRENT_BN 開 始 (64-bit)"

    rm -rf output
    mkdir -p output/isodir/boot/grub
    show_progress 10

    nasm -f elf64 arch/boot64.s -o output/boot.o || return 1
    show_progress 15
    nasm -f elf64 arch/isr64.s -o output/isr.o || return 1
    nasm -f elf64 arch/setjmp64.s -o output/setjmp.o || return 1
    show_progress 20

    COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m64 -mno-red-zone -mcmodel=kernel -fno-pic -fno-pie -DBUILD_NUMBER=$CURRENT_BN"
    KERNEL_CFLAGS="$COMMON_CFLAGS -msse2"
    LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89   -Ilua-master"

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
    show_progress 75
    compile_c files.c output/files.o "$COMMON_CFLAGS" || return 1
    show_progress 78
    compile_c lua_impl.c output/lua.o "$LUA_CFLAGS" || return 1
    show_progress 80
    compile_c lua_glue.c output/lua_glue.o "$LUA_CFLAGS" || return 1
    show_progress 82

    link_kernel || return 1
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
