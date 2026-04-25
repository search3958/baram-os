#!/bin/bash

# --- 進捗表示関数 (垂れ流し版) ---
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
    echo "  🚀 BaramOS Build #$CURRENT_BN 開始 (32-bit)"

    rm -rf output
    mkdir -p output/isodir/boot/grub
    show_progress 10

    nasm -f elf32 arch/boot.s -o output/boot.o || return 1
    show_progress 15
    nasm -f elf32 arch/isr.s -o output/isr.o || return 1
    show_progress 18
    nasm -f elf32 arch/setjmp.s -o output/setjmp.o || return 1
    show_progress 20

    CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m32 -march=pentium4 -mno-sse -mno-sse2 -mstackrealign -DBUILD_NUMBER=$CURRENT_BN"
    KERNEL_CFLAGS="$CFLAGS -msse2"

    i686-elf-gcc $KERNEL_CFLAGS -c kernel.c -o output/kernel.o || return 1
    show_progress 40
    i686-elf-gcc $CFLAGS -c drivers.c -o output/drivers.o || return 1
    show_progress 50
    i686-elf-gcc $CFLAGS -c storage.c -o output/storage.o || return 1
    show_progress 55
    i686-elf-gcc $CFLAGS -c fs.c -o output/fs.o || return 1
    show_progress 60
    i686-elf-gcc $CFLAGS -c ui/warp_engine.c -o output/warp_engine.o || return 1
    show_progress 70
    i686-elf-gcc $CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o || return 1
    show_progress 75
    show_progress 78
    LUA_CFLAGS="$CFLAGS -DLUA_USE_C89   -Ilua-master"
    i686-elf-gcc $LUA_CFLAGS -c lua_impl.c -o output/lua.o || return 1
    show_progress 80
    i686-elf-gcc $LUA_CFLAGS -c lua_glue.c -o output/lua_glue.o || return 1
    show_progress 82

    i686-elf-gcc -T link.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/lua.o output/lua_glue.o \
        -ffreestanding -O2 -m32 -nostdlib -static-libgcc -lgcc || return 1
    show_progress 85

    mkdir -p output/isodir/boot/grub
    cp output/kernel.bin output/isodir/boot/

    INITRD_DIR="output/initrd_tmp"
    rm -rf "$INITRD_DIR"
    mkdir -p "$INITRD_DIR"

    cp ui/*.warp ui/*.warpc ui/*.svg "$INITRD_DIR/" 2>/dev/null
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
menuentry "baram-os (32-bit)" {
    multiboot /boot/kernel.bin
    module /boot/HarmonyOS_Sans_Regular.ttf
    module /boot/initrd.tar initrd
    boot
}
EOF

    if [ -f "font/HarmonyOS_Sans_Regular.ttf" ]; then
        cp font/HarmonyOS_Sans_Regular.ttf output/isodir/boot/
    fi

    show_progress 90

    i686-elf-grub-mkrescue -o output/os.iso output/isodir || return 1
    show_progress 100
    echo ""

    if [ ! -f "output/os.img" ]; then
        dd if=/dev/zero of=output/os.img bs=1M count=64 2>/dev/null
    fi

    echo "  ✅ Build #$CURRENT_BN Success"

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

if [ -f "warp_launcher.sh" ]; then
    ./warp_launcher.sh > /dev/null
fi

INITIAL_CMD=""
if [ "$1" = "r" ] || [ "$2" = "r" ]; then
    INITIAL_CMD="r"
fi

echo "========================================"
echo "  🚀 BaramOS Interactive Build System (32-bit)"
echo "  [r]: Build & Run"
echo "  [c]: Stop"
echo "  [q]: Quit"
echo "========================================"
if [ "$PERF_MODE" = "max" ]; then
    echo "  🔥 Performance Mode: MAX"
fi

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
    echo "  🚀 BaramOS Build #$CURRENT_BN 開 始 (32-bit)"

    rm -rf output
    mkdir -p output/isodir/boot/grub
    show_progress 10

    nasm -f elf32 arch/boot.s -o output/boot.o || return 1
    show_progress 15
    nasm -f elf32 arch/isr.s -o output/isr.o || return 1
    show_progress 18
    nasm -f elf32 arch/setjmp.s -o output/setjmp.o || return 1
    show_progress 20

    CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m32 -march=pentium4 -mno-sse -mno-sse2 -mstackrealign -DBUILD_NUMBER=$CURRENT_BN"
    KERNEL_CFLAGS="$CFLAGS -msse2"

    i686-elf-gcc $KERNEL_CFLAGS -c kernel.c -o output/kernel.o || return 1
    show_progress 40
    i686-elf-gcc $CFLAGS -c drivers.c -o output/drivers.o || return 1
    show_progress 50
    i686-elf-gcc $CFLAGS -c storage.c -o output/storage.o || return 1
    show_progress 55
    i686-elf-gcc $CFLAGS -c fs.c -o output/fs.o || return 1
    show_progress 60
    show_progress 65
    i686-elf-gcc $CFLAGS -DLUA_USE_C89   -Ilua-master -c lua_impl.c -o output/lua.o || return 1
    show_progress 68
    i686-elf-gcc $CFLAGS -DLUA_USE_C89   -Ilua-master -c lua_glue.c -o output/lua_glue.o || return 1
    show_progress 70
    i686-elf-gcc $CFLAGS -c ui/warp_engine.c -o output/warp_engine.o || return 1
    show_progress 75
    i686-elf-gcc $CFLAGS -c ui/warp1_engine.c -o output/warp1_engine.o || return 1
    show_progress 80

    i686-elf-gcc -T link.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/lua.o output/lua_glue.o output/warp_engine.o output/warp1_engine.o \
        -ffreestanding -O2 -m32 -nostdlib -static-libgcc -lgcc || return 1
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
