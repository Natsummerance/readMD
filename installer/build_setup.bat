@echo off
rem ============================================================
rem  ReadMD Setup Builder - build ReadMDSetup.exe (installer)
rem  and ReadMDUninstall.exe (uninstaller).
rem  Requires dist\ReadMD\ReadMD.exe (onedir install edition).
rem  The installer embeds the whole onedir directory (ReadMD.exe +
rem  _internal) so installation is a fast directory copy (instant start).
rem ============================================================
setlocal
cd /d "%~dp0.."
title ReadMD Setup Builder

echo [1/3] Checking venv and dist\ReadMD\ReadMD.exe ...
if not exist ".venv\Scripts\python.exe" (
    python -m venv .venv
    ".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -q -r config/requirements.txt pyinstaller
)
if not exist "dist\ReadMD\ReadMD.exe" (
    echo ReadMD onedir build not found. Run package.bat first.
    pause
    exit /b 1
)

echo [2/3] Building ReadMDUninstall.exe ...
".venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onefile --windowed ^
    --name ReadMDUninstall --icon "assets\readmd.ico" ^
    --add-data "installer;installer" --add-data "VERSION;." ^
    installer\setup_app.py
if errorlevel 1 goto :err

echo [3/3] Building ReadMDSetup.exe (embedding onedir app) ...
".venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onefile --windowed ^
    --name ReadMDSetup --icon "assets\readmd.ico" ^
    --add-data "installer;installer" --add-data "VERSION;." ^
    --add-binary "dist\ReadMD;ReadMD" ^
    --add-binary "dist\ReadMDUninstall.exe;." ^
    installer\setup_app.py
if errorlevel 1 goto :err

echo.
echo Done! dist\ReadMDSetup.exe ^(installer^) + dist\ReadMDUninstall.exe
echo.
pause
exit /b 0

:err
echo.
echo Build failed. Check the error messages above.
echo.
pause
exit /b 1
