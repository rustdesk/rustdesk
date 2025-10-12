# Complete RustDesk Reset Script
# Run as regular user (NOT administrator)

Write-Host "=== RustDesk Complete Reset ===" -ForegroundColor Cyan
Write-Host ""

# Stop RustDesk
Write-Host "[1/5] Stopping RustDesk..." -ForegroundColor Yellow
Get-Process rustdesk -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2
Write-Host "[OK]" -ForegroundColor Green
Write-Host ""

# Backup current config
$backupDir = "$env:TEMP\RustDesk_Backup_$(Get-Date -Format 'yyyyMMdd_HHmmss')"
Write-Host "[2/5] Creating backup: $backupDir" -ForegroundColor Yellow
if (Test-Path "$env:APPDATA\RustDesk") {
    Copy-Item "$env:APPDATA\RustDesk" -Destination $backupDir -Recurse -ErrorAction SilentlyContinue
    Write-Host "[OK]" -ForegroundColor Green
} else {
    Write-Host "[SKIP] No config to backup" -ForegroundColor Gray
}
Write-Host ""

# Remove user config completely
Write-Host "[3/5] Removing user configuration..." -ForegroundColor Yellow
if (Test-Path "$env:APPDATA\RustDesk") {
    Remove-Item "$env:APPDATA\RustDesk" -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "[OK] User config removed" -ForegroundColor Green
} else {
    Write-Host "[SKIP] No user config found" -ForegroundColor Gray
}
Write-Host ""

# Restart RustDesk
Write-Host "[4/5] Restarting RustDesk..." -ForegroundColor Yellow
$rustdeskPath = "C:\Program Files\RustDesk\rustdesk.exe"
if (Test-Path $rustdeskPath) {
    Start-Process $rustdeskPath
    Start-Sleep -Seconds 3
    Write-Host "[OK] RustDesk restarted" -ForegroundColor Green
} else {
    Write-Host "[ERROR] RustDesk not found at: $rustdeskPath" -ForegroundColor Red
    Write-Host "Please start RustDesk manually" -ForegroundColor Yellow
}
Write-Host ""

# Reconfigure
Write-Host "[5/5] Configuration needed:" -ForegroundColor Yellow
Write-Host ""
Write-Host "Please configure RustDesk manually:" -ForegroundColor Cyan
Write-Host "1. Open RustDesk" -ForegroundColor White
Write-Host "2. Click [...] -> Settings -> Network" -ForegroundColor White
Write-Host "3. Fill in:" -ForegroundColor White
Write-Host "   - ID Server: hbbs.cislink.nl (or 142.132.187.134)" -ForegroundColor Green
Write-Host "   - Relay Server: hbbr.cislink.nl (or 142.132.187.134)" -ForegroundColor Green
Write-Host "   - Key: AAAAC3NzaC1lZDI1NTE5AAAAIBAjWWVdpMda/rF5zAObc92HsyO2xWyNNaUtQByf0RYI" -ForegroundColor Green
Write-Host "4. Click OK and restart RustDesk" -ForegroundColor White
Write-Host ""
Write-Host "=== Reset Complete! ===" -ForegroundColor Green
Write-Host "Backup saved to: $backupDir" -ForegroundColor Gray
Write-Host ""
pause
