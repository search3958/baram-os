# BaramOS

ARM64とx86_64向けOSです。
Raspberry Pi 4B(マウスが動作しない)と一般的なx86_64ラップトップでの動作が確認できています。
滑らかな角丸，美しいブラー効果があるリッチなGUIのOSです。OSとして基礎的な機能は備えていますが，標準アプリはそこまでの数を用意できていません。

## 起動構成

通常のUEFI起動では、すべてのBaramOS実行バイナリが `nano-system` の共通エントリを最初に通ります。Nano Systemは、UEFI補助機能とウォッチドッグ、最低限のフレームバッファ描画、キーボード・マウスの検出、任意の周期タイマー、ファームウェア経由のリセットを担当します。入力取得はタイマーに依存せず、タイマーを提供できないファームウェアでもNano Systemは起動します。初期化後は、取得したハードウェア情報と、利用可能な場合はタイマーをメインのBaramOSカーネルまたはアプリへ渡します。Nano Systemの起動に失敗した場合は、GOPを利用できる限り画面全体を赤色にして停止します。

`nano-system` はBaramOS本体のクレートに依存しない独立したUEFIアプリとしてもビルドできます。現在のメインカーネルと各サブシステムはNano Systemを同じUEFIバイナリへリンクしていますが、入口と引き渡し境界は分離してあるため、将来はこの境界を検証付きバイナリローダーによる起動へ置き換えられます。

```text
UEFI entry
  -> nano-system
       -> framebufferを取得して単色描画
       -> 入力デバイスをリセット・取得
       -> 周期タイマーを作成
  -> BaramOSメインカーネルまたはアプリ
       -> Nano Systemからデバイス情報・タイマーを受け取る
       -> 現在のGUIとOS機能を開始
```

ビルドスクリプトはCargo metadataを参照し、`nano-system` 自身と、それに直接依存するパッケージ内のUEFIバイナリを自動検出します。通常は `bootaa64` を起動バイナリとして選び、BaramOS本体が存在しない構成では独立した `nano-system` を選びます。`src/bin` 配下の対象はNano Systemを通るアプリとして自動的にビルド・収録されます。

カーネルが存在しない独立起動では、`nano-system` が診断画面を表示します。背景は `#000044`、マウス／トラックパッド位置は16px角の `#ffffff` で表示され、キーボード入力時は `#ffff00` になります。診断の入力ループとカーソル描画は周期タイマーを待たず、取得した移動を即座にフレームバッファへ反映します。カーネルが存在する通常構成では診断画面を実行せず、Nano Systemが所有するキーボードと、UEFIがSimple PointerまたはAbsolute Pointerとして公開するマウス・トラックパッドをカーネルへ引き渡します。現在値は公開された `NanoInputState` から参照できます。

## ライセンス
基本的に自由に使っていただいて構いません。自分でOSを作りたい時にコードを持っていったりしてもいいです。
しかし，[Apache License 2.0](LICENSE) の下で提供されています。ですので商用利用は可能ですが、使用・再配布の際はクレジット表記（Copyright notice）をお願いしております。

## オープンソース利用ライブラリ

HTML/CSSアプリ表示機能には追加の外部ライブラリを使用していません。以下はプロジェクト全体で利用しているライブラリです。

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

### KCC-KP-CheonRiMa-Normal-KP-2011KPS
- **ライセンス**：提供された TTF ファイル自体にライセンス通知が同梱されていないため、再配布条件は確認が必要です（OSS ライセンスとしては扱っていません）。
- **用途**：HarmonyOS Sans にグリフがないハングル文字の描画フォールバックとして `data/KCC-KP-CheonRiMa-Normal-KP-2011KPS.ttf` を使用します。

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

### 開発・検証限定の依存

次は OS イメージには含まれず、`png-decoder` のベンチマーク／検証時だけ利用します。

| ライブラリ | ライセンス | 用途 |
| --- | --- | --- |
| `criterion` | Apache-2.0 / MIT | PNG デコーダのベンチマーク |
| `image` | MIT | PNG デコード結果の検証 |
