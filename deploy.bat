@echo off
chcp 65001 >nul
rem ============================================================
rem  ReadMD Deploy - one-click: test + build + push + release
rem
rem  Usage:
rem    deploy.bat                   test + commit + push main + push v2.2.0 tag
rem    deploy.bat --skip-tests      skip the local test round
rem    deploy.bat --tag v2.2.0      release tag (default v2.2.0)
rem
rem  Requirements: GITHUB_TOKEN in system/user env vars.
rem ============================================================
setlocal
cd /d "%~dp0"
title ReadMD Deploy

set "TAG=v2.2.0"
set "SKIP_TESTS=0"

:parse
if "%~1"=="" goto :parse_done
if /i "%~1"=="--skip-tests" set "SKIP_TESTS=1"
if /i "%~1"=="--tag" set "TAG=%~2"
shift
goto :parse
:parse_done

echo ============================================================
echo  ReadMD Deploy  (tag=%TAG%)
echo ============================================================

echo [1/6] Checking GITHUB_TOKEN ...
if "%GITHUB_TOKEN%"=="" (
    echo.
    echo   GITHUB_TOKEN is not set in this terminal.
    echo   Set it once in Windows system env vars, then reopen terminal:
    echo     setx GITHUB_TOKEN ghp_xxxxxxxxxxxx
    echo.
    pause
    exit /b 1
)
echo   token OK (starts with %GITHUB_TOKEN:~0,4%)

echo [2/6] Preparing venv ...
if not exist ".venv\Scripts\python.exe" (
    echo   creating .venv ...
    python -m venv .venv
    if errorlevel 1 goto :err
)
".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -q -r requirements.txt pyinstaller
if errorlevel 1 goto :err
echo   venv ready

if "%SKIP_TESTS%"=="1" goto :tests_done
echo [3/6] Running tests ...
".venv\Scripts\python.exe" readmd_fix_test.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" readmd.py --selftest
if errorlevel 1 goto :err
".venv\Scripts\python.exe" readmd.py --mods
if errorlevel 1 goto :err
echo   all tests passed
:tests_done

echo [4/6] Committing and pushing main ...
git add -A
git diff --cached --quiet
if errorlevel 1 (
    git commit -m "ReadMD deploy: %TAG%"
    if errorlevel 1 goto :err
) else (
    echo   nothing new to commit
)
git branch --show-current | findstr /x "main" >nul
if errorlevel 1 (
    echo   deploy.bat must run from main after the release branch is merged.
    goto :err
)
git push origin main
if errorlevel 1 goto :err
echo   pushed

echo [5/6] Creating release tag ...
git rev-parse "%TAG%" >nul 2>&1
if not errorlevel 1 (
    echo   tag %TAG% already exists; refusing to move it.
    goto :err
)
git tag -a "%TAG%" -m "ReadMD %TAG%"
git push origin "%TAG%"
if errorlevel 1 goto :err

echo [6/6] GitHub Actions will test, package and publish the Release.
echo   https://github.com/Natsummerance/readMD/actions

echo.
echo ============================================================
echo  Deploy finished! https://github.com/Natsummerance/readMD/releases
echo ============================================================
echo.
pause
exit /b 0

:err
echo.
echo  Deploy failed. See messages above.
echo.
pause
exit /b 1
