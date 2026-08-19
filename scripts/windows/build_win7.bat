@echo off

rem ============================================================

rem  ReadMD Win7 兼容版构建链（Python 3.9.13 + pywebview 4.4.1）

rem  产物:

rem    dist\ReadMD-win7\ReadMD.exe           onedir 应用（秒开）

rem    dist\ReadMDUninstall-win7.exe         Win7 卸载器

rem    dist\ReadMDSetup-win7.exe             Win7 安装器（onefile）

rem    dist\ReadMDSetup-2.1.1-Beta-win7-x64.exe  最终发布资产

rem ============================================================

setlocal
cd /d "%~dp0..\.."
title ReadMD Win7 Packager




rem ---- 版本串（Win7 版专用；常规链保持 2.1.1）----

if "%READMD_VERSION_OVERRIDE%"=="" set "READMD_VERSION_OVERRIDE=2.2.9-win7"




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

"%PY39%\python.exe" --version

if errorlevel 1 goto :err



echo [2/7] venv + dependencies ...

if not exist "%WVENV%\Scripts\python.exe" (

    "%PY39%\python.exe" -m venv "%WVENV%"

    if errorlevel 1 goto :err

)

"%WVENV%\Scripts\python.exe" -m pip install --disable-pip-version-check -q -r config/win7-reqs.txt

if errorlevel 1 goto :err



echo [3/7] pywebview 4.4.1 edgechromium patch ...

"%WVENV%\Scripts\python.exe" tools\win7_pywebview_edgechromium.patch

if errorlevel 1 goto :err



echo [4/7] bundle fixed WebView2 109 runtime ...

"%WVENV%\Scripts\python.exe" tools\bundle_runtime.py

if errorlevel 1 goto :err



echo [5/7] version + icon ...

if not exist "build" mkdir build

> "build\version.txt" echo %READMD_VERSION_OVERRIDE%

"%WVENV%\Scripts\python.exe" tools\make_icon.py

if errorlevel 1 goto :err



echo [6/7] build onedir app + uninstaller ...

"%WVENV%\Scripts\python.exe" -m PyInstaller --noconfirm --clean release/ReadMD-win7.spec

if errorlevel 1 goto :err

"%WVENV%\Scripts\python.exe" -m PyInstaller --noconfirm --clean release/ReadMDUninstall-win7.spec

if errorlevel 1 goto :err



echo [7/7] build setup (embeds app + uninstaller + runtime) ...

"%WVENV%\Scripts\python.exe" -m PyInstaller --noconfirm --clean release/ReadMDSetup-win7.spec

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

