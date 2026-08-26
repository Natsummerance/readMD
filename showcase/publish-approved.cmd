@echo off
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\publish_approved_latest.ps1" %*
set EXIT_CODE=%ERRORLEVEL%
if /I not "%CI%"=="true" if /I not "%READMD_NO_PAUSE%"=="1" pause
exit /b %EXIT_CODE%
