# RustDesk 配置验证脚本
# 用于验证当前系统的 RustDesk 配置是否正确

param(
    [string]$ExpectedKey = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
)

Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host "RustDesk Configuration Verification Tool" -ForegroundColor Cyan
Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host ""

$configPaths = @(
    "$env:APPDATA\RustDesk\config\RustDesk.toml",
    "$env:APPDATA\RustDesk\config\RustDesk2.toml",
    "$env:ProgramData\RustDesk\config\RustDesk.toml",
    "$env:ProgramData\RustDesk\config\RustDesk2.toml",
    "$env:LOCALAPPDATA\RustDesk\config\RustDesk.toml",
    "$env:LOCALAPPDATA\RustDesk\config\RustDesk2.toml"
)

$foundConfigs = 0
$correctConfigs = 0

foreach ($path in $configPaths) {
    if (Test-Path $path) {
        $foundConfigs++
        Write-Host "✓ Found: " -ForegroundColor Green -NoNewline
        Write-Host $path
        
        $content = Get-Content $path -Raw
        
        # 检查服务器配置
        if ($content -match 'custom-rendezvous-server\s*=\s*"([^"]+)"') {
            $server = $matches[1]
            Write-Host "  Server: $server" -ForegroundColor Gray
        }
        
        # 检查 relay 服务器
        if ($content -match 'relay-server\s*=\s*"([^"]+)"') {
            $relay = $matches[1]
            Write-Host "  Relay:  $relay" -ForegroundColor Gray
        }
        
        # 检查 key
        if ($content -match 'key\s*=\s*"([^"]+)"') {
            $key = $matches[1]
            if ($key -eq $ExpectedKey) {
                Write-Host "  Key:    ✓ CORRECT" -ForegroundColor Green
                $correctConfigs++
            } else {
                Write-Host "  Key:    ✗ WRONG" -ForegroundColor Red
                Write-Host "          Expected: $ExpectedKey" -ForegroundColor Yellow
                Write-Host "          Found:    $key" -ForegroundColor Yellow
            }
        } else {
            Write-Host "  Key:    ✗ NOT FOUND" -ForegroundColor Red
        }
        
        $lastWrite = (Get-Item $path).LastWriteTime
        Write-Host "  Modified: $lastWrite" -ForegroundColor Gray
        Write-Host ""
    }
}

if ($foundConfigs -eq 0) {
    Write-Host "✗ No RustDesk configuration files found!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please run the deployment tool first:" -ForegroundColor Yellow
    Write-Host "  RustDesk_Config_Installer.exe" -ForegroundColor Cyan
} else {
    Write-Host "Summary:" -ForegroundColor Cyan
    Write-Host "  Total configs found: $foundConfigs" -ForegroundColor White
    Write-Host "  Correct configs: $correctConfigs" -ForegroundColor $(if ($correctConfigs -eq $foundConfigs) { "Green" } else { "Yellow" })
    
    if ($correctConfigs -eq $foundConfigs) {
        Write-Host ""
        Write-Host "✓ All configurations are CORRECT!" -ForegroundColor Green
    } else {
        Write-Host ""
        Write-Host "⚠ Some configurations need to be updated!" -ForegroundColor Yellow
        Write-Host "  Please run the deployment tool again." -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host ""
Write-Host "Press any key to exit..." -ForegroundColor Gray
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
