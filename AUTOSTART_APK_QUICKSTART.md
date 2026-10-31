# 快速开始：自启动 APK 构建

## 一句话总结

在 GitHub Actions 中自动编译带开机自启动功能的 RustDesk Android APK。

## 快速步骤

### 1️⃣ 查看工作流状态
进入 **Actions** → **Build RustDesk Android APK with Autostart**

### 2️⃣ 手动触发编译（可选）
点击 **Run workflow** → 选择架构（推荐 `aarch64`）→ 点击绿色 **Run workflow**

### 3️⃣ 等待编译完成
工作流运行约 30-45 分钟（首次编译时间较长）

### 4️⃣ 下载 APK
两种方式下载：
- **Artifacts**：工作流详情页 → 找 `rustdesk-autostart-*.apk` 
- **Release**：Code → Releases → 找 `autostart-*` 版本

### 5️⃣ 安装到手机
```bash
adb install -r rustdesk-autostart-1.4.7-aarch64.apk
```

### 6️⃣ 授予权限（需 Root）
```bash
# 如果手机已 Root，使用以下命令
adb shell pm grant com.carriez.flutter_hbb android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS
adb shell pm grant com.carriez.flutter_hbb android.permission.SYSTEM_ALERT_WINDOW
```

### 7️⃣ 测试
重启手机，检查 RustDesk 是否自动启动

## 文件说明

| 文件 | 说明 |
|------|------|
| `.github/workflows/flutter-build-autostart-apk.yml` | GitHub Actions 工作流定义 |
| `flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/BootReceiver.kt` | 开机启动接收器（已修改） |
| `flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainService.kt` | 主服务（已修改） |
| `.github/AUTOSTART_APK_BUILD.md` | 详细文档 |

## 关键修改

✅ **已完成**：
1. BootReceiver - 开机启动默认打开
2. MainService - 服务被关闭后自动重启

## 架构选择

| 架构 | 适用设备 | 说明 |
|------|---------|------|
| **aarch64** | 大多数现代手机 | ARM64，推荐选择 |
| **armv7** | 较旧的手机 | 32 位 ARM |
| **x86_64** | Android 模拟器 | 用于开发测试 |

## 常见问题

**Q: 自启动不工作？**
A: 
1. 确保手机已 Root
2. 检查权限是否授予
3. 某些 ROM（如小米）需要额外配置

**Q: 如何查看编译错误？**
A: 点击工作流运行 → 查看各步骤的日志输出

**Q: 能否定制架构？**
A: 可以，在工作流中手动运行时选择特定架构

## 下一步

详见 [完整文档](.github/AUTOSTART_APK_BUILD.md)
