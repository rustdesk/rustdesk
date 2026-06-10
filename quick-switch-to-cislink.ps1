# Quick Switch to Cislink Server (Simple Method)
# This script configures RustDesk to use Cislink custom server
# No EXE renaming required - uses config files only
#
# Usage: Right-click -> Run with PowerShell

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Quick Switch to Cislink Server" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Server configuration
$configContent = @"
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
disable-update-check = true
disable-installation = true
"@

Write-Host "Target Server: hbbs.cislink.nl" -ForegroundColor Yellow
Write-Host ""

# Stop RustDesk
Write-Host "[1/3] Stopping RustDesk..." -ForegroundColor Cyan
$processes = Get-Process | Where-Object { $_.Name -like "rustdesk*" }
if ($processes) {
    $processes | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Write-Host "  [OK] RustDesk stopped" -ForegroundColor Green
} else {
    Write-Host "  [OK] RustDesk not running" -ForegroundColor Green
}

# Create config directories and files
Write-Host "`n[2/3] Writing configuration files..." -ForegroundColor Cyan

$configPaths = @(
    "$env:APPDATA\RustDesk\config",
    "$env:PROGRAMDATA\RustDesk\config"
)

$filesCreated = 0
foreach ($configPath in $configPaths) {
    try {
        if (-not (Test-Path $configPath)) {
            New-Item -ItemType Directory -Path $configPath -Force | Out-Null
        }

        $configFile1 = Join-Path $configPath "RustDesk.toml"
        $configFile2 = Join-Path $configPath "RustDesk2.toml"

        $configContent | Out-File -FilePath $configFile1 -Encoding utf8 -NoNewline -Force
        $configContent | Out-File -FilePath $configFile2 -Encoding utf8 -NoNewline -Force

        Write-Host "  [OK] $configFile1" -ForegroundColor Green
        Write-Host "  [OK] $configFile2" -ForegroundColor Green
        $filesCreated += 2
    } catch {
        Write-Host "  [ERROR] Failed to write to $configPath" -ForegroundColor Red
    }
}

# Try to update registry (optional, may require admin)
Write-Host "`n[3/3] Updating registry (optional)..." -ForegroundColor Cyan
$regPaths = @(
    "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RustDesk",
    "HKLM:\Software\Wow6432Node\Microsoft\Windows\CurrentVersion\Uninstall\RustDesk"
)

$regUpdated = $false
foreach ($regPath in $regPaths) {
    if (Test-Path $regPath) {
        try {
            Set-ItemProperty -Path $regPath -Name "Host" -Value "hbbs.cislink.nl" -ErrorAction Stop
            Set-ItemProperty -Path $regPath -Name "Key" -Value "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=" -ErrorAction Stop
            Write-Host "  [OK] Registry updated" -ForegroundColor Green
            $regUpdated = $true
            break
        } catch {
            Write-Host "  [INFO] Registry update skipped (requires admin)" -ForegroundColor Gray
        }
    }
}

if (-not $regUpdated) {
    Write-Host "  [INFO] Registry not updated (not required)" -ForegroundColor Gray
}

# Summary
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Configuration Complete!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Files created: $filesCreated" -ForegroundColor White
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Start RustDesk" -ForegroundColor White
Write-Host "2. Open Settings -> Network" -ForegroundColor White
Write-Host "3. Verify server: hbbs.cislink.nl" -ForegroundColor White
Write-Host ""
Write-Host "If server settings are not applied:" -ForegroundColor Yellow
Write-Host "- Make sure RustDesk is completely closed" -ForegroundColor White
Write-Host "- Run this script again as Administrator" -ForegroundColor White
Write-Host "- Or use the full migration script: migrate-to-cislink.ps1" -ForegroundColor White
Write-Host ""

# Ask to start RustDesk
$response = Read-Host "Start RustDesk now? (Y/N)"
if ($response -eq "Y" -or $response -eq "y") {
    # Find and start RustDesk
    $possiblePaths = @(
        "$env:ProgramFiles\RustDesk",
        "${env:ProgramFiles(x86)}\RustDesk",
        "$env:LocalAppData\RustDesk"
    )

    foreach ($path in $possiblePaths) {
        if (Test-Path $path) {
            $exe = Get-ChildItem -Path $path -Filter "rustdesk*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($exe) {
                Start-Process -FilePath $exe.FullName
                Write-Host "[OK] RustDesk started" -ForegroundColor Green
                break
            }
        }
    }
}

Write-Host ""
Write-Host "Press any key to exit..." -ForegroundColor Gray
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
