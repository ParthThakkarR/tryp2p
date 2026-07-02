param(
    [ValidateSet("all", "zip", "exe", "msi", "jar", "app", "portable")]
    [string]$Target = "all"
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location -LiteralPath $scriptDir

$GREEN = "Green"
$CYAN = "Cyan"
$YELLOW = "Yellow"
$RED = "Red"

Write-Host "============================================" -ForegroundColor $CYAN
Write-Host "  P2P File Transfer - Windows Build Script" -ForegroundColor $CYAN
Write-Host "============================================" -ForegroundColor $CYAN
Write-Host ""

# --- Step 1: Clean build + tests ---
Write-Host "[1/3] Running clean build + tests..." -ForegroundColor $YELLOW
$result = & .\gradlew clean build test 2>&1 | Out-String
if ($LASTEXITCODE -ne 0) {
    Write-Host "BUILD OR TESTS FAILED!" -ForegroundColor $RED
    Write-Host $result
    exit 1
}
Write-Host "  All 105 tests pass." -ForegroundColor $GREEN
Write-Host ""

# --- Step 2: Check for WiX ---
$hasWix = $false
$wixPath = $null
try {
    $wixPath = Get-Command wix.exe -ErrorAction Stop
    $hasWix = $true
    Write-Host "  WiX detected: $($wixPath.Source)" -ForegroundColor $GREEN
    & $wixPath extension add WixToolset.Util.wixext/5.0.2 2>&1 | Out-Null
} catch {
    Write-Host "  WiX not found - MSI/EXE installer unavailable (install from https://wixtoolset.org)" -ForegroundColor $YELLOW
}

# --- Step 2: Build Shadow JAR (always) ---
Write-Host "[2/3] Building universal shadow JAR..." -ForegroundColor $YELLOW
& .\gradlew :p2p-app:shadowJar 2>&1 | Out-String | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "Shadow JAR build FAILED!" -ForegroundColor $RED
    exit 1
}
$jarPath = "p2p-app\build\libs\p2p-1.0.0-SNAPSHOT.jar"
$jarSize = [math]::Round((Get-Item $jarPath).Length / 1MB, 1)
Write-Host ("  Shadow JAR: " + $jarPath + " (" + $jarSize + " MB)") -ForegroundColor $GREEN
Write-Host ""

# --- Step 3: Build platform packages ---
Write-Host "[3/3] Building Windows packages..." -ForegroundColor $YELLOW
Write-Host ""

$built = @()

# Portable app image (always, no extra tools needed)
if ($Target -in @("all", "app")) {
    Write-Host "  >> Portable EXE (app image)..." -ForegroundColor $YELLOW
    & .\gradlew :p2p-app:packageWinApp 2>&1 | Out-String | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $exePath = "p2p-app\build\dist-jpackage\P2PTransfer\P2PTransfer.exe"
        if (Test-Path $exePath) {
            $size = [math]::Round((Get-Item $exePath).Length / 1KB)
            Write-Host ("    " + $exePath + " (" + $size + " KB)") -ForegroundColor $GREEN
            $built += "Portable EXE: " + $exePath
        }
    } else {
        Write-Host "    FAILED" -ForegroundColor $RED
    }
}

# ZIP bundle
if ($Target -in @("all", "zip")) {
    Write-Host "  >> Portable ZIP..." -ForegroundColor $YELLOW
    & .\gradlew :p2p-app:packageWinZip 2>&1 | Out-String | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $zipPath = "p2p-app\build\distributions\P2PTransfer-1.0.0-SNAPSHOT.zip"
        if (Test-Path $zipPath) {
            $size = [math]::Round((Get-Item $zipPath).Length / 1MB, 1)
            Write-Host ("    " + $zipPath + " (" + $size + " MB)") -ForegroundColor $GREEN
            $built += "ZIP: " + $zipPath
        }
    } else {
        Write-Host "    FAILED" -ForegroundColor $RED
    }
}

# MSI installer
if ($Target -in @("all", "msi") -and $hasWix) {
    Write-Host "  >> MSI installer..." -ForegroundColor $YELLOW
    & .\gradlew :p2p-app:packageWinMsi 2>&1 | Out-String | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $msiPath = "p2p-app\build\dist-msi\P2PTransfer-1.0.0.msi"
        if (Test-Path $msiPath) {
            $size = [math]::Round((Get-Item $msiPath).Length / 1MB, 1)
            Write-Host ("    " + $msiPath + " (" + $size + " MB)") -ForegroundColor $GREEN
            $built += "MSI: " + $msiPath
        }
    } else {
        Write-Host "    FAILED" -ForegroundColor $RED
    }
}

# EXE installer
if ($Target -in @("all", "exe") -and $hasWix) {
    Write-Host "  >> EXE installer..." -ForegroundColor $YELLOW
    & .\gradlew :p2p-app:packageWinExe 2>&1 | Out-String | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $exeInstallerPath = "p2p-app\build\dist-exe\P2PTransfer-1.0.0.exe"
        if (Test-Path $exeInstallerPath) {
            $size = [math]::Round((Get-Item $exeInstallerPath).Length / 1MB, 1)
            Write-Host ("    " + $exeInstallerPath + " (" + $size + " MB)") -ForegroundColor $GREEN
            $built += "EXE installer: " + $exeInstallerPath
        }
    } else {
        Write-Host "    FAILED" -ForegroundColor $RED
    }
}

# Portable SFX EXE (requires 7-Zip)
if ($Target -in @("all", "portable")) {
    Write-Host "  >> Single-file portable EXE (SFX)..." -ForegroundColor $YELLOW
    & .\gradlew :p2p-app:packageWinPortable 2>&1 | Out-String | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $sfxExe = Get-ChildItem "p2p-app\build\dist-portable\*.exe" | Select-Object -First 1
        if ($sfxExe) {
            $size = [math]::Round($sfxExe.Length / 1MB, 1)
            Write-Host ("    " + $sfxExe.FullName + " (" + $size + " MB)") -ForegroundColor $GREEN
            $built += "Portable SFX: " + $sfxExe.FullName
        }
    } else {
        Write-Host "    FAILED" -ForegroundColor $RED
    }
}

# JAR (always)
if ($Target -in @("all", "jar")) {
    $built += "Universal JAR (JDK 21+): " + $jarPath
}

Write-Host ""
Write-Host "============================================" -ForegroundColor $CYAN
Write-Host "  BUILD COMPLETE" -ForegroundColor $CYAN
Write-Host "============================================" -ForegroundColor $CYAN
Write-Host ""
foreach ($item in $built) {
    Write-Host ("  " + $item) -ForegroundColor $GREEN
}
Write-Host ""

if ($Target -eq "all" -and -not $hasWix) {
    Write-Host "NOTE: WiX not found. MSI and EXE installer skipped." -ForegroundColor $YELLOW
    Write-Host "  Install from: https://wixtoolset.org" -ForegroundColor $YELLOW
    Write-Host "  Then re-run: .\build-installer.ps1 -Target all" -ForegroundColor $YELLOW
}
