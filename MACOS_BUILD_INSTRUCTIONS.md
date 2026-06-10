# macOS RustDesk 安装程序构建指南

本文档介绍如何在 macOS 上构建 RustDesk 的 DMG 安装包。

## 目录

- [系统要求](#系统要求)
- [环境准备](#环境准备)
- [快速开始](#快速开始)
- [详细步骤](#详细步骤)
- [配置选项](#配置选项)
- [代码签名和公证](#代码签名和公证)
- [常见问题](#常见问题)
- [故障排除](#故障排除)

## 系统要求

- **操作系统**: macOS 10.14 (Mojave) 或更高版本
- **处理器**: Intel 或 Apple Silicon (M1/M2/M3)
- **磁盘空间**: 至少 5GB 可用空间
- **内存**: 建议 8GB 或更多

## 环境准备

### 1. 安装 Xcode Command Line Tools

```bash
xcode-select --install
```

### 2. 安装 Homebrew（如果尚未安装）

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

### 3. 安装 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

验证安装：
```bash
rustc --version
cargo --version
```

### 4. 安装 Flutter

下载并安装 Flutter SDK：

```bash
cd ~/Development
git clone https://github.com/flutter/flutter.git -b stable
export PATH="$PATH:$HOME/Development/flutter/bin"
```

将 Flutter 添加到 PATH（添加到 ~/.zshrc 或 ~/.bash_profile）：

```bash
echo 'export PATH="$PATH:$HOME/Development/flutter/bin"' >> ~/.zshrc
source ~/.zshrc
```

验证安装：
```bash
flutter doctor
```

### 5. 安装 create-dmg 工具

```bash
brew install create-dmg
```

### 6. 安装 vcpkg 和依赖库

```bash
# 安装 vcpkg
cd ~/Development
git clone https://github.com/microsoft/vcpkg
cd vcpkg
./bootstrap-vcpkg.sh

# 设置环境变量
export VCPKG_ROOT="$HOME/Development/vcpkg"
echo 'export VCPKG_ROOT="$HOME/Development/vcpkg"' >> ~/.zshrc

# 安装 RustDesk 依赖
$VCPKG_ROOT/vcpkg install libvpx libyuv opus aom
```

## 快速开始

### 基本构建（无签名）

```bash
cd /path/to/RustDesk
chmod +x build-macos-installer.sh
./build-macos-installer.sh
```

### 带签名构建（推荐用于分发）

```bash
./build-macos-installer.sh --sign "Developer ID Application: Your Name (TEAM_ID)"
```

## 详细步骤

### 步骤 1: 准备配置文件

编辑 `RustDesk_Config_Template.toml` 文件，设置您的自定义服务器：

```toml
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
disable-update-check = true
disable-installation = true
```

### 步骤 2: 自定义应用信息（可选）

如需自定义应用名称、Bundle ID 等信息，编辑以下文件：

**flutter/macos/Runner/Info.plist**:
- 修改 `CFBundleIdentifier`（默认：com.carriez.rustdesk）
- 修改应用权限描述

**flutter/pubspec.yaml**:
- 修改应用名称和版本号

### 步骤 3: 运行构建脚本

```bash
# 进入项目目录
cd /path/to/RustDesk

# 赋予执行权限
chmod +x build-macos-installer.sh

# 执行构建
./build-macos-installer.sh
```

构建过程包括：
1. 清理之前的构建文件
2. 构建 Flutter macOS 应用
3. 注入配置文件
4. 代码签名（如果指定）
5. 创建 DMG 安装包

### 步骤 4: 测试安装包

```bash
# 打开生成的 DMG 文件
open RustDesk-Cislink-Installer-*.dmg

# 在 Finder 中拖拽 RustDesk.app 到 Applications 文件夹
# 然后从 Applications 文件夹启动应用
```

## 配置选项

### 构建脚本参数

| 参数 | 说明 | 示例 |
|------|------|------|
| `--sign CERT_NAME` | 使用指定证书进行代码签名 | `--sign "Developer ID Application: ..."` |
| `--clean-only` | 仅清理构建文件 | `--clean-only` |
| `--help` | 显示帮助信息 | `--help` |

### 示例命令

```bash
# 仅清理
./build-macos-installer.sh --clean-only

# 构建并签名
./build-macos-installer.sh --sign "Developer ID Application: John Doe (ABC123)"

# 查看帮助
./build-macos-installer.sh --help
```

## 代码签名和公证

### 为什么需要签名？

- **安全性**: 确保应用未被篡改
- **用户体验**: 避免 macOS Gatekeeper 警告
- **分发要求**: 通过互联网分发需要公证

### 获取开发者证书

1. 加入 [Apple Developer Program](https://developer.apple.com/programs/)（需付费 $99/年）
2. 在 Xcode 中：Preferences → Accounts → Manage Certificates
3. 创建 "Developer ID Application" 证书

### 查看可用证书

```bash
security find-identity -v -p codesigning
```

### 签名应用

```bash
./build-macos-installer.sh --sign "Developer ID Application: Your Name (TEAM_ID)"
```

### 公证（Notarization）步骤

公证是 Apple 要求的额外安全步骤：

```bash
# 1. 上传应用进行公证
xcrun notarytool submit RustDesk-Cislink-Installer-*.dmg \
  --apple-id "your-email@example.com" \
  --team-id "YOUR_TEAM_ID" \
  --password "app-specific-password" \
  --wait

# 2. 检查公证状态
xcrun notarytool info SUBMISSION_ID \
  --apple-id "your-email@example.com" \
  --team-id "YOUR_TEAM_ID" \
  --password "app-specific-password"

# 3. 将公证票据附加到 DMG（公证成功后）
xcrun stapler staple RustDesk-Cislink-Installer-*.dmg

# 4. 验证
xcrun stapler validate RustDesk-Cislink-Installer-*.dmg
```

**注意**:
- `app-specific-password` 需要在 [appleid.apple.com](https://appleid.apple.com) 生成
- 公证过程可能需要 5-30 分钟

## 常见问题

### Q1: 构建失败，提示找不到 Flutter

**A**: 确保 Flutter 已添加到 PATH：

```bash
which flutter
flutter doctor
```

### Q2: 提示缺少依赖库

**A**: 安装 vcpkg 依赖：

```bash
cd $VCPKG_ROOT
./vcpkg install libvpx libyuv opus aom
```

### Q3: DMG 创建失败

**A**: 确保已安装 create-dmg：

```bash
brew install create-dmg
```

### Q4: 应用在其他 Mac 上无法打开

**A**: 这是因为应用未签名。解决方案：
1. 进行代码签名和公证（推荐）
2. 或者在目标 Mac 上右键点击应用 → 打开，并在警告中选择"打开"

### Q5: 如何更改应用图标？

**A**: 替换 `flutter/macos/Runner/AppIcon.icns` 文件。可以使用在线工具将 PNG 转换为 ICNS 格式。

### Q6: 构建的应用体积很大

**A**: Release 构建已经进行了优化。如需进一步减小：
- 移除不需要的资源文件
- 考虑使用 `strip` 命令去除调试符号

## 故障排除

### 日志位置

构建过程的详细日志：
- Flutter 构建日志: `flutter/build/macos/Build/Products/Release/`
- 系统日志: 使用 `Console.app` 查看应用启动日志

### 清理并重新构建

```bash
# 清理所有构建文件
./build-macos-installer.sh --clean-only
rm -rf flutter/build
flutter clean

# 重新构建
./build-macos-installer.sh
```

### 检查应用包完整性

```bash
# 检查应用结构
ls -la flutter/build/macos/Build/Products/Release/RustDesk.app/Contents/

# 验证签名（如果已签名）
codesign -vv --deep --strict flutter/build/macos/Build/Products/Release/RustDesk.app

# 查看应用信息
plutil -p flutter/build/macos/Build/Products/Release/RustDesk.app/Contents/Info.plist
```

### 调试应用启动问题

```bash
# 从终端启动应用查看日志
./flutter/build/macos/Build/Products/Release/RustDesk.app/Contents/MacOS/RustDesk
```

## 高级配置

### 自定义 DMG 外观

创建 `dmg_background.png` 文件（800x400 像素）放在项目根目录，脚本会自动使用。

### 批量构建

如需为不同配置构建多个版本：

```bash
#!/bin/bash
# build-multiple.sh

configs=("config1.toml" "config2.toml" "config3.toml")

for config in "${configs[@]}"; do
    cp "$config" RustDesk_Config_Template.toml
    ./build-macos-installer.sh
    mv RustDesk-Cislink-Installer-*.dmg "RustDesk-${config%.toml}.dmg"
done
```

### 自动化部署

集成到 CI/CD 流程（例如 GitHub Actions）：

```yaml
name: Build macOS DMG

on: [push, pull_request]

jobs:
  build:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      - uses: subosito/flutter-action@v2
      - name: Install dependencies
        run: |
          brew install create-dmg
      - name: Build DMG
        run: ./build-macos-installer.sh
      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: RustDesk-DMG
          path: RustDesk-Cislink-Installer-*.dmg
```

## 相关资源

- [RustDesk 官方文档](https://rustdesk.com/docs/)
- [Flutter macOS 开发文档](https://docs.flutter.dev/desktop)
- [Apple 代码签名指南](https://developer.apple.com/documentation/security/code_signing)
- [Apple 公证文档](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)

## 联系支持

如有问题或需要帮助：
- 查看项目 README
- 提交 Issue 到项目仓库
- 联系技术支持

---

**祝您构建成功！** 🚀
