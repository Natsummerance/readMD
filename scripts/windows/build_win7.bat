@echo off

rem ============================================================

rem  ReadMD Win7 兼容版构建链（Python 3.9.13 + pywebview 4.4.1）

rem  产物:

rem    dist\ReadMD-win7\ReadMD.exe           onedir 应用（秒开）

rem    dist\ReadMDUninstall-win7.exe         Win7 卸载器

rem    dist\ReadMDSetup-win7.exe             Win7 安装器（onefile）

rem    dist\ReadMDSetup-%READMD_VERSION_OVERRIDE%-win7-x64.exe  测试发布资产

rem ============================================================

setlocal
cd /d "%~dp0..\.."
title ReadMD Win7 Packager




rem ---- 版本串（Win7 兼容版；严格从 .env / VERSION 绑定，禁止代码硬编码）----
if "%READMD_VERSION_OVERRIDE%"=="" (
    for /f "usebackq tokens=1,* delims==" %%A in (".env") do (
        if "%%A"=="READMD_VERSION" set "READMD_VERSION_OVERRIDE=%%B"
    )
)
if "%READMD_VERSION_OVERRIDE%"=="" (
    if exist "VERSION" set /p READMD_VERSION_OVERRIDE=<VERSION
)

set "PY39=%LOCALAPPDATA%\ReadMD-build\python39"
set "WVENV=.venv-win7"
set "RELVER=%READMD_VERSION_OVERRIDE: =-%"

echo [1/7] Python 3.9.13 ...
if not exist "%PY39%\python.exe" (
    echo   downloading python-3.9.13-amd64.exe ...
    curl.exe -L --fail -sS -o "%TEMP%\python-3.9.13-amd64.exe" ^
        https://www.python.org/ftp/python/3.9.13/python-3.9.13-amd64.exe
    if errorlevel 1 goto :err
    "%TEMP%\python-3.9.13-amd64.exe" /quiet InstallAllUsers=0 PrependPath=0 ^
        Include_launcher=0 Include_test=0 Include_doc=0 Include_tcltk=0 ^
        Shortcuts=0 AssociateFiles=0 TargetDir="%PY39%"
    if errorlevel 1 goto :err
)

echo [2/7] pip dependencies ...
"%PY39%\python.exe" -m pip install --quiet --upgrade pip
"%PY39%\python.exe" -m pip install --quiet virtualenv
if errorlevel 1 goto :err

echo [3/7] virtualenv .venv-win7 ...
if not exist "%WVENV%\Scripts\python.exe" (
    "%PY39%\python.exe" -m virtualenv "%WVENV%"
    if errorlevel 1 goto :err
)

echo [4/8] install pinned Win7 dependencies in .venv-win7 ...
"%WVENV%\Scripts\python.exe" -m pip install --quiet pywebview==4.4.1 pyinstaller
if errorlevel 1 goto :err
if exist "config\requirements-windows.txt" (
    "%WVENV%\Scripts\python.exe" -m pip install --quiet -r config\requirements-common.txt
)

echo [5/8] patch pywebview and bundle the fixed WebView2 109 runtime ...
"%WVENV%\Scripts\python.exe" tools\win7_pywebview_edgechromium.patch
if errorlevel 1 goto :err
"%WVENV%\Scripts\python.exe" tools\bundle_runtime.py
if errorlevel 1 goto :err
if not exist "installer\webview2_runtime\msedgewebview2.exe" (
    echo Win7 runtime is missing after bundling.
    goto :err
)

echo [6/8] version + icon ...
if not exist "build" mkdir build
> "build\version.txt" echo %READMD_VERSION_OVERRIDE%
"%WVENV%\Scripts\python.exe" tools\make_icon.py
if errorlevel 1 goto :err

echo [7/8] build onedir app + uninstaller ...
"%WVENV%\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onedir --windowed --name ReadMD-win7 --icon assets/readmd.ico --add-data "assets;assets" --add-data "src/readmd_core;src/readmd_core" --add-data "src/readmd_modules;src/readmd_modules" --add-data "src/readmd_fix.py;src" --hidden-import src.readmd_fix --hidden-import src.readmd_core --collect-data magika --collect-data docx --collect-data reportlab --collect-data matplotlib --collect-data trafilatura --collect-submodules src.readmd_core --collect-submodules src.readmd_modules readmd.py
if errorlevel 1 goto :err

"%WVENV%\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onefile --windowed --name ReadMDUninstall-win7 --icon assets/readmd.ico --add-data "installer;installer" installer/setup_app.py
if errorlevel 1 goto :err

echo [8/8] build setup (embeds app + uninstaller + runtime) ...
"%WVENV%\Scripts\python.exe" -m PyInstaller --noconfirm --clean --onefile --windowed --name ReadMDSetup-win7 --icon assets/readmd.ico --add-data "installer;installer" --add-binary "dist/ReadMD-win7;ReadMD" --add-binary "dist/ReadMDUninstall-win7.exe;." installer/setup_app.py
if errorlevel 1 goto :err

copy /y "dist\ReadMDSetup-win7.exe" "dist\ReadMDSetup-%RELVER%-win7-x64.exe" >nul

echo.

echo Done!

echo   onedir : %~dp0dist\ReadMD-win7\ReadMD.exe

echo   setup  : %~dp0dist\ReadMDSetup-%RELVER%-win7-x64.exe

echo   version: %READMD_VERSION_OVERRIDE%

echo.

pause

exit /b 0



:err

echo.

echo Build failed. Check the error messages above.

echo.

pause

exit /b 1

