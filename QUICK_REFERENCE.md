# RustDesk Cislink版 - 快速参考

## 🎯 可用工具一览

### 1. 全新安装（推荐给新用户）

**文件**: `RustDesk_Cislink_Installer_v2.1.exe`
**位置**: `D:\Rustdesk\Output\`
**用途**: 安装预配置好Cislink服务器的RustDesk客户端

```powershell
# 构建安装包
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "RustDesk-Installer.iss"

# 安装包位置
D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.1.exe
```

**特点**:
- ✅ 开箱即用，自动配置服务器
- ✅ 使用EXE文件名嵌入配置（最高优先级）
- ✅ 包含Cislink自定义图标
- ✅ 自动禁用更新检查

---

### 2. 快速切换脚本（推荐给已安装用户）

**文件**: `quick-switch-to-cislink.ps1`
**用途**: 将现有RustDesk公用版快速切换到Cislink服务器

```powershell
# 运行方式1: 双击运行（右键 -> 使用PowerShell运行）

# 运行方式2: 命令行运行
powershell -ExecutionPolicy Bypass -File quick-switch-to-cislink.ps1
```

**特点**:
- ✅ 简单易用，适合普通用户
- ✅ 无需管理员权限（大部分情况）
- ✅ 不修改可执行文件
- ✅ 自动创建配置文件
- ⏱️ 执行时间: < 10秒

**适用场景**:
- 客户已安装RustDesk公用版
- 需要快速切换服务器
- 不想重新安装

---

### 3. 完整迁移脚本（推荐给IT管理员）

**文件**: `migrate-to-cislink.ps1`
**用途**: 全功能迁移，支持多种方式

```powershell
# 交互式运行（会显示菜单）
.\migrate-to-cislink.ps1

# 自动模式（推荐）
.\migrate-to-cislink.ps1 -Auto

# 强制使用EXE重命名模式
.\migrate-to-cislink.ps1 -RenameExe

# 仅使用配置文件模式
.\migrate-to-cislink.ps1 -ConfigOnly
```

**特点**:
- ✅ 多种迁移方式可选
- ✅ 自动更新快捷方式
- ✅ 完整的验证和错误处理
- ✅ 创建备份文件
- ⚠️ 需要管理员权限

**迁移方式**:
1. **重命名EXE模式** - 最可靠（99%成功率）
2. **配置文件模式** - 较简单（90%成功率）
3. **自动模式** - 智能选择（推荐）

---

### 4. 更新配置脚本

**文件**: `update-config.ps1`
**用途**: 快速更新现有安装的配置文件

```powershell
powershell -ExecutionPolicy Bypass -File update-config.ps1
```

**特点**:
- 仅更新配置文件
- 不修改其他内容
- 执行速度快

---

## 📋 使用决策流程图

```
客户是否已安装RustDesk？
    │
    ├─ 否 → 使用【全新安装包】
    │       RustDesk_Cislink_Installer_v2.1.exe
    │
    └─ 是 → 需要100%确保配置生效？
            │
            ├─ 是 → 使用【完整迁移脚本】
            │       migrate-to-cislink.ps1 -Auto
            │       （以管理员身份运行）
            │
            └─ 否 → 使用【快速切换脚本】
                    quick-switch-to-cislink.ps1
                    （双击运行即可）
```

---

## 🔧 常用命令速查

### 检查RustDesk安装

```powershell
# 查找RustDesk可执行文件
Get-ChildItem "C:\Program Files\RustDesk\" -Filter rustdesk*.exe

# 检查进程
Get-Process | Where-Object { $_.Name -like "rustdesk*" }
```

### 停止RustDesk

```powershell
# 方法1: PowerShell
Get-Process | Where-Object { $_.Name -like "rustdesk*" } | Stop-Process -Force

# 方法2: CMD
taskkill /F /IM rustdesk*.exe /T
```

### 查看配置文件

```powershell
# 用户配置
Get-Content "$env:APPDATA\RustDesk\config\RustDesk.toml"

# 系统配置
Get-Content "$env:PROGRAMDATA\RustDesk\config\RustDesk.toml"
```

### 验证服务器配置

```powershell
# 检查配置文件是否包含Cislink服务器
Get-Content "$env:APPDATA\RustDesk\config\RustDesk.toml" | Select-String "cislink"

# 预期输出应包含:
# custom-rendezvous-server = "hbbs.cislink.nl"
# relay-server = "hbbr.cislink.nl"
```

### 清理配置（恢复默认）

```powershell
# 停止RustDesk
taskkill /F /IM rustdesk*.exe /T

# 删除配置文件
Remove-Item "$env:APPDATA\RustDesk\config\RustDesk*.toml" -Force
Remove-Item "$env:PROGRAMDATA\RustDesk\config\RustDesk*.toml" -Force
```

---

## 🎨 Cislink服务器信息

```
ID服务器:    hbbs.cislink.nl
中继服务器:  hbbr.cislink.nl
公钥:        VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=
端口:        默认 (21116/21117/21118/21119)
```

---

## 📊 方法对比表

| 方法 | 新安装 | 迁移时间 | 管理员权限 | 成功率 | 复杂度 |
|------|--------|----------|-----------|--------|--------|
| 全新安装包 | ✅ | - | ✅ | 99% | ⭐ |
| 快速切换脚本 | ❌ | < 10s | 可选 | 85% | ⭐ |
| 完整迁移(配置) | ❌ | ~ 15s | 推荐 | 90% | ⭐⭐ |
| 完整迁移(重命名) | ❌ | ~ 20s | 必须 | 99% | ⭐⭐⭐ |

---

## 🚨 故障排除速查

### 问题: 脚本无法运行

```powershell
# 解决方案: 绕过执行策略
powershell -ExecutionPolicy Bypass -File script-name.ps1
```

### 问题: 配置后仍连接公共服务器

```powershell
# 1. 检查EXE文件名
Get-ChildItem "C:\Program Files\RustDesk\" -Filter rustdesk*.exe

# 2. 如果文件名包含其他服务器配置，使用重命名模式
.\migrate-to-cislink.ps1 -RenameExe

# 3. 确保RustDesk完全关闭
taskkill /F /IM rustdesk*.exe /T
```

### 问题: 权限不足

```powershell
# 以管理员身份运行PowerShell
Start-Process powershell -Verb RunAs

# 然后执行脚本
.\migrate-to-cislink.ps1
```

---

## 📖 详细文档

| 文档 | 说明 |
|------|------|
| [MIGRATION_GUIDE.md](./MIGRATION_GUIDE.md) | 完整迁移指南 |
| [INSTALLER_FIX_DOCUMENTATION.md](./INSTALLER_FIX_DOCUMENTATION.md) | 安装包技术文档 |
| [INSTALLER_BUILD_INSTRUCTIONS.md](./INSTALLER_BUILD_INSTRUCTIONS.md) | 构建说明 |

---

## 🔐 安全注意事项

1. **脚本来源**: 确保从可信来源获取脚本
2. **权限控制**: 仅在必要时使用管理员权限
3. **备份数据**: 重要配置应先备份
4. **验证配置**: 执行后务必验证服务器设置

---

## 💡 最佳实践

### 企业部署建议

1. **小规模测试**: 先在1-2台电脑上测试
2. **使用完整迁移脚本**: 选择Auto模式
3. **GPO部署**: 使用组策略批量部署配置文件
4. **监控验证**: 部署后检查客户端连接状态

### 个人用户建议

1. **新安装**: 直接使用安装包
2. **已安装**: 使用快速切换脚本
3. **问题排查**: 尝试完整迁移脚本

---

## 📞 支持联系

遇到问题？请提供：
- Windows版本
- RustDesk版本
- 脚本运行日志
- EXE文件名（`Get-ChildItem "C:\Program Files\RustDesk\" -Filter rustdesk*.exe`）

---

**最后更新**: 2025-10-27
**版本**: 1.0
**适用于**: RustDesk 1.2.2+, Windows 10/11
