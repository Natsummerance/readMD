@echo off
chcp 65001 >nul
rem ============================================================
rem  ReadMD Deploy - one-click: test + build + push + release
rem
rem  Usage:
rem    deploy.bat                   test + commit + push main + push v2.2.3 tag
rem    deploy.bat --skip-tests      skip the local test round
rem    deploy.bat --tag v2.2.3      release tag (default v2.2.3)
rem
rem  GitHub Actions is the only Release publisher.
rem ============================================================
setlocal
cd /d "%~dp0"
title ReadMD Deploy

set "TAG=v2.2.3"
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

echo [1/6] Checking repository ...
git rev-parse --is-inside-work-tree >nul 2>&1
if errorlevel 1 goto :err
echo   repository OK

echo [2/6] Preparing venv ...
if not exist ".venv\Scripts\python.exe" (
    echo   creating .venv ...
    python -m venv .venv
    if errorlevel 1 goto :err
)
".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -q -r requirements-test.txt
if errorlevel 1 goto :err
echo   venv ready

if "%SKIP_TESTS%"=="1" goto :tests_done
echo [3/6] Running tests ...
".venv\Scripts\python.exe" readmd_fix_test.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" readmd_convert_test.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" readmd_export_test.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" readmd_web_test.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" readmd_api_test.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" tools\privacy_scan.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" readmd.py --selftest
if errorlevel 1 goto :err
echo   all tests passed
:tests_done

echo [4/6] Committing and pushing main ...
rem Preserve the user's local IDEA.md and never stage it for a release.
git add -A -- . ":(exclude)IDEA.md"
git diff --cached --name-only | findstr /x /i "IDEA.md" >nul
if not errorlevel 1 (
    echo   refusing to stage IDEA.md
    goto :err
)
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
