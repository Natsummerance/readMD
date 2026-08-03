@echo off
rem ============================================================
rem  ReadMD Installer - setup venv, install deps, register .md
rem ============================================================
setlocal
cd /d "%~dp0"
title ReadMD Installer

echo [1/3] Checking Python ...
where python >nul 2>nul
if errorlevel 1 (
    echo.
    echo Python not found. Please install Python 3.9+ first:
    echo   https://www.python.org/downloads/
    echo Make sure "Add Python to PATH" is checked during install.
    echo.
    pause
    exit /b 1
)

echo [2/3] Creating virtual environment and installing dependencies ...
if not exist ".venv\Scripts\python.exe" (
    python -m venv .venv
    if errorlevel 1 goto :err
)
".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -q -r requirements.txt
if errorlevel 1 goto :err

echo [3/3] Registering file association (current user, no admin needed) ...
set "PYW=%~dp0.venv\Scripts\pythonw.exe"
set "SCRIPT=%~dp0readmd.py"
set "ICON=%~dp0assets\readmd.ico"

if not exist "%APPDATA%\ReadMD\backup" mkdir "%APPDATA%\ReadMD\backup"
reg export "HKCU\Software\Classes\.md" "%APPDATA%\ReadMD\backup\.md.reg.bak" /y >nul 2>nul

for %%E in (.md .markdown .mdown .mkd) do (
    reg add "HKCU\Software\Classes\%%E" /ve /d "ReadMD.markdown" /f >nul
)
reg add "HKCU\Software\Classes\ReadMD.markdown" /ve /d "ReadMD Markdown Reader" /f >nul
reg add "HKCU\Software\Classes\ReadMD.markdown\DefaultIcon" /ve /d "\"%ICON%\",0" /f >nul
reg add "HKCU\Software\Classes\ReadMD.markdown\shell\open\command" /ve /t REG_EXPAND_SZ /d "\"%PYW%\" \"%SCRIPT%\" \"%%1\"" /f >nul
reg add "HKCU\Software\Classes\ReadMD.markdown\shell\openwith" /ve /d "" /f >nul
reg add "HKCU\Software\Classes\Applications\readmd.py\shell\open\command" /ve /t REG_EXPAND_SZ /d "\"%PYW%\" \"%SCRIPT%\" \"%%1\"" /f >nul

rem refresh explorer icon cache
ie4uinit.exe -show >nul 2>nul

echo.
echo Done! Now double-click any .md file to open it with ReadMD.
echo If Windows still uses another app:
echo   right-click a .md file - Open with - ReadMD - Always use this app
echo.
pause
exit /b 0

:err
echo.
echo Install failed. Check the error message above.
echo.
pause
exit /b 1