@echo off
rem ============================================================
rem  ReadMD Installer - Native Rust Edition
rem  Registers .md file associations and starts ReadMD
rem ============================================================
setlocal
cd /d "%~dp0..\.."
title ReadMD Installer

set "EXE="
if exist "%CD%\ReadMD.exe" set "EXE=%CD%\ReadMD.exe"
if "%EXE%"=="" if exist "%CD%\rust\target\release\readmd.exe" set "EXE=%CD%\rust\target\release\readmd.exe"

if "%EXE%"=="" (
    echo [ReadMD] ReadMD.exe not found.
    echo Please compile the release binary first:
    echo   cargo build --release -p readmd-kernel
    pause
    exit /b 1
)

echo [1/2] Registering file associations via native Win32 FFI...
"%EXE%" --assoc

echo [2/2] Done!
echo Double-click any .md file to open it with ReadMD.
echo.
pause
exit /b 0
