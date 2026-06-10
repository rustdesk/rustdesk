# ===================================================
# 一键安装Cislink版RustDesk - PowerShell版本
# 自动请求管理员权限并静默安装
# ===================================================

param(
    [switch]$Silent = $false,  # 完全静默安装（不显示安装界面）
    [switch]$NoAutoStart = $false  # 安装后不自动启动RustDesk
)

# 颜色输出函数
function Write-ColorText {
    param([string]$Text, [string]$Color = "White")
    Write-Host $Text -ForegroundColor $Color
}

# 检查并请求管理员权限
function Request-AdminPrivileges {
    $isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

    if (-not $isAdmin) {
        Write-ColorText "`n[提示] 正在请求管理员权限..." "Yellow"
        Write-ColorText "      请在UAC提示中点击'是'" "Gray"

        # 构建参数
        $arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
        if ($Silent) { $arguments += " -Silent" }
        if ($NoAutoStart) { $arguments += " -NoAutoStart" }

        try {
            Start-Process powershell -Verb RunAs -ArgumentList $arguments
            exit
        } catch {
            Write-ColorText "`n[错误] 无法获取管理员权限" "Red"
            Write-ColorText "      请右键点击脚本 -> 以管理员身份运行" "Yellow"
            pause
            exit 1
        }
    }
}

# 查找安装包
function Find-Installer {
    Write-ColorText "`n[1/4] 查找安装包..." "Cyan"

    $scriptDir = Split-Path -Parent $PSCommandPath
    $possiblePaths = @(
        Join-Path $scriptDir "RustDesk_Cislink_Installer_v2.1.exe",
        Join-Path $scriptDir "Output\RustDesk_Cislink_Installer_v2.1.exe",
        (Get-ChildItem -Path $scriptDir -Filter "RustDesk*Installer*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1).FullName
    )

    foreach ($path in $possiblePaths) {
        if ($path -and (Test-Path $path)) {
            Write-ColorText "   [OK] 找到: $(Split-Path -Leaf $path)" "Green"
            return $path
        }
    }

    Write-ColorText "   [错误] 未找到安装包" "Red"
    Write-ColorText "   请确保以下文件存在：" "Yellow"
    Write-ColorText "   - RustDesk_Cislink_Installer_v2.1.exe" "Gray"
    Write-ColorText "   - Output\RustDesk_Cislink_Installer_v2.1.exe" "Gray"
    return $null
}

# 停止RustDesk进程
function Stop-RustDeskProcesses {
    Write-ColorText "`n[2/4] 停止RustDesk进程..." "Cyan"

    $processes = Get-Process | Where-Object { $_.Name -like "rustdesk*" }

    if ($processes) {
        $processes | ForEach-Object {
            Write-ColorText "   停止: $($_.Name) (PID: $($_.Id))" "Gray"
            Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        }
        Start-Sleep -Seconds 2
        Write-ColorText "   [OK] 进程已停止" "Green"
    } else {
        Write-ColorText "   [提示] RustDesk未运行" "Gray"
    }
}

# 运行安装包
function Start-Installation {
    param([string]$InstallerPath)

    Write-ColorText "`n[3/4] 开始安装..." "Cyan"

    # 构建安装参数
    $arguments = "/LOG"  # 生成安装日志

    if ($Silent) {
        $arguments += " /VERYSILENT /NORESTART"
        Write-ColorText "   [模式] 静默安装" "Gray"
    } else {
        $arguments += " /SILENT /NORESTART"
        Write-ColorText "   [模式] 简化安装界面" "Gray"
    }

    Write-ColorText "   [提示] 正在安装..." "Yellow"
    Write-ColorText "   安装包: $(Split-Path -Leaf $InstallerPath)" "Gray"

    try {
        # 启动安装进程并等待完成
        $process = Start-Process -FilePath $InstallerPath -ArgumentList $arguments -Wait -PassThru

        if ($process.ExitCode -eq 0) {
            Write-ColorText "`n   [OK] 安装成功完成！" "Green"
            return $true
        } else {
            Write-ColorText "`n   [警告] 安装程序退出码: $($process.ExitCode)" "Yellow"
            return $true  # 仍然认为成功，因为某些退出码是正常的
        }
    } catch {
        Write-ColorText "`n   [错误] 安装失败: $_" "Red"
        return $false
    }
}

# 验证安装
function Test-Installation {
    Write-ColorText "`n[4/4] 验证安装..." "Cyan"

    $installPaths = @(
        "$env:ProgramFiles\RustDesk",
        "${env:ProgramFiles(x86)}\RustDesk"
    )

    foreach ($path in $installPaths) {
        if (Test-Path $path) {
            $exeFiles = Get-ChildItem -Path $path -Filter "rustdesk*.exe" -ErrorAction SilentlyContinue

            if ($exeFiles) {
                Write-ColorText "   [OK] 安装路径: $path" "Green"

                foreach ($exe in $exeFiles) {
                    $isConfigured = $exe.Name -match "host="
                    if ($isConfigured) {
                        Write-ColorText "   [OK] 已配置Cislink服务器: $($exe.Name)" "Green"
                    } else {
                        Write-ColorText "   [提示] 标准文件名: $($exe.Name)" "Yellow"
                    }
                }

                # 检查配置文件
                $configFile = "$env:APPDATA\RustDesk\config\RustDesk.toml"
                if (Test-Path $configFile) {
                    $configContent = Get-Content $configFile -Raw
                    if ($configContent -match "hbbs.cislink.nl") {
                        Write-ColorText "   [OK] 配置文件已设置Cislink服务器" "Green"
                    }
                }

                return $path
            }
        }
    }

    Write-ColorText "   [警告] 无法验证安装" "Yellow"
    return $null
}

# 启动RustDesk
function Start-RustDesk {
    param([string]$InstallPath)

    if ($NoAutoStart) {
        Write-ColorText "`n[跳过] 不自动启动RustDesk" "Gray"
        return
    }

    Write-ColorText "`n[启动] 正在启动RustDesk..." "Cyan"

    $exeFile = Get-ChildItem -Path $InstallPath -Filter "rustdesk*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1

    if ($exeFile) {
        try {
            Start-Process -FilePath $exeFile.FullName
            Write-ColorText "   [OK] RustDesk已启动" "Green"
        } catch {
            Write-ColorText "   [警告] 无法自动启动，请手动运行" "Yellow"
        }
    }
}

# ===================================================
# 主程序
# ===================================================

Clear-Host
Write-ColorText "========================================" "Cyan"
Write-ColorText "  Cislink RustDesk 一键安装" "Cyan"
Write-ColorText "========================================" "Cyan"

# 显示配置信息
Write-ColorText "`n目标服务器配置:" "Yellow"
Write-ColorText "  ID服务器:    hbbs.cislink.nl" "White"
Write-ColorText "  中继服务器:  hbbr.cislink.nl" "White"
Write-ColorText "  公钥:        VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=" "Gray"

# 请求管理员权限
Request-AdminPrivileges

Write-ColorText "`n[OK] 已获得管理员权限" "Green"

# 执行安装流程
$installer = Find-Installer
if (-not $installer) {
    Write-ColorText "`n安装失败：未找到安装包" "Red"
    pause
    exit 1
}

Stop-RustDeskProcesses

$success = Start-Installation -InstallerPath $installer
if (-not $success) {
    Write-ColorText "`n安装失败：请查看错误信息" "Red"
    pause
    exit 1
}

$installPath = Test-Installation

if ($installPath) {
    Start-RustDesk -InstallPath $installPath

    # 显示完成信息
    Write-ColorText "`n========================================" "Cyan"
    Write-ColorText "  安装完成！" "Green"
    Write-ColorText "========================================" "Cyan"

    Write-ColorText "`n下一步：" "Yellow"
    Write-ColorText "  1. 启动RustDesk（如果未自动启动）" "White"
    Write-ColorText "  2. 打开 设置 -> 网络" "White"
    Write-ColorText "  3. 确认服务器: hbbs.cislink.nl" "White"

    # 查找安装日志
    $logFiles = Get-ChildItem -Path $env:TEMP -Filter "Setup Log*.txt" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($logFiles) {
        Write-ColorText "`n安装日志: $($logFiles.FullName)" "Gray"
    }

} else {
    Write-ColorText "`n[警告] 安装可能未完成，请手动检查" "Yellow"
}

Write-ColorText "`n按任意键退出..." "Gray"
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
