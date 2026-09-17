# RustDesk Autostart APK Build Workflow

此工作流用于编译启用开机自启动功能的 RustDesk Android APK。

## 功能改动

该工作流编译的 APK 包含以下自启动功能：

1. **开机自启动**：手机开机后自动启动 RustDesk 服务
2. **后台重启**：服务被系统或用户关闭后自动重新启动
3. **前台服务**：使用 Android 前台服务，即使在后台也能继续运行

### 核心修改

修改的文件位置：
- `flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/BootReceiver.kt`
  - 将开机启动默认值改为 `true`（无需手动UI操作）
  - 移除不必要的权限检查

- `flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainService.kt`
  - 返回 `START_STICKY` 标志
  - 实现 `onTaskRemoved()` 处理，任务被移除时自动重启

## 使用方式

### 自动触发

工作流会在以下情况自动触发：

1. **代码更新时**：当推送到 `master` 分支时，如果修改了以下文件：
   - `flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/BootReceiver.kt`
   - `flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainService.kt`
   - `.github/workflows/flutter-build-autostart-apk.yml`

### 手动触发

在 GitHub Actions 中选择 **"Build RustDesk Android APK with Autostart"** 工作流，点击 **"Run workflow"**，选择要编译的架构：
- `aarch64` (ARM64，推荐用于现代手机)
- `armv7` (ARM，用于较旧手机)
- `x86_64` (x86 仿真器)

## 编译输出

编译成功后，会生成以下 APK 文件：

- `rustdesk-autostart-1.4.7-aarch64.apk` (ARM64 版本)
- `rustdesk-autostart-1.4.7-armv7.apk` (ARM 版本)
- `rustdesk-autostart-1.4.7-x86_64.apk` (x86_64 版本)

### 下载位置

1. **GitHub Actions Artifacts**：在工作流运行详情页面，点击 "Artifacts" 部分
2. **GitHub Release**：自动创建 `autostart-1.4.7` Release，包含所有编译好的 APK

## 安装到 Root 手机

### 方法一：通过 ADB 安装

```bash
# 连接设备
adb devices

# 安装 APK
adb install -r rustdesk-autostart-1.4.7-aarch64.apk

# 授予自启动权限（需要 root）
adb shell pm grant com.carriez.flutter_hbb android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS
adb shell pm grant com.carriez.flutter_hbb android.permission.SYSTEM_ALERT_WINDOW
```

### 方法二：手动安装 + Root 授权

1. 下载 APK 文件
2. 将文件传输到手机
3. 打开文件管理器，点击 APK 安装
4. 安装后，使用 Root 应用（如 Magisk 或 SuperSU）授予必要权限

## 所需权限

该 APK 需要以下权限才能正常自启动：

- `RECEIVE_BOOT_COMPLETED` - 接收开机广播
- `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` - 忽略电池优化
- `SYSTEM_ALERT_WINDOW` - 系统提示窗口
- `FOREGROUND_SERVICE` - 前台服务

## 测试自启动功能

### 测试方法

1. 安装并授权 APK
2. 重启手机
3. 检查 RustDesk 是否自动启动
4. 查看通知栏是否有 RustDesk 前台服务通知

### 日志查看

```bash
# 查看 BootReceiver 日志
adb logcat | grep tagBootReceiver

# 查看 MainService 日志
adb logcat | grep LOG_SERVICE
```

## 故障排除

### 自启动不工作

1. **检查权限**：确保已授予所有必要权限
2. **电池优化**：将 RustDesk 从电池优化白名单中移除
3. **ROM 限制**：某些定制 ROM（如 MIUI、ColorOS）可能限制后台启动
4. **Adb logcat**：运行 `adb logcat` 查看是否有错误消息

### APK 编译失败

1. 检查工作流日志
2. 确保所有依赖项已正确安装
3. 检查 Rust 版本是否为 1.75

## 工作流参数

工作流使用以下环境变量：

| 变量 | 值 | 说明 |
|------|-----|------|
| RUST_VERSION | 1.75 | Rust 编译器版本 |
| FLUTTER_VERSION | 3.24.5 | Flutter SDK 版本 |
| NDK_VERSION | r28c | Android NDK 版本 |
| VERSION | 1.4.7 | RustDesk 版本 |

## 相关文件

- 工作流定义：`.github/workflows/flutter-build-autostart-apk.yml`
- Boot receiver：`flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/BootReceiver.kt`
- Main service：`flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainService.kt`
- AndroidManifest：`flutter/android/app/src/main/AndroidManifest.xml`

## 注意事项

⚠️ **重要**：
- 此 APK 需要在 Root 手机上安装以获得最佳效果
- 某些权限可能需要 Magisk 或其他 Root 解决方案才能授予
- 不同 ROM 的自启动策略可能有所不同
