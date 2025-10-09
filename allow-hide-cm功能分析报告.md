# allow-hide-cm 参数功能分析报告

## 概述
`allow-hide-cm` 是 RustDesk 中用于控制是否隐藏连接管理窗口（Connection Management Window）的配置参数。

## 功能定位
- **参数名称**: `allow-hide-cm`
- **取值**: `Y` (启用) / `N` (禁用)
- **默认值**: `Y`
- **功能描述**: 允许隐藏 RustDesk 的连接管理主窗口，实现后台运行

## 生效条件（安全限制）
该功能仅在满足以下两个安全条件时才能启用：
1. **验证模式** (`approve-mode`) = `password`（密码验证）
2. **验证方法** (`verification-method`) = `use-permanent-password`（仅使用固定密码）

### 条件检查实现
```rust
// libs/hbb_common/src/password_security.rs
pub fn hide_cm() -> bool {
    approve_mode() == ApproveMode::Password
        && verification_method() == VerificationMethod::OnlyUsePermanentPassword
        && crate::config::option2bool("allow-hide-cm", &Config::get_option("allow-hide-cm"))
}
```

## 代码实现层次

### 1. 配置层 (Rust)
**文件**: `libs/hbb_common/src/config.rs`

- **常量定义**:
  ```rust
  pub const OPTION_ALLOW_HIDE_CM: &str = "allow-hide-cm";
  ```

- **默认配置**:
  ```rust
  map.insert("allow-hide-cm".to_string(), "Y".to_string());
  ```

- **配置项注册**: 添加到 `KEYS_BUILDIN_SETTINGS` 数组中

### 2. 原生 UI 层 (Sciter/TIS)

#### 主界面 (`src/ui/index.tis`)
- **菜单项显示**:
  ```javascript
  <li #allow-hide-cm disabled={ pin_locked || !enable_hide_options ? "true" : "false" }>
      <span>{svg_checkmark}</span>
      {translate('Hide connection management window')}
  </li>
  ```

- **选项切换**:
  ```javascript
  handler.set_option('allow-hide-cm', 
      handler.get_option('allow-hide-cm') == 'Y' ? 'N' : 'Y');
  ```

- **条件检查与自动禁用**:
  ```javascript
  if (!enable_hide_options) {
      if (handler.get_option('allow-hide-cm') == 'Y') {
          handler.set_option('allow-hide-cm', 'N');
      }
  }
  ```

#### 连接管理窗口 (`src/ui/cm.tis`)
- **窗口状态控制**:
  ```javascript
  function setWindowState(state) {
      var allow_hide_cm_option = handler.get_option('allow-hide-cm');
      if (allow_hide_cm_option == 'Y') {
          hide_cm = true;
      } else if (allow_hide_cm_option == 'N') {
          hide_cm = false;
      }
      if (hide_cm) return;  // 阻止窗口显示
      view.windowState = state;
  }
  ```

- **动态监控与更新**:
  ```javascript
  function check_update_ui() {
      self.timer(1s, function() {
          var allow_hide_cm = handler.get_option('allow-hide-cm');
          if (ui_status_cache[1] != allow_hide_cm) {
              var should_hide = allow_hide_cm == 'Y';
              if (should_hide) {
                  view.windowState = View.WINDOW_HIDDEN;
                  hide_cm = true;
              } else {
                  // 显示窗口
              }
          }
      });
  }
  ```

### 3. Flutter UI 层 (Dart)

#### 设置页面 (`flutter/lib/desktop/pages/desktop_setting_page.dart`)
- **条件检查**:
  ```dart
  final enableHideCm = model.approveMode == 'password' &&
      model.verificationMethod == kUsePermanentPassword;
  ```

- **选项切换**:
  ```dart
  onHideCmChanged(bool? b) {
      if (b != null) {
          bind.mainSetOption(
              key: 'allow-hide-cm', 
              value: bool2option('allow-hide-cm', b));
      }
  }
  ```

- **提示信息**: 不满足条件时显示 `hide_cm_tip` 提示

#### 服务器模型 (`flutter/lib/models/server_model.dart`)
- **自动禁用逻辑**:
  ```dart
  // 当验证方法不是固定密码时，自动禁用
  if (method != kUsePermanentPassword) {
      await bind.mainSetOption(
          key: 'allow-hide-cm', 
          value: bool2option('allow-hide-cm', false));
  }
  
  // 当验证模式不是密码时，自动禁用
  if (mode != 'password') {
      await bind.mainSetOption(
          key: 'allow-hide-cm', 
          value: bool2option('allow-hide-cm', false));
  }
  ```

## 功能特性

### 1. 安全机制
- 仅在使用固定密码且密码验证模式下可用
- 防止在不安全的配置下隐藏管理窗口
- 自动检查并强制禁用不满足条件的配置

### 2. 动态响应
- 实时监控配置变化（每秒检查）
- 配置改变时立即更新窗口状态
- 多端同步（原生界面和 Flutter 界面）

### 3. 用户交互
- 菜单项显示勾选状态
- 不满足条件时菜单项自动禁用（灰化）
- 提供多语言提示信息

### 4. 联动机制
- 与 `approve-mode` 联动
- 与 `verification-method` 联动
- 与 `hide-tray`（隐藏托盘）功能逻辑一致

## 用户界面文本
- **英文**: "Hide connection management window"
- **中文**: "隐藏连接管理窗口"
- **提示信息**: "在只允许密码连接并且只用固定密码的情况下才允许隐藏"

## 使用场景
1. **无人值守服务器**: 需要后台运行，不显示主窗口
2. **安全远程访问**: 在确保使用固定密码的前提下，隐藏管理界面
3. **减少干扰**: 不希望主窗口占用任务栏或屏幕空间

## 相关文件清单
- `libs/hbb_common/src/config.rs` - 配置定义
- `libs/hbb_common/src/password_security.rs` - 安全条件检查
- `src/ui/index.tis` - 主界面选项控制
- `src/ui/cm.tis` - 连接管理窗口状态控制
- `flutter/lib/desktop/pages/desktop_setting_page.dart` - Flutter 设置界面
- `flutter/lib/models/server_model.dart` - Flutter 服务器模型
- `src/lang/*.rs` - 多语言翻译文件

## 总结
`allow-hide-cm` 是一个安全性优先的功能开关，通过严格的条件限制（必须同时满足密码验证模式和固定密码使用）来确保远程访问的安全性。该功能在多个层次（Rust 后端、Sciter UI、Flutter UI）都有完整实现，包括配置管理、条件检查、动态更新和用户交互等方面，实现了前后端一致的行为。
