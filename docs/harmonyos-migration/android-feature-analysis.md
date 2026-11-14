# RustDesk Android 端功能分析 —— 鸿蒙（HarmonyOS）迁移指南

> 源码版本：rustdesk 1.5.1（Rust 核心 + Flutter UI + Android 原生 Kotlin 三层架构）
> 分析范围：`flutter/android/`（原生层）、`src/` + `libs/scrap/`（Rust 层 Android 分支）、`flutter/lib/`（Dart 层 Android 逻辑）

---

## 一、项目架构总览

```
┌─────────────────────────────────────────────┐
│  Flutter/Dart UI 层 (flutter/lib/)           │  平台无关（少量 isAndroid 分支）
├─────────────────────────────────────────────┤
│  flutter_rust_bridge / JNI 双向桥             │  Android: libs/scrap/src/android/ffi.rs (JNI)
├─────────────────────────────────────────────┤
│  Rust 核心 (src/)                            │  ~90% 平台无关，Android 分支多为"排除桌面功能"
│  协议栈/编解码/会话/文件传输/配置               │
├─────────────────────────────────────────────┤
│  Android 原生层 (Kotlin, 4238 行)             │  系统能力代理：采集/输入注入/剪贴板/通知/保活
│  MainService / InputService / AudioRecord…  │  ← 迁移鸿蒙时的主要重写对象
└─────────────────────────────────────────────┘
```

Android 端的设计模式是 **"系统服务代理"**：Rust 核心把所有 OS 能力外包给 Kotlin 前台服务，
通过 6 个 Rust→Java 回调 + 9 个 Java→Rust 入口交互（详见第三章）。
**鸿蒙移植的核心工作 = 用 ArkTS/鸿蒙 API 重写这套"系统服务代理"语义，Rust 核心几乎原样复用。**

---

## 二、Android 端功能清单（按模块）

### 2.1 被控端（Share Screen 服务）—— `MainService.kt` (1003 行)

| # | 功能 | 实现方式 | 鸿蒙对应 API |
|---|------|---------|-------------|
| 1 | **屏幕视频采集** | MediaProjection + VirtualDisplay + ImageReader(RGBA_8888)，帧通过 `FFI.onVideoFrameUpdate(ByteBuffer)` 零拷贝送 Rust（MainService.kt:390-420, 709-759） | `@ohos.multimedia.media` 的 **AVScreenCapture**（需要用户授权弹窗，同样有前台服务要求）；或 Root/系统权限下 `AVScreenCapturePicker`；帧送 Rust 方式不变（NAPI 传 ArrayBuffer） |
| 2 | **半分辨率缩放** | 屏幕超过 1200px 时可选 scale=2（half_scale），由 Rust `rustSetByName("half_scale")` 通知（MainService.kt:173-180, 308-315） | 采集侧自己做（AVScreenCapture 支持设定分辨率），逻辑可复用 |
| 3 | **横竖屏/DPI 变化响应** | `onConfigurationChanged` → `updateScreenInfo` → 通知 Rust `refreshScreen` → 编码器重建（MainService.kt:277-331, 367-370） | 监听 `@ohos.app.ability.ConfigurationConstant` / display 变化事件，同逻辑 |
| 4 | **音频采集（被控播放声）** | AudioRecord + AudioPlaybackCaptureConfiguration（捕获媒体/闹钟/游戏音），PCM Float 48kHz 立体声 → `FFI.onAudioFrameUpdate`（AudioRecordHandle.kt:57-101） | `@ohos.multimedia.media` 的 **AVScreenCapture.captureInfo() 音频回调**（`AVScreenCaptureOptions` 含音频）或 AudioCapturer；API 12+ 支持内部录音 |
| 5 | **语音通话（麦克风双向）** | AudioRecord 声源切换 VOICE_COMMUNICATION ↔ 播放捕获；通话时前台服务切 microphone 类型（MainService.kt:532-566, AudioRecordHandle.kt:193-266） | `@ohos.multimedia.audio` **AudioCapturer** + 声源 AUDIO_SOURCE_VOICE_COMMUNICATION，AudioRenderer 播放 |
| 6 | **前台服务 + 常驻通知** | startForeground(mediaProjection/microphone/specialUse 三类 FGS type)；通知栏显示服务状态（MainService.kt:816-920） | `@ohos.app.ability` **ContinuousTask** 长时任务（DATA_TRANSFER/音频采集等 backgroundMode）+ 通知由 RustDesk 核心已有 notification 接口（需适配） |
| 7 | **连接请求通知** | 远端连接到达 → `rustSetByName("add_connection")` → 通知栏弹 "Do you accept?"，直接授权时开始采集（MainService.kt:115-141, 922-954） | HarmonyOS **emitter/NotificationKit**；授权弹窗也可走 UI 层（当前 Android 逻辑是通知即授权入口，鸿蒙可用系统弹窗替代） |
| 8 | **亮屏/唤醒** | 远端鼠标按下时若息屏 → WakeLock.acquire(5000) 亮屏（MainService.kt:68-89） | `@ohos.multimedia.audio` 不可；用 **keep screen on**（`window.setWindowKeepScreenOn`）+ `@ohos.power` PowerManager（亮屏 API 系统应用限定，需评估降级策略） |
| 9 | **开机自启** | BootReceiver 监听 BOOT_COMPLETED，检查"开机自启"开关 + 电池白名单 + 悬浮窗权限后启动 MainService（BootReceiver.kt:16-47） | `app.json5` 无对应；HarmonyOS 用 **SystemAbility 事件**仅系统应用可开机自启。普通应用建议降级：`@ohos.multimedia.ohos` 不可用 → 使用常驻通知 + 延迟任务（WorkScheduler）模拟，或引导用户手动启动 |
| 10 | **UI 划掉后台清理** | onTaskRemoved/onDestroy(isFinishing) → `FFI.closeAllSessions()` 关闭出站会话（MainService.kt:271-275, MainActivity.kt:245-261） | HarmonyOS 应用切后台不销毁 UI（无 onTaskRemoved 等价语义），按应用生命周期回调对应处理 |

### 2.2 输入注入（远端控制本机）—— `InputService.kt` (808 行)

| # | 功能 | 实现方式 | 鸿蒙对应 API |
|---|------|---------|-------------|
| 11 | **无障碍服务承接远端输入** | AccessibilityService（`accessibility_service_config.xml`: canPerformGestures + canRetrieveWindowContent），用户手动开启 | ⚠️ **鸿蒙无公开无障碍手势注入 API**。替代路径：① ArkTS `@ohos.multimodalInput.inputEventClient`（注入事件，需 `ohos.permission.INJECT_EVENT`，**仅系统应用/签名授权**）；② UIAbility 内通过 AXE/自动化测试通道；③ `inputEventClient.injectEvent` 系统签名方案。**这是被控端最大风险点，需提前验证权限可行性** |
| 12 | **鼠标 → 手势转换** | LEFT_DOWN/MOVE/UP → `dispatchGesture` 连续 stroke（continueStroke 拼接长路径）；RIGHT_UP → 长按；BACK_UP → GLOBAL_ACTION_BACK；中键长按 → 最近任务、短按 → HOME；滚轮 → 模拟滑动（InputService.kt:121-270, 351-441） | 若可拿到注入权限：`inputEventClient.injectEvent` 支持鼠标/触摸/按键序列，路径逻辑（Path/Stroke/时长）可翻译；GLOBAL_ACTION_BACK/HOME/RECENTS → 鸿蒙无公开全局 action，用注入 BACK 键值替代 |
| 13 | **触摸协议（pad 控制手机）** | TOUCH_PAN_START/UPDATE/END（mask 4/5/6）→ 同手势分发，坐标 ×scale（InputService.kt:273-294） | 同上，`inputEventClient` 触摸事件注入 |
| 14 | **键盘输入注入** | KeyEvent protobuf → Android KeyEvent 转换（KeyboardKeyEventMapper.kt:7-122）：ControlKey 枚举映射 KEYCODE_*、Unicode→KeyCharacterMap、修饰键 meta state；API33+ 走 InputMethod.commitText，低版本用 AccessibilityNodeInfo ACTION_SET_TEXT + 伪 EditText 计算文本状态（InputService.kt:443-517, 653-768） | `inputEventClient.injectEvent` 的 KeyEvent 注入（含 Ctrl/Alt/Shift 修饰键）；ControlKey→鸿蒙键值映射表需重写（参考 `@ohos.multimodalInput.keyEvent` 键值常量）；文本输入优先走 RustDesk 已有的 soft-keyboard → seq 文本提交路径（省去无障碍 Node hack） |
| 15 | **音量/电源键远控** | 音量键 → VolumeController 调 AudioManager；电源键 ACTION_UP → GLOBAL_ACTION_POWER_DIALOG（InputService.kt:519-554, VolumeController.kt） | `@ohos.multimedia.audio` AudioVolumeManager（系统接口）；电源对话框无公开 API → 可注入 POWER 键值（系统签名）或降级不做 |
| 16 | **输入服务开关状态上报** | `onServiceConnected/onUnbind` → Flutter `on_state_changed(input)`（InputService.kt:774-805） | UI 状态机不变，改由鸿蒙侧注入通道状态回调 |

### 2.3 剪贴板双向同步 —— `RdClipboardManager.kt` (197 行)

| # | 功能 | 实现方式 | 鸿蒙对应 API |
|---|------|---------|-------------|
| 17 | **本机剪贴板 → 远端** | ClipboardManager 监听/手动 sync；支持 Text + HTML；去重（lastUpdatedClipData）；`FFI.onClipboardUpdate`（首字节 host/client 标志）（RdClipboardManager.kt:44-96） | `@ohos.pasteboard`（系统剪贴板；Text/HTML/PixelMap 均支持）；无 change 监听 → 保留"手动 sync + 退到前台时 sync"策略（RemotePage 已有 `try_sync_clipboard`，App 生命周期切换时机照搬） |
| 18 | **远端剪贴板 → 本机** | Rust 回调 `rustUpdateClipboard` → setPrimaryClip（RdClipboardManager.kt:151-196） | `pasteboard.setData()`，直接替换 |
| 19 | **客户/被控方向控制** | `rustEnableClientClipboard(bool)` 开关 + `isServiceClipboardEnabled` 查询（RdClipboardManager.kt:136-141） | 逻辑不变 |

### 2.4 悬浮窗（保活 + 快捷入口）—— `FloatingWindowService.kt` (394 行)

| # | 功能 | 实现方式 | 鸿蒙对应 API |
|---|------|---------|-------------|
| 20 | **半隐藏悬浮球** | TYPE_APPLICATION_OVERLAY 悬浮窗：可拖动、贴边半隐（左右半图切换）、可调大小(32-320)/透明度(0-10)/不可触摸/自定义 SVG（FloatingWindowService.kt:91-301） | `@ohos.window` **SystemWindow.subWindow / systemWindow**（`createWindow(WINDOW_TYPE_SYSTEM_ALERT)`），`window.on('touchOutside')`+moveTo；SVG 渲染用 ArkUI Image 组件。需 `ohos.permission.SYSTEM_FLOAT_WINDOW`（应用市场签名可申请） |
| 21 | **悬浮窗菜单** | 点击弹 PopupMenu：显示主界面 / 同步剪贴板 / 停止服务（FloatingWindowService.kt:303-339） | ArkUI Menu/Popup 组件直接实现 |
| 22 | **保活说明** | 注：Android 端悬浮窗的重要目的是"空窗保活"，鸿蒙长时任务机制不同（ContinuousTask），保活依赖改为长时任务而非悬浮窗 | — |

### 2.5 文件传输（SAF 沙箱模型）—— `MainActivity.kt` (1021 行)

| # | 功能 | 实现方式 | 鸿蒙对应 API |
|---|------|---------|-------------|
| 23 | **App 沙箱路径管理** | `getExternalFilesDir` 为 home；`is_peer_path_allowed()`（Rust `src/common.rs:252-283`）限制对端只能访问 home 内路径 + canonicalize 防逃逸 | `getContext().filesDir` / `distributedFilesDir`；Rust 侧路径校验逻辑原样复用，只换 home 路径来源 |
| 24 | **导入文件/文件夹** | ACTION_OPEN_DOCUMENT picker → content URI → 拷入沙箱（临时文件+原子重命名+备份回滚）（MainActivity.kt:408-614） | `@ohos.file.picker` **DocumentPicker**（DocumentSelectOptions/Bundle）→ `@ohos.file.fs` 复制；沙箱内可完整复用原子写入策略 |
| 25 | **导出文件/文件夹** | ACTION_CREATE_DOCUMENT / OPEN_DOCUMENT_TREE → SAF 写入（含目录递归）（MainActivity.kt:615-871） | DocumentPicker(DocumentSaveOptions) + DocumentViewPicker（文件夹选择）→ fs 写入 |
| 26 | **导出日志** | 文件管理页 Export Logs → export_files（file_manager_page.dart:204-229） | 同上 |

### 2.6 其他系统能力

| # | 功能 | 实现方式 | 鸿蒙对应 API |
|---|------|---------|-------------|
| 27 | **权限管理** | XXPermissions：通知(33+)、RECORD_AUDIO、悬浮窗、电池白名单；`requestPermission`→`on_android_permission_result` 回调（common.kt:82-95） | `@ohos.abilityAccessCtrl` **requestPermissionsFromUser**（麦克风 ohos.permission.MICROPHONE、通知等），回调改 ability 上下文实现 |
| 28 | **硬件编解码能力上报** | MediaCodecList 枚举 H264/H265 硬编解码器（名称/尺寸范围/surface/NV12/低延迟/码率）→ `FFI.setCodecInfo(JSON)`，Rust hwcodec 据此选编码器（MainActivity.kt:890-967） | `@ohos.multimedia.media` **AVCodecList**（getSupportedCaps）：同样能查 H264/H265 硬编解码器能力，JSON 结构照搬喂给 Rust |
| 29 | **WakeLock（主控端会话保活）** | Rust `android_wakelock` crate JNI PowerManager（`src/platform/mod.rs:119-142`）；Flutter WakelockPlus 会话期间亮屏（remote_page.dart:135） | ① Rust 侧改调 NAPI 封装 `window.setWindowKeepScreenOn`；② 或砍掉 Rust WakeLock，全部走 ArkUI 层（推荐，简单） |
| 30 | **Deep link** | `rustdesk://` scheme（AndroidManifest.xml:77-83），`--connect/--file-transfer` 参数（common.dart:2484-2507） | `module.json5` **uris/scheme + wantConstant**；UIWant 解析逻辑同构 |
| 31 | **多语言** | `FFI.translateLocale` → Rust lang 模块（common.kt:165-168），通知栏文案用 | NAPI 同名接口直接复用 |
| 32 | **设备信息** | brand-model 作设备名、SDK_INT 作 androidVersion（native_model.dart:194-198） | `@ohos.deviceInfo`（deviceBrand/productModel）+ `@ohos.ohosParams` API 版本 |
| 33 | **Rusttls 系统证书校验** | rustls-platform-verifier-android（build.gradle:139） | 鸿蒙 Rust 生态：用 rustls-native-certs 类方案读取 OHOS 系统证书库，或先退化为 webpki-roots |
| 34 | **日志** | android_logger → logcat（Cargo.toml:211-214） | 改 hilog（NAPI 封装）或 stdout 重定向 |
| 35 | **电池白名单** | REQUEST_IGNORE_BATTERY_OPTIMIZATIONS（settings 页"Keep RustDesk background service"） | 鸿蒙无此概念 → 用 ContinuousTask + 应用关联唤醒；设置项可隐藏或改为引导用户"后台弹窗/关联启动"系统设置 |

### 2.7 客户端（主控端）功能 —— Dart 层为主，平台分支少

| # | 功能 | 说明 | 鸿蒙迁移 |
|---|------|------|---------|
| 36 | 远程控制/文件传输/终端/查看摄像头/语音通话 | 会话协议在 Rust（`client/io_loop.rs`、`fs` 协议），平台无关 | **直接复用** |
| 37 | 触控手势模式 | 对端为 Android 时强制 touch mode；长按右键、双指缩放、三指滚轮（对端 Android 禁用三指滚轮）（model.dart:1392, remote_input.dart:198-353, 520-531） | Dart 手势逻辑不变；对端平台判断需加 HarmonyOS 分支（`isPeerAndroid` → `isPeerMobile`），鸿蒙被控同样应强制 touch mode |
| 38 | 软键盘输入 | `enable_soft_keyboard` + 隐藏 TextFormField 差量发送 `sessionInputString`（remote_page.dart:370-480） | ArkUI TextInput 等价实现；Flutter-ohos 保留原实现 |
| 39 | 横竖屏适配/沉浸模式 | OrientationBuilder + SystemChrome（remote_page.dart:130-131, 553-563） | 鸿蒙方向事件 + window API |
| 40 | 硬件解码（主控端） | MediaCodec 硬解 H264/H265（scrap/src/common/mediacodec.rs） | **Android 专有**：需接鸿蒙 AVCodec（OH_VideoDecoder）或 FFmpeg 软解过渡；这是除输入注入外第二个大工程 |
| 41 | 局域网发现 | 广播 ping（lan.rs），平台无关 | 直接复用 |
| 42 | 中继/打洞/ID 注册 | rendezvous 协议（rendezvous_mediator.rs），平台无关 | 直接复用 |
| 43 | 连接录制 | 入/出站会话录制到 MP4（video_qos/recording） | 编码侧平台无关部分复用；写文件走沙箱 |
| 44 | UI 多窗口 | Android 单窗口 push 页面（无多窗口） | 鸿蒙同样单 Ability 窗口，直接照搬移动端导航结构 |

---

## 三、Rust ↔ 原生层 FFI 边界（迁移对接契约）

这是鸿蒙 ArkTS/Native 层必须实现的**全部**接口（Android 上由 JNI 承载，鸿蒙改为 NAPI）：

### Java/Kotlin → Rust（9 个入口，鸿蒙侧主动调用）

| 接口 | Android 定义 | 作用 |
|------|-------------|------|
| `init(ctx)` | ffi.kt:15 / scrap android ffi.rs:192 | 注册上下文（鸿蒙：NAPI env + 能力对象） |
| `startServer(app_dir, home_dir, custom_client_config)` | ffi.kt:18 / flutter_ffi.rs:3011 | 启动被控服务（config 路径 + home 沙箱） |
| `startService()` | ffi.kt:19 | 恢复服务（清除 stop-service 选项） |
| `onVideoFrameUpdate(ByteBuffer)` | ffi.kt:20 / ffi.rs:124 | 视频帧零拷贝送 Rust（存 VIDEO_RAW） |
| `onAudioFrameUpdate(ByteBuffer)` | ffi.kt:21 / ffi.rs:138 | 音频帧送 Rust（存 AUDIO_RAW） |
| `onClipboardUpdate(ByteBuffer)` | ffi.kt:29 / ffi.rs:152 | 剪贴板变化（首字节 host=0/client=1 + MultiClipboards protobuf） |
| `setFrameRawEnable(name, bool)` | ffi.kt:25 / ffi.rs:173 | 开/关 video/audio 帧通道 |
| `setCodecInfo(json)` | ffi.kt:26 / ffi.rs:257 | 硬编解码能力 JSON |
| `setClipboardManager(obj)` | ffi.kt:17 / ffi.rs:210 | 注册剪贴板回调对象 |

### Rust → Java（6 个回调，鸿蒙侧提供实现）

| 回调 | Android 实现 | 作用 |
|------|-------------|------|
| `rustPointerInput(kind, mask, x, y)` | MainService.kt:68 | kind 0=触摸/1=鼠标；息屏时先亮屏 |
| `rustKeyEventInput(bytes)` | MainService.kt:93 | KeyEvent protobuf → 键盘注入 |
| `rustGetByName(name)` | MainService.kt:98 | 查询 screen_size/is_start |
| `rustSetByName(name, arg1, arg2)` | MainService.kt:115 | add_connection（通知授权）/ update_voice_call_state / stop_capture / half_scale |
| `rustUpdateClipboard(bytes)` | RdClipboardManager.kt:152 | 远端剪贴板写入系统 |
| `rustEnableClientClipboard(bool)` | RdClipboardManager.kt:137 | 客户端剪贴板同步开关 |

> 协议中剪贴板使用 `MultiClipboards`/`Clipboard` protobuf（Text/Html/ImageRgba/ImagePng 格式，`libs/base/protos/message.proto`），鸿蒙侧解析逻辑可逐行翻译。

---

## 四、迁移策略建议

### 4.1 分层复用度评估

| 层 | 复用度 | 说明 |
|----|-------|------|
| Rust 协议/会话/文件传输/编解码内核 | **~95% 直接复用** | 交叉编译到 `aarch64-unknown-linux-ohos`（OHOS NDK 工具链），替换 jni→**napi-rs**、android_logger→**hilog**、rustls 平台证书、ndk/OpenSLES 依赖 |
| Dart/Flutter UI 层 | **~85%** | 依赖 flutter-ohos（OpenHarmony Flutter 分支）渲染；Platform.isAndroid 分支需系统性改为"能力探测"式判断；MethodChannel("mChannel") 的原生实现全部重写为 ArkTS |
| Kotlin 原生层（4238 行） | **0%（全部重写为 ArkTS + 少量 C++）** | 见 4.2 |

### 4.2 Kotlin → ArkTS 重写清单（按优先级与风险排序）

| 优先级 | 模块 | 鸿蒙实现 | 风险 |
|-------|------|---------|------|
| P0 | 屏幕采集（MainService 采集部分） | AVScreenCapture（需用户体验弹窗授权，同 Android 类似）+ 前台 ContinuousTask | 中：API 12+ 才支持内部音频；权限弹窗流程要验证 |
| P0 | 剪贴板（RdClipboardManager） | @ohos.pasteboard | 低：直译即可 |
| P0 | 文件选择器/沙箱（MainActivity 文件部分） | @ohos.file.picker + fs | 低 |
| P0 | 权限申请 | abilityAccessCtrl | 低 |
| P1 | 前台服务/通知/常驻 | ContinuousTask + NotificationKit | 中：保活语义差异需产品层面确认 |
| P1 | 硬编解码能力上报 + 硬解（客户端） | AVCodecList + OH_AVCodec（FFI 层 C 接口，可直接被 Rust 调） | 中：AVCodec 是 C API，Rust 绑定工作量可控 |
| P2 | 悬浮窗（FloatingWindowService） | systemWindow API | 中：SYSTEM_FLOAT_WINDOW 权限 |
| **P0-风险** | **输入注入（InputService，808 行）** | inputEventClient.injectEvent（需系统签名 INJECT_EVENT 权限） | **高：普通三方应用拿不到注入权限。替代方案：① 申请受限开放权限（需华为审批）；② 首版降级为"仅观看"模式；③ 键盘走输入法通道（InputMethodAbility）部分替代** |
| P3 | 开机自启 | 三方应用无开机广播 → 降级（引导手动/WorkScheduler） | 高（平台限制，建议首版砍掉） |
| P3 | 电池白名单/息屏唤醒 | 无对等 API → 砍掉或降级 | 中 |

### 4.3 建议的迁移里程碑

1. **M1 主控端先行**（纯客户端，不碰系统权限）：Rust 核心交叉编译 + flutter-ohos 跑通 UI → 远程控制观看、软编 VP9 + 软解、文件传输、剪贴板（客户端方向）。风险最低、最快见效。
2. **M2 被控端基础**：AVScreenCapture 采集 + pasteboard + ContinuousTask + 通知授权流 → Android 版"Share Screen"等价体验（无输入控制）。
3. **M3 输入控制**：解决注入权限（签名/开放权限申请），实现鼠标/触摸/键盘映射（InputService 等价物）。
4. **M4 体验补全**：硬编解码（AVCodec）、语音通话（AudioCapturer/Rrenderer）、悬浮窗、多语言、Deep link、录制。
5. **M5 差异化**：开机自启/保活策略按鸿蒙规范重新设计（长时任务 + 通知常驻）。

### 4.4 关键依赖替换表（Rust 侧）

| Android 依赖 | 用途 | 鸿蒙替代 |
|-------------|------|---------|
| jni 0.21 crate | JNI 桥 | **napi-rs**（或 ohos napi 手写绑定） |
| android_logger | 日志 | hilog（NAPI 封装或 ohos-rs/hilog crate） |
| android-wakelock | 亮屏保活 | ArkUI window.setWindowKeepScreenOn（上移 UI 层） |
| rustls-platform-verifier-android | 系统证书 | 读 OHOS 证书库的自定义 verifier；过渡期用 webpki-roots |
| ndk / OpenSLES / ndk_compat（build.rs:60-92 链接） | 音频输出 | **OHAudio**（C API，可被 Rust 直接链接调用） |
| scrap android 模块（Capture/Display） | 采集适配 | 新增 `scrap/src/common/ohos.rs`：从 NAPI 回调帧缓冲读取 |
| flutter_rust_bridge | Dart↔Rust | 不变（生成物平台无关） |

---

## 五、Android 专有功能"去留"决策速查

| 功能 | 建议动作 |
|------|---------|
| 屏幕采集/音频采集 | ✅ 保留（AVScreenCapture） |
| 剪贴板双向 | ✅ 保留（pasteboard；无监听 → 手动+前台时机同步） |
| 文件传输（SAF 沙箱） | ✅ 保留（file picker + fs，模型同构） |
| 悬浮窗 | ✅ 保留（systemWindow；保活职责移交长时任务） |
| 开机自启 | ⚠️ 降级/砍掉（三方应用限制） |
| 电池优化白名单 | ⚠️ 砍掉（无对等概念） |
| 输入注入（无障碍） | ⚠️ P0 风险项，需权限方案验证；软键盘文本注入可走输入法 |
| 音量/电源键远控 | ⚠️ 随输入注入一起评估 |
| 硬件编解码 | ✅ 保留（AVCodec/AVCodecList，C API 对 Rust 友好） |
| 通知授权/连接请求通知 | ✅ 保留（NotificationKit + emitter） |
| Deep link | ✅ 保留（module.json5 uris） |
| 多语言/设备信息/证书 | ✅ 保留（等价 API） |
