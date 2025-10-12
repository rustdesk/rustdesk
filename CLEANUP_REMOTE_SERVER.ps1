# RustDesk Server 清理脚本 - PowerShell 版本
# 用于在 Elestio 服务器上执行清理操作

param(
    [Parameter(Mandatory=$true)]
    [string]$ElestioHost,  # 例如: vm2.cislink.nl
    
    [Parameter(Mandatory=$true)]
    [int]$SSHPort,  # 例如: 52914
    
    [string]$Username = "root",
    
    [switch]$SkipBackup,  # 跳过备份
    [switch]$DeleteData,  # 删除数据
    [switch]$AutoYes     # 自动确认所有操作
)

$ErrorActionPreference = "Stop"

Write-Host "=========================================="
Write-Host "  RustDesk Server 远程清理工具"
Write-Host "=========================================="
Write-Host ""

$sshTarget = "$Username@$ElestioHost"
Write-Host "🎯 目标服务器: $sshTarget" -ForegroundColor Cyan
Write-Host "📡 SSH 端口: $SSHPort" -ForegroundColor Cyan
Write-Host ""

# 测试 SSH 连接
Write-Host "🔍 测试 SSH 连接..." -ForegroundColor Yellow
try {
    $testResult = ssh -p $SSHPort $sshTarget "echo 'OK'" 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ SSH 连接正常" -ForegroundColor Green
    } else {
        Write-Host "❌ SSH 连接失败" -ForegroundColor Red
        Write-Host $testResult
        exit 1
    }
} catch {
    Write-Host "❌ 无法连接到服务器: $_" -ForegroundColor Red
    exit 1
}
Write-Host ""

# 上传清理脚本
Write-Host "📤 上传清理脚本..." -ForegroundColor Yellow
$cleanupScript = "D:\Rustdesk\cleanup-rustdesk-server.sh"
if (-not (Test-Path $cleanupScript)) {
    Write-Host "❌ 错误: 未找到清理脚本" -ForegroundColor Red
    exit 1
}

scp -P $SSHPort $cleanupScript "${sshTarget}:/tmp/cleanup-rustdesk-server.sh"
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ 上传失败" -ForegroundColor Red
    exit 1
}
Write-Host "✓ 清理脚本已上传" -ForegroundColor Green
Write-Host ""

# 设置执行权限
Write-Host "🔧 设置执行权限..." -ForegroundColor Yellow
ssh -p $SSHPort $sshTarget "chmod +x /tmp/cleanup-rustdesk-server.sh"
Write-Host ""

# 显示当前状态
Write-Host "=========================================="
Write-Host "  当前服务器状态"
Write-Host "=========================================="
Write-Host ""

Write-Host "📊 RustDesk 进程:" -ForegroundColor Cyan
ssh -p $SSHPort $sshTarget "ps aux | grep -E 'hbb[sr]' | grep -v grep || echo '  未发现 RustDesk 进程'"
Write-Host ""

Write-Host "📦 Docker 容器:" -ForegroundColor Cyan
ssh -p $SSHPort $sshTarget "docker ps -a 2>/dev/null | grep -E 'hbb[sr]' || echo '  未发现 RustDesk 容器'"
Write-Host ""

Write-Host "🔌 端口占用:" -ForegroundColor Cyan
ssh -p $SSHPort $sshTarget "netstat -tuln 2>/dev/null | grep -E ':2111[5-9]' || echo '  RustDesk 端口未被占用'"
Write-Host ""

# 确认执行
if (-not $AutoYes) {
    Write-Host "=========================================="
    Write-Host "  警告"
    Write-Host "=========================================="
    Write-Host ""
    Write-Host "此操作将:" -ForegroundColor Yellow
    Write-Host "  • 停止所有 RustDesk 进程" -ForegroundColor White
    Write-Host "  • 停止并删除 Docker 容器" -ForegroundColor White
    Write-Host "  • 停止系统服务" -ForegroundColor White
    if (-not $SkipBackup) {
        Write-Host "  • 备份现有数据" -ForegroundColor Green
    }
    if ($DeleteData) {
        Write-Host "  • 删除所有数据和配置" -ForegroundColor Red
    }
    Write-Host ""
    
    $confirm = Read-Host "确认执行清理? (yes/no)"
    if ($confirm -ne "yes") {
        Write-Host "操作已取消" -ForegroundColor Yellow
        exit 0
    }
}

# 执行清理
Write-Host ""
Write-Host "=========================================="
Write-Host "  执行清理..."
Write-Host "=========================================="
Write-Host ""

# 构建清理命令
$cleanupCmd = "/tmp/cleanup-rustdesk-server.sh"

# 使用 expect 或输入重定向来自动应答
$autoAnswers = ""
if ($AutoYes) {
    if ($SkipBackup) {
        $autoAnswers = "n`ny`ny"  # 不备份, 删除数据, 删除镜像
    } else {
        $autoAnswers = "y`ny`ny"  # 备份, 删除数据, 删除镜像
    }
    
    # 执行清理(自动应答)
    $cleanupCmd = "echo '$autoAnswers' | $cleanupCmd"
}

Write-Host "🧹 执行远程清理脚本..." -ForegroundColor Cyan
ssh -p $SSHPort $sshTarget "bash -c `"$cleanupCmd`""

Write-Host ""
Write-Host "=========================================="
Write-Host "  清理完成!"
Write-Host "=========================================="
Write-Host ""

# 验证清理结果
Write-Host "🔍 验证清理结果..." -ForegroundColor Cyan
Write-Host ""

Write-Host "检查进程:" -ForegroundColor Yellow
$processCheck = ssh -p $SSHPort $sshTarget "pgrep -x 'hbbs|hbbr' || echo 'NONE'" 2>&1
if ($processCheck -eq "NONE") {
    Write-Host "  ✓ 无 RustDesk 进程运行" -ForegroundColor Green
} else {
    Write-Host "  ⚠ 仍有进程运行: $processCheck" -ForegroundColor Yellow
}

Write-Host "检查容器:" -ForegroundColor Yellow
$containerCheck = ssh -p $SSHPort $sshTarget "docker ps -aq -f name=hbbs -f name=hbbr 2>/dev/null || echo 'NONE'" 2>&1
if ($containerCheck -eq "NONE" -or [string]::IsNullOrEmpty($containerCheck)) {
    Write-Host "  ✓ 无 RustDesk 容器" -ForegroundColor Green
} else {
    Write-Host "  ⚠ 仍有容器: $containerCheck" -ForegroundColor Yellow
}

Write-Host "检查端口:" -ForegroundColor Yellow
$portCheck = ssh -p $SSHPort $sshTarget "netstat -tuln 2>/dev/null | grep -E ':2111[5-9]' || echo 'NONE'" 2>&1
if ($portCheck -eq "NONE") {
    Write-Host "  ✓ RustDesk 端口已释放" -ForegroundColor Green
} else {
    Write-Host "  ⚠ 端口仍被占用" -ForegroundColor Yellow
    Write-Host $portCheck
}

Write-Host ""
Write-Host "=========================================="
Write-Host "  ✅ 服务器已清理完成!"
Write-Host "=========================================="
Write-Host ""
Write-Host "📋 下一步操作:" -ForegroundColor Cyan
Write-Host "  1. 部署 Docker 版本:" -ForegroundColor White
Write-Host "     .\DEPLOY_TO_ELESTIO.ps1 -ElestioHost $ElestioHost -SSHPort $SSHPort" -ForegroundColor Gray
Write-Host ""
Write-Host "  2. 或手动执行:" -ForegroundColor White
Write-Host "     ssh -p $SSHPort $sshTarget" -ForegroundColor Gray
Write-Host "     cd /root/rustdesk" -ForegroundColor Gray
Write-Host "     ./elestio-docker-deploy.sh" -ForegroundColor Gray
Write-Host ""
