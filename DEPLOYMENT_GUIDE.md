# 🎉 RustDesk 配置部署工具 - 创建成功!

## ✅ 已生成的文件

### 📦 主要文件:
1. **RustDesk_Config_Installer.exe** (1.9 MB)
   - 位置: `D:\Rustdesk\Output\RustDesk_Config_Installer.exe`
   - 这是用于批量部署的主安装程序
   - 包含自动部署逻辑和验证功能

2. **Deploy-RustDeskConfig.ps1**
   - PowerShell 部署脚本
   - 可独立使用或被 EXE 调用

3. **Deploy-RustDeskConfig.bat**
   - 批处理启动器
   - 用于快速测试

## 🚀 使用方法

### ⭐ 推荐方法: 使用 EXE 安装程序

```
双击运行: D:\Rustdesk\Output\RustDesk_Config_Installer.exe
```

**特点:**
- ✅ 需要管理员权限(会自动提示)
- ✅ 自动停止 RustDesk 进程
- ✅ 备份现有配置
- ✅ 部署到所有配置路径
- ✅ 验证部署结果
- ✅ 可选重启服务
- ✅ 静默安装选项

### 静默安装(用于脚本部署):

```cmd
RustDesk_Config_Installer.exe /VERYSILENT /NORESTART
```

### 快速测试:

右键点击 `Deploy-RustDeskConfig.bat` → "以管理员身份运行"

## 📋 部署的配置信息

**服务器配置:**
- Rendezvous Server: `hbbs.cislink.nl` (142.132.187.134)
- Relay Server: `hbbr.cislink.nl` (142.132.187.134)
- Public Key: `AAAAC3NzaC1lZDI1NTE5AAAAIO08lDeKuMjZRzfSkGQ65QaXptqsBMtvUmbvB8Unhpco`

**部署位置:**
- `%APPDATA%\RustDesk\config\`
- `%ProgramData%\RustDesk\config\`
- `%LOCALAPPDATA%\RustDesk\config\`

**文件名:**
- `RustDesk.toml`
- `RustDesk2.toml`

## 🌐 批量部署方案

### 1. 通过组策略 (GPO)

```powershell
# 1. 将 EXE 复制到网络共享
Copy-Item "D:\Rustdesk\Output\RustDesk_Config_Installer.exe" -Destination "\\server\share\RustDesk\"

# 2. 创建 GPO 启动脚本
# Computer Configuration → Policies → Windows Settings → Scripts → Startup
# 添加: \\server\share\RustDesk\RustDesk_Config_Installer.exe /VERYSILENT
```

### 2. 通过 PowerShell Remoting

```powershell
# 批量部署到多台计算机
$computers = @("PC001", "PC002", "PC003")
$installerPath = "D:\Rustdesk\Output\RustDesk_Config_Installer.exe"

foreach ($computer in $computers) {
    Write-Host "部署到 $computer..." -ForegroundColor Cyan
    
    # 复制安装程序
    Copy-Item $installerPath -Destination "\\$computer\C$\Temp\" -Force
    
    # 远程执行
    Invoke-Command -ComputerName $computer -ScriptBlock {
        Start-Process "C:\Temp\RustDesk_Config_Installer.exe" -ArgumentList "/VERYSILENT" -Wait
    }
    
    Write-Host "✓ $computer 完成" -ForegroundColor Green
}
```

### 3. 通过 Intune/SCCM

**Intune:**
1. 上传 `RustDesk_Config_Installer.exe` 作为 Win32 应用
2. 安装命令: `RustDesk_Config_Installer.exe /VERYSILENT /NORESTART`
3. 卸载命令: (留空,不需要卸载)
4. 检测规则: 检查文件 `%ProgramData%\RustDesk\config\RustDesk.toml` 是否包含正确的 key

**SCCM:**
1. 创建应用程序
2. 部署类型: Windows Installer
3. 安装程序: `RustDesk_Config_Installer.exe /VERYSILENT`
4. 检测方法: 文件系统检测

### 4. 通过 PsExec (传统方法)

```cmd
psexec \\PC001,PC002,PC003 -c RustDesk_Config_Installer.exe /VERYSILENT
```

## 🔍 验证部署

### 方法 1: 检查配置文件

```powershell
# 在目标机器上运行
Get-Content "$env:APPDATA\RustDesk\config\RustDesk.toml"
```

应该看到:
```toml
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
```

### 方法 2: 批量验证脚本

```powershell
$computers = @("PC001", "PC002", "PC003")
foreach ($computer in $computers) {
    $configPath = "\\$computer\C$\Users\*\AppData\Roaming\RustDesk\config\RustDesk.toml"
    $configs = Get-ChildItem $configPath -ErrorAction SilentlyContinue
    
    foreach ($config in $configs) {
        $content = Get-Content $config.FullName -Raw
        if ($content -match "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=") {
            Write-Host "✓ $computer - 配置正确" -ForegroundColor Green
        } else {
            Write-Host "✗ $computer - 配置有误" -ForegroundColor Red
        }
    }
}
```

## 📝 日志文件位置

**部署日志:**
```
%TEMP%\RustDesk_Config_Deploy.log
```

**配置备份:**
```
%TEMP%\RustDesk_Backup_[时间戳]\
```

## ⚠️ 故障排除

### 问题 1: "需要管理员权限"
**解决:** 右键点击 → "以管理员身份运行"

### 问题 2: "配置未生效"
**解决:** 
1. 检查 RustDesk 进程是否已重启
2. 手动停止所有 RustDesk 进程
3. 重新运行部署工具

### 问题 3: "Key 验证失败"
**解决:**
1. 在服务器上重新获取 public key
2. 确认 key 为: `VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=`
3. 如果 key 不同,需要重新生成安装程序

## 🔄 更新配置

如果服务器 key 更改了:

1. 编辑 `Deploy-RustDeskConfig.ps1` 中的 key
2. 重新编译:
   ```cmd
   "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "Deploy-Config.iss"
   ```
3. 使用新的 EXE 重新部署

## 📊 部署统计

创建一个部署报告:

```powershell
$report = @()
$computers = Get-Content "computers.txt"

foreach ($computer in $computers) {
    $result = [PSCustomObject]@{
        Computer = $computer
        Status = "Unknown"
        ConfigExists = $false
        KeyCorrect = $false
        LastModified = $null
    }
    
    try {
        $configPath = "\\$computer\C$\ProgramData\RustDesk\config\RustDesk.toml"
        if (Test-Path $configPath) {
            $result.ConfigExists = $true
            $content = Get-Content $configPath -Raw
            $result.KeyCorrect = $content -match "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
            $result.LastModified = (Get-Item $configPath).LastWriteTime
            $result.Status = if ($result.KeyCorrect) { "Success" } else { "Wrong Key" }
        } else {
            $result.Status = "Not Deployed"
        }
    } catch {
        $result.Status = "Error: $_"
    }
    
    $report += $result
}

$report | Export-Csv "RustDesk_Deployment_Report.csv" -NoTypeInformation
$report | Format-Table -AutoSize
```

## ✅ 测试清单

在大规模部署前,请完成以下测试:

- [ ] 在测试机器上手动运行 EXE
- [ ] 验证配置文件已创建
- [ ] 验证 RustDesk 客户端可以连接服务器
- [ ] 测试远程控制功能
- [ ] 验证备份功能正常
- [ ] 测试静默安装模式
- [ ] 检查日志文件生成

## 📞 技术支持

- 配置文件位置: `D:\Rustdesk\`
- 安装程序: `D:\Rustdesk\Output\RustDesk_Config_Installer.exe`
- 文档: `D:\Rustdesk\DEPLOYMENT_README.md`

---

**版本**: 1.0  
**创建日期**: 2025-10-12  
**文件大小**: 1.9 MB  
**服务器**: hbbs.cislink.nl  
**Key**: VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=
