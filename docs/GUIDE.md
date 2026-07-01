# 自作カーネル開発ガイド

## 概要
このプロジェクトは、UEFI 対応の x86_64/ARM64 両対応の自作カーネルです。Linux のクローンではなく、独自の設計を目指しています。

## ライセンス
- **カーネル**: MIT License（営利目的可能）
- **推奨ブートローダー**: GNU-EFI (BSD License)
- **全ての依存ライブラリ**: 営利目的可能なオープンソースライセンス

## 必要要件

### x86_64 ビルド
- GCC (x86_64-linux-gnu-gcc)
- LD (GNU linker)
- QEMU (テスト用)

### ARM64 ビルド
- GCC (aarch64-linux-gnu-gcc)
- LD (aarch64-linux-gnu-ld)
- QEMU (テスト用)

## ビルド方法

### x86_64
```bash
./build_x86_64.sh
```

### ARM64
```bash
./build_arm64.sh
```

## テスト方法

### x86_64 (QEMU)
```bash
qemu-system-x86_64 -kernel kernel.bin -serial stdio
```

### ARM64 (QEMU)
```bash
qemu-system-aarch64 -machine virt -cpu cortex-a57 -kernel kernel_arm64.bin -serial stdio
```

## UEFI ブートローダーとの連携

### GNU-EFI の使用（推奨）
GNU-EFI は BSD ライセンスで、営利目的でも利用可能です。

1. GNU-EFI をインストール:
```bash
git clone https://github.com/tianocore/edk2.git
cd edk2
# ビルド手順は README を参照
```

2. UEFI アプリケーションとしてカーネルをロードするブートローダーを作成

### 簡単な UEFI スタブ例
```c
#include <efi.h>
#include <efilib.h>

EFI_STATUS
EFIAPI
efi_main(EFI_HANDLE ImageHandle, EFI_SYSTEM_TABLE *SystemTable) {
    InitializeLib(ImageHandle, SystemTable);
    
    // FrameBuffer 情報を取得
    // カーネルをロードして起動
    return EFI_SUCCESS;
}
```

## 実装済み機能

### ディスプレイ出力 (FrameBuffer)
- UEFI Graphics Output Protocol (GOP) から FrameBuffer 情報を取得
- 対応ピクセルフォーマット:
  - RGB888
  - BGR888
  - RGBX8888
  - BGRX8888
- 描画 API:
  - ピクセル描画
  - 四角形描画（輪郭）
  - 線描画
  - 画面クリア

### マウスポインター
- 16x16 のシンプルな四角形カーソル
- 画面内移動制限
- 可視/不可視切り替え

### キーボード入力
- PS/2 キーボード対応 (x86_64)
- スキャンコード処理
- 修飾キー（Shift, Ctrl, Alt）対応
- キーバッファ（256 エントリ）

## アーキテクチャ固有の実装

### x86_64
- I/O ポートアクセス（inb/outb）
- PS/2 キーボードコントローラ
- PIC/APIC 割り込み制御（準備中）

### ARM64
- MMIO アクセス
- UART シリアル（準備中）
- GIC 割り込み制御（準備中）

## 次のステップ

1. **UEFI ブートローダーの統合**
   - GNU-EFI を使用したブートローダーの作成
   - FrameBuffer 情報のカーネルへの受け渡し

2. **割り込みハンドリング**
   - x86_64: IDT の設定
   - ARM64: 例外ベクトルの設定

3. **メモリ管理**
   - ページングの設定
   - ヒープアロケータ

4. **デバイスドライバ**
   - USB マウスドライバ
   - より高度なキーボードドライバ

## 参考資料

### オープンソースプロジェクト
- [GNU-EFI](https://sourceforge.net/projects/gnu-efi/) - BSD License
- [OSDev Wiki](https://wiki.osdev.org/) - 様々なアーキテクチャの情報
- [Raspberry Pi OS](https://github.com/s-matyukevich/raspberry-pi-os) - MIT License

### 仕様書
- [UEFI Specification](https://uefi.org/specifications)
- [AMD64 Architecture Programmer's Manual](https://www.amd.com/system/files/TechDocs/24594.pdf)
- [ARM Architecture Reference Manual](https://developer.arm.com/documentation)

## ライセンス確認済みコンポーネント

| コンポーネント | ライセンス | 営利目的 |
|---------------|-----------|---------|
| カーネル本体 | MIT | ✓ |
| GNU-EFI | BSD | ✓ |
| EDK II | BSD | ✓ |

## 注意事項

- このカーネルは教育・学習目的で作成されています
- 実用的な OS として使用するには更なる開発が必要です
- ハードウェアとの互換性は限定的です
