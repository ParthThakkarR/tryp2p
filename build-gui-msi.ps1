# build-gui-msi.ps1
# Builds the P2P Transfer GUI and produces MSI + NSIS installers.
# Run from the repo root: .\build-gui-msi.ps1
# Output: target\release\bundle\msi\p2ptransfer_*.msi
#         target\release\bundle\nsis\p2ptransfer_*-setup.exe

$ErrorActionPreference = "Stop"

$SCRIPT_DIR = $PSScriptRoot
$GUI_DIR    = Join-Path $SCRIPT_DIR "desktop\p2ptransfer-gui"

Write-Host "==> Building P2P Transfer GUI MSI" -ForegroundColor Cyan
Write-Host "    Project root : $SCRIPT_DIR"
Write-Host "    GUI directory: $GUI_DIR"
Write-Host ""

# 1. Install npm dependencies
Write-Host "[1/3] Installing npm dependencies..." -ForegroundColor Yellow
Push-Location $GUI_DIR
try {
    npm install
    if ($LASTEXITCODE -ne 0) { throw "npm install failed" }
} finally {
    Pop-Location
}

# 2. Build frontend (tsc + vite)
Write-Host ""
Write-Host "[2/3] Building frontend..." -ForegroundColor Yellow
Push-Location $GUI_DIR
try {
    npm run build
    if ($LASTEXITCODE -ne 0) { throw "npm run build failed" }
} finally {
    Pop-Location
}

# 3. Build Tauri app (Rust + bundle MSI/NSIS)
Write-Host ""
Write-Host "[3/3] Bundling MSI/NSIS installer..." -ForegroundColor Yellow
Push-Location $GUI_DIR
try {
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "npm run tauri build failed" }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "==> Build complete!" -ForegroundColor Green

$BUNDLE_DIR = Join-Path $SCRIPT_DIR "target\release\bundle"
$MSI_DIR    = Join-Path $BUNDLE_DIR "msi"
$NSIS_DIR   = Join-Path $BUNDLE_DIR "nsis"

if (Test-Path $MSI_DIR) {
    Get-ChildItem "$MSI_DIR\*.msi" | ForEach-Object {
        Write-Host "  MSI : $($_.FullName)" -ForegroundColor Green
    }
}
if (Test-Path $NSIS_DIR) {
    Get-ChildItem "$NSIS_DIR\*-setup.exe" | ForEach-Object {
        Write-Host "  NSIS: $($_.FullName)" -ForegroundColor Green
    }
}
