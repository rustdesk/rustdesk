# RustDesk Docker 部署 - Windows 控制脚本
# 通过 PuTTY/plink 在远程服务器部署 Docker 版本

param(
    [switch]$Deploy,
    [switch]$Status,
    [switch]$Logs,
    [switch]$Restart,
    [switch]$Stop,
    [switch]$GetKey
)

$plinkPath = "D:\Program Files\PuTTY\plink.exe"
$keyPath = "d:\Rustdesk\cislink.ppk"
$server = "root@142.132.187.134"

function Invoke-RemoteCommand {
    param([string]$Command)
    & $plinkPath -batch -i $keyPath $server $Command
}

if ($Deploy) {
    Write-Host "=========================================" -ForegroundColor Cyan
    Write-Host "部署 RustDesk Docker 版本" -ForegroundColor Cyan
    Write-Host "=========================================" -ForegroundColor Cyan
    
    # 上传部署脚本
    Write-Host "`n上传部署脚本..." -ForegroundColor Yellow
    & "D:\Program Files\PuTTY\pscp.exe" -i $keyPath "d:\Rustdesk\deploy-docker.sh" "${server}:/tmp/deploy-docker.sh"
    
    # 执行部署
    Write-Host "`n执行部署..." -ForegroundColor Yellow
    Invoke-RemoteCommand "chmod +x /tmp/deploy-docker.sh && /tmp/deploy-docker.sh"
    
    Write-Host "`n部署完成！" -ForegroundColor Green
    Write-Host "现在获取服务器公钥..." -ForegroundColor Yellow
    
    Start-Sleep -Seconds 2
    & $PSCommandPath -GetKey
}
elseif ($Status) {
    Write-Host "=========================================" -ForegroundColor Cyan
    Write-Host "服务状态" -ForegroundColor Cyan
    Write-Host "=========================================" -ForegroundColor Cyan
    Invoke-RemoteCommand "cd /opt/rustdesk && docker-compose ps"
}
elseif ($Logs) {
    Write-Host "=========================================" -ForegroundColor Cyan
    Write-Host "服务日志 (Ctrl+C 退出)" -ForegroundColor Cyan
    Write-Host "=========================================" -ForegroundColor Cyan
    Invoke-RemoteCommand "cd /opt/rustdesk && docker-compose logs -f --tail=50"
}
elseif ($Restart) {
    Write-Host "重启服务..." -ForegroundColor Yellow
    Invoke-RemoteCommand "cd /opt/rustdesk && docker-compose restart"
    Write-Host "服务已重启" -ForegroundColor Green
}
elseif ($Stop) {
    Write-Host "停止服务..." -ForegroundColor Yellow
    Invoke-RemoteCommand "cd /opt/rustdesk && docker-compose down"
    Write-Host "服务已停止" -ForegroundColor Green
}
elseif ($GetKey) {
    Write-Host "=========================================" -ForegroundColor Cyan
    Write-Host "服务器公钥" -ForegroundColor Cyan
    Write-Host "=========================================" -ForegroundColor Cyan
    
    $pubKey = Invoke-RemoteCommand "cat /opt/rustdesk/data/id_ed25519.pub"
    
    if ($pubKey) {
        Write-Host "`n完整公钥:" -ForegroundColor Yellow
        Write-Host $pubKey -ForegroundColor White
        
        # 提取 Base64 部分（去掉 ssh-ed25519 和注释）
        $base64Key = ($pubKey -split '\s+')[1]
        Write-Host "`nBase64 部分（用于客户端配置）:" -ForegroundColor Yellow
        Write-Host $base64Key -ForegroundColor Green
        
        # 保存到文件
        $base64Key | Set-Content "d:\Rustdesk\server_public_key.txt"
        Write-Host "`n✓ 公钥已保存到: d:\Rustdesk\server_public_key.txt" -ForegroundColor Green
        
        # 更新客户端配置
        Write-Host "`n是否立即更新本机客户端配置？(Y/N)" -ForegroundColor Yellow
        $response = Read-Host
        if ($response -eq 'Y' -or $response -eq 'y') {
            $configContent = @"
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "$base64Key"
"@
            
            # 更新 ProgramData 配置
            $configContent | Set-Content "C:\ProgramData\RustDesk\config\RustDesk.toml"
            $configContent | Set-Content "C:\ProgramData\RustDesk\config\RustDesk2.toml"
            
            # 更新 AppData 配置
            $appDataConfig = "$env:APPDATA\RustDesk\config\RustDesk2.toml"
            if (Test-Path $appDataConfig) {
                $content = Get-Content $appDataConfig -Raw
                if ($content -match "\[options\]") {
                    $content = $content -replace "key = '[^']*'", "key = '$base64Key'"
                    if ($content -notmatch "key = ") {
                        $content = $content -replace "(\[options\][^\r\n]*)", "`$1`r`nkey = '$base64Key'"
                    }
                    $content | Set-Content $appDataConfig -NoNewline
                }
            }
            
            Write-Host "✓ 客户端配置已更新" -ForegroundColor Green
            Write-Host "`n请重启 RustDesk 客户端以应用新配置" -ForegroundColor Yellow
        }
    } else {
        Write-Host "无法获取公钥，请稍后重试" -ForegroundColor Red
    }
}
else {
    Write-Host @"
RustDesk Docker 管理脚本

用法:
    .\Deploy-RustDesk-Docker.ps1 -Deploy    # 部署/重新部署服务
    .\Deploy-RustDesk-Docker.ps1 -Status    # 查看服务状态
    .\Deploy-RustDesk-Docker.ps1 -Logs      # 查看实时日志
    .\Deploy-RustDesk-Docker.ps1 -Restart   # 重启服务
    .\Deploy-RustDesk-Docker.ps1 -Stop      # 停止服务
    .\Deploy-RustDesk-Docker.ps1 -GetKey    # 获取服务器公钥

示例:
    .\Deploy-RustDesk-Docker.ps1 -Deploy
    .\Deploy-RustDesk-Docker.ps1 -GetKey
"@ -ForegroundColor Cyan
}
