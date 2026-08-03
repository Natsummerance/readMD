@echo off
rem ============================================================
rem  ReadMD Packager - one-click build single-file exe
rem  Output: dist\ReadMD.exe
rem ============================================================
setlocal
cd /d "%~dp0"
title ReadMD Packager

echo [1/4] Checking Python ...
where python >nul 2>nul
if errorlevel 1 (
    echo Python not found. Please install Python 3.9+ first:
    echo   https://www.python.org/downloads/
    pause
    exit /b 1
)

echo [2/4] Preparing venv and build dependencies ...
if not exist ".venv\Scripts\python.exe" (
    python -m venv .venv
    if errorlevel 1 goto :err
)
".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -q -r requirements.txt pyinstaller
if errorlevel 1 goto :err

echo [3/4] Generating icon ...
".venv\Scripts\python.exe" tools\make_icon.py
if errorlevel 1 goto :err

echo [4/4] Building ReadMD.exe ...
".venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onefile --windowed ^
    --name ReadMD --icon "assets\readmd.ico" ^
    --add-data "assets;assets" ^
    --add-data "readmd_modules;readmd_modules" ^
    --add-data "readmd_fix.py;." ^
    --hidden-import readmd_fix ^
    --collect-data magika ^
    --collect-submodules readmd_modules ^
    readmd.py
if errorlevel 1 goto :err

echo.
echo Done! exe: %~dp0dist\ReadMD.exe
echo Run setup.bat to install it as the default .md app.
echo.
pause
exit /b 0

:err
echo.
echo Build failed. Check the error messages above.
echo.
pause
exit /b 1