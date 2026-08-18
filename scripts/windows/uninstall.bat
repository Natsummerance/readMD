@echo off
rem ============================================================
rem  ReadMD Uninstaller - remove file association
rem ============================================================
setlocal
cd /d "%~dp0"
title ReadMD Uninstaller

echo Removing ReadMD file association ...
for %%E in (.md .markdown .mdown .mkd) do (
    reg delete "HKCU\Software\Classes\%%E" /f >nul 2>nul
)
reg delete "HKCU\Software\Classes\ReadMD.markdown" /f >nul 2>nul
reg delete "HKCU\Software\Classes\Applications\readmd.py" /f >nul 2>nul
reg delete "HKCU\Software\Classes\Applications\ReadMD.exe" /f >nul 2>nul

if exist "%APPDATA%\ReadMD\backup\.md.reg.bak" (
    reg import "%APPDATA%\ReadMD\backup\.md.reg.bak" >nul 2>nul
    echo Previous .md association restored from backup.
)

echo.
echo Done. The readmd folder and its venv are kept on disk.
echo To fully remove them, delete this folder manually.
echo.
pause
exit /b 0