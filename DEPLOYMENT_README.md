# RustDesk 配置部署工具

## 📦 包含文件

1. **Deploy-RustDeskConfig.ps1** - PowerShell 部署脚本
2. **Deploy-RustDeskConfig.bat** - 批处理启动器
3. **Deploy-Config.iss** - Inno Setup 安装脚本
4. **RustDesk_Config_Installer.exe** - 编译后的安装程序 (需要生成)

## 🚀 使用方法

### 方法 1: 使用批处理文件 (推荐快速测试)

1. 右键点击 `Deploy-RustDeskConfig.bat`
2. 选择 "以管理员身份运行"
3. 等待部署完成

### 方法 2: 使用 EXE 安装程序 (推荐批量部署)

1. 双击 `RustDesk_Config_Installer.exe`
2. 按照向导提示操作
3. 自动部署完成

### 方法 3: 直接运行 PowerShell 脚本

```powershell
# 需要管理员权限
PowerShell.exe -ExecutionPolicy Bypass -File "Deploy-RustDeskConfig.ps1" -RestartService
```

## 🔧 编译 EXE 安装程序

### 使用 Inno Setup:

1. 安装 [Inno Setup](https://jrsoftware.org/isdl.php)
2. 打开 `Deploy-Config.iss`
3. 点击 "Build" → "Compile"
4. 生成的 EXE 在 `Output` 文件夹

### 使用命令行:

```powershell
# 假设 Inno Setup 安装在默认位置
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "Deploy-Config.iss"
```

## 📋 部署内容

脚本会自动部署以下配置:

- **服务器地址**: hbbs.cislink.nl
- **中继服务器**: hbbr.cislink.nl  
- **公钥**: VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=

部署位置:
- `%APPDATA%\RustDesk\config\RustDesk.toml`
- `%APPDATA%\RustDesk\config\RustDesk2.toml`
- `%ProgramData%\RustDesk\config\RustDesk.toml`
- `%ProgramData%\RustDesk\config\RustDesk2.toml`
- `%LOCALAPPDATA%\RustDesk\config\RustDesk.toml`
- `%LOCALAPPDATA%\RustDesk\config\RustDesk2.toml`

## 🔒 功能特性

✅ 自动备份现有配置  
✅ 停止运行中的 RustDesk 进程  
✅ 部署到所有可能的配置路径  
✅ 验证部署结果  
✅ 可选重启 RustDesk 服务  
✅ 详细的日志记录  
✅ 管理员权限检查  

## 📝 日志文件

部署日志保存在:
```
%TEMP%\RustDesk_Config_Deploy.log
```

配置备份保存在:
```
%TEMP%\RustDesk_Backup_[时间戳]\
```

## 🔄 参数选项

PowerShell 脚本支持以下参数:

```powershell
# 静默模式 (无输出)
.\Deploy-RustDeskConfig.ps1 -Silent

# 部署后重启服务
.\Deploy-RustDeskConfig.ps1 -RestartService

# 组合使用
.\Deploy-RustDeskConfig.ps1 -Silent -RestartService
```

## 🌐 批量部署

### 通过组策略 (GPO):

1. 将 `RustDesk_Config_Installer.exe` 放到网络共享
2. 创建 GPO 启动脚本
3. 分配给目标计算机

### 通过 PowerShell Remoting:

```powershell
$computers = Get-Content "computers.txt"
foreach ($computer in $computers) {
    Copy-Item "Deploy-RustDeskConfig.ps1" -Destination "\\$computer\C$\Temp\"
    Invoke-Command -ComputerName $computer -ScriptBlock {
        PowerShell.exe -ExecutionPolicy Bypass -File "C:\Temp\Deploy-RustDeskConfig.ps1" -Silent -RestartService
    }
}
```

### 通过 SCCM/Intune:

1. 创建应用程序包
2. 使用安装命令: `RustDesk_Config_Installer.exe /VERYSILENT /NORESTART`
3. 部署到目标设备集合

## ⚠️ 注意事项

- ✅ 需要管理员权限
- ✅ 会自动停止 RustDesk 进程
- ✅ 会备份原有配置
- ⚠️ 确保服务器地址和 Key 正确
- ⚠️ 建议先在测试机器上验证

## 🆘 故障排除

### 问题: "执行策略不允许"
**解决**: 以管理员身份运行或使用 `.bat` 文件

### 问题: "访问被拒绝"
**解决**: 确保以管理员身份运行

### 问题: "Key 验证失败"
**解决**: 检查服务器的 public key 是否正确

## 📞 支持

如有问题,请检查日志文件或联系技术支持。

---

**版本**: 1.0  
**更新日期**: 2025-10-12  
**服务器**: hbbs.cislink.nl  
