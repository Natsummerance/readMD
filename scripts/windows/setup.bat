@echo off
rem ============================================================
rem  ReadMD Setup - Native Rust One-Click Setup
rem ============================================================
setlocal
cd /d "%~dp0..\.."
title ReadMD Setup

echo [1/3] Building release binary with Cargo...
cargo build --release -p readmd-kernel
if errorlevel 1 (
    echo [ReadMD] Cargo build failed.
    pause
    exit /b 1
)

echo [2/3] Preparing ReadMD.exe...
copy /y "rust\target\release\readmd.exe" "ReadMD.exe" >nul

echo [3/3] Registering file associations...
"%CD%\ReadMD.exe" --assoc

echo.
echo Launching ReadMD...
start "" "%CD%\ReadMD.exe"

echo Done! ReadMD is installed and ready.
pause
exit /b 0
