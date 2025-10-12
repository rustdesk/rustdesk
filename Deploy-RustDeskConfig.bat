@echo off
:: RustDesk Configuration Deployment Launcher
:: This batch file will run the PowerShell deployment script with proper execution policy

echo ================================================
echo RustDesk Configuration Deployment Tool
echo ================================================
echo.

:: Check for admin rights
net session >nul 2>&1
if %errorLevel% == 0 (
    echo [OK] Running with Administrator privileges
) else (
    echo [WARNING] Not running as Administrator
    echo Some operations may fail without admin rights.
    echo.
    echo Right-click this file and select "Run as Administrator"
    echo.
    pause
)

echo.
echo Starting deployment...
echo.

:: Run PowerShell script with bypass execution policy
PowerShell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Deploy-RustDeskConfig.ps1" -RestartService

echo.
echo.
pause
