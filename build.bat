@echo off
setlocal EnableDelayedExpansion
::=============================================================================
:: build.bat -- full Ghostlight stack build (Rust binary + extension zip)
::
:: Kills any running ghostlight.exe instances so the release binary can be
:: written to disk, then builds the Rust service/adapter binary in release mode
:: and packages the Chromium extension into dist/.
::
:: Usage:
::   build.bat              release build + extension zip
::   build.bat --no-ext     release build only (skip the extension zip)
::   build.bat --debug      debug build (faster compile, larger binary)
::=============================================================================

set "MODE=release"
set "BUILD_EXT=1"
set "CARGO_FLAGS=--release"

:: Parse args
:parse
if "%~1"=="" goto :start
if /I "%~1"=="--debug" (
    set "MODE=debug"
    set "CARGO_FLAGS="
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
echo Usage: build.bat [--debug] [--no-ext]
echo.
echo   --debug    Debug build (default: release)
echo   --no-ext   Skip extension packaging
exit /b 0

:start
echo.
echo === Ghostlight full-stack build (!MODE!) ===
echo.

:: --- Step 1: Kill running instances so binaries can be written ---
echo [1/4] Stopping running ghostlight processes...
taskkill /F /IM ghostlight.exe >NUL 2>&1
if !ERRORLEVEL! equ 0 (
    echo       Killed ghostlight.exe instances.
    timeout /t 2 /nobreak >NUL
) else (
    echo       No running instances found.
)
echo.

:: --- Step 2: Rust format check ---
echo [2/4] Checking formatting...
cargo fmt --check
if !ERRORLEVEL! neq 0 (
    echo       Formatting check failed. Run 'cargo fmt' and review the changes.
    exit /b 1
)
echo       OK.
echo.

:: --- Step 3: Rust build ---
echo [3/4] Building Rust binary (!MODE!)...
cargo build !CARGO_FLAGS!
if !ERRORLEVEL! neq 0 (
    echo       Build FAILED.
    exit /b 1
)
echo       Build succeeded.
if "!MODE!"=="release" (
    echo       Binary: target\release\ghostlight.exe
) else (
    echo       Binary: target\debug\ghostlight.exe
)
echo.

:: --- Step 4: Extension packaging (optional) ---
if "!BUILD_EXT!"=="0" (
    echo [4/4] Extension packaging skipped (--no-ext^).
    echo.
    echo === Build complete ===
    exit /b 0
)

echo [4/4] Packaging Chromium extension...
where pwsh >NUL 2>&1
if !ERRORLEVEL! equ 0 (
    pwsh -File scripts\package-extension.ps1
) else (
    powershell -ExecutionPolicy Bypass -File scripts\package-extension.ps1
)
echo.

echo === Build complete ===
endlocal
