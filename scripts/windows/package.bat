@echo off
rem ============================================================
rem  ReadMD Packager - one-click build
rem  Output:
rem    dist\ReadMD\ReadMD.exe         安装版（onedir，秒开，安装包内嵌）
rem    dist\ReadMD-portable.exe       便携版（单文件，免安装）
rem ============================================================
setlocal
cd /d "%~dp0..\.."
title ReadMD Packager
set "READMD_VERSION=2.2.6"
set "READMD_VERSION_OVERRIDE=%READMD_VERSION%"

echo [1/5] Checking Python for ReadMD v%READMD_VERSION% ...
where python >nul 2>nul
if errorlevel 1 (
    echo Python not found. Please install Python 3.9+ first:
    echo   https://www.python.org/downloads/
    pause
    exit /b 1
)

echo [2/5] Preparing venv and build dependencies ...
if not exist ".venv\Scripts\python.exe" (
    python -m venv .venv
    if errorlevel 1 goto :err
)
".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -q -r config/requirements.txt pyinstaller
if errorlevel 1 goto :err

echo [3/5] Generating icon ...
".venv\Scripts\python.exe" tools\make_icon.py
if errorlevel 1 goto :err

echo [4/5] Building install edition (onedir, instant start) ...
".venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onedir --windowed ^
    --name ReadMD --icon "assets\readmd.ico" ^
    --add-data "assets;assets" ^
    --add-data "src/readmd_modules;src/readmd_modules" ^
    --add-data "src/readmd_fix.py;src" ^
    --hidden-import src.readmd_fix ^
    --collect-data magika ^
    --collect-data docx ^
    --collect-data reportlab ^
    --collect-data matplotlib ^
    --collect-data trafilatura ^
    --collect-submodules src.readmd_modules ^
    readmd.py
if errorlevel 1 goto :err

echo [5/5] Building portable edition (single-file) ...
".venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onefile --windowed ^
    --name ReadMD-portable --icon "assets\readmd.ico" ^
    --add-data "assets;assets" ^
    --add-data "src/readmd_modules;src/readmd_modules" ^
    --add-data "src/readmd_fix.py;src" ^
    --hidden-import src.readmd_fix ^
    --collect-data magika ^
    --collect-data docx ^
    --collect-data reportlab ^
    --collect-data matplotlib ^
    --collect-data trafilatura ^
    --collect-submodules src.readmd_modules ^
    readmd.py
if errorlevel 1 goto :err


echo.
echo Done!
echo   install : %~dp0dist\ReadMD\ReadMD.exe   (onedir, fast cold start)
echo   portable: %~dp0dist\ReadMD-portable.exe (single-file)
echo Run installer\build_setup.bat to build ReadMDSetup.exe.
echo.
pause
exit /b 0

:err
echo.
echo Build failed. Check the error messages above.
echo.
pause
exit /b 1
