@echo off
echo ============================================================
echo RustDesk Configuration Check
echo ============================================================
echo.

echo Checking ProgramData config...
echo.

if exist "C:\ProgramData\RustDesk\config\RustDesk.toml" (
    echo File: C:\ProgramData\RustDesk\config\RustDesk.toml
    findstr /C:"key" "C:\ProgramData\RustDesk\config\RustDesk.toml"
    echo.
) else (
    echo [NOT FOUND] C:\ProgramData\RustDesk\config\RustDesk.toml
    echo.
)

if exist "C:\ProgramData\RustDesk\config\RustDesk2.toml" (
    echo File: C:\ProgramData\RustDesk\config\RustDesk2.toml
    findstr /C:"key" "C:\ProgramData\RustDesk\config\RustDesk2.toml"
    echo.
) else (
    echo [NOT FOUND] C:\ProgramData\RustDesk\config\RustDesk2.toml
    echo.
)

echo Checking user config...
echo.

if exist "%APPDATA%\RustDesk\config\RustDesk2.toml" (
    echo File: %APPDATA%\RustDesk\config\RustDesk2.toml
    findstr /C:"key" "%APPDATA%\RustDesk\config\RustDesk2.toml"
    echo.
) else (
    echo [NOT FOUND] %APPDATA%\RustDesk\config\RustDesk2.toml
    echo.
)

echo ============================================================
echo Expected key should be:
echo AAAAC3NzaC1lZDI1NTE5AAAAIBAjWWVdpMda/rF5zAObc92HsyO2xWyNNaUtQByf0RYI
echo ============================================================
echo.

pause
