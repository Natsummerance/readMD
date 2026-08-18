@echo off
rem ============================================================
rem  ReadMD Setup - one-click: install deps + package exe
rem  + register as default .md app + launch
rem ============================================================
setlocal enabledelayedexpansion
cd /d "%~dp0..\.."
title ReadMD Setup

echo [1/6] Checking Python ...
where python >nul 2>nul
if errorlevel 1 (
    echo Python not found. Please install Python 3.9+ first:
    echo   https://www.python.org/downloads/
    pause
    exit /b 1
)

echo [2/6] Creating venv and installing dependencies ...
if not exist ".venv\Scripts\python.exe" (
    python -m venv .venv
    if errorlevel 1 goto :err
)
".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -q -r config/requirements.txt pyinstaller
if errorlevel 1 goto :err

echo [3/6] Generating icon ...
".venv\Scripts\python.exe" tools\make_icon.py
if errorlevel 1 goto :err

echo [4/6] Packaging ReadMD (onedir install edition) ...
".venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onedir --windowed ^
    --name ReadMD --icon "assets\readmd.ico" ^
    --add-data "assets;assets" ^
    --add-data "src/readmd_modules;src/readmd_modules" ^
    --add-data "src/readmd_fix.py;src" ^
    --hidden-import src.readmd_fix ^
    --collect-data magika ^
    --collect-submodules src.readmd_modules ^
    readmd.py
if errorlevel 1 goto :err

echo [5/6] Registering as default .md app (current user) ...
set "EXE=%CD%\dist\ReadMD\ReadMD.exe"

if not exist "%APPDATA%\ReadMD\backup" mkdir "%APPDATA%\ReadMD\backup"
reg export "HKCU\Software\Classes\.md" "%APPDATA%\ReadMD\backup\.md.reg.bak" /y >nul 2>nul
for %%E in (.md .markdown .mdown .mkd) do (
    reg add "HKCU\Software\Classes\%%E" /ve /d "ReadMD.markdown" /f >nul
)
reg add "HKCU\Software\Classes\ReadMD.markdown" /ve /d "ReadMD Markdown Reader" /f >nul
reg add "HKCU\Software\Classes\ReadMD.markdown\DefaultIcon" /ve /d "\"%EXE%\",0" /f >nul
reg add "HKCU\Software\Classes\ReadMD.markdown\shell\open\command" /ve /t REG_EXPAND_SZ /d "\"%EXE%\" \"%%1\"" /f >nul
reg add "HKCU\Software\Classes\ReadMD.markdown\shell\openwith" /ve /d "" /f >nul
reg add "HKCU\Software\Classes\Applications\ReadMD.exe\shell\open\command" /ve /t REG_EXPAND_SZ /d "\"%EXE%\" \"%%1\"" /f >nul
ie4uinit.exe -show >nul 2>nul

echo [6/6] Launching ReadMD ...
start "" "dist\ReadMD\ReadMD.exe"

echo.
echo Done! ReadMD.exe is packaged and set as the default .md app.
echo If Windows still uses another program: right-click .md - Open with - ReadMD - Always.
echo.
pause
exit /b 0

:err
echo.
echo Setup failed. Check the error messages above.
echo.
pause
exit /b 1
