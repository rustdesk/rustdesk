# 更新客户端配置脚本 - 从 Docker 部署获取新 Key
# 在 Docker 部署完成后,运行此脚本更新客户端配置和安装程序

param(
    [Parameter(Mandatory=$true)]
    [string]$NewPublicKey,
    
    [switch]$CompileInstaller
)

$ErrorActionPreference = "Stop"

Write-Host "=========================================="
Write-Host "  RustDesk 客户端配置更新工具"
Write-Host "=========================================="
Write-Host ""

# 验证 Key 格式
if ($NewPublicKey -notmatch '^[A-Za-z0-9+/]+=*$') {
    Write-Host "❌ 错误: Public Key 格式不正确" -ForegroundColor Red
    Write-Host "   应该是 Base64 编码字符串,例如: AbCdEf1234567890...=" -ForegroundColor Yellow
    exit 1
}

Write-Host "🔑 新的 Public Key: $NewPublicKey" -ForegroundColor Cyan
Write-Host ""

# 1. 更新 RustDesk.toml
$rustDeskToml = "D:\Rustdesk\RustDesk.toml"
if (Test-Path $rustDeskToml) {
    Write-Host "📝 更新 RustDesk.toml..." -ForegroundColor Yellow
    $content = Get-Content $rustDeskToml -Raw
    $content = $content -replace 'key = "[^"]*"', "key = `"$NewPublicKey`""
    Set-Content -Path $rustDeskToml -Value $content -NoNewline
    Write-Host "   ✓ RustDesk.toml 已更新" -ForegroundColor Green
}

# 2. 更新 RustDesk2.toml
$rustDesk2Toml = "D:\Rustdesk\RustDesk2.toml"
if (Test-Path $rustDesk2Toml) {
    Write-Host "📝 更新 RustDesk2.toml..." -ForegroundColor Yellow
    $content = Get-Content $rustDesk2Toml -Raw
    $content = $content -replace "key = '[^']*'", "key = '$NewPublicKey'"
    Set-Content -Path $rustDesk2Toml -Value $content -NoNewline
    Write-Host "   ✓ RustDesk2.toml 已更新" -ForegroundColor Green
}

# 3. 更新 Deploy-RustDeskConfig.ps1
$deployScript = "D:\Rustdesk\Deploy-RustDeskConfig.ps1"
if (Test-Path $deployScript) {
    Write-Host "📝 更新 Deploy-RustDeskConfig.ps1..." -ForegroundColor Yellow
    $content = Get-Content $deployScript -Raw
    $content = $content -replace 'Key = "[^"]*"', "Key = `"$NewPublicKey`""
    Set-Content -Path $deployScript -Value $content -NoNewline
    Write-Host "   ✓ Deploy-RustDeskConfig.ps1 已更新" -ForegroundColor Green
}

# 4. 更新 Deploy-RustDesk2Config.ps1
$deploy2Script = "D:\Rustdesk\Deploy-RustDesk2Config.ps1"
if (Test-Path $deploy2Script) {
    Write-Host "📝 更新 Deploy-RustDesk2Config.ps1..." -ForegroundColor Yellow
    $content = Get-Content $deploy2Script -Raw
    $content = $content -replace 'Key = "[^"]*"', "Key = `"$NewPublicKey`""
    Set-Content -Path $deploy2Script -Value $content -NoNewline
    Write-Host "   ✓ Deploy-RustDesk2Config.ps1 已更新" -ForegroundColor Green
}

Write-Host ""
Write-Host "✅ 所有配置文件已更新!" -ForegroundColor Green
Write-Host ""

# 5. 可选: 编译新的安装程序
if ($CompileInstaller) {
    Write-Host "🔨 编译新的安装程序..." -ForegroundColor Yellow
    $innoSetup = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
    $issFile = "D:\Rustdesk\Deploy-Config.iss"
    
    if (-not (Test-Path $innoSetup)) {
        Write-Host "   ⚠️  未找到 Inno Setup" -ForegroundColor Yellow
        Write-Host "   请手动编译: & `"$innoSetup`" `"$issFile`"" -ForegroundColor Cyan
    } elseif (-not (Test-Path $issFile)) {
        Write-Host "   ⚠️  未找到 Deploy-Config.iss" -ForegroundColor Yellow
    } else {
        & $innoSetup $issFile
        if ($LASTEXITCODE -eq 0) {
            Write-Host "   ✓ 安装程序编译成功!" -ForegroundColor Green
            $installer = "D:\Rustdesk\Output\RustDesk_Config_Installer.exe"
            if (Test-Path $installer) {
                $fileInfo = Get-Item $installer
                Write-Host ""
                Write-Host "   📦 安装程序信息:" -ForegroundColor Cyan
                Write-Host "      文件: $installer" -ForegroundColor White
                Write-Host "      大小: $([math]::Round($fileInfo.Length/1MB,2)) MB" -ForegroundColor White
                Write-Host "      时间: $($fileInfo.LastWriteTime)" -ForegroundColor White
            }
        } else {
            Write-Host "   ❌ 编译失败,请检查错误信息" -ForegroundColor Red
        }
    }
}

Write-Host ""
Write-Host "=========================================="
Write-Host "  更新完成!"
Write-Host "=========================================="
Write-Host ""
Write-Host "📋 下一步操作:" -ForegroundColor Cyan
Write-Host "1. 测试本地配置:" -ForegroundColor White
Write-Host "   powershell -ExecutionPolicy Bypass -File `"D:\Rustdesk\Deploy-RustDesk2Config.ps1`"" -ForegroundColor Gray
Write-Host ""
Write-Host "2. 如未编译安装程序,请手动编译:" -ForegroundColor White
Write-Host "   & `"C:\Program Files (x86)\Inno Setup 6\ISCC.exe`" `"D:\Rustdesk\Deploy-Config.iss`"" -ForegroundColor Gray
Write-Host ""
Write-Host "3. 分发到客户端:" -ForegroundColor White
Write-Host "   D:\Rustdesk\Output\RustDesk_Config_Installer.exe" -ForegroundColor Gray
Write-Host ""
Write-Host "🔑 新 Public Key: $NewPublicKey" -ForegroundColor Green
Write-Host "=========================================="
