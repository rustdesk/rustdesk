# macOS 快速参考 - Cislink RustDesk 构建

## 🚀 快速开始（推荐）

### 一键构建

```bash
chmod +x 一键构建Cislink版RustDesk-macOS.sh
./一键构建Cislink版RustDesk-macOS.sh
```

这将：
✅ 自动检查所有依赖
✅ 使用 Cislink 服务器配置
✅ 构建并打包 DMG 安装程序
✅ 显示友好的进度提示

---

## 📋 前置要求

### 必需工具

```bash
# 1. Xcode Command Line Tools
xcode-select --install

# 2. Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 3. Flutter（添加到PATH后）
flutter doctor

# 4. create-dmg
brew install create-dmg
```

### 系统要求
- macOS 10.14+ (Mojave or later)
- 5GB+ 磁盘空间
- 8GB+ 内存（推荐）

---

## 🔧 配置文件

**位置**: `RustDesk_Config_Template.toml`

```toml
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
disable-update-check = true
disable-installation = true
```

**重要**: 此配置将自动注入到应用中

---

## 📦 构建命令

### 基础构建（最常用）

```bash
./一键构建Cislink版RustDesk-macOS.sh
```

### 手动构建

```bash
# 基本构建
./build-macos-installer.sh

# 带签名构建（需要Apple开发者账号）
./build-macos-installer.sh --sign "Developer ID Application: Your Name (TEAM_ID)"

# 仅清理构建文件
./build-macos-installer.sh --clean-only
```

---

## 📂 输出文件

构建完成后生成：

```
RustDesk-Cislink-Installer-<版本号>.dmg
```

**位置**: 项目根目录
**用途**: 分发给 Mac 用户安装

---

## 🧪 测试安装

```bash
# 1. 打开 DMG
open RustDesk-Cislink-Installer-*.dmg

# 2. 在 Finder 中：
#    拖拽 RustDesk.app → Applications 文件夹

# 3. 首次运行：
#    右键点击 RustDesk.app → 打开
#    （或在系统偏好设置 → 安全性中允许）
```

---

## 🔐 代码签名（可选）

### 查看可用证书

```bash
security find-identity -v -p codesigning
```

### 签名构建

```bash
./build-macos-installer.sh --sign "Developer ID Application: Your Name (ABC123)"
```

### 验证签名

```bash
codesign -vv --deep --strict RustDesk.app
```

---

## ⚡ 常用操作

### 完全清理重构

```bash
./build-macos-installer.sh --clean-only
rm -rf flutter/build
flutter clean
./一键构建Cislink版RustDesk-macOS.sh
```

### 检查应用配置

```bash
# 查看应用信息
plutil -p flutter/build/macos/Build/Products/Release/RustDesk.app/Contents/Info.plist

# 验证配置文件是否注入
cat flutter/build/macos/Build/Products/Release/RustDesk.app/Contents/Resources/rustdesk.toml
```

### 调试运行

```bash
# 从终端启动查看日志
./flutter/build/macos/Build/Products/Release/RustDesk.app/Contents/MacOS/RustDesk
```

---

## ❌ 常见问题速查

| 问题 | 解决方案 |
|------|---------|
| 找不到 Flutter | `export PATH="$PATH:$HOME/Development/flutter/bin"` |
| DMG 创建失败 | `brew install create-dmg` |
| 应用无法打开 | 右键 → 打开（首次运行）或进行代码签名 |
| 构建错误 | `./build-macos-installer.sh --clean-only && flutter clean` |
| 权限被拒绝 | `chmod +x *.sh` |

---

## 📊 构建流程图

```
┌─────────────────────┐
│  检查环境和依赖      │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│  清理之前的构建      │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│  构建 Flutter 应用   │
│  (flutter build)    │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│  注入 Cislink 配置   │
│  (rustdesk.toml)    │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│  代码签名 (可选)     │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│  创建 DMG 安装包     │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│  ✅ 完成！           │
└─────────────────────┘
```

---

## 🌐 Cislink 服务器

已配置的服务器信息：

- **ID/中继服务器**: `hbbs.cislink.nl`
- **中继服务器**: `hbbr.cislink.nl`
- **公钥**: 已预配置 ✓

用户无需手动配置，开箱即用！

---

## 📚 完整文档

详细说明请参考：
- **[MACOS_BUILD_INSTRUCTIONS.md](MACOS_BUILD_INSTRUCTIONS.md)** - 完整构建指南
- **[README_CISLINK.md](README_CISLINK.md)** - Cislink 版本说明

---

## 💡 提示

1. **首次构建**需要下载依赖，可能较慢
2. **后续构建**会快很多（使用缓存）
3. **分发给多用户**建议进行代码签名
4. **遇到问题**先尝试清理重构

---

## 🆘 获取帮助

```bash
# 查看构建脚本帮助
./build-macos-installer.sh --help

# 检查 Flutter 环境
flutter doctor -v

# 查看日志
tail -f /var/log/system.log
```

---

**祝构建顺利！** 🎉
