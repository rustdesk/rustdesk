# RustDesk macOS 安装程序 - Cislink 定制版

本目录包含用于构建 macOS 版本 Cislink RustDesk 安装程序的所有工具和脚本。

## 🎯 核心特性

- ✅ **预配置 Cislink 服务器** - 无需用户手动设置
- ✅ **DMG 安装包** - macOS 标准分发格式
- ✅ **一键构建** - 自动化构建流程
- ✅ **代码签名支持** - 可选的应用签名
- ✅ **完整文档** - 详细的构建和使用指南

## 🚀 快速开始

### 前提条件

在 macOS 上需要安装：
- Xcode Command Line Tools
- Rust
- Flutter
- Homebrew (用于安装 create-dmg)

### 一键构建

```bash
chmod +x 一键构建Cislink版RustDesk-macOS.sh
./一键构建Cislink版RustDesk-macOS.sh
```

构建完成后，您将获得：
```
RustDesk-Cislink-Installer-<版本>.dmg
```

## 📁 文件说明

| 文件 | 说明 |
|------|------|
| `build-macos-installer.sh` | 主构建脚本（支持签名等高级选项） |
| `一键构建Cislink版RustDesk-macOS.sh` | 一键构建脚本（推荐使用） |
| `MACOS_BUILD_INSTRUCTIONS.md` | 完整构建指南和故障排除 |
| `MACOS_QUICK_REFERENCE.md` | 快速参考手册 |
| `RustDesk_Config_Template.toml` | Cislink 服务器配置文件 |

## 🔧 配置

服务器配置在 `RustDesk_Config_Template.toml` 中：

```toml
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
disable-update-check = true
disable-installation = true
```

此配置会自动注入到构建的应用中。

## 📦 分发

### 基本分发（未签名）

适用于：
- 内部测试
- 小范围部署
- 可信用户

用户首次运行时需要：右键 → 打开

### 专业分发（已签名+公证）

适用于：
- 大规模部署
- 通过互联网分发
- 专业发布

需要：
- Apple Developer Program 账号
- 代码签名证书
- 公证（notarization）

详见：[MACOS_BUILD_INSTRUCTIONS.md](MACOS_BUILD_INSTRUCTIONS.md#代码签名和公证)

## 🎓 使用流程

### 1. 开发者：构建安装包

```bash
# 克隆/更新代码
git pull

# 确认配置
cat RustDesk_Config_Template.toml

# 构建
./一键构建Cislink版RustDesk-macOS.sh
```

### 2. 分发给用户

将生成的 `.dmg` 文件分发给 Mac 用户

### 3. 用户：安装应用

1. 双击打开 DMG 文件
2. 拖拽 RustDesk.app 到 Applications 文件夹
3. 从 Applications 启动应用
4. 首次运行时授予必要权限

## 🔍 验证安装

用户安装后，RustDesk 将自动连接到 Cislink 服务器：
- ID/中继服务器：`hbbs.cislink.nl`
- 中继服务器：`hbbr.cislink.nl`

无需任何额外配置！

## 🆘 常见问题

**Q: 构建失败，提示找不到 Flutter？**
```bash
export PATH="$PATH:$HOME/Development/flutter/bin"
echo 'export PATH="$PATH:$HOME/Development/flutter/bin"' >> ~/.zshrc
```

**Q: 用户安装后无法打开应用？**

首次运行：右键点击 → 打开 → 在警告中选择"打开"

或者使用签名版本避免此问题。

**Q: 如何进行代码签名？**

```bash
# 查看可用证书
security find-identity -v -p codesigning

# 使用证书构建
./build-macos-installer.sh --sign "Developer ID Application: Your Name (TEAM_ID)"
```

**Q: 如何更新服务器配置？**

编辑 `RustDesk_Config_Template.toml`，然后重新构建。

## 📚 深入学习

- **快速参考**: [MACOS_QUICK_REFERENCE.md](MACOS_QUICK_REFERENCE.md)
- **完整指南**: [MACOS_BUILD_INSTRUCTIONS.md](MACOS_BUILD_INSTRUCTIONS.md)
- **项目文档**: [CLAUDE.md](CLAUDE.md)

## 🌟 与 Windows 版本的区别

| 特性 | Windows | macOS |
|------|---------|-------|
| 安装包格式 | EXE (Inno Setup) | DMG |
| 一键脚本 | PowerShell (.ps1) | Bash (.sh) |
| 权限管理 | UAC | Gatekeeper + 系统偏好设置 |
| 签名工具 | SignTool | codesign + notarytool |
| 分发要求 | 可选签名 | 建议签名+公证 |

## 🤝 支持

如有问题：
1. 查看 [MACOS_BUILD_INSTRUCTIONS.md](MACOS_BUILD_INSTRUCTIONS.md) 的故障排除部分
2. 检查 Flutter 和 Rust 环境：`flutter doctor` 和 `rustc --version`
3. 清理重构：`./build-macos-installer.sh --clean-only && flutter clean`

## 📝 开发笔记

- 最低支持 macOS 10.14 (Mojave)
- 支持 Intel 和 Apple Silicon (M1/M2/M3)
- 使用 Flutter 构建 UI
- 配置文件位置：`Contents/Resources/rustdesk.toml`
- Bundle ID：`com.carriez.rustdesk`

---

**Cislink RustDesk - 让远程连接更简单** 🚀
