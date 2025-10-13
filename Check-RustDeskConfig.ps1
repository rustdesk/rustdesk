# 检查 RustDesk 配置脚本
Write-Host "=== 检查 RustDesk 配置 ===" -ForegroundColor Cyan

$configPath = "$env:APPDATA\RustDesk\config\RustDesk2.toml"

if (Test-Path $configPath) {
    Write-Host "`n配置文件内容:" -ForegroundColor Green
    Get-Content $configPath
    
    $content = Get-Content $configPath -Raw
    if ($content -match 'custom-rendezvous-server\s*=\s*"([^"]+)"') {
        Write-Host "`n✓ ID服务器: $($Matches[1])" -ForegroundColor Green
    } else {
        Write-Host "`n✗ 未配置 ID服务器" -ForegroundColor Red
    }
    
    if ($content -match 'key\s*=\s*"([^"]+)"') {
        Write-Host "✓ Key: $($Matches[1])" -ForegroundColor Green
    } else {
        Write-Host "✗ 未配置 Key" -ForegroundColor Red
    }
} else {
    Write-Host "`n✗ 配置文件不存在: $configPath" -ForegroundColor Red
}

Write-Host "`n=== 检查 RustDesk 进程 ===" -ForegroundColor Cyan
Get-Process rustdesk -ErrorAction SilentlyContinue | Select-Object Id, ProcessName, StartTime
