# Baram OS

Baram OS は、UI とグラフィックスに特化した x86 アーキテクチャ（32-bit）向けの自作オペレーティングシステムです。
独自の宣言型 UI エンジン「Warp Engine」を搭載し、高解像度でのスムーズなグラフィックス体験を目指しています。

## 🌟 主な特徴

- **Warp Engine**: 独自の DSL（`.warp` ファイル）による宣言型 UI システム。リアクティブな状態管理とコンポーネントベースの設計をサポートしています。
- **ベクターグラフィックス**: NanoSVG を統合し、カーネルレベルでの高品質な SVG レンダリングを実現。
- **TrueType フォント**: `stb_truetype` を使I've created a detailed `README.md` for "baram-os," highlighting its UI-first design, Warp Engine, and hardware support. I'll now double-check the full `package.json` to ensure I haven't missed any build scripts or important details, particularly regarding the `sharp` dependency.
I'll now check `grub.cfg` for any special boot parameters or menu entries, given that `package.json` lacks a `scripts` section and `sharp`'s role remains unclear.
The `module` entries in `grub.cfg` for `MPLUS2-Regular.ttf` and `main.warp` are crucial; they're loaded by the kernel from GRUB-provided Multiboot modules. I'll update the `README.md` to include this important detail in the "How it works" or "File Structure" section.
I'll re-read `README.md` to confirm the exact content, as I likely messed up the indentation or whitespace in `old_string`.
I'll re-apply the changes with the correct context. The previous attempt failed, likely due to a mismatch in `old_string`. I'll now replace the `📂 ディレクトリ構成` section and insert the `🔧 内部の仕組み` section beforehand.
I've updated `README.md` with features, internal mechanisms, directory structure, and build instructions. Request fulfilled.


Baram OS のプロジェクト内容に基づき、UI/グラフィックスの特長や内部構造を網羅した `README.md` を作成しました。

主な記載内容：
- **独自 UI エンジン (Warp Engine)**: DSL による宣言型 UI とリアクティブな状態管理。
- **モダンなグラフィックス**: SVG (NanoSVG) および TrueType フォント (stb_truetype) のサポート。
- **効率的な描画**: 1280x720 の高解像度を実現するダーティレクト管理。
- **Multiboot モジュール**: GRUB を介したフォントや UI 定義のロード。
- **ビルド・実行手順**: クロスコンパイラの設定から QEMU での起動まで。
/os.iso -vga std -m 512
```

## 🔧 内部の仕組み

### Multiboot Modules
Baram OS は、GRUB から渡される Multiboot モジュールを利用して外部リソースを読み込みます。
- **モジュール 1**: フォントファイル (`MPLUS2-Regular.ttf`)
- **モジュール 2**: UI 定義ファイル (`main.warp`)

これらは起動時にカーネルによって自動的にメモリへマッピングされ、レンダリングエンジンによって使用されます。

### グラフィックス・パイプライン
1. Warp Engine が `.warp` ファイルを読み込み、UI 木を構築。
2. 必要に応じて SVG データをラスタライズ。
3. `stb_truetype` を使用してフォントをビットマップ化。
4. ダーティレクト管理により、変更があった部分のみをフレームバッファへ転送。

## 📂 ディレクトリ構成

- `kernel.c`: カーネルのエントリポイント、描画ループ、イベント処理、モジュールロード。
- `warp_engine.c`: UI エンジンのコアロジック（`.warp` ファイルのパースと状態管理）。
- `drivers.c/h`: 各種ハードウェアドライバ（VBE, PS/2 Mouse/KB, PIT）とグラフィックス層。
- `ui/`: Warp Engine 用の UI 定義ファイル。`main.warp` がメインの UI です。
- `font/`: TrueType フォントとレンダリングライブラリ（`stb_truetype`）。
- `nanosvg/`: SVG デコードおよびラスタライズライブラリ。
- `arch/`: CPU 依存のアセンブリコード（ブートコード、ISR）。

## 📝 ライセンス
このプロジェクトは学習および研究目的で開発されています。
各種ライブラリ（NanoSVG, stb_truetype等）はそれぞれのライセンスに従います。
