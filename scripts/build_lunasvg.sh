#!/bin/bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <target-triple> <output-dir>" >&2
    exit 1
fi

TARGET="$1"
OUTDIR="$2"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

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

CLANG="$(find_tool clang)"
CLANGXX="$(find_tool clang++)"

mkdir -p "$OUTDIR"

CXXFLAGS=(
    "--target=$TARGET"
    -std=c++17
    -fno-exceptions
    -fno-rtti
    -fno-threadsafe-statics
    -fno-use-cxa-atexit
    -nostdinc++
    -ffreestanding
    -O2
    -Wall
    -Wno-unused-function
    -DLUNASVG_BUILD
    -DLUNASVG_BUILD_STATIC
    -DLUNASVG_DISABLE_LOAD_SYSTEM_FONTS
    -DPLUTOVG_DISABLE_FONT_FACE_CACHE_LOAD
    -I"$ROOT/lunasvg/include"
    -I"$ROOT/lunasvg/source"
    -I"$ROOT/lunasvg/plutovg/include"
    -I"$ROOT/lunasvg/plutovg/source"
    -isystem "$ROOT/lunasvg/compat"
    -isystem /opt/homebrew/opt/llvm/include/c++/v1
    -isystem "$ROOT/include"
    -isystem "$ROOT"
)

CFLAGS=(
    "--target=$TARGET"
    -std=c11
    -ffreestanding
    -O2
    -Wall
    -Wno-unused-function
    -DPLUTOVG_DISABLE_FONT_FACE_CACHE_LOAD
    -I"$ROOT/lunasvg/include"
    -I"$ROOT/lunasvg/source"
    -I"$ROOT/lunasvg/plutovg/include"
    -I"$ROOT/lunasvg/plutovg/source"
    -isystem "$ROOT/include"
    -isystem "$ROOT"
)

CPP_SOURCES=(
    "$ROOT/gpu/gpu_svg.cpp"
    "$ROOT/gpu/gpu_svg_runtime.cpp"
    "$ROOT/gpu/libcpp_support.cpp"
    "$ROOT/lunasvg/source/graphics.cpp"
    "$ROOT/lunasvg/source/lunasvg.cpp"
    "$ROOT/lunasvg/source/svgelement.cpp"
    "$ROOT/lunasvg/source/svggeometryelement.cpp"
    "$ROOT/lunasvg/source/svglayoutstate.cpp"
    "$ROOT/lunasvg/source/svgpaintelement.cpp"
    "$ROOT/lunasvg/source/svgparser.cpp"
    "$ROOT/lunasvg/source/svgproperty.cpp"
    "$ROOT/lunasvg/source/svgrenderstate.cpp"
    "$ROOT/lunasvg/source/svgtextelement.cpp"
)

C_SOURCES=(
    "$ROOT/lunasvg/plutovg/source/plutovg-blend.c"
    "$ROOT/lunasvg/plutovg/source/plutovg-canvas.c"
    "$ROOT/lunasvg/plutovg/source/plutovg-font.c"
    "$ROOT/lunasvg/plutovg/source/plutovg-ft-math.c"
    "$ROOT/lunasvg/plutovg/source/plutovg-ft-raster.c"
    "$ROOT/lunasvg/plutovg/source/plutovg-ft-stroker.c"
    "$ROOT/lunasvg/plutovg/source/plutovg-matrix.c"
    "$ROOT/lunasvg/plutovg/source/plutovg-paint.c"
    "$ROOT/lunasvg/plutovg/source/plutovg-path.c"
    "$ROOT/lunasvg/plutovg/source/plutovg-rasterize.c"
    "$ROOT/lunasvg/plutovg/source/plutovg-surface.c"
)

OBJECTS=()

for src in "${CPP_SOURCES[@]}"; do
    base="$(basename "$src" .cpp)"
    obj="$OUTDIR/lunasvg_${base}.o"
    "$CLANGXX" "${CXXFLAGS[@]}" -c "$src" -o "$obj"
    OBJECTS+=("$obj")
done

for src in "${C_SOURCES[@]}"; do
    base="$(basename "$src" .c)"
    obj="$OUTDIR/lunasvg_${base}.o"
    "$CLANG" "${CFLAGS[@]}" -c "$src" -o "$obj"
    OBJECTS+=("$obj")
done

printf '%s ' "${OBJECTS[@]}" > "$OUTDIR/lunasvg_objects.list"
printf '\n' >> "$OUTDIR/lunasvg_objects.list"
