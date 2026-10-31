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
| `ohos_session_new(peer_id, session_kind: c_int, password, force_relay: c_int) -> *mut c_char` | 创建会话，返回 session uuid 字符串（失败返回空串）。`session_kind`：`0`=远程控制，`1`=文件传输，`2`=摄像头查看（VIEW_CAMERA） |
| `ohos_session_start(uuid) -> c_int` | 启动 io_loop（连接、握手） |
| `ohos_session_login(uuid, password, remember: c_int) -> c_int` | 应答 `input-password` msgbox（OS 用户名/密码桌面流程，M1 传空） |
| `ohos_session_send2fa(uuid, code, trust_this_device: c_int) -> c_int` | 应答 2FA 验证码 |
| `ohos_session_reconnect(uuid, force_relay: c_int) -> c_int` | 重连 |
| `ohos_session_close(uuid) -> c_int` | 关闭会话 |

### 音频 / 语音通话 / 录制
| 函数 | 说明 |
|------|------|
| `ohos_record_screen(uuid, start: c_int) -> c_int` | 开始/停止会话录制（对端须授予 recording 权限；文件落在 `<home>/<AppName>/ScreenRecord`，经 `record_status` 事件回报状态） |
| `ohos_request_voice_call(uuid) -> c_int` | 发起语音通话请求（对端接受后开始麦克风采集 → opus → 发送；事件见 §3） |
| `ohos_close_voice_call(uuid) -> c_int` | 挂断语音通话 |
| `ohos_send_clipboard_image_png(uuid, png_base64) -> c_int` | 发送本机剪贴板图片（PNG 的 base64）到远端 |
| `ohos_session_get_option(uuid, key) -> *mut c_char` | 读会话级 peer 选项（`image_quality` / `codec-preference` / `custom-fps` / `custom_image_quality` 等），返回串需 `ohos_free_string` 释放 |
| `ohos_session_set_option(uuid, key, value) -> c_int` | 写会话级 peer 选项（`PeerConfig.options`，与 Flutter `sessionPeerOption` 同路径） |
| `ohos_save_image_quality(uuid, value) -> c_int` | 切换画质：`low` / `balanced` / `best`（同时向对端发 OptionMessage） |
| `ohos_save_custom_image_quality(uuid, quality) -> c_int` | 保存自定义码率（10..2000 百分比，画质随之置为 `custom`） |
| `ohos_set_custom_fps(uuid, fps) -> c_int` | 保存自定义帧率上限（5..120） |
| `ohos_toggle_option(uuid, name) -> c_int` | 切换会话开关选项（`view-only` / `show-remote-cursor` / `disable-clipboard` 等，含对端 OptionMessage） |
| `ohos_get_toggle_option(uuid, name) -> c_int` | 查询开关选项状态：1 开 / 0 关 / -1 会话不存在 |
| `ohos_update_supported_decodings(uuid) -> c_int` | 重新上报本端解码能力（修改 `codec-preference` 后必须调用，对端重协商编码器） |
| `ohos_alternative_codecs(uuid) -> *mut c_char` | 双端可用编解码 JSON `{"vp8":bool,"av1":bool,"h264":bool,"h265":bool}`（编解码菜单使能），需 `ohos_free_string` |

### 输入
| 函数 | 说明 |
|------|------|
| `ohos_send_mouse(uuid, mask, x, y, alt, ctrl, shift, command) -> c_int` | `mask = (buttons << 3) \| type`；type（低 3 位）：`MOVE=0, DOWN=1, UP=2, WHEEL=3, TRACKPAD=4`；buttons：`LEFT=0x01, RIGHT=0x02, WHEEL=0x04, BACK=0x08, FORWARD=0x10`（`src/common.rs::input`）。滚轮/触控板时 `x`/`y` 为滚动增量 |
| `ohos_input_key(uuid, name, down, press, alt, ctrl, shift, command) -> c_int` | `name` 为单字符或 CtrlKey 名（同 Flutter `CtrlKey` 枚举名） |
| `ohos_input_string(uuid, value) -> c_int` | 软键盘差量文本提交 |
| `ohos_session_refresh(uuid) -> c_int` | 请求重绘（全帧刷新） |
| `ohos_switch_display(uuid, display: c_int) -> c_int` | 切换显示器 |

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
| `voice_call_waiting` | — | 语音通话请求已发出，等待对端应答 |
| `voice_call_started` | — | 对端已接受，麦克风采集已开始 |
| `voice_call_closed` | — | 通话结束（任一方挂断/拒绝） |
| `record_status` | `start`（"true"/"false"） | 会话录制状态变化（录制实际开始/停止时） |
| `clipboard_image` | `png`（base64 的 PNG 字节） | 远端剪贴板图片（ImagePng 格式），写入系统剪贴板用 |
| `clipboard_html` | `html`（HTML 文本） | 远端剪贴板 HTML（MultiClipboards 的 Html 格式），与同消息的 Text 变体一起写入系统剪贴板 |

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
- `module.json5` 权限：`ohos.permission.INTERNET`、`ohos.permission.GET_NETWORK_INFO`、
  `ohos.permission.MICROPHONE`（语音通话，user_grant，首次通话时请求）。

## 5. Rust 侧能力说明（M2）

- 音频播放：OHAudio renderer（`libohaudio.so`，f32le / 48 kHz），opus 解码 → 重采样 →
  ring buffer → 渲染回调（`src/client/ohos_audio_player.rs`）。
- 硬件解码：OH_VideoDecoder 缓冲模式（`libnative_media_vdec.so`），H264/H265 →
  NV12 → libyuv 转 RGB（`libs/scrap/src/ohos/ohcodec.rs`）；AV1 仍为存桩门控（不可用）。
- 硬件编码（被控端）：OH_VideoEncoder 缓冲模式（`libnative_media_venc.so`），NV12 →
  Annex-B H264/H265（`OhCodecEncoder`，CBR，能力探测进程内缓存）；被控
  `supported_encoding` 据此上报 h264/h265，主控硬解随之自动启用。
- 语音通话：OHAudio capturer（MIC）→ opus → AudioFrame（`src/ohos_voice_call.rs`）；
  ArkTS 需先请求 `ohos.permission.MICROPHONE`。
- 会话录制：Recorder 链路（VP8/VP9 → webm；H264/H265 → mp4，需 hwcodec feature，
  ohos 当前仅 webm），保存到 `<home>/<AppName>/ScreenRecord`。
- 剪贴板：文本双向 + 远端→本机图片（`clipboard_image` 事件）+
  本机→远端图片（`ohos_send_clipboard_image_png`）。

## 6. 被控端 C ABI（M3，`src/ohos_ffi.rs`）

被控端 = RendezvousMediator + 连接服务器 + AVScreenCapture 采集，全部复用
`src/server.rs` / `src/server/*`（gating 与 Android 同组）。采集在 Rust 内部闭环
（`libs/scrap/src/ohos/screencap.rs`，OH_ORIGINAL_STREAM 模式，RGBA8888 视频 +
s16le 48k 音频经 FrameRaw 通道喂给 video/audio service），ArkTS 不参与推帧。

### 服务生命周期
| 函数 | 说明 |
|------|------|
| `ohos_server_start() -> c_int` | 启动被控服务（`crate::start_server(true)`，仅首次生效） |
| `ohos_server_start_service() -> c_int` | 清除 stop-service 标记并重启 mediator |
| `ohos_server_stop_service() -> c_int` | 置 stop-service 标记并重启 mediator |

### 凭据展示
| 函数 | 说明 |
|------|------|
| `ohos_get_server_id() -> *mut c_char` | 本机 ID（`Config::get_id`），需 free |
| `ohos_get_server_password() -> *mut c_char` | 临时密码，需 free |
| `ohos_update_server_password() -> c_int` | 重新生成临时密码 |
| `ohos_has_permanent_password() -> c_int` | 是否已设固定密码 |

### 连接管理（CM 通道）
Rust 侧 `ohos_ffi::connection_manager::start_channel` 与 Android 的
`flutter.rs::connection_manager::start_channel` 同构（in-process mpsc），
CM 事件经 event_cb 下发（见 §7）。

| 函数 | 说明 |
|------|------|
| `ohos_server_authorize(id, accept) -> c_int` | 应答连接请求（接受 / 拒绝即关闭） |
| `ohos_server_close_connection(id) -> c_int` | 断开已建立的入站连接 |
| `ohos_cm_handle_incoming_voice_call(id, accept) -> c_int` | 应答对端发来的语音通话（接受 / 拒绝） |
| `ohos_cm_close_voice_call(id) -> c_int` | 挂断进行中的语音通话 |
| `ohos_server_switch_permission(id, name, enabled) -> c_int` | 按连接切换权限 |
| `ohos_server_send_chat(id, text) -> c_int` | 被控端发消息给对端 |
| `ohos_server_push_clipboard(text, html) -> c_int` | 本机剪贴板推入被控通道（host 侧） |

### 采集控制
| 函数 | 说明 |
|------|------|
| `ohos_capture_start(w, h) -> c_int` | 启动 AVScreenCapture（弹系统授权 picker），w/h 为 display 尺寸；采集为混合模型：SetDataCallback 收音频/视频 buffer，回调未见视频时泵线程回退轮询 AcquireVideoBuffer（模拟器视频不可用时退避等待） |
| `ohos_capture_stop() -> c_int` | 停止并释放采集 |
| `ohos_capture_mic(enable) -> c_int` | 语音通话时开/关麦克风流 |
| `ohos_capture_running() -> c_int` | 采集是否运行 |

## 7. 被控端事件 JSON（event_cb 复用，`cm_*` / `server_*` 前缀）

| name | 字段 | 用途 |
|------|------|------|
| `cm_add_connection` | `client`（JSON 字符串：`{id, authorized, name, peer_id, is_file_transfer, keyboard, clipboard, audio, file, ...}`，snake_case） | 新入站连接（未授权时弹接受/拒绝） |
| `cm_remove_connection` | `id, close` | 连接移除 |
| `cm_chat` | `id, text` | 对端消息 |
| `cm_voice_call_state` | `client`（同 cm_add_connection，含 `incoming_voice_call` / `in_voice_call`） | 语音通话状态（来电时 SharePage 卡片弹接受/拒绝，接听前先请求麦克风权限） |
| `cm_file_transfer_log` | action, log | 传输日志 |
| `cm_clipboard_write` | `text, html, png`（base64 PNG） | 远端剪贴板 → 本机 pasteboard |
| `server_capture_state` | `state` | 采集状态：`pending` 请求已发出；`0` 已开始；`1` 用户取消；`2/3/4/10` 被系统/用户/来电/切换停止 |

ArkTS 侧：`pages/SharePage.ets`（ID/密码展示、启停、授权弹窗、连接列表）+
`EntryAbility.ets`（`cm_clipboard_write` → pasteboard，含 PixelMap 图片）。
长时任务：`backgroundTaskManager.startBackgroundRunning(DATA_TRANSFER)`，
module.json5 `backgroundModes: ["dataTransfer"]`。
