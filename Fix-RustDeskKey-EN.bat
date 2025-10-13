@echo off
:: RustDesk Key Fix Script for Windows Server 2012
:: Run as Administrator

setlocal enabledelayedexpansion

echo ============================================================
echo RustDesk Key Update Tool - English Version
echo ============================================================
echo.

:: Check admin rights
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [ERROR] Administrator rights required!
    echo Please right-click and select "Run as administrator"
    echo.
    pause
    exit /b 1
)

echo [OK] Administrator rights verified
echo.

set "NEW_KEY=AAAAC3NzaC1lZDI1NTE5AAAAIBAjWWVdpMda/rF5zAObc92HsyO2xWyNNaUtQByf0RYI"
set "LOG_FILE=%TEMP%\rustdesk-fix.log"

echo New key: %NEW_KEY% > "%LOG_FILE%"
echo. >> "%LOG_FILE%"
echo New key: %NEW_KEY%
echo.
echo ============================================================
echo Updating configuration files...
echo ============================================================
echo.

:: Stop RustDesk process
echo [Step 1/5] Stopping RustDesk process...
echo [Step 1/5] Stopping RustDesk process... >> "%LOG_FILE%"
taskkill /F /IM rustdesk.exe >nul 2>&1
if %errorLevel% equ 0 (
    echo [OK] RustDesk stopped
    echo [OK] RustDesk stopped >> "%LOG_FILE%"
) else (
    echo [INFO] RustDesk was not running
    echo [INFO] RustDesk was not running >> "%LOG_FILE%"
)
timeout /t 2 >nul
echo.

:: Update ProgramData config file 1
set "FILE1=C:\ProgramData\RustDesk\config\RustDesk.toml"
echo [Step 2/5] Updating: %FILE1%
echo [Step 2/5] Updating: %FILE1% >> "%LOG_FILE%"

if exist "%FILE1%" (
    powershell -Command "(Get-Content '%FILE1%' -Raw) -replace 'key\s*=\s*\"[^\"]*\"', 'key = \"%NEW_KEY%\"' | Set-Content '%FILE1%' -NoNewline" 2>> "%LOG_FILE%"
    if !errorLevel! equ 0 (
        echo [OK] File 1 updated successfully
        echo [OK] File 1 updated successfully >> "%LOG_FILE%"
    ) else (
        echo [ERROR] Failed to update file 1
        echo [ERROR] Failed to update file 1 >> "%LOG_FILE%"
    )
) else (
    echo [SKIP] File 1 does not exist
    echo [SKIP] File 1 does not exist >> "%LOG_FILE%"
)
echo.

:: Update ProgramData config file 2
set "FILE2=C:\ProgramData\RustDesk\config\RustDesk2.toml"
echo [Step 3/5] Updating: %FILE2%
echo [Step 3/5] Updating: %FILE2% >> "%LOG_FILE%"

if exist "%FILE2%" (
    powershell -Command "(Get-Content '%FILE2%' -Raw) -replace 'key\s*=\s*\"[^\"]*\"', 'key = \"%NEW_KEY%\"' | Set-Content '%FILE2%' -NoNewline" 2>> "%LOG_FILE%"
    if !errorLevel! equ 0 (
        echo [OK] File 2 updated successfully
        echo [OK] File 2 updated successfully >> "%LOG_FILE%"
    ) else (
        echo [ERROR] Failed to update file 2
        echo [ERROR] Failed to update file 2 >> "%LOG_FILE%"
    )
) else (
    echo [SKIP] File 2 does not exist
    echo [SKIP] File 2 does not exist >> "%LOG_FILE%"
)
echo.

:: Update user config
set "USER_CONFIG=%APPDATA%\RustDesk\config\RustDesk2.toml"
echo [Step 4/5] Updating user config: %USER_CONFIG%
echo [Step 4/5] Updating user config: %USER_CONFIG% >> "%LOG_FILE%"

if exist "%USER_CONFIG%" (
    powershell -Command "(Get-Content '%USER_CONFIG%' -Raw) -replace 'key\s*=\s*[''\""][^''\"]*[''\""]', 'key = ''%NEW_KEY%''' | Set-Content '%USER_CONFIG%' -NoNewline" 2>> "%LOG_FILE%"
    if !errorLevel! equ 0 (
        echo [OK] User config updated successfully
        echo [OK] User config updated successfully >> "%LOG_FILE%"
    ) else (
        echo [ERROR] Failed to update user config
        echo [ERROR] Failed to update user config >> "%LOG_FILE%"
    )
) else (
    echo [SKIP] User config does not exist
    echo [SKIP] User config does not exist >> "%LOG_FILE%"
)
echo.

:: Reset key confirmation
set "USER_CONFIG_MAIN=%APPDATA%\RustDesk\config\RustDesk.toml"
echo [Step 5/5] Resetting key confirmation status...
echo [Step 5/5] Resetting key confirmation status... >> "%LOG_FILE%"

if exist "%USER_CONFIG_MAIN%" (
    powershell -Command "(Get-Content '%USER_CONFIG_MAIN%' -Raw) -replace 'key_confirmed\s*=\s*true', 'key_confirmed = false' | Set-Content '%USER_CONFIG_MAIN%' -NoNewline" 2>> "%LOG_FILE%"
    if !errorLevel! equ 0 (
        echo [OK] Key confirmation reset
        echo [OK] Key confirmation reset >> "%LOG_FILE%"
    ) else (
        echo [ERROR] Failed to reset confirmation
        echo [ERROR] Failed to reset confirmation >> "%LOG_FILE%"
    )
) else (
    echo [SKIP] Main config does not exist
    echo [SKIP] Main config does not exist >> "%LOG_FILE%"
)
echo.

echo ============================================================
echo Update completed!
echo ============================================================
echo.
echo Next steps:
echo 1. Restart RustDesk application
echo 2. Check network settings for server configuration
echo 3. Try connecting to another computer
echo.
echo Log file saved to: %LOG_FILE%
echo.
echo ============================================================
echo.

:: Display log file
echo Would you like to view the log file? (Y/N)
set /p view_log=
if /i "%view_log%"=="Y" (
    type "%LOG_FILE%"
    echo.
    echo.
)

pause
