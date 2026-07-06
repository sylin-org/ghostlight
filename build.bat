@echo off
setlocal EnableDelayedExpansion
::=============================================================================
:: build.bat -- full Ghostlight stack build (debug + release + extension zip)
::
:: Kills any running ghostlight.exe instances so the binaries can be written to
:: disk, then builds BOTH profiles (debug for live testing via Chrome's native
:: host, release for the MCP client config), and packages the Chromium extension.
::
:: Usage:
::   build.bat              debug + release + extension zip
::   build.bat --no-ext     debug + release only (skip the extension zip)
::   build.bat --debug      debug only
::   build.bat --release    release only
::=============================================================================

set "BUILD_DEBUG=1"
set "BUILD_RELEASE=1"
set "BUILD_EXT=1"

:: Parse args
:parse
if "%~1"=="" goto :start
if /I "%~1"=="--debug" (
    set "BUILD_DEBUG=1"
    set "BUILD_RELEASE=0"
    shift
    goto :parse
)
if /I "%~1"=="--release" (
    set "BUILD_DEBUG=0"
    set "BUILD_RELEASE=1"
    shift
    goto :parse
)
if /I "%~1"=="--no-ext" (
    set "BUILD_EXT=0"
    shift
    goto :parse
)
if /I "%~1"=="--help" goto :help
echo Unknown argument: %~1
exit /b 1

:help
echo Usage: build.bat [--debug] [--release] [--no-ext]
echo.
echo   --debug     Debug build only (default: both debug + release)
echo   --release   Release build only (default: both debug + release)
echo   --no-ext    Skip extension packaging
exit /b 0

:start
echo.
echo === Ghostlight full-stack build ===
echo.

:: --- Step 1: Kill running instances so binaries can be written ---
echo [1/3] Stopping running ghostlight processes...
:: Chrome auto-relaunches its native-host within seconds of a kill, so we loop:
:: kill, wait, check again, repeat until clear (or give up after 3 tries).
set "KILL_TRIES=0"
:kill_loop
taskkill /F /IM ghostlight.exe >NUL 2>&1
if !ERRORLEVEL! equ 0 (
    set /a KILL_TRIES+=1
    if !KILL_TRIES! geq 3 (
        echo       WARNING: ghostlight.exe keeps relaunching ^(Chrome native-host?^).
        echo       Close Chrome, or the debug build may fail with "Access is denied".
    )
    timeout /t 3 /nobreak >NUL 2>&1
    tasklist /FI "IMAGENAME eq ghostlight.exe" 2>NUL | findstr /I "ghostlight.exe" >NUL 2>&1
    if !ERRORLEVEL! equ 0 goto :kill_loop
    echo       Killed ghostlight.exe instances.
) else (
    echo       No running instances found.
)
echo.

:: --- Step 2: Rust builds ---
set "STEP=2"
if "!BUILD_DEBUG!"=="1" if "!BUILD_RELEASE!"=="1" set "STEP=2/4"
if "!BUILD_DEBUG!"=="1" if "!BUILD_RELEASE!"=="0" set "STEP=2/3"
if "!BUILD_DEBUG!"=="0" if "!BUILD_RELEASE!"=="1" set "STEP=2/3"

if "!BUILD_DEBUG!"=="1" (
    echo [!STEP!] Building debug...
    cargo build
    if !ERRORLEVEL! neq 0 (
        echo       Debug build FAILED.
        exit /b 1
    )
    echo       OK: target\debug\ghostlight.exe
    echo.
)

if "!BUILD_RELEASE!"=="1" (
    echo [!STEP!] Building release...
    cargo build --release
    if !ERRORLEVEL! neq 0 (
        echo       Release build FAILED.
        exit /b 1
    )
    echo       OK: target\release\ghostlight.exe
    echo.
)

:: --- Step 3: Extension packaging (optional) ---
if "!BUILD_EXT!"=="0" (
    echo [3/3] Extension packaging skipped (--no-ext^).
    echo.
    echo === Build complete ===
    exit /b 0
)

echo [3/3] Packaging Chromium extension...
where pwsh >NUL 2>&1
if !ERRORLEVEL! equ 0 (
    pwsh -File scripts\package-extension.ps1
) else (
    powershell -ExecutionPolicy Bypass -File scripts\package-extension.ps1
)
echo.

echo === Build complete ===
endlocal
