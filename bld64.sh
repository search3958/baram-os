#!/bin/bash
# ============================================================
# BaramOS 64-bit Build & Run Script (macOS / Linux 両対応)
# ============================================================

# ---------- コマンド検索関数（Flatpak/VS Code対応） ----------
# Flatpak環境では flatpak-spawn --host を経由してホストOSのコマンドを実行
HOST_PREFIX=""
detect_flatpak() {
    if [ -n "$FLATPAK_ID" ] || grep -q flatpak /proc/1/environ 2>/dev/null; then
        HOST_PREFIX="flatpak-spawn --host"
    fi
}
detect_flatpak

# ツール実行関数（Flatpak対応）
run_tool() {
    if [ -n "$HOST_PREFIX" ]; then
        eval "$HOST_PREFIX $*"
    else
        eval "$*"
    fi
}

find_cmd() {
    local cmd=$1
    # 通常のcommand -vで探す
    if command -v "$cmd" >/dev/null 2>&1; then
        command -v "$cmd"
        return 0
    fi
    # Flatpak環境: flatpak-spawn --host でホストOSのコマンドを探す
    if [ -n "$HOST_PREFIX" ]; then
        local host_cmd
        host_cmd=$(eval "$HOST_PREFIX which $cmd" 2>/dev/null)
        if [ -n "$host_cmd" ]; then
            echo "$host_cmd"
            return 0
        fi
    fi
    return 1
}

# デバッグモード: ./bld64.sh debug でツールチェックのみ実行
if [ "$1" = "debug" ]; then
    echo "=== BaramOS Debug Mode ==="
    echo "OS: $(uname -s) ($(uname -m))"
    echo "PATH: $PATH"
    echo ""
    echo "--- Tool Check ---"
    for cmd in nasm clang ld.lld grub2-mkrescue grub-mkrescue xorriso qemu-system-x86_64 gcc x86_64-elf-gcc i686-elf-gcc; do
        found=$(find_cmd "$cmd")
        if [ $? -eq 0 ]; then
            echo "  ✅ $cmd: $found"
        else
            echo "  ❌ $cmd: NOT FOUND"
        fi
    done
    exit 0
fi

# ---------- OS 判定 ----------
detect_os() {
    case "$(uname -s)" in
        Darwin*)  OS="macos" ;;
        Linux*)   OS="linux" ;;
        *)        OS="unknown" ;;
    esac
}
detect_os

# ---------- ホストアーキテクチャ判定 ----------
HOST_ARCH="$(uname -m)"

# ---------- ターゲット判定 ----------
TARGET="x86_64-elf"

# ============================================================
# OS別 QEMU オプション
# ============================================================
setup_qemu_options() {
    Q_ACCEL=""
    Q_CPU="max"
    Q_SMP="-smp 4,cores=4,threads=1"
    Q_DISPLAY=""

    if [ "$OS" = "macos" ]; then
        # macOS: Hypervisor Framework
        if [ "$HOST_ARCH" = "x86_64" ]; then
            Q_ACCEL="-accel hvf"
        else
            # Apple Silicon では x86_64 エミュレーション → TCG
            Q_ACCEL="-accel tcg"
            Q_CPU="max"
        fi
        Q_DISPLAY="-display sdl,show-cursor=off"
        # Cocoa が使える場合は cocoa も可
        if command -v qemu-system-x86_64 >/dev/null 2>&1; then
            : # OK
        fi

    elif [ "$OS" = "linux" ]; then
        # Linux: KVM (x86_64 ホストのみ)
        if [ "$HOST_ARCH" = "x86_64" ] && [ -e /dev/kvm ]; then
            Q_ACCEL="-accel kvm"
        else
            Q_ACCEL="-accel tcg"
        fi
        # ディスプレイ判定
        if [ -n "$DISPLAY" ] || [ -n "$WAYLAND_DISPLAY" ]; then
            Q_DISPLAY="-display sdl,show-cursor=off"
        else
            Q_DISPLAY="-nographic"
        fi
    fi

    if [ "$PERF_MODE" = "max" ]; then
        Q_CPU="max"
        Q_SMP="-smp 4,cores=4,threads=1"
        if [ "$OS" = "linux" ] && [ "$HOST_ARCH" = "x86_64" ] && [ -e /dev/kvm ]; then
            Q_ACCEL="-accel kvm"
        else
            Q_ACCEL="-accel tcg,tb-size=1024"
        fi
    fi
}

# ============================================================
# OS別 ツールチェーン解決
# ============================================================
resolve_x64_toolchain() {
    CC=""
    GRUB_MKRESCUE=""
    LD_IS_CLANG=0
    USE_XORRISO=0

    if [ "$OS" = "macos" ]; then
        if find_cmd clang >/dev/null 2>&1; then
            CC="clang --target=x86_64-elf"
            LD_IS_CLANG=1
            if find_cmd grub-mkrescue >/dev/null 2>&1; then
                GRUB_MKRESCUE="grub-mkrescue"
            elif find_cmd grub2-mkrescue >/dev/null 2>&1; then
                GRUB_MKRESCUE="grub2-mkrescue"
            elif find_cmd xorriso >/dev/null 2>&1; then
                USE_XORRISO=1
                GRUB_MKRESCUE="xorriso"
            else
                echo "  ❌ grub-mkrescue または xorriso が見つかりません"
                return 1
            fi
            return 0
        fi
    fi

    # Linux: x86_64-elf-gcc
    if find_cmd x86_64-elf-gcc >/dev/null 2>&1; then
        CC="x86_64-elf-gcc"
        LD_IS_CLANG=0
        if find_cmd x86_64-elf-grub-mkrescue >/dev/null 2>&1; then
            GRUB_MKRESCUE="x86_64-elf-grub-mkrescue"
        elif find_cmd grub2-mkrescue >/dev/null 2>&1; then
            GRUB_MKRESCUE="grub2-mkrescue"
        elif find_cmd grub-mkrescue >/dev/null 2>&1; then
            GRUB_MKRESCUE="grub-mkrescue"
        elif find_cmd xorriso >/dev/null 2>&1; then
            USE_XORRISO=1
            GRUB_MKRESCUE="xorriso"
        else
            echo "  ❌ grub-mkrescue または xorriso が見つかりません"
            return 1
        fi
        return 0
    fi

    # Linux: clang + lld
    if find_cmd clang >/dev/null 2>&1 && find_cmd ld.lld >/dev/null 2>&1; then
        CC="clang --target=x86_64-elf"
        LD_IS_CLANG=1
        if find_cmd grub2-mkrescue >/dev/null 2>&1; then
            GRUB_MKRESCUE="grub2-mkrescue"
        elif find_cmd grub-mkrescue >/dev/null 2>&1; then
            GRUB_MKRESCUE="grub-mkrescue"
        elif find_cmd xorriso >/dev/null 2>&1; then
            USE_XORRISO=1
            GRUB_MKRESCUE="xorriso"
        else
            echo "  ❌ grub-mkrescue または xorriso が見つかりません"
            return 1
        fi
        return 0
    fi

    # Linux: gcc
    if find_cmd gcc >/dev/null 2>&1; then
        CC="gcc"
        LD_IS_CLANG=0
        if find_cmd grub2-mkrescue >/dev/null 2>&1; then
            GRUB_MKRESCUE="grub2-mkrescue"
        elif find_cmd grub-mkrescue >/dev/null 2>&1; then
            GRUB_MKRESCUE="grub-mkrescue"
        elif find_cmd xorriso >/dev/null 2>&1; then
            USE_XORRISO=1
            GRUB_MKRESCUE="xorriso"
        else
            echo "  ❌ grub-mkrescue または xorriso が見つかりません"
            return 1
        fi
        return 0
    fi

    return 1
}

check_x64_linker() {
    if [ "$LD_IS_CLANG" -eq 1 ]; then
        if find_cmd ld.lld >/dev/null 2>&1 || find_cmd lld >/dev/null 2>&1; then
            return 0
        fi
    else
        if find_cmd x86_64-elf-gcc >/dev/null 2>&1; then
            return 0
        fi
        if find_cmd x86_64-elf-ld >/dev/null 2>&1; then
            return 0
        fi
        # gcc の場合は gcc 自身がリンカになる
        if [ "$CC" = "gcc" ]; then
            return 0
        fi
    fi
    return 1
}

# ---------- コンパイル関数 ----------
compile_c() {
    local src=$1
    local out=$2
    local flags=$3

    if [ "$LD_IS_CLANG" -eq 1 ]; then
        run_tool clang --target=x86_64-elf $flags -c "$src" -o "$out"
    else
        run_tool $CC $flags -c "$src" -o "$out"
    fi
}

# ---------- リンク関数 ----------
link_kernel() {
    if [ "$LD_IS_CLANG" -eq 1 ]; then
        local LLD_CMD=$(find_cmd ld.lld)
        if [ -z "$LLD_CMD" ]; then
            LLD_CMD=$(find_cmd lld)
        fi

        if [ -n "$LLD_CMD" ]; then
            run_tool $LLD_CMD -T link64.ld -o output/kernel.bin \
                output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/gpu_driver.o output/gpu_blur.o output/gpu_svg.o output/tess_bucketalloc.o output/tess_dict.o output/tess_geom.o output/tess_mesh.o output/tess_priorityq.o output/tess_sweep.o output/tess_tess.o output/lua.o output/lua_glue.o
            return $?
        fi
        echo "  ❌ lld リンカが見つかりません"
        return 1
    fi

    # x86_64-elf-gcc または gcc でリンク
    run_tool $CC -T link64.ld -o output/kernel.bin \
        output/boot.o output/isr.o output/setjmp.o output/kernel.o output/drivers.o output/storage.o output/fs.o output/warp_engine.o output/warp1_engine.o output/gpu_driver.o output/gpu_blur.o output/gpu_svg.o output/tess_bucketalloc.o output/tess_dict.o output/tess_geom.o output/tess_mesh.o output/tess_priorityq.o output/tess_sweep.o output/tess_tess.o output/lua.o output/lua_glue.o \
        -ffreestanding -O2 -nostdlib -static-libgcc -lgcc
}

# ============================================================
# 進捗表示
# ============================================================
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

# ============================================================
# ビルド＆実行
# ============================================================
do_build_and_run() {
    # OS 判定
    detect_os

    if [ "$OS" = "unknown" ]; then
        echo "  ❌ サポートされていないOSです"
        return 1
    fi

    echo "  📱 OS: $OS ($(uname -m))"

    # ツールチェーン解決
    if ! resolve_x64_toolchain; then
        echo "  ❌ 64-bit ビルド用ツールチェーンが見つかりません"
        echo "     macOS: brew install clang llvm grub nasm qemu"
        echo "     Linux: sudo dnf install clang lld nasm grub2-tools xorriso qemu-system-x86-core"
        return 1
    fi

    if ! check_x64_linker; then
        echo "  ❌ 64-bit ELF リンカが見つかりません"
        return 1
    fi

    # QEMU オプション設定
    setup_qemu_options

    # ビルド番号
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

    local NASM_CMD=$(find_cmd nasm)
    $NASM_CMD -f elf64 arch/boot64.s -o output/boot.o || return 1
    show_progress 15
    $NASM_CMD -f elf64 arch/isr64.s -o output/isr.o || return 1
    $NASM_CMD -f elf64 arch/setjmp64.s -o output/setjmp.o || return 1
    show_progress 20

    COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m64 -mno-red-zone -mcmodel=kernel -fno-pic -fno-pie -DBUILD_NUMBER=$CURRENT_BN"
    KERNEL_CFLAGS="$COMMON_CFLAGS -msse2"
    LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89 -Ilua-master"

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
    compile_c gpu/gpu_driver.c output/gpu_driver.o "$COMMON_CFLAGS" || return 1
    show_progress 74
    compile_c gpu/gpu_blur.c output/gpu_blur.o "$COMMON_CFLAGS" || return 1
    show_progress 75
    # libtess2 がある場合のみ svg と tess 関連をコンパイル
    if [ -d "gpu/libtess2" ]; then
        compile_c gpu/gpu_svg.c output/gpu_svg.o "$COMMON_CFLAGS" || return 1
        compile_c gpu/libtess2/Source/bucketalloc.c output/tess_bucketalloc.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/dict.c output/tess_dict.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/geom.c output/tess_geom.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/mesh.c output/tess_mesh.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/priorityq.c output/tess_priorityq.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/sweep.c output/tess_sweep.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/tess.c output/tess_tess.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
    else
        echo "  ⚠️  libtess2 がないため svg/tess スキップ"
        # gpu_svg のスタブ (正しいシグネチャ)
        cat > output/gpu_svg.c << 'STUBEOF'
#include <stdint.h>
typedef struct { float *vertices; int vertex_count; int vertex_cap; uint32_t width; uint32_t height; float scale; float tx, ty; } gpu_svg_renderer_t;
int gpu_svg_init(gpu_svg_renderer_t *r, int w, int h) { return 0; }
int gpu_svg_render(gpu_svg_renderer_t *r, void *img, float s, float tx, float ty, uint32_t *buf, int bw, int bh) { return 0; }
void gpu_svg_cleanup(gpu_svg_renderer_t *r) {}
STUBEOF
        compile_c output/gpu_svg.c output/gpu_svg.o "$COMMON_CFLAGS" 2>/dev/null || true
        # tess のスタブ
        for f in tess_bucketalloc tess_dict tess_geom tess_mesh tess_priorityq tess_sweep tess_tess; do
            echo "void ${f}_stub(void) {}" > "output/${f}.c"
            compile_c "output/${f}.c" "output/${f}.o" "$COMMON_CFLAGS" 2>/dev/null || true
        done
    fi
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
    cat > "$GRUB_CFG" <<'EOF'
set timeout=0
set default=0
set quiet=1
set gfxmode=1280x720x32,auto
set gfxpayload=keep
terminal_output gfxterm
insmod all_video

# UEFIモードとBIOSモードの両方に対応
if [ "${grub_platform}" = "efi" ]; then
    # UEFIモード
    menuentry "baram-os (64-bit UEFI)" {
        insmod part_gpt
        insmod fat
        insmod ext2
        multiboot2 /boot/kernel.bin
        module2 /boot/MPLUS2-Regular.ttf
        module2 /boot/initrd.tar initrd
        boot
    }
else
    # BIOSモード
    menuentry "baram-os (64-bit BIOS)" {
        multiboot /boot/kernel.bin
        module /boot/MPLUS2-Regular.ttf
        module /boot/initrd.tar initrd
        boot
    }
fi
EOF

    if [ -f "font/MPLUS2-Regular.ttf" ]; then
        cp font/MPLUS2-Regular.ttf output/isodir/boot/
    fi

    show_progress 90

    # ISO イメージ作成 (grub-mkrescue または xorriso)
    if [ "$USE_XORRISO" -eq 1 ]; then
        local XORRISO_PATH=$(find_cmd xorriso)
        # xorriso で直接 ISO 作成 (grub-mkrescue なし)
        $XORRISO_PATH -as genisoimage -o output/os.iso \
            -b boot/grub/i386-pc/eltorito.img \
            -no-emul-boot -boot-load-size 4 -boot-info-table \
            -c boot/grub/i386-pc/boot.cat \
            -R -J output/isodir 2>/dev/null || {
            # grub のブートローダーなしで単純に ISO 作成
            $XORRISO_PATH -as genisoimage -o output/os.iso \
                -R -J -V "BARAM_OS" output/isodir 2>/dev/null || return 1
        }
    else
        $GRUB_MKRESCUE -o output/os.iso output/isodir || return 1
    fi
    show_progress 100
    echo ""

    if [ ! -f "output/os.img" ]; then
        dd if=/dev/zero of=output/os.img bs=1M count=64 2>/dev/null
    fi

    echo "  ✅ Build #$CURRENT_BN Success"
    echo "  🖥  QEMU 起動中... (Ctrl+C で停止)"

    local QEMU_PATH=$(find_cmd qemu-system-x86_64)

    # BIOSモードで起動（デフォルト）
    echo "  📱 BIOSモードで起動"
    $QEMU_PATH -cdrom output/os.iso \
        -hda output/os.img \
        -vga virtio \
        -m 2G \
        $Q_SMP \
        $Q_ACCEL \
        -cpu $Q_CPU \
        -rtc base=localtime \
        -net none \
        $Q_DISPLAY
}

# ============================================================
# ビルドのみ
# ============================================================
do_build_only() {
    detect_os

    if [ "$OS" = "unknown" ]; then
        echo "  ❌ サポートされていないOSです"
        return 1
    fi

    echo "  📱 OS: $OS ($(uname -m))"

    if ! resolve_x64_toolchain; then
        echo "  ❌ 64-bit ビルド用ツールチェーンが見つかりません"
        return 1
    fi

    if ! check_x64_linker; then
        echo "  ❌ 64-bit ELF リンカが見つかりません"
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

    local NASM_CMD=$(find_cmd nasm)
    $NASM_CMD -f elf64 arch/boot64.s -o output/boot.o || return 1
    show_progress 15
    $NASM_CMD -f elf64 arch/isr64.s -o output/isr.o || return 1
    $NASM_CMD -f elf64 arch/setjmp64.s -o output/setjmp.o || return 1
    show_progress 20

    COMMON_CFLAGS="-Iinclude -I. -Iui -ffreestanding -O2 -Wall -Wno-unused-function -m64 -mno-red-zone -mcmodel=kernel -fno-pic -fno-pie -DBUILD_NUMBER=$CURRENT_BN"
    KERNEL_CFLAGS="$COMMON_CFLAGS -msse2"
    LUA_CFLAGS="$COMMON_CFLAGS -DLUA_USE_C89 -Ilua-master"

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
    compile_c gpu/gpu_driver.c output/gpu_driver.o "$COMMON_CFLAGS" || return 1
    show_progress 74
    compile_c gpu/gpu_blur.c output/gpu_blur.o "$COMMON_CFLAGS" || return 1
    show_progress 75
    # libtess2 がある場合のみ svg と tess 関連をコンパイル
    if [ -d "gpu/libtess2" ]; then
        compile_c gpu/gpu_svg.c output/gpu_svg.o "$COMMON_CFLAGS" || return 1
        compile_c gpu/libtess2/Source/bucketalloc.c output/tess_bucketalloc.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/dict.c output/tess_dict.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/geom.c output/tess_geom.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/mesh.c output/tess_mesh.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/priorityq.c output/tess_priorityq.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/sweep.c output/tess_sweep.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
        compile_c gpu/libtess2/Source/tess.c output/tess_tess.o "$COMMON_CFLAGS -Igpu/libtess2/Include" || return 1
    else
        echo "  ⚠️  libtess2 がないため svg/tess スキップ"
        # gpu_svg のスタブ (正しいシグネチャ)
        cat > output/gpu_svg.c << 'STUBEOF'
#include <stdint.h>
typedef struct { float *vertices; int vertex_count; int vertex_cap; uint32_t width; uint32_t height; float scale; float tx, ty; } gpu_svg_renderer_t;
int gpu_svg_init(gpu_svg_renderer_t *r, int w, int h) { return 0; }
int gpu_svg_render(gpu_svg_renderer_t *r, void *img, float s, float tx, float ty, uint32_t *buf, int bw, int bh) { return 0; }
void gpu_svg_cleanup(gpu_svg_renderer_t *r) {}
STUBEOF
        compile_c output/gpu_svg.c output/gpu_svg.o "$COMMON_CFLAGS" 2>/dev/null || true
        # tess のスタブ
        for f in tess_bucketalloc tess_dict tess_geom tess_mesh tess_priorityq tess_sweep tess_tess; do
            echo "void ${f}_stub(void) {}" > "output/${f}.c"
            compile_c "output/${f}.c" "output/${f}.o" "$COMMON_CFLAGS" 2>/dev/null || true
        done
    fi
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

# ============================================================
# メインタインルーチン
# ============================================================
if [ -f "warp_launcher.sh" ]; then
    ./warp_launcher.sh > /dev/null
fi

INITIAL_CMD=""
if [ "$1" = "r" ] || [ "$2" = "r" ]; then
    INITIAL_CMD="r"
fi

echo "========================================"
echo "  🚀 BaramOS Interactive Build System (64-bit)"
echo "  📱 OS: $(uname -s) ($(uname -m))"
echo "  [r]: Build & Run"
echo "  [c]: Stop"
echo "  [b]: Build only"
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
        b)
            stop_current
            do_build_only
            ;;
    esac
done
