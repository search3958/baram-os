# BaramOS

ARM64とx86_64向けOSです。
Raspberry Pi 4B(マウスが動作しない)と一般的なx86_64ラップトップでの動作が確認できています。
滑らかな角丸，美しいブラー効果があるリッチなGUIのOSです。OSとして基礎的な機能は備えていますが，標準アプリはそこまでの数を用意できていません。

## HTMLアプリ

`app/index.yaml` に `type: html-1` として登録した `.html` ファイルを、通常のBaramOSアプリとして表示できます。HTMLアプリと同じ `app/` ディレクトリに置いたCSSは、次のように読み込めます。

```yaml
apps:
  example.html:
    icon: noname.png
    type: html-1
    title: HTMLアプリ
```

```html
<link rel="stylesheet" href="example.css">
```

現在はネットワークブラウザではなく、UEFI上で安定して動作するアプリUI向けのHTML/CSSサブセットです。見出し、段落、リスト、`div`、`section`、`span`、リンクなどの構造と、色、背景、余白、枠線、角丸、幅・高さ、文字揃え、Flexの行／列レイアウトなどに対応しています。インラインの`<style>`と`style=""`も使用できます。JavaScript、HTTP通信、iframeは実行しません。

BaramOS固有の連携は次の通りです。

```html
<!-- OS設定を変更する -->
<a href="os://display/hud?enabled=0">HUDを隠す</a>

<!-- index.yamlに登録済みのアプリを開く -->
<a href="app://calc.warp">計算機を開く</a>

<!-- OS設定値を表示する。os://リンク実行後は再取得される -->
<span config="os://display/hud/enabled">値がない場合の表示</span>
```

`app://` は安全のため、パスを含まない登録済みファイル名（`.warp`、`.html`、`.htm`、`.u1`、`.w3a`）だけを受け付けます。実装例は `app/webdemo.html` と `app/webdemo.css` にあります。

## Warp 3アプリ

Warp 3はブラウザやJavaScriptを介さず、WindowServerでネイティブ描画されます。HTML版コンパイラと同じ`config.ini`、画面ごとの`<screen>.w3u`、`script("...")`で参照する`.w3s`を`app/<アプリ名>.w3a/`へまとめ、`app/index.yaml`へ次のように登録します。ビルド時に同名の単一アーカイブへ変換され、OSではアプリ名を名前空間として内部ファイルを読み出します。

```yaml
apps:
  example.w3a:
    icon: noname.png
    type: warp-3
    title: Warp 3アプリ
```

ボタン、カード、Flex、入力欄、タブ、スイッチ、ツールバー、リスト、コード、スクロールポイントに対応しています。ホバーと入力フォーカス、画面遷移、相対／名前指定スクロール、変数、関数、`setText`、`getText`も利用できます。アニメーションを行わないため、`wait`は構文として受理しますが表示待ちは発生しません。

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
