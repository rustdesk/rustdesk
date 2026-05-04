# RustDesk Configuration Deployment Script
# 此脚本会自动部署 RustDesk 配置文件到正确的位置

param(
    [switch]$Silent,
    [switch]$RestartService
)

# 设置错误处理
$ErrorActionPreference = "Stop"

# 服务器配置信息
$serverConfig = @{
    Server = "hbbs.cislink.nl"
    Relay = "hbbr.cislink.nl"
    Key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
}

# 定义部署路径
$deployPaths = @(
    "$env:APPDATA\RustDesk\config",
    "$env:ProgramData\RustDesk\config",
    "$env:LOCALAPPDATA\RustDesk\config"
)

$configFiles = @("RustDesk.toml", "RustDesk2.toml")

function Write-Log {
    param($Message, $Type = "INFO")
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $logMessage = "[$timestamp] [$Type] $Message"
    
    if (-not $Silent) {
        switch ($Type) {
            "ERROR" { Write-Host $logMessage -ForegroundColor Red }
            "SUCCESS" { Write-Host $logMessage -ForegroundColor Green }
            "WARNING" { Write-Host $logMessage -ForegroundColor Yellow }
            default { Write-Host $logMessage -ForegroundColor Cyan }
        }
    }
    
    # 写入日志文件
    $logFile = "$env:TEMP\RustDesk_Config_Deploy.log"
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

function Start-RustDeskService {
    Write-Log "正在启动 RustDesk 服务..."
    try {
        # 尝试启动服务
        $service = Get-Service -Name "RustDesk" -ErrorAction SilentlyContinue
        if ($service) {
            if ($service.Status -ne "Running") {
                Start-Service -Name "RustDesk"
                Write-Log "RustDesk 服务已启动" "SUCCESS"
            } else {
                Write-Log "RustDesk 服务已在运行中"
            }
        } else {
            Write-Log "未找到 RustDesk 服务" "WARNING"
        }
    } catch {
        Write-Log "启动服务时出错: $_" "WARNING"
    }
}

function Deploy-Configuration {
    Write-Log "=" * 60
    Write-Log "RustDesk 配置部署工具 v1.0"
    Write-Log "=" * 60
    Write-Log ""
    
    # 检查管理员权限
    $isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if ($isAdmin) {
        Write-Log "✓ 以管理员权限运行" "SUCCESS"
    } else {
        Write-Log "⚠ 未以管理员权限运行，某些操作可能失败" "WARNING"
    }
    Write-Log ""
    
    # 停止 RustDesk 进程
    Stop-RustDeskProcess
    Write-Log ""
    
    # 备份现有配置
    Write-Log "正在备份现有配置..."
    $backupPath = "$env:TEMP\RustDesk_Backup_$(Get-Date -Format 'yyyyMMdd_HHmmss')"
    New-Item -ItemType Directory -Path $backupPath -Force | Out-Null
    
    foreach ($path in $deployPaths) {
        if (Test-Path $path) {
            foreach ($file in $configFiles) {
                $sourceFile = Join-Path $path $file
                if (Test-Path $sourceFile) {
                    $backupFile = Join-Path $backupPath "$($path -replace '[:\\]','_')_$file"
                    Copy-Item -Path $sourceFile -Destination $backupFile -Force
                    Write-Log "  备份: $sourceFile"
                }
            }
        }
    }
    Write-Log "配置已备份到: $backupPath" "SUCCESS"
    Write-Log ""
    
    # 部署新配置
    Write-Log "正在部署新配置..."
    $successCount = 0
    $failCount = 0
    
    foreach ($path in $deployPaths) {
        try {
            # 创建目录
            if (-not (Test-Path $path)) {
                New-Item -ItemType Directory -Path $path -Force | Out-Null
                Write-Log "  创建目录: $path"
            }
            
            # 写入配置文件
            foreach ($file in $configFiles) {
                $targetFile = Join-Path $path $file
                
                # 读取现有配置(如果存在)
                $existingContent = ""
                $hasOptions = $false
                
                if (Test-Path $targetFile) {
                    $existingContent = Get-Content -Path $targetFile -Raw
                    $hasOptions = $existingContent -match '\[options\]'
                }
                
                # 如果文件存在且有客户端配置,则合并配置
                if ($existingContent -and -not $hasOptions) {
                    # 文件存在但没有 [options] 部分,追加服务器配置
                    $newContent = $existingContent.TrimEnd()
                    $newContent += "`n`n[options]`n"
                    $newContent += "custom-rendezvous-server = `"$($serverConfig.Server)`"`n"
                    $newContent += "relay-server = `"$($serverConfig.Relay)`"`n"
                    $newContent += "key = `"$($serverConfig.Key)`"`n"
                    
                    Set-Content -Path $targetFile -Value $newContent -Force -Encoding UTF8
                    Write-Log "  ✓ 合并配置: $targetFile" "SUCCESS"
                } elseif ($existingContent -and $hasOptions) {
                    # 文件存在且有 [options] 部分,更新服务器配置
                    $newContent = $existingContent
                    $newContent = $newContent -replace 'custom-rendezvous-server\s*=\s*"[^"]*"', "custom-rendezvous-server = `"$($serverConfig.Server)`""
                    $newContent = $newContent -replace 'relay-server\s*=\s*"[^"]*"', "relay-server = `"$($serverConfig.Relay)`""
                    $newContent = $newContent -replace 'key\s*=\s*"[^"]*"', "key = `"$($serverConfig.Key)`""
                    
                    Set-Content -Path $targetFile -Value $newContent -Force -Encoding UTF8
                    Write-Log "  ✓ 更新配置: $targetFile" "SUCCESS"
                } else {
                    # 文件不存在,创建新文件(仅包含服务器配置)
                    $newContent = "[options]`n"
                    $newContent += "custom-rendezvous-server = `"$($serverConfig.Server)`"`n"
                    $newContent += "relay-server = `"$($serverConfig.Relay)`"`n"
                    $newContent += "key = `"$($serverConfig.Key)`"`n"
                    
                    Set-Content -Path $targetFile -Value $newContent -Force -Encoding UTF8
                    Write-Log "  ✓ 创建配置: $targetFile" "SUCCESS"
                }
                
                $successCount++
            }
        } catch {
            Write-Log "  ✗ 部署失败 ($path): $_" "ERROR"
            $failCount++
        }
    }
    
    Write-Log ""
    Write-Log "部署完成: 成功 $successCount 个, 失败 $failCount 个" $(if ($failCount -eq 0) { "SUCCESS" } else { "WARNING" })
    Write-Log ""
    
    # 验证部署
    Write-Log "正在验证配置..."
    foreach ($path in $deployPaths) {
        foreach ($file in $configFiles) {
            $targetFile = Join-Path $path $file
            if (Test-Path $targetFile) {
                $content = Get-Content -Path $targetFile -Raw
                $hasOptions = $content -match '\[options\]'
                $hasKey = $content -match "key\s*=\s*`"$($serverConfig.Key)`""
                $hasServer = $content -match "custom-rendezvous-server\s*=\s*`"$($serverConfig.Server)`""
                
                if ($hasOptions -and $hasKey -and $hasServer) {
                    Write-Log "  ✓ 验证通过: $targetFile" "SUCCESS"
                } else {
                    if (-not $hasOptions) {
                        Write-Log "  ✗ 验证失败: $targetFile (缺少 [options] 部分)" "ERROR"
                    } elseif (-not $hasKey) {
                        Write-Log "  ✗ 验证失败: $targetFile (Key 不匹配)" "ERROR"
                    } elseif (-not $hasServer) {
                        Write-Log "  ✗ 验证失败: $targetFile (Server 不匹配)" "ERROR"
                    }
                }
            }
        }
    }
    
    Write-Log ""
    
    # 重启服务
    if ($RestartService) {
        Start-RustDeskService
        Write-Log ""
    }
    
    Write-Log "=" * 60
    Write-Log "部署完成!"
    Write-Log "备份位置: $backupPath"
    Write-Log "日志文件: $env:TEMP\RustDesk_Config_Deploy.log"
    Write-Log "=" * 60
    
    if (-not $Silent) {
        Write-Host ""
        Write-Host "按任意键退出..." -ForegroundColor Yellow
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    }
}

# 执行部署
try {
    Deploy-Configuration
    exit 0
} catch {
    Write-Log "部署过程中发生错误: $_" "ERROR"
    Write-Log $_.ScriptStackTrace "ERROR"
    if (-not $Silent) {
        Write-Host ""
        Write-Host "按任意键退出..." -ForegroundColor Red
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    }
    exit 1
}
