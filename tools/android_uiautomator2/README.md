# Android UI 自动化（uiautomator2 + uv）

本目录用于通过 `uiautomator2` 对安卓应用做 UI 自动化测试；依赖用 `uv` 管理。

## 前置要求

- 已安装 Android SDK（含 `platform-tools`、`emulator`）
- 目标设备/模拟器已开启开发者选项与 USB 调试
- 设备已允许调试授权弹窗（首次连接需要手动点允许）
- Windows 侧已安装 `uv`（用于在 Windows 侧管理并执行 `uiautomator2`）

## 安装依赖

首次安装（需要外网拉包）：

```bash
cd tools/android_uiautomator2
uv sync
```

## 启动模拟器（示例）

```bash
/home/sein/Android/Sdk/emulator/emulator -avd codex_emulator -no-snapshot-save
```

## Windows 侧 Emulator + UIAutomator2（强制：仅 WSL 触发）

本仓库的验收路径统一为：
- Android Emulator 运行在 Windows
- `uiautomator2`（以及 `uv`/Python 环境）运行在 Windows
- WSL 仅负责触发 Windows PowerShell 执行（不在 WSL 内跑 u2、不在 WSL 内启动模拟器）

### 一键触发（推荐）

在 WSL 中执行（会调用 Windows PowerShell，使用 Windows 的 `adb.exe`）：

```bash
# 1) Windows 侧安装依赖（首次/依赖变更时）
bash tools/android_uiautomator2/scripts/wsl_u2.sh sync

# 2) 连接校验
bash tools/android_uiautomator2/scripts/wsl_u2.sh u2-connect --serial emulator-5554

# 3) 冒烟测试（可选保存截图）
bash tools/android_uiautomator2/scripts/wsl_u2.sh u2-smoketest --serial emulator-5554 --screenshot .\\u2.png

# 4) Story 10：禁用更新检查（验收用例）
bash tools/android_uiautomator2/scripts/wsl_u2.sh u2-story10-disable-update --serial emulator-5554
```

## 连接校验（示例）

```bash
bash tools/android_uiautomator2/scripts/wsl_u2.sh u2-connect --serial emulator-5554
```

## 冒烟测试（推荐）

做一套更“接近真实自动化”的最小动作（info + 当前前台 + Home + 可选截图）：

```bash
bash tools/android_uiautomator2/scripts/wsl_u2.sh u2-smoketest --serial emulator-5554
```

可选保存截图：

```bash
bash tools/android_uiautomator2/scripts/wsl_u2.sh u2-smoketest --serial emulator-5554 --screenshot .\\u2.png
```

如果你不想设置 `PYTHONPATH`，也可以直接运行脚本文件：

```bash
bash tools/android_uiautomator2/scripts/wsl_u2.sh python src\\u2runner\\smoketest.py --serial emulator-5554 --screenshot .\\u2.png
```

想看更详细的启动/连接过程（判断是不是卡住、是否 adb 命令超时等）：

```bash
bash tools/android_uiautomator2/scripts/wsl_u2.sh u2-connect --serial emulator-5554 -v --timeout 600
```

成功后会输出设备信息与 `uiautomator2` 连接状态。

## WSL 侧怎么“连到 Windows AVD”

当前推荐方式是不在 WSL 里“直连 Windows adb server”，而是由 WSL 触发 Windows PowerShell 执行（见上面的 `scripts/wsl_u2.sh`）。这样可以避免 WSL/Windows adb server 端口、路径、权限差异导致的不稳定。
