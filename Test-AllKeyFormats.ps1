# RustDesk Key 格式测试脚本

$keys = @(
    # 测试 1: 空 key
    @{Name="无 Key"; Value=""},
    
    # 测试 2: 从 SSH 公钥提取的 32 字节
    @{Name="SSH 转换 (32字节)"; Value="d8x7Mld0wRsaFVvs4rqEraoIAq2upu3jpey/+jSuX0Y="},
    
    # 测试 3: SSH 公钥的完整 base64 部分
    @{Name="SSH 完整 Base64"; Value="AAAAC3NzaC1lZDI1NTE5AAAAIHfMezJXdMEbGhVb7OK6hK2qCAKtrqbt46Xsv/o0rl9G"},
    
    # 测试 4: 原来使用的 key (从其他位置)
    @{Name="原 Key (var/lib)"; Value="wrrkMLBXkBGYVlvErzCFMHabakrxKQCsEX2lIbap5Jo="}
)

$configPath = "$env:APPDATA\RustDesk\config\RustDesk.toml"

Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host "RustDesk Key 格式测试工具" -ForegroundColor Cyan
Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host ""

foreach ($key in $keys) {
    Write-Host "测试: $($key.Name)" -ForegroundColor Yellow
    
    if ($key.Value -eq "") {
        $config = @"
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
"@
    } else {
        $config = @"
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "$($key.Value)"
"@
    }
    
    Set-Content -Path $configPath -Value $config -Force
    Write-Host "  配置已更新" -ForegroundColor Gray
    Write-Host "  Key: $($key.Value)" -ForegroundColor Gray
    Write-Host ""
    Write-Host "  请:" -ForegroundColor Green
    Write-Host "    1. 完全关闭 RustDesk (任务管理器)" -ForegroundColor White
    Write-Host "    2. 重新启动 RustDesk" -ForegroundColor White
    Write-Host "    3. 尝试连接" -ForegroundColor White
    Write-Host ""
    
    $response = Read-Host "  是否成功? (y=成功,n=失败继续下一个,q=退出)"
    
    if ($response -eq 'y') {
        Write-Host ""
        Write-Host "✅ 找到正确的 Key!" -ForegroundColor Green
        Write-Host "   Key: $($key.Value)" -ForegroundColor Green
        Write-Host ""
        exit 0
    } elseif ($response -eq 'q') {
        Write-Host "测试中止" -ForegroundColor Red
        exit 1
    }
    
    Write-Host ""
}

Write-Host "=" * 70 -ForegroundColor Red
Write-Host "所有测试都失败了!" -ForegroundColor Red
Write-Host "=" * 70 -ForegroundColor Red
