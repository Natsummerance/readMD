@echo off
rem ============================================================
rem  ReadMD Packager - Native Rust Release Build
rem ============================================================
setlocal
cd /d "%~dp0..\.."
title ReadMD Packager

echo [1/2] Building release binary with Cargo...
cargo build --release -p readmd-kernel
if errorlevel 1 (
    echo [ReadMD] Cargo build failed.
    pause
    exit /b 1
)

echo [2/2] Copying binary to root ReadMD.exe...
copy /y "rust\target\release\readmd.exe" "ReadMD.exe" >nul

echo.
echo Done! Output: %CD%\ReadMD.exe
pause
exit /b 0
