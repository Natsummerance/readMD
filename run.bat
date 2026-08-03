@echo off
rem ============================================================
rem  ReadMD - one-click run (venv pythonw)
rem ============================================================
setlocal
cd /d "%~dp0"
if not exist ".venv\Scripts\pythonw.exe" (
    echo [ReadMD] Please run install.bat or setup.bat first.
    pause
    exit /b 1
)
start "" ".venv\Scripts\pythonw.exe" "%~dp0readmd.py" %*
exit /b 0