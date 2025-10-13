# 需要管理员权限运行
# 右键点击 -> 以管理员身份运行

$newKey = 'AAAAC3NzaC1lZDI1NTE5AAAAIBAjWWVdpMda/rF5zAObc92HsyO2xWyNNaUtQByf0RYI'

Write-Host "=== 更新 RustDesk 配置为新密钥 ===" -ForegroundColor Cyan
Write-Host "新密钥: $newKey`n" -ForegroundColor Yellow

# 更新 ProgramData 配置文件
$files = @(
    'C:\ProgramData\RustDesk\config\RustDesk.toml',
    'C:\ProgramData\RustDesk\config\RustDesk2.toml'
)

foreach ($file in $files) {
    if (Test-Path $file) {
        Write-Host "正在更新: $file" -ForegroundColor Cyan
        
        # 读取内容
        $content = Get-Content $file -Raw
        
        # 替换密钥
        $content = $content -replace 'key\s*=\s*"[^"]*"', "key = `"$newKey`""
        
        # 写回文件
        $content | Set-Content $file -NoNewline
        
        Write-Host "✓ 完成" -ForegroundColor Green
    } else {
        Write-Host "✗ 文件不存在: $file" -ForegroundColor Red
    }
}

# 同时清理用户配置中的本地密钥对
$userConfig = "$env:APPDATA\RustDesk\config\RustDesk.toml"
if (Test-Path $userConfig) {
    Write-Host "`n正在清理用户配置中的本地密钥对..." -ForegroundColor Cyan
    
    $content = Get-Content $userConfig -Raw
    
    # 设置 key_confirmed = false，强制重新验证
    $content = $content -replace 'key_confirmed\s*=\s*true', 'key_confirmed = false'
    
    $content | Set-Content $userConfig -NoNewline
    Write-Host "✓ 已重置密钥确认状态" -ForegroundColor Green
}

Write-Host "`n=== 完成！请重启 RustDesk ===" -ForegroundColor Green
Write-Host "1. 完全退出 RustDesk（托盘右键 -> 退出）" -ForegroundColor Yellow
Write-Host "2. 重新启动 RustDesk" -ForegroundColor Yellow
Write-Host "3. 在两台电脑上都执行此操作" -ForegroundColor Yellow

pause
