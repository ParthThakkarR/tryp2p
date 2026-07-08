param([string]$Version = "1.0.0")

$env:Path = "C:\msys64\msys64\mingw64\bin;$env:USERPROFILE\.cargo\bin;$env:Path"

$ReleaseDir = "releases\v$Version\Windows"
mkdir -Force $ReleaseDir | Out-Null

Write-Host "Building p2ptransfer v$Version for Windows..."

# Build CLI
Write-Host "  [1/3] Building CLI binary..."
cargo build -p p2ptransfer-cli --release
Copy-Item "target\release\p2p.exe" "$ReleaseDir\p2ptransfer-cli-$Version-windows-x86_64.exe"

# Build GUI (Tauri)
Write-Host "  [2/3] Building GUI (Tauri)..."
$TauriTarget = if (Get-Command cl.exe -ErrorAction SilentlyContinue) { "x86_64-pc-windows-msvc" } else { "x86_64-pc-windows-gnu" }
$CargoTarget = "target\$(if ($TauriTarget -ne '') { $TauriTarget + '\' } else { '' })release"
$CargoTargetSimple = if ($TauriTarget -ne 'x86_64-pc-windows-msvc') { "target\$TauriTarget\release" } else { "target\release" }

Push-Location desktop\p2ptransfer-gui
npm run tauri build -- --target $TauriTarget
Pop-Location

# Copy portable GUI exe (Tauri outputs it in the target-triple dir)
Copy-Item "$CargoTargetSimple\p2ptransfer.exe" "$ReleaseDir\p2ptransfer-gui-$Version-windows-portable.exe"

# Copy MSI installer (built by Tauri automatically)
$MsiFile = Resolve-Path "$CargoTargetSimple\bundle\msi\*.msi" -ErrorAction SilentlyContinue
if ($MsiFile) {
    Copy-Item $MsiFile "$ReleaseDir\p2ptransfer-gui-$Version-windows.msi"
}

# Copy NSIS installer (built by Tauri automatically)
$NsisFile = Resolve-Path "$CargoTargetSimple\bundle\nsis\*.exe" -ErrorAction SilentlyContinue
if ($NsisFile) {
    Copy-Item $NsisFile "$ReleaseDir\p2ptransfer-gui-$Version-windows-setup.exe"
}

# Build MSI (requires WiX Toolset)
Write-Host "  [3/3] Building MSI installer (requires WiX)..."
if (Get-Command heat -ErrorAction SilentlyContinue) {
    cd wix
    .\build-wix.bat
    Copy-Item "output\p2ptransfer-cli.msi" "..\$ReleaseDir\p2ptransfer-gui-$Version-windows.msi"
    cd ..
} else {
    Write-Host "    [WARN] WiX Toolset not installed. Skipping MSI build."
    Write-Host "    Download from: https://wixtoolset.org/"
}

Write-Host "Windows build complete: $ReleaseDir"
Get-ChildItem $ReleaseDir
