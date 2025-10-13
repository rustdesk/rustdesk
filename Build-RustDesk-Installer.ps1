# RustDesk 客户端打包脚本
# 此脚本会下载 RustDesk 官方客户端，并创建预配置的安装包

param(
    [string]$RustDeskVersion = "latest",
    [switch]$SkipDownload,
    [switch]$OpenOutput
)

$ErrorActionPreference = "Stop"

# 配置
$ServerAddress = "hbbs.cislink.nl"
$RelayServer = "hbbr.cislink.nl"
$ServerKey = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "RustDesk 客户端打包工具" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan

# 检查 Inno Setup
$InnoSetupPath = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
if (-not (Test-Path $InnoSetupPath)) {
    Write-Host "错误: 未找到 Inno Setup" -ForegroundColor Red
    Write-Host "请从以下地址下载并安装 Inno Setup 6:" -ForegroundColor Yellow
    Write-Host "https://jrsoftware.org/isdl.php" -ForegroundColor Yellow
    exit 1
}

# 检查是否需要下载 RustDesk 客户端
if (-not $SkipDownload) {
    Write-Host "`n下载 RustDesk 客户端..." -ForegroundColor Yellow
    
    $DownloadUrl = if ($RustDeskVersion -eq "latest") {
        "https://github.com/rustdesk/rustdesk/releases/latest/download/rustdesk-1.3.6-x86_64.exe"
    } else {
        "https://github.com/rustdesk/rustdesk/releases/download/$RustDeskVersion/rustdesk-$RustDeskVersion-x86_64.exe"
    }
    
    $ExePath = "rustdesk.exe"
    
    try {
        Write-Host "下载地址: $DownloadUrl" -ForegroundColor Gray
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $ExePath -UseBasicParsing
        Write-Host "✓ 下载完成" -ForegroundColor Green
    }
    catch {
        Write-Host "✗ 下载失败: $_" -ForegroundColor Red
        Write-Host "`n请手动下载 RustDesk 客户端:" -ForegroundColor Yellow
        Write-Host "1. 访问: https://github.com/rustdesk/rustdesk/releases" -ForegroundColor Yellow
        Write-Host "2. 下载 Windows 版本 (rustdesk-xxx-x86_64.exe)" -ForegroundColor Yellow
        Write-Host "3. 重命名为 rustdesk.exe 并放在此目录" -ForegroundColor Yellow
        Write-Host "4. 运行: .\Build-RustDesk-Installer.ps1 -SkipDownload" -ForegroundColor Yellow
        exit 1
    }
}

# 检查 rustdesk.exe 是否存在
if (-not (Test-Path "rustdesk.exe")) {
    Write-Host "错误: 未找到 rustdesk.exe" -ForegroundColor Red
    Write-Host "请确保 rustdesk.exe 在当前目录中" -ForegroundColor Yellow
    exit 1
}

$ExeInfo = Get-Item "rustdesk.exe"
Write-Host "`n✓ 找到 RustDesk 客户端:" -ForegroundColor Green
Write-Host "  文件大小: $([math]::Round($ExeInfo.Length / 1MB, 2)) MB" -ForegroundColor Gray
Write-Host "  修改时间: $($ExeInfo.LastWriteTime)" -ForegroundColor Gray

# 检查图标文件
$IconFile = "res\icon.ico"
if (Test-Path $IconFile) {
    Write-Host "✓ 找到自定义图标: $IconFile" -ForegroundColor Green
} else {
    Write-Host "⚠ 未找到自定义图标，安装包将使用默认图标" -ForegroundColor Yellow
}

# 显示服务器配置
Write-Host "`n服务器配置:" -ForegroundColor Cyan
Write-Host "  ID 服务器: $ServerAddress" -ForegroundColor White
Write-Host "  中继服务器: $RelayServer" -ForegroundColor White
Write-Host "  公钥: $ServerKey" -ForegroundColor White

# 创建配置文件模板
Write-Host "`n创建配置模板..." -ForegroundColor Yellow
$ConfigContent = @"
[options]
custom-rendezvous-server = "$ServerAddress"
relay-server = "$RelayServer"
key = "$ServerKey"
"@

$ConfigContent | Set-Content "RustDesk_Config_Template.toml" -Encoding UTF8
Write-Host "✓ 配置模板已创建" -ForegroundColor Green

# 编译安装包
Write-Host "`n编译安装包..." -ForegroundColor Yellow
$IssFile = "RustDesk-Installer.iss"

if (-not (Test-Path $IssFile)) {
    Write-Host "错误: 未找到 $IssFile" -ForegroundColor Red
    exit 1
}

try {
    $Output = & $InnoSetupPath $IssFile 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ 安装包编译成功!" -ForegroundColor Green
        
        # 查找生成的安装包
        $OutputDir = "Output"
        if (Test-Path $OutputDir) {
            $Installer = Get-ChildItem $OutputDir -Filter "RustDesk_Cislink_Installer*.exe" | Select-Object -First 1
            if ($Installer) {
                Write-Host "`n=========================================" -ForegroundColor Cyan
                Write-Host "安装包已生成:" -ForegroundColor Green
                Write-Host "  位置: $($Installer.FullName)" -ForegroundColor White
                Write-Host "  大小: $([math]::Round($Installer.Length / 1MB, 2)) MB" -ForegroundColor White
                Write-Host "=========================================" -ForegroundColor Cyan
                
                # 复制到根目录方便分发
                $FinalName = "RustDesk_Cislink_Setup.exe"
                Copy-Item $Installer.FullName $FinalName -Force
                Write-Host "`n✓ 已复制到: $FinalName" -ForegroundColor Green
                
                if ($OpenOutput) {
                    explorer.exe /select,$Installer.FullName
                }
                
                # 显示使用说明
                Write-Host "`n使用说明:" -ForegroundColor Cyan
                Write-Host "1. 分发 $FinalName 给用户" -ForegroundColor White
                Write-Host "2. 用户运行安装包即可自动配置服务器设置" -ForegroundColor White
                Write-Host "3. 无需手动配置，开箱即用" -ForegroundColor White
            }
        }
    }
    else {
        Write-Host "✗ 编译失败:" -ForegroundColor Red
        Write-Host $Output -ForegroundColor Red
        exit 1
    }
}
catch {
    Write-Host "✗ 编译出错: $_" -ForegroundColor Red
    exit 1
}

Write-Host "`n完成!" -ForegroundColor Green
