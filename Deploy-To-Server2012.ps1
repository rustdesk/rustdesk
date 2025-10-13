# 远程更新 Windows Server 2012 上的 RustDesk 配置
# 使用方法: .\Deploy-To-Server2012.ps1 -ComputerName "服务器名或IP"

param(
    [Parameter(Mandatory=$true)]
    [string]$ComputerName,
    
    [Parameter(Mandatory=$false)]
    [PSCredential]$Credential
)

$newKey = 'AAAAC3NzaC1lZDI1NTE5AAAAIBAjWWVdpMda/rF5zAObc92HsyO2xWyNNaUtQByf0RYI'

Write-Host "=== 远程部署 RustDesk 配置到 $ComputerName ===" -ForegroundColor Cyan
Write-Host ""

# 准备连接参数
$sessionParams = @{
    ComputerName = $ComputerName
}

if ($Credential) {
    $sessionParams.Credential = $Credential
}

try {
    Write-Host "正在连接到 $ComputerName..." -ForegroundColor Yellow
    
    Invoke-Command @sessionParams -ScriptBlock {
        param($key)
        
        Write-Host "已连接！正在更新配置..." -ForegroundColor Green
        
        # 停止 RustDesk
        Stop-Process -Name rustdesk -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 2
        
        # 更新配置文件
        $files = @(
            'C:\ProgramData\RustDesk\config\RustDesk.toml',
            'C:\ProgramData\RustDesk\config\RustDesk2.toml'
        )
        
        foreach ($file in $files) {
            if (Test-Path $file) {
                Write-Host "更新: $file"
                $content = Get-Content $file -Raw
                $content = $content -replace 'key\s*=\s*"[^"]*"', "key = `"$key`""
                $content | Set-Content $file -NoNewline
                Write-Host "  [OK]" -ForegroundColor Green
            }
        }
        
        Write-Host "配置更新完成！" -ForegroundColor Green
        
    } -ArgumentList $newKey
    
    Write-Host ""
    Write-Host "=== 部署完成！===" -ForegroundColor Green
    Write-Host "请在服务器上重启 RustDesk" -ForegroundColor Yellow
    
} catch {
    Write-Host "错误: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "提示: 请确保:" -ForegroundColor Yellow
    Write-Host "1. 服务器允许 PowerShell 远程连接" -ForegroundColor Yellow
    Write-Host "2. 防火墙允许 WinRM (端口 5985/5986)" -ForegroundColor Yellow
    Write-Host "3. 您有管理员权限" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "可以使用以下命令启用远程管理:" -ForegroundColor Cyan
    Write-Host "  Enable-PSRemoting -Force" -ForegroundColor Cyan
}
