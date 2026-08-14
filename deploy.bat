@echo off
chcp 65001 >nul
rem ============================================================
rem  ReadMD Deploy - one-click: test + build + push + release
rem
rem  Usage:
rem    deploy.bat                   full deploy (test + build + push + release)
rem    deploy.bat --skip-build      reuse existing dist exes (faster)
rem    deploy.bat --skip-tests      skip the self-test round
rem    deploy.bat --tag v2.0.1      release tag (default v2.0.1)
rem
rem  Requirements: GITHUB_TOKEN in system/user env vars.
rem ============================================================
setlocal
cd /d "%~dp0"
title ReadMD Deploy

set "TAG=v2.0.1"
set "SKIP_BUILD=0"
set "SKIP_TESTS=0"

:parse
if "%~1"=="" goto :parse_done
if /i "%~1"=="--skip-build" set "SKIP_BUILD=1"
if /i "%~1"=="--skip-tests" set "SKIP_TESTS=1"
if /i "%~1"=="--tag" (
    shift
    set "TAG=%~1"
)
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

echo [4/6] Building packages ...
if "%SKIP_BUILD%"=="1" goto :build_done
if exist "dist\ReadMDSetup.exe" if exist "dist\ReadMD\ReadMD.exe" (
    echo   dist exes already exist. Rebuilding anyway (delete dist to skip).
)
".venv\Scripts\python.exe" tools\make_icon.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onedir --windowed ^
    --name ReadMD --icon "assets\readmd.ico" ^
    --add-data "assets;assets" ^
    --add-data "readmd_modules;readmd_modules" ^
    --add-data "readmd_fix.py;." ^
    --hidden-import readmd_fix ^
    --collect-data magika ^
    --collect-submodules readmd_modules ^
    readmd.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onefile --windowed ^
    --name ReadMD-portable --icon "assets\readmd.ico" ^
    --add-data "assets;assets" ^
    --add-data "readmd_modules;readmd_modules" ^
    --add-data "readmd_fix.py;." ^
    --hidden-import readmd_fix ^
    --collect-data magika ^
    --collect-submodules readmd_modules ^
    readmd.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onefile --windowed ^
    --name ReadMDUninstall --icon "assets\readmd.ico" ^
    --add-data "installer;installer" ^
    installer\setup_app.py
if errorlevel 1 goto :err
".venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onefile --windowed ^
    --name ReadMDSetup --icon "assets\readmd.ico" ^
    --add-data "installer;installer" ^
    --add-binary "dist\ReadMD;ReadMD" ^
    --add-binary "dist\ReadMDUninstall.exe;." ^
    installer\setup_app.py
if errorlevel 1 goto :err
echo   build done
:build_done

echo [5/6] Committing and pushing to GitHub ...
git add -A
git diff --cached --quiet
if errorlevel 1 (
    git commit -m "ReadMD deploy: %TAG%"
    if errorlevel 1 goto :err
) else (
    echo   nothing new to commit
)
git push origin main
if errorlevel 1 goto :err
echo   pushed

echo [6/6] Publishing GitHub Release %TAG% ...
if exist "release_notes.md" (
    ".venv\Scripts\python.exe" release.py --tag %TAG% --name "ReadMD %TAG%" --update --body-file release_notes.md
    if errorlevel 1 goto :err
)
".venv\Scripts\python.exe" release.py --tag %TAG%
if errorlevel 1 goto :err
".venv\Scripts\python.exe" release.py --tag %TAG% --verify
if errorlevel 1 goto :err

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
