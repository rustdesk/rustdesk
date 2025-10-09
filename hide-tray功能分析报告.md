# hide-tray 参数功能分析报告

## 1. 功能概述

`hide-tray` 是 RustDesk 中用于控制系统托盘图标显示/隐藏的配置参数。该功能允许用户在满足特定安全条件下隐藏托盘图标，提供更隐蔽的远程控制体验。

## 2. 配置定义

### 2.1 配置常量
- **键名**: `hide-tray`
- **常量定义**: `config::keys::OPTION_HIDE_TRAY`
- **值类型**: 字符串 ("Y" 表示启用, "N" 表示禁用)
- **默认值**: `"Y"` (在 `DEFAULT_SETTINGS` 中定义)

**代码位置**: `libs/hbb_common/src/config.rs:115, 2616`

```rust
map.insert("hide-tray".to_string(), "Y".to_string());
pub const OPTION_HIDE_TRAY: &str = "hide-tray";
```

## 3. 启用条件限制

### 3.1 安全约束
仅当同时满足以下条件时，`hide-tray` 功能才可用：

1. **审批模式** (`approve-mode`) = `"password"` (密码模式)
2. **验证方式** (`verification-method`) = `"use-permanent-password"` (固定密码)

**实现位置**:
- `src/ui/index.tis:1206-1222` - UI 自动检查并禁用
- `flutter/lib/desktop/pages/desktop_setting_page.dart:1402-1403` - Flutter UI 条件判断
- `flutter/lib/models/server_model.dart:136-142` - 设置变更时自动重置

### 3.2 自动重置机制

当用户更改 `approve-mode` 或 `verification-method` 导致不满足条件时，系统会自动将 `hide-tray` 强制设置为 `"N"` (禁用状态)。

```dart
// Flutter 代码示例
if (mode != 'password') {
  await bind.mainSetOption(
      key: 'hide-tray', value: bool2option('hide-tray', false));
}
```

## 4. 实现机制

### 4.1 架构概览

采用 **IPC (进程间通信)** 架构实现动态控制：

```
[主界面/Flutter UI] 
    ↓ 设置选项
[ui_interface.rs / flutter_ffi.rs] 
    ↓ IPC 连接 "hide-tray"
[tray.rs 监听器]
    ↓ 接收 Data::HideTray(bool)
[托盘图标控制]
```

### 4.2 核心组件

#### 4.2.1 托盘进程初始化
**文件**: `src/tray.rs:126-131`

```rust
let hide_tray = crate::ui_interface::get_option(
    hbb_common::config::keys::OPTION_HIDE_TRAY) == "Y";
#[cfg(windows)]
if hide_tray {
    ipc_sender.send(Data::HideTray(true)).ok();
}
```

启动时读取配置，若已启用则立即发送隐藏指令。

#### 4.2.2 IPC 监听器 (Windows)
**文件**: `src/tray.rs:362-383`

```rust
#[cfg(windows)]
fn start_ipc_listener(sender: std::sync::mpsc::Sender<Data>) {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        if let Ok(mut incoming) = crate::ipc::new_listener("hide-tray").await {
            loop {
                if let Some(Ok(stream)) = incoming.next().await {
                    // 处理 Data::HideTray(hide) 消息
                    sender_clone.send(Data::HideTray(hide)).ok();
                }
            }
        }
    });
}
```

监听名为 `"hide-tray"` 的 IPC 通道，接收隐藏/显示指令。

#### 4.2.3 消息发送者
**文件**: `src/ui_interface.rs:380-391` 和 `src/flutter_ffi.rs:932-940`

```rust
fn send_hide_tray_message(hide: bool) {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        if let Ok(mut conn) = ipc::connect(1000, "hide-tray").await {
            let _ = conn.send(&Data::HideTray(hide)).await;
        }
    });
}
```

通过 IPC 向托盘进程发送控制消息。

#### 4.2.4 选项变更监听
**文件**: `src/ui_interface.rs:433-440` 和 `src/flutter_ffi.rs:980-985`

```rust
if key.eq(config::keys::OPTION_HIDE_TRAY) {
    let hide = value == "Y";
    send_hide_tray_message(hide);
}
```

当 `hide-tray` 选项变更时，自动触发 IPC 通知。

### 4.3 托盘图标控制逻辑

**文件**: `src/tray.rs:210-229`

```rust
Data::HideTray(hide) => {
    let mut tray_guard = _tray_icon.lock().unwrap();
    if hide {
        // 销毁图标对象以隐藏
        *tray_guard = None;
        #[cfg(windows)]
        refresh_tray_area();
    } else if tray_guard.is_none() {
        // 重建图标对象以显示
        if let Ok(tray) = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu.clone()))
            .with_tooltip(tooltip(0))
            .with_icon(icon.clone())
            .with_icon_as_template(true)
            .build()
        {
            *tray_guard = Some(tray);
        }
    }
}
```

**关键机制**:
- **隐藏**: 将托盘图标对象设置为 `None`，调用 `refresh_tray_area()` 强制刷新
- **显示**: 重新构建 `TrayIcon` 对象并赋值

### 4.4 Windows 特定优化

**文件**: `src/tray.rs:301-329` (部分显示)

```rust
#[cfg(windows)]
fn refresh_tray_area() {
    unsafe {
        // 查找任务栏窗口
        let taskbar_hwnd = FindWindowW(
            encode_wide("Shell_TrayWnd").as_ptr(),
            std::ptr::null(),
        );
        
        // 查找托盘通知区域
        let tray_hwnd = FindWindowExW(taskbar_hwnd, ...);
        
        // 发送 WM_MOUSEMOVE 消息触发重绘
        SendMessageW(tray_hwnd, WM_MOUSEMOVE, ...);
    }
}
```

模拟鼠标移动强制刷新 Windows 系统托盘区域，避免"幽灵图标"残留。

## 5. UI 集成

### 5.1 Sciter UI (桌面旧版)
**文件**: `src/ui/index.tis:1084, 1198-1201`

- 菜单项显示条件: `!pin_locked && enable_hide_options`
- 点击切换: `handler.set_option('hide-tray', ...)`
- 自动同步: 通过 IPC 通知托盘进程

### 5.2 Flutter UI (跨平台新版)
**文件**: `flutter/lib/desktop/pages/desktop_setting_page.dart:1398-1420`

```dart
Widget hide_tray(bool enabled) {
  final enableHideTray = model.approveMode == 'password' &&
      model.verificationMethod == kUsePermanentPassword;
  
  onHideTrayChanged(bool? b) {
    if (b != null) {
      bind.mainSetOption(
          key: 'hide-tray', value: bool2option('hide-tray', b));
    }
  }
  
  return Tooltip(
      message: enableHideTray ? "" : translate('hide_cm_tip'),
      child: GestureDetector(...));
}
```

**特性**:
- 条件禁用时显示提示信息 (`hide_cm_tip`)
- 通过 `mainSetOption` FFI 调用设置选项

### 5.3 状态同步
**文件**: `flutter/lib/models/server_model.dart:175-180`

```dart
_hideTray = option2bool(
    'hide-tray', bind.mainGetOptionSync(key: 'hide-tray'));
if (!(approveMode == 'password' &&
    verificationMethod == kUsePermanentPassword)) {
  _hideTray = false;
}
```

启动时从配置读取状态，并根据安全条件自动校正。

## 6. 平台支持

| 平台 | 支持状态 | 备注 |
|------|---------|------|
| Windows | ✅ 完整支持 | 包含托盘刷新优化 |
| macOS | ✅ 支持 | 使用 `ActivationPolicy::Accessory` |
| Linux | ✅ 支持 | 标准托盘图标控制 |
| Android | ❌ 不适用 | 使用 `#[cfg(not(target_os = "android"))]` 排除 |
| iOS | ❌ 不适用 | 使用 `#[cfg(not(target_os = "ios"))]` 排除 |

## 7. 数据流时序

```
用户操作 → UI 设置界面
    ↓
检查安全条件 (password + permanent)
    ↓ (满足条件)
set_option("hide-tray", "Y")
    ↓
触发选项变更回调
    ↓
send_hide_tray_message(true)
    ↓
IPC 连接 "hide-tray" 通道
    ↓
托盘进程接收 Data::HideTray(true)
    ↓
*tray_guard = None (销毁图标)
    ↓
refresh_tray_area() (Windows)
    ↓
托盘图标消失
```

## 8. 关键技术点

1. **进程隔离**: 托盘独立进程运行，通过 IPC 通信
2. **动态控制**: 无需重启即可实时隐藏/显示图标
3. **安全约束**: 自动检查并强制执行启用条件
4. **平台适配**: Windows 特殊处理系统托盘刷新
5. **状态持久化**: 配置保存到 `DEFAULT_SETTINGS` 配置文件

## 9. 注意事项

1. **隐藏后访问**: 图标隐藏后，用户需通过其他方式 (如命令行、开始菜单) 打开主界面
2. **安全风险**: 隐藏托盘可能降低用户对远程连接的感知
3. **自动重置**: 切换到非密码模式会自动禁用该功能
4. **代码注释**: 多处标注"修复隐藏托盘图标功能"，表明该功能经过针对性修复

## 10. 相关文件清单

### 核心实现
- `libs/hbb_common/src/config.rs` - 配置定义
- `src/tray.rs` - 托盘图标控制核心
- `src/ui_interface.rs` - UI 接口层
- `src/flutter_ffi.rs` - Flutter FFI 桥接

### UI 层
- `src/ui/index.tis` - Sciter UI 实现
- `flutter/lib/desktop/pages/desktop_setting_page.dart` - Flutter 设置页面
- `flutter/lib/models/server_model.dart` - Flutter 数据模型

### 通信
- `src/ipc.rs` - IPC 通信基础 (未在本次分析中详细展开)

---

**生成日期**: 2025年10月9日  
**分析版本**: RustDesk master 分支
