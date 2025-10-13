# RustDesk 服务器密钥备份脚本
# 定期运行此脚本以备份服务器密钥

param(
    [string]$BackupPath = "d:\Rustdesk\backup",
    [switch]$Verify
)

$plinkPath = "D:\Program Files\PuTTY\plink.exe"
$pscpPath = "D:\Program Files\PuTTY\pscp.exe"
$keyPath = "d:\Rustdesk\cislink.ppk"
$server = "root@142.132.187.134"

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "RustDesk 密钥备份工具" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan

# 创建备份目录
if (-not (Test-Path $BackupPath)) {
    New-Item -ItemType Directory -Path $BackupPath -Force | Out-Null
    Write-Host "✓ 创建备份目录: $BackupPath" -ForegroundColor Green
}

# 添加时间戳
$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$backupDir = Join-Path $BackupPath $timestamp
New-Item -ItemType Directory -Path $backupDir -Force | Out-Null

Write-Host "`n正在备份密钥文件..." -ForegroundColor Yellow

try {
    # 备份私钥
    & $pscpPath -batch -i $keyPath `
        "${server}:/opt/rustdesk/data/id_ed25519" `
        "$backupDir\id_ed25519"
    
    # 备份公钥
    & $pscpPath -batch -i $keyPath `
        "${server}:/opt/rustdesk/data/id_ed25519.pub" `
        "$backupDir\id_ed25519.pub"
    
    # 备份数据库
    & $pscpPath -batch -i $keyPath `
        "${server}:/opt/rustdesk/data/db_v2.sqlite3" `
        "$backupDir\db_v2.sqlite3"
    
    Write-Host "✓ 密钥文件备份成功" -ForegroundColor Green
    
    # 读取并保存公钥文本
    $pubKeyContent = Get-Content "$backupDir\id_ed25519.pub" -Raw
    $pubKeyContent | Set-Content "$backupDir\PUBLIC_KEY.txt"
    
    # 创建备份信息文件
    $backupInfo = @"
RustDesk 密钥备份信息
=====================

备份时间: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
服务器: 142.132.187.134
域名: hbbs.cislink.nl

公钥内容:
$pubKeyContent

文件列表:
- id_ed25519 (私钥)
- id_ed25519.pub (公钥)
- db_v2.sqlite3 (数据库)
- PUBLIC_KEY.txt (公钥文本)

恢复方法:
---------
1. 停止 Docker 服务:
   cd /opt/rustdesk && docker-compose down

2. 复制密钥文件到服务器:
   pscp -i cislink.ppk id_ed25519* root@142.132.187.134:/opt/rustdesk/data/

3. 重启服务:
   cd /opt/rustdesk && docker-compose up -d

注意事项:
---------
- 妥善保管此备份文件
- 不要将私钥文件上传到公开位置
- 定期验证备份完整性
"@
    
    $backupInfo | Set-Content "$backupDir\BACKUP_INFO.txt" -Encoding UTF8
    
    Write-Host "`n=========================================" -ForegroundColor Cyan
    Write-Host "备份完成！" -ForegroundColor Green
    Write-Host "=========================================" -ForegroundColor Cyan
    Write-Host "备份位置: $backupDir" -ForegroundColor White
    Write-Host "`n备份文件:" -ForegroundColor Cyan
    Get-ChildItem $backupDir | Format-Table Name, Length, LastWriteTime -AutoSize
    
    Write-Host "`n公钥内容:" -ForegroundColor Cyan
    Write-Host $pubKeyContent -ForegroundColor Green
    
} catch {
    Write-Host "✗ 备份失败: $_" -ForegroundColor Red
    exit 1
}

# 如果指定了验证
if ($Verify) {
    Write-Host "`n验证备份完整性..." -ForegroundColor Yellow
    
    # 验证文件存在
    $requiredFiles = @("id_ed25519", "id_ed25519.pub", "PUBLIC_KEY.txt", "BACKUP_INFO.txt")
    $allExists = $true
    
    foreach ($file in $requiredFiles) {
        $filePath = Join-Path $backupDir $file
        if (Test-Path $filePath) {
            Write-Host "✓ $file" -ForegroundColor Green
        } else {
            Write-Host "✗ $file 缺失" -ForegroundColor Red
            $allExists = $false
        }
    }
    
    if ($allExists) {
        Write-Host "`n✓ 备份验证通过" -ForegroundColor Green
    } else {
        Write-Host "`n✗ 备份验证失败" -ForegroundColor Red
        exit 1
    }
}

# 清理旧备份（保留最近10个）
Write-Host "`n清理旧备份..." -ForegroundColor Yellow
$backups = Get-ChildItem $BackupPath -Directory | Sort-Object Name -Descending
if ($backups.Count -gt 10) {
    $toDelete = $backups | Select-Object -Skip 10
    foreach ($old in $toDelete) {
        Remove-Item $old.FullName -Recurse -Force
        Write-Host "✓ 删除旧备份: $($old.Name)" -ForegroundColor Gray
    }
} else {
    Write-Host "✓ 当前备份数量: $($backups.Count)" -ForegroundColor Gray
}

Write-Host "`n建议:" -ForegroundColor Yellow
Write-Host "- 将备份文件保存到安全位置（如加密U盘、云存储）" -ForegroundColor White
Write-Host "- 定期运行此脚本以保持备份最新" -ForegroundColor White
Write-Host "- 不要删除最近的备份" -ForegroundColor White

Write-Host "`n完成！" -ForegroundColor Green
