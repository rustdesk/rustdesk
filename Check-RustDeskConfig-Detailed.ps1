# 详细检查 RustDesk 配置
Write-Host "=== 详细检查 RustDesk 配置 ===" -ForegroundColor Cyan

# 检查所有可能的配置文件位置
$configLocations = @(
    "$env:APPDATA\RustDesk\config\RustDesk.toml",
    "$env:APPDATA\RustDesk\config\RustDesk2.toml",
    "$env:ProgramData\RustDesk\config\RustDesk.toml",
    "$env:ProgramData\RustDesk\config\RustDesk2.toml"
)

foreach ($path in $configLocations) {
    if (Test-Path $path) {
        Write-Host "`n📄 找到配置文件: $path" -ForegroundColor Green
        Write-Host "内容:" -ForegroundColor Yellow
        Get-Content $path | ForEach-Object {
            if ($_ -match 'key|server|password') {
                Write-Host $_ -ForegroundColor Cyan
            } else {
                Write-Host $_
            }
        }
        Write-Host ("-" * 80)
    }
}

# 检查客户端密钥文件
$keyLocations = @(
    "$env:APPDATA\RustDesk\config\",
    "$env:ProgramData\RustDesk\config\"
)

Write-Host "`n=== 检查密钥文件 ===" -ForegroundColor Cyan
foreach ($dir in $keyLocations) {
    if (Test-Path $dir) {
        Write-Host "`n📁 目录: $dir" -ForegroundColor Green
        Get-ChildItem $dir -File | Where-Object { $_.Name -match 'key|id_' } | ForEach-Object {
            Write-Host "  - $($_.Name) ($($_.Length) bytes)" -ForegroundColor Yellow
        }
    }
}
