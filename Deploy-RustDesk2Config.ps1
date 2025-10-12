# RustDesk2.toml Configuration Deployment Script
# ⭐ 此脚本专门部署 RustDesk2.toml (服务器配置文件)
# RustDesk2.toml 不会被客户端自动覆盖!

param(
    [switch]$Silent,
    [switch]$RestartService
)

$ErrorActionPreference = "Stop"

# 服务器配置信息
$serverConfig = @{
    Server = "hbbs.cislink.nl"
    ServerPort = "21116"
    Relay = "hbbr.cislink.nl"
    Key = "3LyfStjRsPhYPBqqpPQN7Tamhkk1L2Mw6ksqpiLaj1s="
}

function Write-Log {
    param($Message, $Type = "INFO")
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $colors = @{
        "INFO" = "White"
        "SUCCESS" = "Green"
        "WARNING" = "Yellow"
        "ERROR" = "Red"
    }
    
    $color = if ($colors.ContainsKey($Type)) { $colors[$Type] } else { "White" }
    $logMessage = "[$timestamp] [$Type] $Message"
    
    if (-not $Silent) {
        Write-Host $logMessage -ForegroundColor $color
    }
    
    $logFile = "$env:TEMP\RustDesk2_Config_Deploy.log"
    Add-Content -Path $logFile -Value $logMessage
}

function Stop-RustDeskProcess {
    Write-Log "正在停止 RustDesk 进程..."
    try {
        $processes = Get-Process | Where-Object { $_.ProcessName -like "*rustdesk*" }
        if ($processes) {
            $processes | ForEach-Object {
                Write-Log "  停止进程: $($_.ProcessName) (PID: $($_.Id))"
                $_ | Stop-Process -Force -ErrorAction SilentlyContinue
            }
            Start-Sleep -Seconds 2
            Write-Log "RustDesk 进程已停止" "SUCCESS"
        } else {
            Write-Log "未发现运行中的 RustDesk 进程"
        }
    } catch {
        Write-Log "停止进程时出错: $_" "WARNING"
    }
}

function Deploy-RustDesk2Config {
    Write-Log "=" * 60
    Write-Log "RustDesk2.toml 配置部署工具 v1.4"
    Write-Log "=" * 60
    Write-Log ""
    
    # 配置目录
    $configPath = "$env:APPDATA\RustDesk\config"
    $configFile = Join-Path $configPath "RustDesk2.toml"
    
    # 创建备份
    if (Test-Path $configFile) {
        $backupDir = "$env:TEMP\RustDesk_Backup_$(Get-Date -Format 'yyyyMMdd_HHmmss')"
        New-Item -ItemType Directory -Path $backupDir -Force | Out-Null
        Copy-Item $configFile -Destination $backupDir -Force
        Write-Log "已备份配置到: $backupDir" "SUCCESS"
        Write-Log ""
    }
    
    # 停止 RustDesk
    Stop-RustDeskProcess
    Write-Log ""
    
    # 确保配置目录存在
    if (-not (Test-Path $configPath)) {
        New-Item -ItemType Directory -Path $configPath -Force | Out-Null
        Write-Log "已创建配置目录: $configPath"
    }
    
    # 读取现有配置
    $existingContent = $null
    if (Test-Path $configFile) {
        $existingContent = Get-Content -Path $configFile -Raw
        Write-Log "读取现有配置: $configFile"
    }
    
    # 构建新配置
    Write-Log "正在构建新配置..."
    
    # 保留现有的基本设置
    $baseSettings = @()
    if ($existingContent) {
        # 提取 [options] 之前的所有设置
        $lines = $existingContent -split "`n"
        foreach ($line in $lines) {
            $trimmed = $line.Trim()
            if ($trimmed -eq "[options]") {
                break
            }
            if ($trimmed -and $trimmed -notmatch '^#') {
                $baseSettings += $line
            }
        }
    }
    
    # 如果没有基本设置,使用默认值
    if ($baseSettings.Count -eq 0) {
        $baseSettings = @(
            "rendezvous_server = '$($serverConfig.Server):$($serverConfig.ServerPort)'",
            "nat_type = 1",
            "serial = 0"
        )
    } else {
        # 更新 rendezvous_server
        $baseSettings = $baseSettings | ForEach-Object {
            if ($_ -match '^rendezvous_server\s*=') {
                "rendezvous_server = '$($serverConfig.Server):$($serverConfig.ServerPort)'"
            } else {
                $_
            }
        }
    }
    
    # 构建完整配置
    $newConfig = $baseSettings -join "`n"
    $newConfig += "`n`n[options]`n"
    $newConfig += "relay-server = '$($serverConfig.Relay)'`n"
    $newConfig += "key = '$($serverConfig.Key)'`n"
    $newConfig += "custom-rendezvous-server = '$($serverConfig.Server)'`n"
    
    # 写入配置
    try {
        Set-Content -Path $configFile -Value $newConfig -Force -Encoding UTF8
        Write-Log "✓ 配置已更新: $configFile" "SUCCESS"
        Write-Log ""
        
        # 验证配置
        Write-Log "正在验证配置..."
        $content = Get-Content -Path $configFile -Raw
        $hasOptions = $content -match '\[options\]'
        $hasKey = $content -match "key\s*=\s*'$($serverConfig.Key)'"
        $hasServer = $content -match "custom-rendezvous-server\s*=\s*'$($serverConfig.Server)'"
        
        if ($hasOptions -and $hasKey -and $hasServer) {
            Write-Log "✓ 配置验证通过!" "SUCCESS"
            Write-Log ""
            Write-Log "配置内容预览:" "INFO"
            Write-Log "-" * 60
            $content -split "`n" | Select-Object -First 15 | ForEach-Object { Write-Log "  $_" }
            Write-Log "-" * 60
        } else {
            Write-Log "✗ 配置验证失败!" "ERROR"
            if (-not $hasOptions) { Write-Log "  - 缺少 [options] 部分" "ERROR" }
            if (-not $hasKey) { Write-Log "  - Key 不匹配" "ERROR" }
            if (-not $hasServer) { Write-Log "  - Server 不匹配" "ERROR" }
        }
        
    } catch {
        Write-Log "✗ 写入配置失败: $_" "ERROR"
        throw
    }
    
    Write-Log ""
    Write-Log "=" * 60
    Write-Log "部署完成!"
    Write-Log "配置文件: $configFile"
    Write-Log "日志文件: $env:TEMP\RustDesk2_Config_Deploy.log"
    Write-Log "=" * 60
    Write-Log ""
    Write-Log "⚠️ 重要提示:" "WARNING"
    Write-Log "1. 请重新启动 RustDesk 客户端"
    Write-Log "2. RustDesk2.toml 不会被客户端自动覆盖"
    Write-Log "3. 服务器配置已持久化保存"
    Write-Log ""
    
    if (-not $Silent) {
        Write-Host "按任意键退出..." -ForegroundColor Yellow
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    }
}

# 执行部署
try {
    Deploy-RustDesk2Config
    exit 0
} catch {
    Write-Log "部署失败: $_" "ERROR"
    Write-Log $_.ScriptStackTrace "ERROR"
    if (-not $Silent) {
        Write-Host ""
        Write-Host "按任意键退出..." -ForegroundColor Red
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    }
    exit 1
}
