# Android 远控会话：U2/E2E 测试开关与输入日志面板

本说明用于配合 `docs/android-remote-session-touch-ui-prd.md` 的 Milestone 1 基建验收。

## 启用 Android E2E Mode（Debug 构建）

- 仅 **Android + Debug** 构建生效。
- 入口：`Settings` → `Android E2E Mode`（开关打开后，远控会话页会显示输入日志面板）。

## Input Event Log Overlay（输入事件日志面板）

- 位置：远控会话画面左上角。
- 文本断言（Semantics）：
  - 面板容器：`u2_remote_input_log_overlay`
  - 日志文本：`u2_remote_input_log_text`
  - 清空按钮：`u2_remote_input_log_clear`
  - 展开/收起按钮：`u2_remote_input_log_toggle`

日志格式为：`<timestamp_ms> <event_type> <json_data?>`，例如：

`1700000000000 left_click {"x":120,"y":300}`

