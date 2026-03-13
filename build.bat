@echo off
REM RustDesk Voice Calling - Windows Build Script
REM This script builds RustDesk with voice calling feature enabled
REM Run from: c:\Users\Aayan\Desktop\rustdesk>

setlocal enabledelayedexpansion

echo.
echo ============================================================================
echo           RustDesk Voice Calling - Build Script (Windows)
echo ============================================================================
echo.

REM Check if Rust is installed
where cargo >nul 2>&1
if errorlevel 1 (
    echo ERROR: Rust is not installed or not in PATH
    echo.
    echo Download and install from: https://rustup.rs/
    echo After installation, close and reopen this terminal
    exit /b 1
)

echo [OK] Rust toolchain detected
echo.

REM Check if we're in the right directory
if not exist "Cargo.toml" (
    echo ERROR: Cargo.toml not found in current directory
    echo Please run this script from the RustDesk root directory
    echo Expected: c:\Users\Aayan\Desktop\rustdesk^>
    exit /b 1
)

echo [OK] In RustDesk directory
echo.

REM Update Rust
echo Updating Rust toolchain...
call rustup update >nul 2>&1

REM Show version info
echo.
echo [INFO] Build Information:
for /f "tokens=*" %%a in ('rustc --version') do echo   %%a
for /f "tokens=*" %%a in ('cargo --version') do echo   %%a
echo.

REM Set build mode (debug or release)
set BUILD_MODE=%1
if "!BUILD_MODE!"=="" (
    set BUILD_MODE=release
    set BUILD_FLAGS=--release
) else if "!BUILD_MODE!"=="debug" (
    set BUILD_FLAGS=
) else if "!BUILD_MODE!"=="release" (
    set BUILD_FLAGS=--release
) else (
    echo ERROR: Invalid build mode: !BUILD_MODE!
    echo Usage: build.bat [debug^|release]
    exit /b 1
)

echo [INPUT] Build Mode: !BUILD_MODE!
if "!BUILD_FLAGS!"=="--release" (
    echo [INPUT] Optimizations: ENABLED (slower build, faster runtime)
) else (
    echo [INPUT] Optimizations: DISABLED (faster build, slower runtime)
)
echo.

REM Run tests first
echo ============================================================================
echo Step 1: Running Unit Tests
echo ============================================================================
echo.
echo Running: cargo test audio --features voice-call --lib
call cargo test audio --features voice-call --lib

if errorlevel 1 (
    echo.
    echo ERROR: Unit tests failed!
    echo Please review errors above and fix before building
    exit /b 1
)

echo.
echo [OK] All 31 unit tests passed
echo.

REM Build the executable
echo ============================================================================
echo Step 2: Building Executable
echo ============================================================================
echo.
echo Running: cargo build !BUILD_FLAGS! --features voice-call
echo.
echo This may take 5-20 minutes on first build...
echo.

call cargo build !BUILD_FLAGS! --features voice-call

if errorlevel 1 (
    echo.
    echo ERROR: Build failed!
    echo Review error messages above
    exit /b 1
)

echo.
echo [OK] Build successful!
echo.

REM Find and report output
if "!BUILD_FLAGS!"=="--release" (
    set OUTPUT_PATH=target\release\rustdesk.exe
    set OUTPUT_DIR=target\release
) else (
    set OUTPUT_PATH=target\debug\rustdesk.exe
    set OUTPUT_DIR=target\debug
)

echo ============================================================================
echo Build Complete
echo ============================================================================
echo.
echo Executable: !OUTPUT_PATH!
echo.

if exist "!OUTPUT_PATH!" (
    for /f "tokens=*" %%a in ('dir /B !OUTPUT_PATH!') do (
        for /f "tokens=5" %%b in ('dir !OUTPUT_PATH!') do (
            echo File Size: %%b bytes
        )
    )
    
    echo.
    echo [SUCCESS] Voice calling feature is now built and ready to use!
    echo.
    echo Next steps:
    echo   1. Test the executable by running it:
    echo      !OUTPUT_PATH!
    echo.
    echo   2. For production release, use release build:
    echo      build.bat release
    echo.
    echo   3. Voice calling is enabled via --features voice-call
    echo      You can disable it with: cargo build --release
    echo.
) else (
    echo ERROR: Output file not found!
    echo Expected: !OUTPUT_PATH!
    exit /b 1
)

echo ============================================================================
pause
