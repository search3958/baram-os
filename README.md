# BaramOS 使用者操作体系

## 概要
ARM64とx86_64向け操作体系です。
Raspberry Pi 4B(マウスが動作しない)と一般的なx86_64ラップトップでの動作が確認できています。
滑らかな角丸，美しいブラー効果があるリッチなGUIのOSです。OSとして基礎的な機能は備えていますが，標準アプリはそこまでの数を用意できていません。

## v1.3 変更点
- ソフトウェアキーボード
- UI Scriptのサポート打ち切り
- Warp4アプリ
- 使用可能なファイルシステム

## ファイルシステム

アプリケーションとランタイムデータは `files/app` と `files/data` にまとめています。各ビルドではこの2つを `files.tar` にパッケージし、UEFI が読める FAT ボリュームのルートへ配置します。

OS の VFS は `/apps/...`、`/app/...`、`/data/...`、`/files/...` を `files.tar` のメンバーとして透過的に読み込みます。`write_file` でこれらを書き換えた場合は TAR を再生成して同じストレージへ保存するため、再起動後も変更が残ります。旧来の FAT 上の個別ファイルはアップグレード互換の読み込みフォールバックとして扱います。

W4S からは次のファイル API を利用できます。

```text
BaramOS.getFile fileText (files://data/ui/min.svg)
BaramOS.uploadFile selectedText (files://data/)
```

`getFile` は指定ファイルの UTF-8 内容を変数へ設定します。`uploadFile` はOSが管理する読み取り専用のファイル選択ダイアログを開き、選択してアップロードしたファイルの内容を変数へ設定します。

## OSSの感謝
### uefi-rs
- **ライセンス**：MIT ライセンス または Apache-2.0 ライセンス
- **用途**：UEFI プロトコルとの連携、メモリアロケータ、パニック発生時の処理機能を提供します。

### uefi-raw
- **ライセンス**：MIT ライセンス または Apache-2.0 ライセンス
- **用途**：UEFI の基本的な型定義を生の形式で提供します。

### libm
- **ライセンス**：MIT ライセンス
- **用途**：`aarch64-unknown-uefi` 環境向けに、標準ライブラリ非依存（`no_std`）で動作する浮動小数点演算機能を提供します。
- ※ このリポジトリは 2025年4月28日にアーカイブ（読み取り専用）となっています。

### stb_truetype_rust
- **ライセンス**：Unlicense
- **用途**：TrueType フォントの解析と、画面上への文字描画処理を行います。Rust 版に移植されたライブラリです。

### kurbo
- **ライセンス**：MIT ライセンス または Apache-2.0 ライセンス
- **用途**：2次元曲線を扱うライブラリで、ベジエ曲線の計算、線の太さ調整、SVG 形式のパスデータの解析などに利用しています。

### png-decoder
- **ライセンス**：MIT ライセンス、Apache-2.0 ライセンス または Zlib ライセンス
- **用途**：Rust のみで記述され、標準ライブラリ非依存（`no_std`）環境でも動作する PNG 画像のデコード機能を提供します。

### crc32fast
- **ライセンス**：MIT ライセンス または Apache-2.0 ライセンス
- **用途**：SIMD 命令を活用した高速な CRC32 チェックサム計算を行い、データの整合性確認に使用します。

### miniz_oxide
- **ライセンス**：MIT ライセンス、Apache-2.0 ライセンス または Zlib ライセンス
- **用途**：DEFLATE 形式で圧縮されたデータの展開処理を行います。

### num_enum
- **ライセンス**：BSD-3-Clause ライセンス、MIT ライセンス または Apache-2.0 ライセンス
- **用途**：`png-decoder` 内部で、数値と列挙型の間の安全な変換を行います。

### Mozc OSS dictionary
- **ライセンス**：Mozc 本体は BSD-3-Clause。採用した `dictionary_oss` のエントリは IPAdic／ICOT Free Software／沖縄辞書に由来するため、それぞれの通知と無保証条項を [third_party/mozc_dictionary_oss/README.txt](third_party/mozc_dictionary_oss/README.txt) に保持しています。
- **用途**：かな漢字変換の候補索引です。公式辞書約 129 万エントリから、低コストの一般語 51,832 読み・最大3候補を `crates/baram-boot/src/mozc_dictionary.tsv` に生成して利用します。元データは [google/mozc](https://github.com/google/mozc/tree/master/src/data/dictionary_oss) です。
- **更新方法**：`tools/generate_mozc_dictionary.rs` を使って公式 `dictionary_oss` から再生成します。

### WanaKana Rust (`wana_kana`)
- **ライセンス**：MIT ライセンス
- **用途**：ローマ字からひらがなへの変換を行います。UEFI の `no_std` 環境向けに必要最小限の適合を加えたソースを `crates/wana-kana` に同梱しています。

### AOSP PinyinIME dictionary
- **ライセンス**：Apache-2.0
- **用途**：簡体字拼音入力の変換候補です。Android Open Source Project の [PinyinIME](https://android.googlesource.com/platform/packages/inputmethods/PinyinIME/) にある `jni/data/rawdict_utf16_65105_freq.txt` を、GBK フラグが 0 の標準簡体字エントリに限定して、入力用の `crates/baram-boot/src/pinyin_dictionary.tsv` へ生成しています。生成元は commit `49aebad1c1cfbbcaa9288ffed5161e79e57c3679` です。手書きの候補表は使用していません。
- **更新方法**：`tools/generate_pinyin_dictionary.rs` を使って、AOSP の同じ辞書ソースから再生成します。

### blake3
- **ライセンス**：CC0-1.0、Apache-2.0 または Apache-2.0 WITH LLVM-exception
- **用途**：設定・データのハッシュ計算を行います。

### fnv
- **ライセンス**：Apache-2.0 または MIT ライセンス
- **用途**：`wana_kana` のハッシュ実装依存です。

### lazy_static / spin
- **ライセンス**：`lazy_static` は MIT または Apache-2.0、`spin` は MIT ライセンス
- **用途**：`wana_kana` の変換表を `no_std` 環境で安全に初期化します。

### 間接依存ライブラリ

UEFI、描画、PNG デコード、手続きマクロのビルドで取り込まれる OSS も、次のとおり記載します。

| ライブラリ | ライセンス | 用途 |
| --- | --- | --- |
| `adler2` | 0BSD / MIT / Apache-2.0 | DEFLATE の Adler-32 チェックサム |
| `arrayref` | BSD-2-Clause | 固定長配列参照 |
| `arrayvec` | MIT / Apache-2.0 | 固定容量ベクタ |
| `bit_field` | Apache-2.0 / MIT | ビットフィールド操作 |
| `bitflags` | MIT / Apache-2.0 | ビットフラグ型 |
| `cfg-if` | MIT / Apache-2.0 | 条件付きコンパイル補助 |
| `constant_time_eq` | CC0-1.0 / MIT-0 / Apache-2.0 | 定数時間の比較 |
| `log` | MIT / Apache-2.0 | ログ API |
| `miniz_oxide` | MIT / Apache-2.0 / Zlib | DEFLATE 展開 |
| `polycool` | MIT / Apache-2.0 | `kurbo` の多項式計算 |
| `ptr_meta` / `ptr_meta_derive` | MIT | DST ポインタのメタデータ |
| `proc-macro2` / `quote` / `syn` | MIT / Apache-2.0 | Rust 手続きマクロ基盤 |
| `rustversion` | MIT / Apache-2.0 | Rust バージョン条件分岐 |
| `smallvec` | MIT / Apache-2.0 | 小容量最適化ベクタ |
| `ucs2` | MPL-2.0 | UEFI 文字列処理 |
| `uefi-macros` | MIT / Apache-2.0 | UEFI 用手続きマクロ |
| `uguid` | MIT / Apache-2.0 | UEFI GUID 型 |
| `unicode-ident` | MIT / Apache-2.0 / Unicode-3.0 | Rust 識別子の Unicode 判定 |
| `cc` | MIT / Apache-2.0 | `blake3` のビルドスクリプト |
| `cpufeatures` | MIT / Apache-2.0 | CPU 機能検出 |
| `find-msvc-tools` | MIT / Apache-2.0 | `cc` のツールチェーン検出 |
| `shlex` | MIT / Apache-2.0 | `cc` のコマンドライン解析 |

### 開発・検証限定の依存

次は OS イメージには含まれず、`png-decoder` のベンチマーク／検証時だけ利用します。

| ライブラリ | ライセンス | 用途 |
| --- | --- | --- |
| `criterion` | Apache-2.0 / MIT | PNG デコーダのベンチマーク |
| `image` | MIT | PNG デコード結果の検証 |
| `cast` / `criterion-plot` / `oorandom` | MIT / Apache-2.0 | ベンチマークの計測・描画 |
| `plotters` / `plotters-backend` / `plotters-svg` | MIT / Apache-2.0 | ベンチマーク結果のグラフ |
| `tinytemplate` | MIT / Apache-2.0 | ベンチマークレポートのテンプレート |

`image` の検証機能に付随する依存も開発時だけ使用します。

| ライブラリ | ライセンス | 用途 |
| --- | --- | --- |
| `adler` / `adler32` | MIT / Apache-2.0 | チェックサム計算 |
| `atty` | MIT | 端末判定 |
| `autocfg` | Apache-2.0 / MIT | ビルド時の機能判定 |
| `bytemuck` | Zlib / Apache-2.0 / MIT | バイト列と型の変換 |
| `byteorder` | Unlicense / MIT | バイトオーダー変換 |
| `clap` / `textwrap` | MIT / Apache-2.0 | ベンチマーク CLI |
| `csv` / `csv-core` | MIT / Unlicense | ベンチマーク結果の出力 |
| `deflate` | MIT / Apache-2.0 | 画像形式の展開 |
| `gif` / `jpeg-decoder` / `tiff` | MIT | `image` の画像形式対応 |
| `itertools` / `either` | MIT / Apache-2.0 | イテレータ補助 |
| `libc` | MIT | Unix API の宣言 |
| `memchr` | MIT / Unlicense | バイト列検索 |
| `num-integer` / `num-iter` / `num-rational` / `num-traits` | MIT / Apache-2.0 | 数値演算 |
| `rayon` / `rayon-core` | MIT / Apache-2.0 | 並列処理 |
| `serde` / `serde_core` / `serde_derive` / `serde_json` | MIT / Apache-2.0 | データのシリアライズ |
| `serde_cbor` | Apache-2.0 / MIT | CBOR シリアライズ |
| `itoa` / `ryu` / `zmij` | MIT / Apache-2.0 | 数値の文字列化 |
| `scoped_threadpool` | MIT | スレッドプール |
| `walkdir` / `same-file` | MIT / Unlicense | ディレクトリ走査 |
| `unicode-width` | MIT / Apache-2.0 | 端末表示幅の計算 |
