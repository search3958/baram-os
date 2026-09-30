param([string]$Mode = "")

$ErrorActionPreference = 'Stop'

$RepoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$TargetDir = Join-Path $RepoDir 'target\x86_64-unknown-uefi\release'
$RuntimeDir = Join-Path $RepoDir 'runtime'
$ImageName = 'osdisk-x64.img'
$ImageSizeMB = 64
$FwCode = Join-Path $RuntimeDir 'edk2-x86_64-code.fd'
$FwVars = Join-Path $RuntimeDir 'edk2-x86_64-vars.fd'
$RustToolchain = 'nightly-x86_64-pc-windows-gnu'
$MingwBin = 'C:\msys64\mingw64\bin'
$MsysBin = 'C:\msys64\usr\bin'
$QemuBin = Join-Path $env:LOCALAPPDATA 'QEMU'

function Assert-ToolExists($name, $path) {
    if (-not (Test-Path $path)) {
        Write-Host "[ERROR] $name not found at $path"
        exit 1
    }
}

Assert-ToolExists 'QEMU' (Join-Path $QemuBin 'qemu-system-x86_64w.exe')
Assert-ToolExists 'cargo' (Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe')
Assert-ToolExists 'MinGW GCC' (Join-Path $MingwBin 'x86_64-w64-mingw32-gcc.exe')
Assert-ToolExists 'mkfs.vfat' (Join-Path $MsysBin 'mkfs.vfat.exe')
Assert-ToolExists 'mcopy' (Join-Path $MingwBin 'mcopy.exe')
Assert-ToolExists 'mmd' (Join-Path $MingwBin 'mmd.exe')
Assert-ToolExists 'tar' (Join-Path $MsysBin 'tar.exe')

$env:PATH = "$QemuBin;$MingwBin;$MsysBin;" + (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:PATH
$env:RUSTUP_TOOLCHAIN = $RustToolchain

Write-Host '==========================================================='
Write-Host '  BaramOS x86_64 Build and QEMU Launcher (Windows)'
Write-Host '==========================================================='
Write-Host ""

$DoBuild = $Mode -in @('','build','image','run')
$DoImage = $Mode -in @('','image','run')
$DoRun   = $Mode -in @('','run')

if ($DoBuild) {
    Write-Host '[INFO] Building bootx64.efi...'

    rustup toolchain list | Select-String -Quiet $RustToolchain
    if (-not $?) {
        Write-Host '[WARN] Installing Rust toolchain...'
        rustup toolchain install $RustToolchain --component rust-src
    }

    rustup target list --toolchain $RustToolchain | Select-String -Quiet 'x86_64-unknown-uefi'
    if (-not $?) {
        Write-Host '[WARN] Installing target x86_64-unknown-uefi...'
        rustup target add x86_64-unknown-uefi --toolchain $RustToolchain
    }

    Push-Location $RepoDir
    cargo +$RustToolchain build --release -Z build-std --target x86_64-unknown-uefi -p baram-boot --bin bootaa64
    if ($LASTEXITCODE -ne 0) {
        Write-Host '[ERROR] Build bootaa64 failed'
        Pop-Location
        exit 1
    }

    $bins = @('baram-kernel','windowserver','font','graphics','iokit','bsd')
    foreach ($b in $bins) {
        Write-Host "[INFO] Building $b..."
        cargo +$RustToolchain build --release -Z build-std --target x86_64-unknown-uefi -p baram-boot --bin $b
        if ($LASTEXITCODE -ne 0) {
            Write-Host "[ERROR] Build $b failed"
            Pop-Location
            exit 1
        }
    }
    Pop-Location

    $bootaa64 = Join-Path $TargetDir 'bootaa64.efi'
    $bootx64 = Join-Path $TargetDir 'bootx64.efi'
    if (Test-Path $bootx64) { Remove-Item $bootx64 -Force }
    if (Test-Path $bootaa64) {
        Copy-Item $bootaa64 $bootx64 -Force
    }
    if (-not (Test-Path $bootx64)) {
        Write-Host '[ERROR] bootx64.efi not found after build'
        exit 1
    }
    Write-Host "[INFO] Build complete: $bootx64"
    Write-Host ''

    if ($Mode -eq 'build') { exit 0 }
}

if ($DoImage) {
    Write-Host '[INFO] Creating FAT32 disk image...'

    $Out = Join-Path $RuntimeDir $ImageName
    $Efi = Join-Path $TargetDir 'bootx64.efi'
    if (Test-Path $Out) { Remove-Item $Out -Force }
    $fs = [System.IO.File]::Create($Out)
    $fs.SetLength($ImageSizeMB * 1MB)
    $fs.Close()

    mkfs.vfat -F 32 -n EFI $Out
    mmd -i $Out ::/EFI
    mmd -i $Out ::/EFI/BOOT
    mmd -i $Out ::/EFI/BOOT/bin
    mmd -i $Out ::/files
    mmd -i $Out ::/files/app
    mmd -i $Out ::/files/data

    mcopy -i $Out $Efi ::/EFI/BOOT/BOOTX64.EFI
    $bins = @('baram-kernel','windowserver','font','graphics','iokit','bsd')
    foreach ($b in $bins) {
        $p = Join-Path $TargetDir "$b.efi"
        if (Test-Path $p) {
            mcopy -i $Out $p ::/EFI/BOOT/bin/
            Write-Host "  [INFO] copied $b.efi"
        }
    }
    $config = Join-Path $RepoDir 'config.xml'
    if (Test-Path $config) {
        mcopy -i $Out $config ::/EFI/BOOT/config.xml
        Write-Host '  [INFO] copied config.xml'
    }

    Write-Host '[INFO] Packaging app data...'
    $StageDir = Join-Path $env:TEMP 'baramos-files-stage'
    if (Test-Path $StageDir) { Remove-Item $StageDir -Recurse -Force }
    $StageApp = Join-Path $StageDir 'app'
    $StageData = Join-Path $StageDir 'data'
    New-Item -ItemType Directory -Path $StageApp -Force | Out-Null
    New-Item -ItemType Directory -Path $StageData -Force | Out-Null

    $srcApp = Join-Path $RepoDir 'files\app'
    $srcData = Join-Path $RepoDir 'files\data'
    if (Test-Path $srcApp) { Copy-Item "$srcApp\*" $StageApp -Recurse -Force }
    if (Test-Path $srcData) { Copy-Item "$srcData\*" $StageData -Recurse -Force }

    if (Test-Path $StageApp) {
        $cygpath = Join-Path $MsysBin 'cygpath.exe'
        Get-ChildItem $StageApp -Directory | ForEach-Object {
            if ($_.Extension -in '.w4a','.w3a','.s4a') {
                $name = $_.Name
                $archive = Join-Path $StageApp $name
                $tmpArchive = Join-Path $StageApp "$name.tar.tmp"
                $srcPosix = & $cygpath (Join-Path $srcApp $name)
                $tmpArchivePosix = & $cygpath $tmpArchive
        & (Join-Path $MsysBin 'tar.exe') --format=ustar -cf $tmpArchivePosix -C $srcPosix .
        if ($LASTEXITCODE -eq 0) {
            Remove-Item $_.FullName -Recurse -Force
            Move-Item -LiteralPath $tmpArchive -Destination $archive -Force
        } else {
            Write-Host "[ERROR] tar failed for $name"
            Pop-Location
            exit 1
        }
            }
        }
        Get-ChildItem $StageApp -Force | ForEach-Object {
            $dest = "::/files/app/" + $_.Name
            if ($_.PSIsContainer) {
                mcopy -s -i $Out $_.FullName $dest
            } else {
                mcopy -i $Out $_.FullName $dest
            }
        }
    }
    if (Test-Path $StageData) {
        Get-ChildItem $StageData -Force | ForEach-Object {
            $dest = "::/files/data/" + $_.Name
            if ($_.PSIsContainer) {
                mcopy -s -i $Out $_.FullName $dest
            } else {
                mcopy -i $Out $_.FullName $dest
            }
        }
    }

    $Startup = Join-Path $env:TEMP 'startup.nsh'
    "fs0:`r`nEFI\BOOT\BOOTX64.EFI" | Set-Content -Path $Startup -Encoding ASCII
    mcopy -i $Out $Startup ::/startup.nsh

    Write-Host "[INFO] Image created: $Out"
    Write-Host ''

    if ($Mode -eq 'image') { exit 0 }
}

if ($DoRun) {
    if (-not (Test-Path $FwCode) -or -not (Test-Path $FwVars)) {
        Write-Host "[ERROR] UEFI firmware not found in $RuntimeDir"
        exit 1
    }

    Write-Host '[INFO] Launching QEMU...'
    Write-Host "[INFO]   disk: $(Join-Path $RuntimeDir $ImageName)"
    Write-Host '[INFO] Press Ctrl+C to exit.'
    Write-Host ''

    $qemuArgs = @(
        '-cpu', 'qemu64',
        '-m', '0.15G',
        '-drive', "if=pflash,format=raw,readonly=on,file=$FwCode",
        '-drive', "if=pflash,format=raw,file=$FwVars",
        '-drive', "if=none,file=$(Join-Path $RuntimeDir $ImageName),format=raw,id=hd0",
        '-device', 'virtio-blk-pci,drive=hd0',
        '-device', 'virtio-vga,edid=on,xres=1280,yres=720',
        '-device', 'qemu-xhci',
        '-device', 'usb-tablet',
        '-device', 'usb-mouse',
        '-device', 'usb-kbd',
        '-display', 'default',
        '-serial', 'stdio',
        '-monitor', 'none'
    )

    & (Join-Path $QemuBin 'qemu-system-x86_64w.exe') @qemuArgs
}
