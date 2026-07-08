@echo off
REM Build MSI installer for p2ptransfer CLI using WiX Toolset
REM Prerequisites: WiX Toolset installed (http://wixtoolset.org)

setlocal
set "P2P_SRC_DIR=%~dp0.."
set "P2P_TARGET_DIR=%P2P_SRC_DIR%\target"

echo Building release binary...
call cargo build --release -p p2ptransfer-cli
if %ERRORLEVEL% neq 0 exit /b %ERRORLEVEL%

echo Compiling WiX source...
candle -dP2P_TARGET_DIR="%P2P_TARGET_DIR%" -dP2P_SRC_DIR="%P2P_SRC_DIR%" -out "%TEMP%\p2ptransfer.wixobj" "%P2P_SRC_DIR%\wix\p2ptransfer-cli.wxs"
if %ERRORLEVEL% neq 0 exit /b %ERRORLEVEL%

echo Linking MSI...
light -out "%P2P_SRC_DIR%\target\release\p2ptransfer-0.1.0.msi" "%TEMP%\p2ptransfer.wixobj"
if %ERRORLEVEL% neq 0 exit /b %ERRORLEVEL%

echo MSI created: target\release\p2ptransfer-0.1.0.msi
