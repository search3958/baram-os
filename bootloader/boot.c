/**
 * @file boot.c
 * @brief UEFI Bootloader using GNU-EFI
 * @license BSD License
 */

#include <efi.h>
#include <efilib.h>

// カーネルのエントリーポイント型定義
typedef void (*kernel_entry_t)(struct FramebufferInfo*);

// FrameBuffer 情報をカーネルに渡す構造体
struct FramebufferInfo {
    void* base_address;
    UINT32 width;
    UINT32 height;
    UINT32 pitch;
    UINT32 pixel_format;
};

/**
 * @brief UEFI メインエントリーポイント
 */
EFI_STATUS
EFIAPI
efi_main(EFI_HANDLE ImageHandle, EFI_SYSTEM_TABLE *SystemTable) {
    EFI_STATUS Status;
    
    // 初期化
    InitializeLib(ImageHandle, SystemTable);
    
    // 画面にメッセージ表示
    Print(L"Custom Kernel Bootloader\n");
    Print(L"Loading kernel...\n");
    
    // Graphics Output Protocol の取得
    EFI_GRAPHICS_OUTPUT_PROTOCOL *Gop;
    Status = uefi_call_wrapper(
        BS->LocateProtocol,
        3,
        &gEfiGraphicsOutputProtocolGuid,
        NULL,
        (VOID**)&Gop
    );
    
    if (EFI_ERROR(Status)) {
        Print(L"Failed to locate GOP: %r\n", Status);
        return Status;
    }
    
    // FrameBuffer 情報の準備
    struct FramebufferInfo fb_info;
    fb_info.base_address = (void*)Gop->Mode->FrameBufferBase;
    fb_info.width = Gop->Mode->Info->HorizontalResolution;
    fb_info.height = Gop->Mode->Info->VerticalResolution;
    fb_info.pitch = Gop->Mode->Info->PixelsPerScanLine * 4; // 4 bytes per pixel
    fb_info.pixel_format = Gop->Mode->Info->PixelFormat;
    
    Print(L"FrameBuffer: %dx%d @ 0x%p\n", 
          fb_info.width, fb_info.height, fb_info.base_address);
    
    // カーネルをメモリ上にロード（実際には別の方法でロードが必要）
    // ここでは簡易的にカーネルを直接呼び出す
    Print(L"Jumping to kernel...\n");
    
    // カーネルエントリーポイントのアドレス（実際には適切に設定）
    // kernel_entry_t kernel_entry = (kernel_entry_t)0x100000;
    // kernel_entry(&fb_info);
    
    Print(L"Kernel returned (should not reach here)\n");
    
    // 無限ループ
    while (1) {
        // halt();
    }
    
    return EFI_SUCCESS;
}
