/*
 * UEFI Application Entry Point for ARM64
 * License: MIT License
 */

#include <stdint.h>
#include <efi.h>
#include <efilib.h>

extern void main_kernel(uint32_t* framebuffer, uint32_t width, uint32_t height, uint32_t pitch);

EFI_STATUS
EFIAPI
efi_main(EFI_HANDLE ImageHandle, EFI_SYSTEM_TABLE *SystemTable) {
    InitializeLib(ImageHandle, SystemTable);
    
    EFI_GRAPHICS_OUTPUT_PROTOCOL *gop;
    EFI_STATUS status = uefi_call_wrapper(
        BS->LocateProtocol,
        3,
        &gop,
        NULL,
        NULL
    );
    
    if (status != EFI_SUCCESS) {
        Print(L"Failed to locate GOP protocol\r\n");
        return status;
    }
    
    uint32_t* framebuffer = (uint32_t*)gop->Mode->FrameBufferBase;
    uint32_t width = gop->Mode->Info->HorizontalResolution;
    uint32_t height = gop->Mode->Info->VerticalResolution;
    uint32_t pitch = gop->Mode->Info->PixelsPerScanLine;
    
    Print(L"Baram OS Starting...\r\n");
    Print(L"Framebuffer: %dx%d\r\n", width, height);
    
    main_kernel(framebuffer, width, height, pitch);
    
    return EFI_SUCCESS;
}
