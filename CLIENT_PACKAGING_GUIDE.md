# RustDesk 客户端打包指南

## 📦 概述

此工具用于创建预配置 Cislink 服务器设置的 RustDesk 客户端安装包。安装包会自动配置服务器地址和密钥，用户无需手动设置。

## 🔧 前置要求

### 1. 安装 Inno Setup
下载并安装 Inno Setup 6.x:
- 官网: https://jrsoftware.org/isdl.php
- 默认安装路径: `C:\Program Files (x86)\Inno Setup 6\`

### 2. 获取 RustDesk 客户端
有两种方式：

**方式 A: 自动下载（推荐）**
```powershell
.\Build-RustDesk-Installer.ps1
```

**方式 B: 手动下载**
1. 访问: https://github.com/rustdesk/rustdesk/releases
2. 下载 Windows 版本: `rustdesk-x.x.x-x86_64.exe`
3. 重命名为 `rustdesk.exe`
4. 放在此目录
5. 运行: `.\Build-RustDesk-Installer.ps1 -SkipDownload`

## 🚀 构建安装包

### 基本用法
```powershell
# 自动下载并构建
.\Build-RustDesk-Installer.ps1

# 使用已有的 rustdesk.exe
.\Build-RustDesk-Installer.ps1 -SkipDownload

# 构建完成后打开输出目录
.\Build-RustDesk-Installer.ps1 -OpenOutput
```

### 指定版本
```powershell
# 下载特定版本
.\Build-RustDesk-Installer.ps1 -RustDeskVersion "1.3.6"
```

## 📋 生成的文件

构建成功后会生成：
- `Output/RustDesk_Cislink_Installer_v1.0.exe` - 完整安装包
- `RustDesk_Cislink_Setup.exe` - 复制到根目录的分发版本

## 🔐 当前服务器配置

安装包会自动配置以下设置：

```toml
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
```

## 📝 修改服务器配置

如果需要更改服务器地址或密钥：

### 方式 1: 修改打包脚本
编辑 `Build-RustDesk-Installer.ps1`:
```powershell
$ServerAddress = "your-server.com"
$RelayServer = "your-relay.com"
$ServerKey = "your-public-key-here"
```

### 方式 2: 修改 Inno Setup 脚本
编辑 `RustDesk-Full-Installer.iss` 中的 `CreateConfigFiles()` 函数:
```pascal
ConfigContent := '[options]' + #13#10 +
                 'custom-rendezvous-server = "your-server.com"' + #13#10 +
                 'relay-server = "your-relay.com"' + #13#10 +
                 'key = "your-public-key"' + #13#10;
```

## 📦 安装包功能

### 安装时
1. ✅ 自动停止正在运行的 RustDesk 进程
2. ✅ 安装 RustDesk 客户端到 `C:\Program Files\RustDesk\`
3. ✅ 创建桌面快捷方式（可选）
4. ✅ 添加开机自启动（可选）
5. ✅ 自动配置服务器设置到：
   - `%APPDATA%\RustDesk\config\`
   - `%ProgramData%\RustDesk\config\`
6. ✅ 注册 URL 协议 `rustdesk://`

### 卸载时
- 可选择保留或删除配置文件
- 自动清理注册表项
- 删除快捷方式

## 🎯 用户使用流程

1. **运行安装包**
   - 双击 `RustDesk_Cislink_Setup.exe`
   - 跟随安装向导

2. **选择安装选项**
   - 创建桌面图标（可选）
   - 开机自动启动（默认勾选）

3. **完成安装**
   - 点击"完成"启动 RustDesk
   - 服务器设置已自动配置
   - 无需手动输入任何信息

4. **开始使用**
   - 查看自己的 ID
   - 输入对方 ID 即可连接

## 🔍 配置文件位置

安装包会在以下位置创建配置文件：

### 用户级别
- `%APPDATA%\RustDesk\config\RustDesk.toml`
- `%APPDATA%\RustDesk\config\RustDesk2.toml`

### 系统级别
- `%ProgramData%\RustDesk\config\RustDesk.toml`
- `%ProgramData%\RustDesk\config\RustDesk2.toml`

## 📊 安装包大小

- RustDesk 客户端: ~20 MB
- 配置文件: < 1 KB
- 最终安装包: ~8-10 MB（压缩后）

## 🌍 多语言支持

安装包支持以下语言：
- 英语 (English)
- 荷兰语 (Nederlands)
- 德语 (Deutsch)
- 法语 (Français)
- 简体中文

系统会自动检测并使用适当的语言。

## 🐛 故障排查

### 问题: 找不到 Inno Setup
**解决**: 确保 Inno Setup 已安装到默认位置，或修改脚本中的 `$InnoSetupPath`

### 问题: rustdesk.exe 下载失败
**解决**: 手动下载并使用 `-SkipDownload` 参数

### 问题: 编译失败
**解决**: 
1. 检查 `RustDesk-Full-Installer.iss` 语法
2. 确保 `rustdesk.exe` 存在
3. 查看详细错误信息

### 问题: 安装后无法连接
**解决**:
1. 确认服务器正在运行: `.\Deploy-RustDesk-Docker.ps1 -Status`
2. 检查防火墙设置
3. 验证 DNS 解析: `nslookup hbbs.cislink.nl`

## 📚 相关文档

- [服务器部署指南](DEPLOYMENT_README.md)
- [Docker 部署指南](DOCKER_DEPLOYMENT_GUIDE.md)
- [故障排查指南](TROUBLESHOOTING_GUIDE.md)

## 🔄 更新服务器密钥

如果服务器密钥更改：

1. 获取新的公钥:
   ```powershell
   .\Deploy-RustDesk-Docker.ps1 -GetKey
   ```

2. 更新打包脚本中的 `$ServerKey`

3. 重新构建安装包:
   ```powershell
   .\Build-RustDesk-Installer.ps1 -SkipDownload
   ```

4. 分发新的安装包给用户

## 📢 分发建议

### 内部部署
- 将安装包放在内部文件服务器
- 通过组策略或 SCCM 推送
- 邮件附件发送（如果大小允许）

### 外部分发
- 上传到云存储（如 OneDrive, Google Drive）
- 通过安全的文件传输服务
- 生成下载链接分享

### 版本管理
- 在文件名中包含版本号和日期
- 保留历史版本以便回滚
- 记录每个版本的服务器配置

## ✅ 验证清单

在分发安装包前，请确认：

- [ ] 服务器地址正确 (`hbbs.cislink.nl`)
- [ ] 中继服务器正确 (`hbbr.cislink.nl`)
- [ ] 公钥正确 (`VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=`)
- [ ] 服务器正在运行
- [ ] DNS 解析正常
- [ ] 防火墙端口开放
- [ ] 测试安装包能正常安装
- [ ] 测试连接功能正常

## 📞 支持

如有问题，请参考：
- GitHub Issues: https://github.com/CislinkNL/rustdesk/issues
- RustDesk 官方文档: https://rustdesk.com/docs/
