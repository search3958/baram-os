#!/bin/bash
# ============================================================
# BaramOS 環境セットアップスクリプト
# macOS / Linux 両対応
# ============================================================

detect_os() {
    case "$(uname -s)" in
        Darwin*)  OS="macos" ;;
        Linux*)   OS="linux" ;;
        *)        OS="unknown" ;;
    esac
}
detect_os

echo "=========================================="
echo "  🔧 BaramOS 環境セットアップ"
echo "  📱 OS: $OS ($(uname -m))"
echo "=========================================="

if [ "$OS" = "macos" ]; then
    echo ""
    echo "macOS 用の依存関係をインストールします。"
    echo "Homebrew が必要です。"
    echo ""
    echo "以下のコマンドを実行してください:"
    echo ""
    echo "  # Homebrew がない場合"
    echo "  /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
    echo ""
    echo "  # 依存関係一式"
    echo "  brew install nasm qemu llvm grub xorriso"
    echo ""
    echo "  # LLVM のパスを通す (Apple Silicon の場合)"
    echo "  export PATH=\"/opt/homebrew/opt/llvm/bin:\$PATH\""
    echo ""
    echo "  # QEMU のエイリアス確認"
    echo "  ls \$(brew --prefix)/bin/qemu-system-x86_64"
    echo ""

elif [ "$OS" = "linux" ]; then
    # ディストリビューション判定
    if [ -f /etc/os-release ]; then
        ID=$(. /etc/os-release && echo "$ID")
    else
        ID="unknown"
    fi

    echo ""
    echo "Linux 用の依存関係インストールコマンド:"
    echo ""

    case "$ID" in
        fedora*|fedora-asahi-remix)
            echo "  # Fedora 系"
            echo "  sudo dnf install -y nasm qemu-system-x86-core clang lld grub2-tools-extra xorriso"
            ;;
        ubuntu*|debian*|linuxmint)
            echo "  # Debian/Ubuntu 系"
            echo "  sudo apt update"
            echo "  sudo apt install -y nasm qemu-system-x86 clang lld grub-pc-bin xorriso"
            ;;
        arch*|manjaro)
            echo "  # Arch 系"
            echo "  sudo pacman -S nasm qemu clang lld grub xorriso"
            ;;
        *)
            echo "  # 自動判定できませんでした。手動でインストールしてください。"
            echo "  必要なパッケージ:"
            echo "    - nasm"
            echo "    - qemu-system-x86"
            echo "    - clang"
            echo "    - lld"
            echo "    - grub (grub-mkrescue)"
            echo "    - xorriso"
            ;;
    esac
    echo ""
else
    echo "❌ サポートされていないOSです"
fi

echo "=========================================="
echo "  セットアップ完了後、以下のコマンドでビルドできます:"
echo "    ./bld64.sh b    # 64-bit ビルド"
echo "    ./bld32.sh b    # 32-bit ビルド"
echo "    ./bld64.sh r    # 64-bit ビルド＆実行"
echo "=========================================="
