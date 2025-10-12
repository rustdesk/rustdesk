# RustDesk 服务器连接诊断脚本

Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host "RustDesk Server Connection Diagnostics" -ForegroundColor Cyan
Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host ""

$server = "hbbs.cislink.nl"
$relay = "hbbr.cislink.nl"
$hbbsPort = 21116
$hbbrPort = 21117

# 测试 DNS 解析
Write-Host "1. DNS 解析测试..." -ForegroundColor Yellow
try {
    $serverIP = [System.Net.Dns]::GetHostAddresses($server) | Select-Object -First 1
    Write-Host "   ✓ $server -> $($serverIP.IPAddressToString)" -ForegroundColor Green
    
    $relayIP = [System.Net.Dns]::GetHostAddresses($relay) | Select-Object -First 1
    Write-Host "   ✓ $relay -> $($relayIP.IPAddressToString)" -ForegroundColor Green
} catch {
    Write-Host "   ✗ DNS 解析失败: $_" -ForegroundColor Red
}

Write-Host ""

# 测试端口连接
Write-Host "2. 端口连接测试..." -ForegroundColor Yellow

# 测试 hbbs 端口 21116
try {
    $tcpClient = New-Object System.Net.Sockets.TcpClient
    $tcpClient.ReceiveTimeout = 3000
    $tcpClient.SendTimeout = 3000
    $connection = $tcpClient.BeginConnect($server, $hbbsPort, $null, $null)
    $wait = $connection.AsyncWaitHandle.WaitOne(3000, $false)
    
    if ($wait) {
        $tcpClient.EndConnect($connection)
        Write-Host "   ✓ ${server}:${hbbsPort} (hbbs) - 可达" -ForegroundColor Green
        $tcpClient.Close()
    } else {
        Write-Host "   ✗ ${server}:${hbbsPort} (hbbs) - 超时" -ForegroundColor Red
    }
} catch {
    Write-Host "   ✗ ${server}:${hbbsPort} (hbbs) - 无法连接: $_" -ForegroundColor Red
}

# 测试 hbbr 端口 21117
try {
    $tcpClient = New-Object System.Net.Sockets.TcpClient
    $tcpClient.ReceiveTimeout = 3000
    $tcpClient.SendTimeout = 3000
    $connection = $tcpClient.BeginConnect($relay, $hbbrPort, $null, $null)
    $wait = $connection.AsyncWaitHandle.WaitOne(3000, $false)
    
    if ($wait) {
        $tcpClient.EndConnect($connection)
        Write-Host "   ✓ ${relay}:${hbbrPort} (hbbr) - 可达" -ForegroundColor Green
        $tcpClient.Close()
    } else {
        Write-Host "   ✗ ${relay}:${hbbrPort} (hbbr) - 超时" -ForegroundColor Red
    }
} catch {
    Write-Host "   ✗ ${relay}:${hbbrPort} (hbbr) - 无法连接: $_" -ForegroundColor Red
}

Write-Host ""

# 检查配置文件
Write-Host "3. 配置文件检查..." -ForegroundColor Yellow
$configPath = "$env:APPDATA\RustDesk\config\RustDesk.toml"

if (Test-Path $configPath) {
    $content = Get-Content $configPath -Raw
    
    # 检查 [options] 部分
    if ($content -match '\[options\]') {
        Write-Host "   ✓ 找到 [options] 部分" -ForegroundColor Green
        
        # 提取配置值
        if ($content -match 'custom-rendezvous-server\s*=\s*"([^"]+)"') {
            $configServer = $matches[1]
            if ($configServer -eq $server) {
                Write-Host "   ✓ Server: $configServer" -ForegroundColor Green
            } else {
                Write-Host "   ✗ Server 不匹配: $configServer (期望: $server)" -ForegroundColor Red
            }
        }
        
        if ($content -match 'relay-server\s*=\s*"([^"]+)"') {
            $configRelay = $matches[1]
            if ($configRelay -eq $relay) {
                Write-Host "   ✓ Relay: $configRelay" -ForegroundColor Green
            } else {
                Write-Host "   ✗ Relay 不匹配: $configRelay (期望: $relay)" -ForegroundColor Red
            }
        }
        
        if ($content -match 'key\s*=\s*"([^"]+)"') {
            $configKey = $matches[1]
            Write-Host "   ✓ Key: $configKey" -ForegroundColor Green
        } else {
            Write-Host "   ✗ 未找到 Key 配置" -ForegroundColor Red
        }
    } else {
        Write-Host "   ✗ 未找到 [options] 部分" -ForegroundColor Red
    }
} else {
    Write-Host "   ✗ 配置文件不存在: $configPath" -ForegroundColor Red
}

Write-Host ""
Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host ""

# 显示需要在服务器上执行的命令
Write-Host "请在 Elestio 服务器上执行以下命令来获取正确的 Public Key:" -ForegroundColor Yellow
Write-Host ""
Write-Host "cat /var/lib/rustdesk-server/id_ed25519.pub" -ForegroundColor Cyan
Write-Host ""
Write-Host "然后将输出的 key 告诉我,我会更新配置。" -ForegroundColor Yellow
Write-Host ""
Write-Host "按任意键退出..." -ForegroundColor Gray
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
