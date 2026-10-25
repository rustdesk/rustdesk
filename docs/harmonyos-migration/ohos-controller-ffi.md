# RustDesk OHOS 主控端 FFI 契约（librustdesk.so ↔ ArkTS）

> 版本：M1（主控端先行，纯客户端，无被控/采集能力）
> Rust 侧实现：`src/ohos_ffi.rs`（`#[cfg(target_env = "ohos")]`）
> OHOS 侧实现：`harmonyos/entry/src/main/cpp/napi_init.cpp`（手写 NAPI 包装）
> 事件 JSON 与 Flutter 端 `FlutterHandler::push_event` 完全同构，ArkTS 解析逻辑可对照
> `flutter/lib/models/model.dart` 的 `sessionListener`。

## 1. 调用约定

- 所有函数 `extern "C"`，字符串为 UTF-8 以 `\0` 结尾。
- Rust 返回的字符串（`*mut c_char`）必须用 `ohos_free_string` 释放；`*const c_char` 返回值（静态字符串）不得释放。
- `int` 返回值：`0` 成功，`-1` 失败（session 不存在等）。`ohos_session_new` 返回字符串（见下）。
- 回调由 `ohos_set_callbacks` 注册一次：
  - `event_cb(const char* json)`：JSON 事件（见 §3），任意线程调用。
  - `rgba_cb(int32_t display, const uint8_t* data, int32_t len, int32_t w, int32_t h, int32_t stride)`：
    帧投递。**回调必须同步拷贝数据**（Rust 在回调返回后复用缓冲）。RGBA8888，`stride` 为字节步长。
  - `clipboard_cb(const char* text)`：远端剪贴板文本（客户端方向接收）。
- NAPI 侧用 `napi_threadsafe_function` 把回调转投 JS 线程。

## 2. C ABI 导出（Rust 侧 `#[no_mangle]`）

### 初始化
| 函数 | 说明 |
|------|------|
| `ohos_init(home_dir, app_dir, custom_client_config) -> c_int` | 设置 `APP_HOME_DIR`/`APP_DIR`，读自定义客户端配置，初始化日志与 tokio runtime。主控端不启动被控服务 |
| `ohos_set_callbacks(event_cb, rgba_cb, clipboard_cb)` | 注册三个回调（函数指针） |
| `ohos_free_string(s)` | 释放 `ohos_session_new` / `ohos_version` / `ohos_get_option` 返回的字符串 |
| `ohos_version() -> *const c_char` | 静态版本号 |
| `ohos_get_option(key) -> *mut c_char` | 读配置项（如 `custom-rendezvous-server`、`relay-server`、`api-server`、`key`），返回字符串需 `ohos_free_string` 释放 |
| `ohos_set_option(key, value) -> c_int` | 写配置项并落盘（`RustDesk2.toml`）；空值清除该键恢复默认。下次连接即生效，无需重启 |

### 会话生命周期
| 函数 | 说明 |
|------|------|
| `ohos_session_new(peer_id, is_file_transfer: c_int, password, force_relay: c_int) -> *mut c_char` | 创建会话，返回 session uuid 字符串（失败返回空串） |
| `ohos_session_start(uuid) -> c_int` | 启动 io_loop（连接、握手） |
| `ohos_session_login(uuid, password, remember: c_int) -> c_int` | 应答 `input-password` msgbox（OS 用户名/密码桌面流程，M1 传空） |
| `ohos_session_send2fa(uuid, code, trust_this_device: c_int) -> c_int` | 应答 2FA 验证码 |
| `ohos_session_reconnect(uuid, force_relay: c_int) -> c_int` | 重连 |
| `ohos_session_close(uuid) -> c_int` | 关闭会话 |

### 输入
| 函数 | 说明 |
|------|------|
| `ohos_send_mouse(uuid, mask, x, y, alt, ctrl, shift, command) -> c_int` | `mask = (buttons << 3) \| type`；type（低 3 位）：`MOVE=0, DOWN=1, UP=2, WHEEL=3, TRACKPAD=4`；buttons：`LEFT=0x01, RIGHT=0x02, WHEEL=0x04, BACK=0x08, FORWARD=0x10`（`src/common.rs::input`）。滚轮/触控板时 `x`/`y` 为滚动增量 |
| `ohos_input_key(uuid, name, down, press, alt, ctrl, shift, command) -> c_int` | `name` 为单字符或 CtrlKey 名（同 Flutter `CtrlKey` 枚举名） |
| `ohos_input_string(uuid, value) -> c_int` | 软键盘差量文本提交 |
| `ohos_session_refresh(uuid) -> c_int` | 请求重绘（全帧刷新） || `ohos_switch_display(uuid, display: c_int) -> c_int` | 切换显示器 |

### 剪贴板（客户端 → 远端）
| 函数 | 说明 |
|------|------|
| `ohos_send_clipboard(uuid, text) -> c_int` | 发送本机剪贴板文本（连接建立后与前台恢复时调用） |

### 文件传输（客户端）
| 函数 | 说明 |
|------|------|
| `ohos_send_files(uuid, act_id, path, to, file_num, include_hidden, is_remote) -> c_int` | 发起传输任务（`is_remote=1` 表示对端 → 本机下载） |
| `ohos_set_confirm_override_file(uuid, act_id, file_num, need_override, remember, is_upload) -> c_int` | 应答覆盖确认 |
| `ohos_cancel_job(uuid, act_id) -> c_int` | 取消任务 |
| `ohos_remove_file(uuid, act_id, path, file_num, is_remote) -> c_int` | 删除文件 |
| `ohos_read_dir(uuid, path, include_hidden, is_remote) -> c_int` | 列远端目录（结果经 `update_folder_files` 事件返回） |

## 3. 事件 JSON（event_cb 参数，全部含 `"name"` 字段）

ArkTS M1 需处理（其余事件忽略即可）：

| name | 字段 | 用途 |
|------|------|------|
| `peer_info` | `username, hostname, platform, displays(JSON字符串), version, current_display, resolutions(JSON字符串), features(JSON字符串)` | 对端信息、显示器列表 |
| `switch_display` | `display, x, y, width, height, cursor_embedded` | 切换显示器 |
| `connection_ready` | `secure, direct, stream_type` | 连接建立 |
| `msgbox` | `type, title, text, link, hasRetry` | 弹窗：`input-password` 密码、`2fa` 验证码、错误/提示（对照 model.dart 的 `Dialog` 分支） |
| `cancel_msgbox` | `tag` | 撤销弹窗 |
| `permission` | 字段名即权限名（`keyboard/clipboard/audio/file/restart`），值为 `"true"/"false"` | 对端授权状态 |
| `update_quality_status` | `speed, fps, delay, target_bitrate, decode`（值为字符串或 `null`） | 工具栏状态 |
| `fingerprint` | `fingerprint` | 指纹展示（可忽略） |
| `job_progress` | `id, file_num, speed, finished_size` | 传输进度 |
| `job_done` | `id, file_num` | 传输完成 |
| `job_error` | `id, err, file_num` | 传输失败 |
| `update_folder_files` | `info`（JSON 字符串：`{id, entries:[{name,type,time,size}], total_size}`，仅 only_count 时） | 目录计数 |
| `file_dir` | `is_local:"false"`, `value`（JSON 字符串：`{id, path, entries:[{entry_type,name,size,modified_time}]}`） | **远端目录列表主通道**。`entry_type`：0=Dir 2=DirLink 3=DirDrive 4=File 5=FileLink（`libs/base/protos/message.proto`），2/3 按目录处理 |
| `override_file_confirm` | `id, file_num, read_path, is_upload, is_identical` | 覆盖确认弹窗 |
| `cursor_data` | `id, hotx, hoty, width, height, colors`（colors 为 base64 的 RGBA8888） | 远端光标形状（解码缓存到 ArkTS 侧，id 为键）；io_loop 已解压校验 |
| `cursor_id` | `id` | 切换到已缓存的光标形状（无位置信息） |
| `cursor_position` | `x, y`（远端像素坐标） | 远端光标位置更新（可与视频帧同频，注意 UI 侧状态更新开销） |
| `load_last_job` | `value`（job JSON） | 恢复上次任务（可忽略） |
| `sync_peer_info` | `displays` | 显示器同步 |

`update_folder_files.info` 中 `type`：`0`→按 `1` 处理（文件），其余为原始值（dir=2）。

## 4. OHOS 侧职责（NAPI + ArkTS）

- `napi_init.cpp`：加载 `librustdesk.so` 符号、注册回调（TSF 转投 JS 线程）、RGBA 帧 → `XComponent` surface 的 `OH_NativeWindow` 写入（等宽拷贝 + `stride` 处理）。
- ArkTS 页面：
  - 首页：ID 输入/最近会话（`@ohos.data.preferences` 持久化） → `ohos_session_new` + `ohos_session_start`（或文件传输模式 `is_file_transfer=1`）。
  - 远程页：`XComponent` + 手势映射（Tap→左键 down/up、长按→右键、双指→滚轮、拖动→move）、软键盘（`TextInput` → `ohos_input_string`）、修饰键 Ctrl/Alt/Shift、显示器切换、断开/刷新。
    - 远端光标渲染：`cursor_data`/`cursor_id`/`cursor_position` 事件 → PixelMap 叠加层，与视频同容器共享缩放变换；工具栏开关（对端 Android 或 `cursor_embedded` 时不绘制）。
    - 剪贴板本机→远端：工具栏"同步剪贴板"按钮 → 粘贴弹窗（长按粘贴本机剪贴板）→ `ohos_send_clipboard`。
      `pasteboard.getData()` 需要 ACL 受限权限 `ohos.permission.READ_PASTEBOARD`（当前调试签名 profile `acls` 为空拿不到），
      故走系统原生粘贴通道，零权限。
    - 会话期间 `window.setWindowKeepScreenOn(true)` 保活亮屏。
  - 文件传输页：本地面板（`@ohos.file.fs` 列沙箱目录 + `@ohos.file.picker` 导入/导出沙箱外文件）+ 远端面板（`ohos_read_dir`/`update_folder_files`）+ 上传/下载（`ohos_send_files`）+ 进度条。
  - 弹窗：`msgbox` 事件 → 密码/2FA/错误对话框。
- `module.json5` 权限：`ohos.permission.INTERNET`（+ 文件读取按需）。

## 5. Rust 侧已知限制（M1）

- 音频播放未实现（`io_loop` 音频帧丢弃）；后续接 OHAudio。
- 无硬件解码（hwcodec/mediacodec 关闭），VP8/VP9 软解（libvpx）；AV1/H264/H265 后续。
- 剪贴板仅文本（`clipboard_cb` / `ohos_send_clipboard`）；本机→远端为手动同步（工具栏按钮 → 粘贴弹窗），
  `pasteboard.getData()` 直读因 READ_PASTEBOARD 为 ACL 受限权限暂不可用（正式签名可申请后替换）。
