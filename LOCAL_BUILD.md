# 本地编译 Autostart APK

由于无法直接推送到官方仓库，可以使用本地构建脚本编译 APK。

## 前置要求

```bash
# 安装必要工具
sudo apt-get update
sudo apt-get install -y \
  rustc \
  cargo \
  flutter \
  openjdk-17-jdk-headless \
  android-sdk-build-tools \
  android-sdk-platform-tools

# 设置 Android NDK
export ANDROID_NDK_HOME=/path/to/android-ndk-r28c
export ANDROID_SDK_ROOT=/path/to/android-sdk
```

## 使用本地构建脚本

### 基本用法
```bash
# 给脚本执行权限
chmod +x build-autostart-apk.sh

# 编译 aarch64 版本（推荐）
./build-autostart-apk.sh aarch64

# 编译 armv7 版本
./build-autostart-apk.sh armv7

# 编译 x86_64 版本
./build-autostart-apk.sh x86_64
```

### 输出
编译成功后会生成：
- `rustdesk-autostart-1.4.7-aarch64.apk`
- `rustdesk-autostart-1.4.7-armv7.apk`
- `rustdesk-autostart-1.4.7-x86_64.apk`

## 使用 GitHub Actions 工作流

### 前提条件
工作流文件已创建：`.github/workflows/flutter-build-autostart-apk.yml`

### 手动方式（需要推送权限）

1. **推送到 GitHub Fork**
```bash
# 添加 fork 作为远程
git remote add fork https://github.com/YOUR_USERNAME/rustdesk.git

# 推送分支
git push fork feature/android-autostart-apk

# 在 GitHub 上创建 PR
gh pr create --head YOUR_USERNAME:feature/android-autostart-apk \
             --title "feat: Add Android APK autostart build workflow"
```

2. **触发工作流**
   - 进入 GitHub Actions
   - 选择 "Build RustDesk Android APK with Autostart"
   - 点击 "Run workflow"
   - 选择架构并确认

### 工作流触发条件

工作流在以下情况自动运行：
- 推送修改到 `master` 分支且修改了：
  - `flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/BootReceiver.kt`
  - `flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainService.kt`

## 快速对比：本地 vs 工作流

| 方式 | 优点 | 缺点 |
|------|------|------|
| **本地脚本** | 快速、无需推送权限 | 需要本地环境配置 |
| **工作流** | 自动化、CI/CD | 需要 GitHub 权限、有延迟 |

## 验证修改

### 查看已修改的文件

```bash
# BootReceiver.kt 修改
git diff flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/BootReceiver.kt

# MainService.kt 修改
git diff flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainService.kt

# 查看工作流
cat .github/workflows/flutter-build-autostart-apk.yml
```

### 检查关键改动

```bash
# 检查 BootReceiver 中的默认启动
grep "putExtra(EXT_INIT_FROM_BOOT, true)" \
  flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/BootReceiver.kt

# 检查 MainService 中的 START_STICKY
grep "return START_STICKY" \
  flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainService.kt

# 检查 onTaskRemoved 实现
grep -A 5 "override fun onTaskRemoved" \
  flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainService.kt
```

## 故障排除

### 脚本权限错误
```bash
chmod +x build-autostart-apk.sh
```

### NDK 路径错误
```bash
export ANDROID_NDK_HOME=/path/to/ndk
```

### Gradle 编译失败
```bash
# 增加 JVM 内存
sed -i "s/org.gradle.jvmargs=-Xmx1024M/org.gradle.jvmargs=-Xmx2g/g" \
  flutter/android/gradle.properties
```

## 下一步

1. **本地测试**: 使用 `build-autostart-apk.sh` 快速测试
2. **推送到 Fork**: 如有 GitHub fork，推送并创建 PR
3. **验证 APK**: 安装到手机并测试自启动功能

详见主文档 `.github/AUTOSTART_APK_BUILD.md`
