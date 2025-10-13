@echo off
echo ========================================
echo 更新 RustDesk 密钥配置
echo ========================================
echo.
echo 正在更新配置文件...
echo.

:: 创建临时 PowerShell 脚本
(
echo $newKey = 'AAAAC3NzaC1lZDI1NTE5AAAAIBAjWWVdpMda/rF5zAObc92HsyO2xWyNNaUtQByf0RYI'
echo.
echo $files = @^(
echo     'C:\ProgramData\RustDesk\config\RustDesk.toml',
echo     'C:\ProgramData\RustDesk\config\RustDesk2.toml'
echo ^)
echo.
echo foreach ^($file in $files^) {
echo     if ^(Test-Path $file^) {
echo         Write-Host "更新: $file" -ForegroundColor Cyan
echo         $content = Get-Content $file -Raw
echo         $content = $content -replace 'key\s*=\s*\"[^\"]*\"', "key = \`"$newKey\`""
echo         $content ^| Set-Content $file -NoNewline
echo         Write-Host "完成!" -ForegroundColor Green
echo     }
echo }
echo.
echo $userConfig = "$env:APPDATA\RustDesk\config\RustDesk.toml"
echo if ^(Test-Path $userConfig^) {
echo     Write-Host "重置用户配置..." -ForegroundColor Cyan
echo     $content = Get-Content $userConfig -Raw
echo     $content = $content -replace 'key_confirmed\s*=\s*true', 'key_confirmed = false'
echo     $content ^| Set-Content $userConfig -NoNewline
echo     Write-Host "完成!" -ForegroundColor Green
echo }
echo.
echo Write-Host ""
echo Write-Host "========================================" -ForegroundColor Green
echo Write-Host "更新完成！" -ForegroundColor Green
echo Write-Host "========================================" -ForegroundColor Green
echo Write-Host ""
echo Write-Host "请执行以下步骤:" -ForegroundColor Yellow
echo Write-Host "1. 完全退出 RustDesk (托盘右键 -^> 退出^)" -ForegroundColor Yellow
echo Write-Host "2. 重新启动 RustDesk" -ForegroundColor Yellow
echo Write-Host "3. 在另一台电脑上也执行此操作" -ForegroundColor Yellow
echo Write-Host ""
echo pause
) > "%TEMP%\fix-rustdesk.ps1"

:: 以管理员权限运行 PowerShell 脚本
powershell -ExecutionPolicy Bypass -File "%TEMP%\fix-rustdesk.ps1"

:: 清理临时文件
del "%TEMP%\fix-rustdesk.ps1"

echo.
echo 按任意键退出...
pause >nul
