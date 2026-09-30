param(
    [string]$Image = "",
    [int]$DiskNumber = 1,
    [char]$DriveLetter = 'E'
)

$ErrorActionPreference = 'Stop'

$RepoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$img = if ($Image) { $Image } else { Join-Path $RepoDir 'runtime\osdisk-x64.img' }
$out = Join-Path $RepoDir 'runtime\flash-result.txt'
$ddLog = Join-Path $RepoDir 'runtime\dd-log.txt'
$diskpartScript = Join-Path $env:TEMP 'kilo-diskpart-write64.txt'

function Write-Result($text) {
    Set-Content -LiteralPath $out -Value $text -Encoding ASCII
    Write-Host $text
}

try {
    if (-not (Test-Path $img)) {
        Write-Result "ERROR:Image not found: $img"
        exit 1
    }

    $imgSize = (Get-Item $img).Length
    Write-Host "[INFO] Image: $img ($imgSize bytes)"

    $disk = Get-Disk -Number $DiskNumber -ErrorAction Stop
    if ($disk.BusType -ne 'USB') {
        Write-Result "ERROR:Disk $DiskNumber is not USB ($($disk.BusType))"
        exit 1
    }
    Write-Host "[INFO] Target disk: $($disk.FriendlyName) ($($disk.Size) bytes)"

    $vol = Get-Volume -DriveLetter $DriveLetter -ErrorAction SilentlyContinue
    if (-not $vol) {
        Write-Result "ERROR:Drive $DriveLetter not found"
        exit 1
    }
    if ($vol.FileSystem -ne 'FAT32') {
        Write-Result "ERROR:Drive $DriveLetter is not FAT32 ($($vol.FileSystem))"
        exit 1
    }
    Write-Host "[INFO] Target volume: $DriveLetter: ($($vol.FileSystem) $($vol.Size) bytes)"

    Write-Host "[INFO] Writing image to PhysicalDrive$DiskNumber ..."
    $dd = 'C:\msys64\usr\bin\dd.exe'
    if (-not (Test-Path $dd)) { $dd = 'dd' }

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $dd
    $psi.Arguments = "if=`"$img`" of=`"\\.\PhysicalDrive$DiskNumber`" bs=4M status=progress"
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $psi.RedirectStandardOutput = $true
    $p = [System.Diagnostics.Process]::Start($psi)
    $p.WaitForExit()
    $stdout = $p.StandardOutput.ReadToEnd()
    $stderr = $p.StandardError.ReadToEnd()
    Set-Content -LiteralPath $ddLog -Value "EXIT=$($p.ExitCode)`r`nSTDOUT=$stdout`r`nSTDERR=$stderr" -Encoding ASCII

    if ($p.ExitCode -ne 0) {
        Write-Result "ERROR:dd exit $($p.ExitCode)"
        exit $p.ExitCode
    }
    Write-Host "[INFO] dd complete. Rescanning disk..."

    Set-Content -LiteralPath $diskpartScript -Value "rescan`r`exit" -Encoding ASCII
    diskpart /s $diskpartScript | Out-Null
    Start-Sleep -Seconds 3

    $volAfter = Get-Volume -DriveLetter $DriveLetter -ErrorAction SilentlyContinue
    if ($volAfter -and $volAfter.OperationalStatus -eq 'OK') {
        Write-Host "[INFO] Volume $DriveLetter: recognized after rescan"
    } else {
        Write-Host "[WARN] Volume $DriveLetter: not recognized after rescan. If needed, unplug/replug USB or run diskpart clean/create partition/format/assign manually."
    }

    Write-Result "OK: flashed to USB (PhysicalDrive$DiskNumber)"
} catch {
    Write-Result "ERROR:$($_.Exception.Message)"
    exit 1
} finally {
    if (Test-Path $diskpartScript) { Remove-Item $diskpartScript -Force -ErrorAction SilentlyContinue }
}
