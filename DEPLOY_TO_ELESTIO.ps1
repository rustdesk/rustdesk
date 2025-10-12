# Elestio 部署助手 - 从本地上传并部署
# 此脚本会将 Docker 配置文件上传到 Elestio 服务器并执行部署

param(
    [Parameter(Mandatory=$true)]
    [string]$ElestioHost,  # 例如: vm2.cislink.nl
    
    [Parameter(Mandatory=$true)]
    [int]$SSHPort,  # 例如: 52914
    
    [string]$Username = "root"
)

$ErrorActionPreference = "Stop"

Write-Host "=========================================="
Write-Host "  Elestio RustDesk Docker 部署助手"
Write-Host "=========================================="
Write-Host ""

# 检查文件
$localFiles = @(
    "D:\Rustdesk\docker-compose.yml",
    "D:\Rustdesk\elestio-docker-deploy.sh"
)

foreach ($file in $localFiles) {
    if (-not (Test-Path $file)) {
        Write-Host "❌ 错误: 未找到文件 $file" -ForegroundColor Red
        exit 1
    }
}

Write-Host "✓ 本地文件检查通过" -ForegroundColor Green
Write-Host ""

# 连接信息
$sshTarget = "$Username@$ElestioHost"
Write-Host "📡 目标服务器: $sshTarget" -ForegroundColor Cyan
Write-Host "📡 SSH 端口: $SSHPort" -ForegroundColor Cyan
Write-Host ""

# 测试 SSH 连接
Write-Host "🔍 测试 SSH 连接..." -ForegroundColor Yellow
$testCommand = "echo 'SSH connection successful'"
try {
    $result = ssh -p $SSHPort $sshTarget $testCommand 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ SSH 连接正常" -ForegroundColor Green
    } else {
        Write-Host "❌ SSH 连接失败" -ForegroundColor Red
        Write-Host $result
        exit 1
    }
} catch {
    Write-Host "❌ 无法连接到服务器: $_" -ForegroundColor Red
    exit 1
}
Write-Host ""

# 创建远程目录
Write-Host "📁 创建远程目录..." -ForegroundColor Yellow
ssh -p $SSHPort $sshTarget "mkdir -p /root/rustdesk"
Write-Host "✓ 远程目录创建完成" -ForegroundColor Green
Write-Host ""

# 上传文件
Write-Host "📤 上传文件到服务器..." -ForegroundColor Yellow
foreach ($file in $localFiles) {
    $fileName = Split-Path $file -Leaf
    Write-Host "  上传: $fileName" -ForegroundColor White
    scp -P $SSHPort $file "${sshTarget}:/root/rustdesk/"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  ❌ 上传失败: $fileName" -ForegroundColor Red
        exit 1
    }
    Write-Host "  ✓ 完成: $fileName" -ForegroundColor Green
}
Write-Host "✓ 所有文件上传完成" -ForegroundColor Green
Write-Host ""

# 赋予执行权限
Write-Host "🔧 设置执行权限..." -ForegroundColor Yellow
ssh -p $SSHPort $sshTarget "chmod +x /root/rustdesk/elestio-docker-deploy.sh"
Write-Host "✓ 权限设置完成" -ForegroundColor Green
Write-Host ""

# 询问是否立即部署
$deploy = Read-Host "是否立即执行部署? (Y/N)"
if ($deploy -eq "Y" -or $deploy -eq "y") {
    Write-Host ""
    Write-Host "=========================================="
    Write-Host "  开始执行远程部署..."
    Write-Host "=========================================="
    Write-Host ""
    
    # 执行部署脚本
    ssh -p $SSHPort $sshTarget "cd /root/rustdesk && ./elestio-docker-deploy.sh"
    
    Write-Host ""
    Write-Host "=========================================="
    Write-Host "  部署执行完成!"
    Write-Host "=========================================="
    Write-Host ""
    Write-Host "📋 请查看上方输出获取 Public Key" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "如果未看到 Public Key,请运行:" -ForegroundColor Yellow
    Write-Host "  ssh -p $SSHPort $sshTarget 'docker logs hbbs 2>&1 | grep Key:'" -ForegroundColor Gray
    Write-Host ""
} else {
    Write-Host ""
    Write-Host "✅ 文件已上传到服务器!" -ForegroundColor Green
    Write-Host ""
    Write-Host "手动部署命令:" -ForegroundColor Cyan
    Write-Host "  ssh -p $SSHPort $sshTarget" -ForegroundColor Gray
    Write-Host "  cd /root/rustdesk" -ForegroundColor Gray
    Write-Host "  ./elestio-docker-deploy.sh" -ForegroundColor Gray
    Write-Host ""
}

Write-Host "=========================================="
