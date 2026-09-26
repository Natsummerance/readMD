@echo off
rem ============================================================
rem  ReadMD - one-click run (native Rust ReadMD.exe)
rem ============================================================
setlocal
cd /d "%~dp0.."
if exist "ReadMD.exe" (
    start "" "ReadMD.exe" %*
    exit /b 0
)
if exist "rust\target\release\readmd.exe" (
    start "" "rust\target\release\readmd.exe" %*
    exit /b 0
)
echo [ReadMD] ReadMD.exe not found. Please build with cargo build --release.
pause
exit /b 1
