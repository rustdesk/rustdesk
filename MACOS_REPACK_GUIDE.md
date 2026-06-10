# macOS DMG 重打包快速指南

## 需要准备的文件

将以下 3 个文件复制到 macOS 电脑的同一个文件夹：

```
📁 RustDesk-Build/
├── repack-macos-dmg.sh          # 重打包脚本
├── RustDesk_Config_Template.toml # 配置文件
└── rustdesk original/
    └── rustdesk-1.4.4-x86_64.dmg # 原始 DMG
```

## 在 macOS 上执行

```bash
# 1. 进入文件夹
cd ~/Desktop/RustDesk-Build  # 或你放置文件的位置

# 2. 给脚本执行权限
chmod +x repack-macos-dmg.sh

# 3. 运行脚本
./repack-macos-dmg.sh
```

## 输出结果

成功后会生成：`RustDesk-Cislink-1.4.4-x86_64.dmg`

## 配置内容

脚本会自动将以下配置注入到 DMG：

```toml
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
enable-check-update = "N"
disable-installation = true
```

## 可选：安装 create-dmg 获得更美观的 DMG

```bash
# 安装 Homebrew（如果没有）
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 安装 create-dmg
brew install create-dmg
```

## 常见问题

**Q: 提示"无法打开应用"怎么办？**
A: 因为 DMG 未签名。右键点击应用 → 打开，然后在弹窗中点击"打开"。

**Q: 如何避免安全警告？**
A: 需要 Apple Developer 账号进行代码签名和公证。
