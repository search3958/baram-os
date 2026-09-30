@echo off
setlocal enabledelayedexpansion

echo ============================================================
echo  BaramOS x86_64 Build and QEMU Launcher (Windows)
echo ============================================================
echo.

set "REPO_DIR=%~dp0"
if "%REPO_DIR:~-1%"=="\" set "REPO_DIR=%REPO_DIR:~0,-1%"
set "TARGET_DIR=%REPO_DIR%\target\x86_64-unknown-uefi\release"
set "RUNTIME_DIR=%REPO_DIR%\runtime"
set "IMAGE_NAME=osdisk-x64.img"
set "IMAGE_SIZE_MB=64"
set "FW_CODE=%RUNTIME_DIR%\edk2-x86_64-code.fd"
set "FW_VARS=%RUNTIME_DIR%\edk2-x86_64-vars.fd"
set "RUST_TOOLCHAIN=nightly-x86_64-pc-windows-gnu"
set "MINGW_BIN=C:\msys64\mingw64\bin"
set "MSYS_BIN=C:\msys64\usr\bin"
set "QEMU_BIN=%LOCALAPPDATA%\QEMU"
set "RUST_BIN=%USERPROFILE%\.cargo\bin"

if not exist "%QEMU_BIN%\qemu-system-x86_64w.exe" (
    echo [ERROR] QEMU not found at "%QEMU_BIN%"
    exit /b 1
)

if not exist "%RUST_BIN%\cargo.exe" (
    echo [ERROR] cargo not found at "%RUST_BIN%"
    echo Install Rust via rustup.rs
    exit /b 1
)

set "PATH=%QEMU_BIN%;%MINGW_BIN%;%MSYS_BIN%;%RUST_BIN%;%PATH%"

echo [INFO] REPO_DIR=%REPO_DIR%
echo [INFO] TARGET_DIR=%TARGET_DIR%
echo [INFO] RUNTIME_DIR=%RUNTIME_DIR%
echo.

:: =============================================================================
::  Build
:: =============================================================================
echo [INFO] Building bootx64.efi...

cd /d "%REPO_DIR%"

cargo +%RUST_TOOLCHAIN% build --release -Z build-std --target x86_64-unknown-uefi --bin nano-system
if errorlevel 1 (
    echo [ERROR] Build failed
    exit /b 1
)

if not exist "%TARGET_DIR%\bootx64.efi" (
    if exist "%TARGET_DIR%\nano-system.efi" (
        copy /y "%TARGET_DIR%\nano-system.efi" "%TARGET_DIR%\bootx64.efi" >nul
    )
)

if not exist "%TARGET_DIR%\bootx64.efi" (
    echo [ERROR] bootx64.efi not found after build
    exit /b 1
)

echo [INFO] Build complete: %TARGET_DIR%\bootx64.efi
echo.

:: =============================================================================
::  Create FAT32 disk image
:: =============================================================================
echo [INFO] Creating FAT32 disk image...

set "OUT=%RUNTIME_DIR%\%IMAGE_NAME%"
set "EFI=%TARGET_DIR%\bootx64.efi"

if exist "%OUT%" del /f "%OUT%"
powershell -NoProfile -Command "$f=[System.IO.File]::Create('%OUT%');$f.SetLength(%IMAGE_SIZE_MB%MB);$f.Close()"
if errorlevel 1 (
    echo [ERROR] Failed to create image file
    exit /b 1
)

mkfs.vfat -F 32 -n EFI "%OUT%"
if errorlevel 1 (
    echo [ERROR] Failed to format FAT32 image
    exit /b 1
)

mmd -i "%OUT%" ::/EFI
mmd -i "%OUT%" ::/EFI/BOOT
mmd -i "%OUT%" ::/files
mmd -i "%OUT%" ::/files/app
mmd -i "%OUT%" ::/files/data

mcopy -i "%OUT%" "%EFI%" ::/EFI/BOOT/BOOTX64.EFI

if exist "%REPO_DIR%\config.xml" (
    mcopy -i "%OUT%" "%REPO_DIR%\config.xml" ::/EFI/BOOT/config.xml
    echo [INFO]   copied config.xml
)

if exist "%REPO_DIR%\files\app" (
    mcopy -s -i "%OUT%" "%REPO_DIR%\files\app\*" ::/files/app/
)

if exist "%REPO_DIR%\files\data" (
    mcopy -s -i "%OUT%" "%REPO_DIR%\files\data\*" ::/files/data/
)

set "STARTUP=%TEMP%\startup.nsh"
echo fs0: > "%STARTUP%"
echo EFI\BOOT\BOOTX64.EFI >> "%STARTUP%"
mcopy -i "%OUT%" "%STARTUP%" ::/startup.nsh

echo [INFO] Image created: %OUT%
echo.

:: =============================================================================
::  Check firmware
:: =============================================================================
if exist "%FW_CODE%" if exist "%FW_VARS%" (
    echo [INFO] Firmware present
) else (
    echo [ERROR] UEFI firmware not found in %RUNTIME_DIR%
    echo Place edk2-x86_64-code.fd and edk2-x86_64-vars.fd there.
    exit /b 1
)

:: =============================================================================
::  Launch QEMU
:: =============================================================================
echo [INFO] Launching QEMU...
echo [INFO]   cpu    : qemu64
echo [INFO]   ram    : 0.15G
echo [INFO]   disk   : %RUNTIME_DIR%\%IMAGE_NAME%
echo [INFO] Press Ctrl+C in this window to exit QEMU.
echo.

qemu-system-x86_64w.exe ^
    -cpu qemu64 ^
    -m 0.15G ^
    -drive "if=pflash,format=raw,readonly=on,file=%FW_CODE%" ^
    -drive "if=pflash,format=raw,file=%FW_VARS%" ^
    -drive "if=none,file=%RUNTIME_DIR%\%IMAGE_NAME%,format=raw,id=hd0" ^
    -device "virtio-blk-pci,drive=hd0" ^
    -device "virtio-vga,edid=on,xres=1280,yres=720" ^
    -device qemu-xhci ^
    -device usb-tablet ^
    -device usb-mouse ^
    -device usb-kbd ^
    -display default ^
    -serial stdio ^
    -monitor none

echo [INFO] QEMU exited
endlocal
