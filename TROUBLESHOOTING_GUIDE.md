# RustDesk 配置疑难解答指南

## 问题症状
- 客户端显示 "连接错误 - Key 不匹配"
- 配置文件中已包含正确的服务器地址和 Key
- 网络连接测试正常

## 可能的原因

### 1. 服务器 Public Key 已更改
**原因**: 
- 服务器重启/重新部署
- 手动删除了 key 文件
- Elestio 更新了配置

**解决方案**:
```bash
# 在服务器上执行
cat /var/lib/rustdesk-server/id_ed25519.pub
```

### 2. Key 格式问题
RustDesk 的 Public Key 应该是:
- 纯 Base64 字符串
- 约 44 字符长度
- 以 = 结尾
- 示例: `wrrkMLBXkBGYVlvErzCFMHabakrxKQCsEX2lIbap5Jo=`

**不应该是**:
- SSH 格式: `ssh-ed25519 AAAAC3Nza...`
- 包含空格或换行
- 包含注释

### 3. 配置文件位置问题
RustDesk 可能从不同位置读取配置:

**优先级顺序**:
1. `%APPDATA%\RustDesk\config\RustDesk.toml` (用户配置)
2. `%ProgramData%\RustDesk\config\RustDesk.toml` (系统配置)
3. 程序目录下的配置

### 4. 配置未生效
**可能原因**:
- RustDesk 进程未重启
- 缓存了旧配置
- 读取了其他位置的配置

**解决方案**:
1. 完全关闭 RustDesk (任务管理器结束所有进程)
2. 删除所有位置的配置文件
3. 重新启动 RustDesk
4. 重新配置服务器

### 5. 服务器端配置问题
**检查项目**:
```bash
# 1. 检查 hbbs 服务状态
docker ps | grep hbbs
systemctl status rustdesk-hbbs

# 2. 检查服务器日志
docker logs rustdesk-hbbs
journalctl -u rustdesk-hbbs -n 50

# 3. 验证 key 文件存在
ls -la /var/lib/rustdesk-server/id_ed25519*

# 4. 查看 key 内容
cat /var/lib/rustdesk-server/id_ed25519.pub
```

## 测试步骤

### Step 1: 获取最新的服务器 Public Key
```bash
# SSH 到服务器
ssh root@your-server

# 获取 key
cat /var/lib/rustdesk-server/id_ed25519.pub
```

### Step 2: 完全清理客户端配置
```powershell
# 停止所有 RustDesk 进程
Get-Process | Where-Object {$_.ProcessName -like "*rustdesk*"} | Stop-Process -Force

# 备份并删除配置
Remove-Item "$env:APPDATA\RustDesk\config\RustDesk*.toml" -Force
Remove-Item "$env:ProgramData\RustDesk\config\RustDesk*.toml" -Force -ErrorAction SilentlyContinue
Remove-Item "$env:LOCALAPPDATA\RustDesk\config\RustDesk*.toml" -Force -ErrorAction SilentlyContinue
```

### Step 3: 手动创建新配置
```powershell
# 创建目录
$configPath = "$env:APPDATA\RustDesk\config"
New-Item -ItemType Directory -Path $configPath -Force

# 创建配置文件 (替换 YOUR_KEY_HERE)
@"
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "YOUR_KEY_HERE"
"@ | Out-File -FilePath "$configPath\RustDesk.toml" -Encoding UTF8
```

### Step 4: 重启 RustDesk 并测试
1. 启动 RustDesk
2. 尝试连接
3. 检查是否还有 Key 错误

## 替代方案: 使用 RustDesk UI 配置

1. 打开 RustDesk
2. 点击设置 (齿轮图标)
3. 找到 "Network" 或"网络"选项卡
4. 手动输入:
   - ID Server: `hbbs.cislink.nl`
   - Relay Server: `hbbr.cislink.nl`
   - Key: `[从服务器获取的 key]`
5. 点击应用/确定
6. 重启 RustDesk

## 高级排查

### 查看 RustDesk 客户端日志
```powershell
# 日志位置
Get-ChildItem "$env:APPDATA\RustDesk" -Recurse -Filter "*.log" | 
    Sort-Object LastWriteTime -Descending | 
    Select-Object -First 1 | 
    Get-Content -Tail 50
```

### 抓包分析
如果上述方法都不行,可以使用 Wireshark 抓包:
1. 启动 Wireshark
2. 过滤器: `tcp.port == 21116 or tcp.port == 21117`
3. 尝试连接
4. 查看是否有连接建立和数据交换

## 紧急联系信息

如果问题持续,请提供:
1. 服务器 public key (完整输出)
2. 客户端配置文件内容
3. 服务器日志 (最近 50 行)
4. 网络连接测试结果
5. RustDesk 客户端版本号

---

**下一步行动**:
请在服务器上执行 `cat /var/lib/rustdesk-server/id_ed25519.pub` 
并将完整输出提供给我。
