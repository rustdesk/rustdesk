# unlock_pin 功能分析报告

## 概述
`unlock_pin` 是 RustDesk 中实现的一个安全功能，用于通过 PIN 码保护关键配置设置，防止未经授权的修改。

## 核心功能

### 1. 数据存储与加密
**位置**: `libs/hbb_common/src/config.rs`

- **存储结构**: `Config2` 结构体中的 `unlock_pin: String` 字段
- **加密机制**: 
  - 使用 `PASSWORD_ENC_VERSION` 版本的加密算法
  - 存储时调用 `encrypt_str_or_original()` 加密
  - 读取时调用 `decrypt_str_or_original()` 解密
  - 最大长度限制: `ENCRYPT_MAX_LEN`

### 2. PIN 码设置与验证

#### 设置 PIN (`set_unlock_pin`)
**调用链**: UI → `ipc::set_unlock_pin()` → `Config::set_unlock_pin()`

**验证规则**:
- 最小长度: 4 个字符
- 最大长度: `ENCRYPT_MAX_LEN`
- 允许空值（表示删除 PIN）
- 自动去除首尾空格

**实现位置**:
- `src/ipc.rs:1028` - 验证逻辑
- `libs/hbb_common/src/config.rs:1302` - 存储逻辑

#### 获取 PIN (`get_unlock_pin`)
**调用链**: UI → `ipc::get_unlock_pin()` → `Config::get_unlock_pin()`

**特性**:
- 优先从 IPC 配置读取
- 回退到内存中的配置值
- 支持默认值 (`DEFAULT_SETTINGS` 中的 `unlock_pin`)

### 3. UI 集成

#### Sciter UI (`src/ui/index.tis`)
**两个菜单位置**:
1. **ID 菜单** (`#config-options`)
   - 菜单项: `#unlock-pin`
   - 功能: 设置/验证 PIN、锁定配置选项

2. **密码菜单** (`#edit-password-context`)
   - 菜单项: `#unlock-pin-password`
   - 功能: 同上，独立的菜单实例

**锁定机制**:
- 全局变量: `menu_unlocked = false`
- 当 `has_pin && !menu_unlocked` 时，菜单项被禁用
- PIN 验证成功后，`menu_unlocked = true`

**受保护的菜单项**:
- RDP 会话共享
- 直接 IP 访问
- 音频输入设备
- 语言设置
- 增强功能
- 键盘/鼠标控制
- 剪贴板
- 文件传输
- 摄像头
- 终端
- 远程重启
- 审批模式
- 密码方式
- Hide CM 相关选项

#### Flutter UI
**位置**: 
- `src/flutter_ffi.rs` - FFI 桥接
- `flutter/lib/common/widgets/dialog.dart` - 对话框
- `flutter/lib/desktop/pages/desktop_setting_page.dart` - 设置页面

**功能**:
- `main_get_unlock_pin()` - 获取 PIN
- `main_set_unlock_pin()` - 设置 PIN
- `setUnlockPinDialog()` - PIN 设置对话框
- `checkUnlockPinDialog()` - PIN 验证对话框

### 4. 命令行支持

**位置**: `src/core_main.rs:387`

**命令**: `--set-unlock-pin <PIN>`

**要求**:
- 必须是已安装版本
- 需要管理员权限 (`is_root()`)
- 仅支持 Flutter 特性

## 工作流程

### 设置 PIN 流程
```
用户点击 "Unlock with PIN" (未设置或已解锁)
  ↓
显示 PIN 设置对话框
  ↓
输入 PIN + 确认 PIN
  ↓
验证: 两次输入一致 + 长度符合要求
  ↓
调用 set_unlock_pin()
  ↓
加密并存储到配置文件
  ↓
menu_unlocked = true (自动解锁)
```

### 验证 PIN 流程
```
用户点击 "Unlock with PIN" (已设置且未解锁)
  ↓
显示 PIN 验证对话框
  ↓
输入 PIN
  ↓
与存储的 PIN 比对
  ↓
匹配成功: menu_unlocked = true
  ↓
受保护菜单项解锁
```

### 删除 PIN 流程
```
已解锁状态下进入设置 PIN 对话框
  ↓
两次输入均留空
  ↓
调用 set_unlock_pin("")
  ↓
清空存储的 PIN
  ↓
menu_unlocked = false
```

## IPC 通信

**位置**: `src/ipc.rs:548-570`

**消息类型**: `Data::Config`

**操作**:
- 读取: `get_config("unlock-pin")`
- 写入: `set_config("unlock-pin", value)`

## 平台限制

**不支持**: Android、iOS
- 相关代码使用 `#[cfg(not(any(target_os = "android", target_os = "ios")))]` 条件编译
- 移动平台返回空字符串

## 安全特性

1. **加密存储**: PIN 不以明文形式保存
2. **长度限制**: 防止过短或过长的 PIN
3. **权限检查**: 命令行设置需要管理员权限
4. **默认锁定**: 设置 PIN 后默认处于锁定状态
5. **会话隔离**: 每次重启应用后需要重新验证

## 使用场景

1. **企业部署**: 防止终端用户修改关键配置
2. **共享环境**: 保护远程访问设置不被篡改
3. **管理员模式**: 提供额外的安全层级
4. **自动化部署**: 通过命令行预设 PIN

## 关键代码路径

```
配置层:    libs/hbb_common/src/config.rs
IPC 层:    src/ipc.rs
UI 层:     src/ui/index.tis (Sciter)
          flutter/lib/common/widgets/dialog.dart (Flutter)
接口层:    src/ui_interface.rs, src/flutter_ffi.rs
命令行:    src/core_main.rs
```

## 总结

`unlock_pin` 功能为 RustDesk 提供了细粒度的配置保护机制，通过 PIN 码验证确保只有授权用户才能修改关键设置。该功能集成在 Sciter 和 Flutter UI 中，支持命令行批量部署，并采用加密存储保证安全性。
