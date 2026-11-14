# M1 主控端迁移实施记录（Android → HarmonyOS）

> 里程碑对应 `android-feature-analysis.md` §4.3 的 **M1 主控端先行**：
> 纯客户端、不碰系统权限——远程观看（软解）、触控/键盘输入、文件传输、剪贴板（客户端方向）。
>
> **M3 被控端（2026-10-27）**：Rust 侧启用 server + rendezvous_mediator
> （gating 与 Android 同组），scrap 新增 AVScreenCapture 原生采集源
> （OH_ORIGINAL_STREAM，RGBA + s16le PCM → FrameRaw 通道）；CM 连接授权、
> 被控剪贴板（pasteboard 双向）、文件沙箱（is_peer_path_allowed）打通；
> ArkTS 新增 SharePage（ID/密码/启停/授权弹窗/连接列表）+ DATA_TRANSFER 长时任务。
> 输入注入未做（需系统权限 INJECT_EVENT，被控端 keyboard 权限恒为 false，
> 远控视角为只读）。详见 `ohos-controller-ffi.md` §6/§7 与下文 M3 小节。
>
> **状态：构建链路全部打通**（2026-10-24）。
> `cargo check --target aarch64-unknown-linux-ohos` 0 error；
> `librustdesk.so`（aarch64 ELF，~16MB）产出；
> `hvigor assembleHap` 成功，HAP 内含 `libentry.so`（NAPI 壳）+ `librustdesk.so`（Rust 核心）。
>
> **M2 增量（2026-10-26）**：音频播放（OHAudio renderer）、硬件解码（OH_VideoDecoder,
> H264/H265，AV1 保持存桩门控）、语音通话（OHAudio capturer → opus）、会话录制、
> 摄像头查看（VIEW_CAMERA 连接模式）、图片剪贴板（双向）。详见
> `ohos-controller-ffi.md` §2/§3/§5 与下文 M2 小节。
>
> **UI 改版（2026-10-26，参照向日葵 UX，见 `sunlogin-controller-reference.md`）**：
> RemotePage 远控界面在功能不减的前提下重排：
> ① 顶部状态改为「绿点 + 对端 ID + 已连接 ›」胶囊（原信号条+盾牌双碎片），点开展开
>   详情卡（延迟/速度/帧率三列 StatCell + 权限行）；
> ② 原浮动球（4 圆钮）+ 文字菜单 drawer 合并为一个**底部图标工具面板**（SymbolGlyph
>   图标 + 文字，深色 rgba(28,32,40,0.96)，两行 4+4 项 + 分隔行：键盘/模式/切屏/
>   显示设置、剪贴板/语音/录制/快捷操作、刷新/只读/断开），呼出钮为白色圆钮
>   `<>`（可拖动），点面板外关闭；「远端光标」开关并入显示设置面板（含与指针模式互斥）；
> ③ 主色统一为向日葵蓝 `#1A6AF5`（原 #3A7BD5）；键盘输入条深色化与工具面板一致；
> ④ 字符串资源清理 9 个 drawer 专用键（remote_menu/menu_* 等）。
> 纯 ArkTS 层改动，Rust/FFI 契约不变。DevEco 检查 0 error。

## 一、技术路线

机器上没有 Flutter SDK / flutter-ohos 分支，M1 分析文档中「flutter-ohos 跑通 UI」的
路线不具备落地条件。实际采用 **原生 ArkTS 应用 + Rust 核心（C ABI / NAPI）**：

```
┌──────────────────────────────────────────────┐
│ ArkTS UI（harmonyos/entry）                   │ Index / RemotePage / FileTransferPage
├──────────────────────────────────────────────┤
│ NAPI 模块（entry/src/main/cpp，C++）          │ dlopen librustdesk.so；事件→TSF 转 JS 线程；
│                                              │ RGBA 帧 → OH_NativeWindow 直渲（XComponent surface）
├──────────────────────────────────────────────┤
│ librustdesk.so（Rust 核心，aarch64-ohos）     │ src/ohos_ffi.rs：C ABI + OhosHandler(InvokeUiSession)
│                                              │ 复用 Session<T> / io_loop / FileManager / 编解码
└──────────────────────────────────────────────┘
```

关键决策：

1. **不复用 flutter_ffi / flutter_rust_bridge**：FRB 生成物（bridge_generated.rs）是
   gitignored 的 CI 产物，本机无法重新生成；`Session<T>` 与 `InvokeUiSession` 本身
   平台无关，`src/ohos_ffi.rs` 自带一个 `InvokeUiSession` 实现（事件 JSON 与
   `FlutterHandler::push_event` 同构）+ 会话注册表，零改动复用会话引擎。
2. **OHOS 目标语义 = `target_env = "ohos"`**（rustc 对 `aarch64-unknown-linux-ohos`
   报 `target_os="linux"` + `target_env="ohos"`）。门控策略：模块级按 **iOS 模板**
   （无 server/被控服务），会话层按 **Android 模板**（mobile_wheel、DEVICE_NAME、
   Config 直读 rendezvous）。
3. **ArkTS↔Rust 全量对齐的接口契约**见 `ohos-controller-ffi.md`（21 个 C ABI 函数 +
   事件 JSON schema）；注意 `ohos_version` 返回 `*mut c_char`，需 `ohos_free_string`。

## 二、代码改动清单（Rust 侧）

新增：

| 文件 | 内容 |
|------|------|
| `src/ohos_ffi.rs` | `OhosHandler: InvokeUiSession`（事件 JSON / RGBA / 剪贴板回调）、会话注册表、21 个 `#[no_mangle]` C ABI |
| `src/platform/ohos.rs` | OHOS 平台桩（WakeLock 无操作、PA_SAMPLE_RATE 等，同 Android 客户端集合） |
| `harmonyos/`（工程） | ArkTS 应用 + NAPI + 构建脚本（详见 `harmonyos/README.md`） |
| `libs/scrap/src/ohos/aom_stub.c` | libaom 缺席时的符号存根（AV1 调用运行时返回错误码，协商自动回退 VP8/VP9） |
| `libs/webrtc-util/`（vendored patch） | rustdesk fork 的 util crate 副本；nix 收窄为 `default-features=false, features=["net","socket"]`（ohos libc 无 mqueue/aio/personality 绑定，而 webrtc-util 只用 ifaddrs/socket） |
| `libs/rdev/`（vendored patch） | rdev 副本；ohos 不编译 linux(X11) 后端，与 android 一样只用核心类型 + 键码表 |
| `libs/machine-uid/`（vendored patch） | machine-uid 副本；① build.rs 按 TARGET 而非 HOST 门控 win.cpp；② IsWow64Process 改纯 Rust extern（新 cargo 不把 build-dep 的静态库传播进 build 脚本链接，win.cpp 方案在 Windows 宿主交叉编译时必挂） |
| `libs/libsodium-sys/`（vendored patch） | 增加 target 作用域的 `SODIUM_LIB_DIR_<target>` 查找（Windows 宿主交叉编译时 aarch64 libsodium 不得泄漏进宿主 build 脚本链接）。副本**不带** libsodium/ 源码与 msvc/mingw 预编译库：非 ohos 宿主经 vcpkg（windows）/pkg-config（unix）取 sodium，见 CI 增加的安装步骤 |

修改（全部为 `target_env = "ohos"` 门控，不改变既有平台行为）：

| 文件 | 改动 |
|------|------|
| `src/lib.rs` | server/tray/port_forward/whiteboard/updater/clipboard/core_main/ui/rendezvous_mediator/clipboard_file 在 ohos 下不编译；注册 `ohos_ffi` |
| `src/platform/mod.rs` | linux 模块在 ohos 下不编译；`get_wakelock` OHOS 分支（ArkTS 层负责亮屏） |
| `Cargo.toml` | 桌面依赖段（sciter/enigo/clipboard/gtk/pulse/tray-icon/tao/wallpaper/keepawake/rdev 等）排除 ohos；rdev/machine-uid/webrtc-util/libsodium-sys 四个 [patch] |
| `build.rs` | `build_windows()` 按 CARGO_CFG_TARGET_OS 而非宿主门控（Windows 宿主交叉编译必挂点） |
| `libs/scrap/build.rs` | OHOS 分支：`OHOS_DEPS_DIR` 预编译库 + bindgen（vpx/yuv），aom 换存根库，无 capture cfg |
| `libs/scrap/Cargo.toml` | gstreamer/wayland 可选依赖段、nokhwa 排除 ohos；build-dependencies 增加 `cc` |
| `libs/scrap/src/common/{mod,codec,convert}.rs` | camera/aom/AV1/Frame(PixelBuffer)/convert_to_yuv 按 ohos 门控（能力通告 ability_av1=0） |
| `libs/hbb_common`（子模块，需上游 PR） | ① Cargo.toml：x11 依赖段排除 ohos；② src/lib.rs：`pub use x11` 排除 ohos |
| `src/client.rs` | `AudioHandler::start_audio` OHOS 分支（明确报不支持）+ 播放流字段门控 + 解码帧丢弃；clipboard 相关 8 处桌面分支排除 ohos |
| `src/client/io_loop.rs` | 剪贴板同步走 iOS 分支（handler.clipboard 回调）；voice_call 走 iOS 分支；桌面 grab/clipboard 导入门控 |
| `src/ui_interface.rs` / `ui_cm_interface.rs` / `ui_session_interface.rs` / `common.rs` / `keyboard.rs` / `keyboard/shortcuts.rs` / `auth_2fa.rs` / `privacy_mode.rs` / `hbbs_http/sync.rs` / `lang.rs` / `lan.rs` / `client/screenshot.rs` | 桌面/移动成对分支补 ohos（约 85 处 cfg，编译器枚举驱动） |

## 三、交叉编译工具链要点（Windows 宿主踩坑记录）

| 问题 | 解法 |
|------|------|
| 路径带空格（`D:/Huawei/DevEco Studio`）破坏 RUSTFLAGS/CFLAGS（按空白切分） | `~/.rustdesk-ohos/ndk`、`sdk` 建 junction |
| msys 风格 `/c/...` 路径 Windows clang.exe 不识别 | 脚本内 `cygpath -m` 统一转 `C:/` 风格 |
| `CARGO_TARGET_<triple>_LINKER` 必须全大写，小写被忽略 | `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER` |
| 链接报 `-latomic` 找不到（OHOS sysroot 无 libatomic） | 移除（aarch64 原生原子操作） |
| 新版 cargo（1.96）不把 build-dep 的 `cargo:rustc-link-lib` 传播进 build 脚本链接 | machine-uid 改纯 Rust IsWow64Process（弃 win.cpp 静态库） |
| libsodium-sys 的 `cfg!(target_env="msvc")` 在 build.rs 里判的是宿主 | vendored patch：`SODIUM_LIB_DIR_<target>` 目标作用域 + 双名库文件（普通 `SODIUM_LIB_DIR` 会同时命中宿主构建脚本，已实测不可行） |
| vendored openssl Configure 需要完整 perl 核心模块 + POSIX 路径 perl + make/ranlib/ar | Git Bash perl + `PERL5LIB`（从 CPAN 补 Locale-Maketext/IPC-Cmd/ExtUtils-MakeMaker/podlators/Pod-Simple 等，注意 `MSYS2_ENV_CONV_EXCL=PERL5LIB` 防 `C:/` 冒号截断）+ msys2 make/sh 前置 PATH + NDK llvm-ranlib/ar shim；**不要用 Strawberry perl（MSWin32 路径风格被 Configure 拒绝）** |
| vcpkg `configure_make` 对 ohos triplet 丢 `--target/--sysroot` | libvpx 用自带 configure 直构（`--target=arm64-linux-gcc`，NEON 启用）；libyuv/opus/libsodium 走 vcpkg cmake 路径正常 |
| aomedia.googlesource 全量克隆 >1GB 且国内极慢 | M1 不编 libaom：aom.rs/AV1 按 ohos 门控 + 存根符号库；后续如需 AV1 再解决源获取 |
| magnum-opus 按 vcpkg `arm64-linux` 布局找库 | `installed/arm64-linux` junction → `arm64-ohos` |

依赖产物（`harmonyos/client/build-ohos-deps.sh`）：vcpkg（gitee 镜像，官方 `arm64-ohos`
社区 triplet，`OHOS_SDK_ROOT` 指 `<sdk>/default/openharmony`）→ libyuv、opus、libsodium；
libvpx 1.16.0 直构（NEON）。Rust 侧 `build-rust.sh` 全自动检测并导出全部环境。

## 四、验证结果

- `cargo check --target aarch64-unknown-linux-ohos --lib --no-default-features --features use_dasp`：**0 error**（310 个 warning 主体是 ohos 裁剪后的 dead_code，与 iOS 构建同类）。
- `cargo build --release` 同参数：产出 `liblibrustdesk.so`（重命名为 `librustdesk.so`，ELF aarch64，~16MB）。
- `hvigor assembleHap`：BUILD SUCCESSFUL，HAP 内含 `libs/arm64-v8a/{libentry.so, librustdesk.so, libc++_shared.so}`。
- ArkTS：DevEco MCP 语法检查通过；C++ NAPI 用 SDK clang `-Wall -Wextra` 编译通过。
- 桌面回归面：x11/rdev/libsodium 等依赖图在 linux/windows/mac 目标不变（`cargo tree` 已核对）；`build.rs` 门控改为按 TARGET 判定，对宿主==目标的原生构建行为不变。本机无法完整跑 Windows 宿主 `cargo check` 是**既有环境要求**所致（magnum-opus 需要 VCPKG_ROOT 的 x64-windows 预编译库、scrap bindgen 需要 libclang，CI 均有提供），与本次改动无关。

## 五、M2 增量实现记录（2026-10-26）

| 功能 | Rust 侧 | OHOS/ArkTS 侧 |
|------|---------|---------------|
| 音频播放 | `src/ohos_audio.rs`（OHAudio C API 手写绑定）+ `src/client/ohos_audio_player.rs`（f32le/48kHz renderer，AudioHandler ohos 分支接重采样 + ring） | 无需参与（native 直渲） |
| 硬件解码 | `libs/scrap/src/ohos/ohcodec.rs`（OH_VideoDecoder 缓冲模式，NV12→libyuv→RGB）+ `codec.rs` 接入与能力探测；scrap/build.rs 链接 `libnative_media_vdec.so`/`libnative_media_core.so` | 无需参与；AV1 保持 aom_stub 门控 |
| 语音通话 | `src/ohos_voice_call.rs`（OHAudio capturer MIC → opus Voip → AudioFrame）+ `io_loop.rs` start_voice_call ohos 分支 | 工具栏"语音通话"按钮 + `ohos.permission.MICROPHONE` 运行时请求 |
| 会话录制 | `ui_interface.rs::video_save_directory` ohos 分支（`<HOME>/<AppName>/ScreenRecord`）+ `ohos_record_screen` + `record_status` 事件 | 工具栏"录制"按钮（对端未授权时提示） |
| 摄像头查看 | `ohos_session_new` 扩展 `session_kind`（0 控制 / 1 文件 / 2 VIEW_CAMERA），连接协议全链路复用 | 首页"查看摄像头"按钮 + 最近会话条目 |
| 图片剪贴板 | io_loop MultiClipboards ohos 分支转发 ImagePng → `clipboard_image` 事件；`ohos_send_clipboard_image_png`（PNG base64 → Clipboard{ImagePng}） | 远端图片自动写入系统剪贴板（`pasteboard.MIMETYPE_PIXELMAP`） |
| 显示设置面板 | `ohos_session_get/set_option`、`ohos_save_image_quality`、`ohos_save_custom_image_quality`、`ohos_set_custom_fps`、`ohos_toggle/get_toggle_option`、`ohos_update_supported_decodings`、`ohos_alternative_codecs`（全部走既有 Session/PeerConfig 路径） | 远程页工具栏"显示设置"：画质（低/均衡/最佳/自定义码率+FPS 滑杆）、编解码偏好（auto/vp8/vp9/h264/h265，按双端能力禁选）、只读模式开关 |
| HTML 剪贴板 | io_loop MultiClipboards ohos 分支转发 Html → `clipboard_html` 事件（`InvokeUiSession::clipboard_html`） | 远端 HTML 连同已有纯文本记录一并写入系统剪贴板（`MIMETYPE_TEXT_HTML`） |

构建验证：`cargo build --release --target aarch64-unknown-linux-ohos` 0 error，
`librustdesk.so` 16.9MB（较 M1 增加约 1MB，为 OHAudio/AVCodec 绑定与重采样）。

已知限制（M2 收尾后再评估）：
- 硬解探测在首次协商时同步创建一次解码器（进程内缓存结果）；部分老模拟器无 HEVC 硬件
  解码时会自动回落 VP8/VP9。
- 语音通话回声消除依赖系统 `VOICE_COMMUNICATION` 源，未做软件 AEC。
- 录制 VP8/VP9 输出 webm；H264/H265 录制需要 hwcodec feature（ohos 未启用），
  当前硬解画面录制不可用（与桌面 hwcodec 行为一致）。
- 本机剪贴板文本自动推送仍走手动同步（READ_PASTEBOARD ACL 限制不变）。
- 自定义码率滑杆上限为 100%（桌面的"更多画质"可到 2000%，鸿蒙面板未开放该档）。

### 语音通话三处修复（2026-10-28）

前两处经真机（Pura 70 ↔ Windows 桌面端）验证暴露并修复，第三处为静态代码流程分析得出：

1. **对端听不到主控端说话**：ohos 主控的 `start_voice_call` 分支只发 opus `AudioFrame`，
   从不发 `AudioFormat`；桌面被控端只在收到 `misc::AudioFormat` 时才启动回放线程
   （`connection.rs` `audio_sender` 初始为 None），麦克风帧全部被丢弃（日志特征
   `Processing audio frame without the voice call audio sender.`）。修复 =
   `io_loop.rs` ohos 分支在 spawn 转发线程前先发一条 48kHz/单声道 `AudioFormat`
   （对齐桌面端"订阅本地 audio_service 转发其 format"的既有路径）。
2. **主控端扬声器通话中无声**：真机日志特征 = 通话中音频 ring 积压 ~900ms
   （`skip 21600`），系统音频焦点策略被 `VOICE_COMMUNICATION` capturer 拉起后
   打断了本应用 renderer。修复 = `ohos_voice_call.rs` 通话期间激活
   AudioSession（`CONCURRENCY_MIX_WITH_OTHERS`，API 12+），`stop` 时反激活
   （`src/ohos_audio.rs` 补 `OH_AudioSessionManager_*` 绑定）；渲染器补挂
   `OnInterruptEvent` 日志（`force=/hint=`，hint 2=pause 3=stop），后续若仍被打断
   日志可直接定位。
3. **被控端声音在主控端失真/无声（单通）**：ohos 的 `AudioHandler::start_audio`
   分支从不创建 `audio_resampler`（cpal 分支有），`handle_frame` 把解码后的 PCM
   按对端原始采样率直接写进按 48kHz 尺寸化的 renderer ring。普通远控音频恰好是
   Windows loopback 48kHz/立体声（与设备速率一致，无感）；语音通话被控端切麦克风
   后按 cpal `default_input_config()` 降档（44.1kHz→24kHz、16kHz→16kHz），解码出
   的 24kHz PCM 以 2 倍速、隔帧饿死的方式进 48kHz ring，表现为断续变调、无法听清。
   修复 = `client.rs` ohos `start_audio` 内补
   `create_audio_resampler(format0.sample_rate, player.sample_rate(), format0.channels)`，
   对齐 cpal 分支；48kHz 对端时该函数本就返回 None，行为不变。

配套诊断技巧：真机日志在 `/data/app/el2/100/base/com.rustdesk.ohos/haps/entry/files/
home/.local/share/logs/RustDesk/rustdesk-ohos/rs_rCURRENT.log`，`hdc file recv`
拉取（路径要双斜杠）；桌面被控端验证格式消息到达 = `recved audio format` 一行
（`%APPDATA%/RustDesk/log/RustDesk_rCURRENT.log`）。

### 被控端通话麦克风采集速率自适应（2026-10-31）

真机↔模拟器通话（手机主控）里手机端听到杂音：手机下行播放 ring 在通话开始 ~3.2s
即填满 3s 容量（`Audio buffer capacity discard` 每 5s 数百次 + `skip 69600` 每 12s），
即生产速率 ≈2 倍实时。模拟器日志确认新 `OhosMicSource` 在跑且 avscreen mic 已关，
所以 2 倍来自 OHAudio capturer 本身：模拟器虚拟麦克风向 RAW capturer 交付宿主机
混音格式（立体声），按 mono 读回数据量正好 2 倍、内容 L/R 交错错乱 = 乱码杂音。

修复 = 不再假设采集器按请求参数交付：pa_impl 在通话开始时实测 500ms 交付速率
（期间静音积压），偏离 48kHz 超过 ~5% 就按"mono 时间轴 @ 实测速率"重采样回 48k
——立体声被按 2x mono 误读时，2:1 重采样恰好等价 (L+R)/2 下混，两种偏差
（96k mono / 48k stereo）都恢复正确语速。同时在 `OhosMicSource` 启动后经
`OH_AudioCapturer_GetSamplingRate/GetChannelCount` 查询并日志记录采集器上报参数
（辅助区分速率/声道偏差）。真机（Pura 70）实测交付速率为请求值，走无重采样快路径，
行为不变；通话开始有 ~0.5s 静音测量窗。

已知限制：主控端 `OhosVoiceCallCapturer` 仍是固定 48k mono 假设——真机实测健康
（模拟器播放其上行 ring 全程干净），未动；若将来用模拟器做主控端出现同类杂音，
把同一"实测+重采样"模式搬过去即可。

## 六、M1 已有能力（保留记录）

| 项 | 状态 |
|----|------|
| 远端光标渲染 | **已实现**：`cursor_data/id/position` 事件 → PixelMap 叠加层，与视频共享缩放变换，工具栏可开关（对端 Android / `cursor_embedded` 时不绘制） |
| 剪贴板自动推送 | 本机→远端为**手动同步**（工具栏按钮 → 粘贴弹窗发送，零权限；`pasteboard.getData()` 需 ACL 受限权限 READ_PASTEBOARD，正式签名可申请后改为直读+回前台自动推） |
| 会话亮屏 | **已实现**：远程页/文件传输页 `setWindowKeepScreenOn` |
| 最近会话持久化 | **已实现**：`@ohos.data.preferences`（重启保留最近 10 条） |
| 沙箱外文件导入/导出 | **已实现**：文件传输页本地面板接 `@ohos.file.picker`。导入 = 文件选择器（仅文件：手机上 FOLDER/MIXED 模式不可用，仅平板/折叠屏/PC）；导出 = `DocumentViewPicker.save`（文件直拷，文件夹先 `zlib.compressFile` 打 zip 再保存） |
| 真机联调 | 未做（需签名 HAP + hdc），DevEco 签名后 `hdc install` |

## 七、M3 被控端实现记录（2026-10-27）

### Rust 侧

| 模块 | 改动 |
|------|------|
| `src/lib.rs` | `server` / `rendezvous_mediator` / `ipc` / `clipboard` 对 ohos 解禁（原与 iOS 同组排除，现同 Android） |
| `src/server.rs` | `start_server` 走 android 变体（`RendezvousMediator::start_all`）；`terminal_service`/`ServiceTmpl`/桌面 `input_service` 排除 ohos |
| `src/server/connection.rs` | CM 通道 `start_channel` 走 ohos 分支（`ohos_ffi::connection_manager`）；`keyboard` 权限恒 false（无注入能力）；`platform`/`whiteboard`/`terminal` 等桌面分支补 ohos 排除（编译器驱动，`target_os="linux"` 在 ohos 为真，全部需 `not(ohos)`） |
| `src/server/video_service.rs` / `display_service.rs` | camera 源排除 ohos（nokhwa 未接）；aom/AV1 编码臂排除；linux 专属路径排除 |
| `src/server/audio_service.rs` | `pa_impl::run` ohos 分支：AVScreenCapture s16le PCM → f32，攒批 960×2 采样（20ms 立体声）→ opus；语音通话经 `VOICE_CALL_INPUT_DEVICE="mic"` 切到专用 `OhosMicSource`（OHAudio VOICE_COMMUNICATION capturer，见 2026-10-29 修复） |
| `src/server/clipboard_service.rs` / `src/clipboard.rs` | android 分支扩至 ohos：`get_clipboards_msg` 读 `scrap::ohos::ffi` 通道；远端→本机经 `cm_write_clipboard` 事件写 pasteboard |
| `src/ui_cm_interface.rs` | `start_listen` / `authorize` / `close` / `switch_permission` / `handle_fs` 等对 ohos 解禁 |
| `src/ohos_ffi.rs` | 新增 §6 的 16 个被控端 C ABI + `connection_manager` 模块（`InvokeUiCM` 实现 → `cm_*` 事件）+ `cm_write_clipboard` |
| `libs/scrap/src/ohos/ffi.rs` | FrameRaw 帧通道（video / audio-inner / audio-mic）+ 被控剪贴板存储；带 300ms 无消费者跳拷贝（避免无人观看时按刷新率 memcpy） |
| `libs/scrap/src/ohos/screencap.rs` | OH_AVScreenCapture C API 手写绑定（`libnative_avscreen_capture.so` + `libnative_buffer.so`）；OH_ORIGINAL_STREAM 模式；**混合采集模型**：API-12 `SetDataCallback`（音频全平台可用，真机视频走它）+ 泵线程在回调未见视频时回退轮询 API-10 `AcquireVideoBuffer`（模拟器镜像的 capture 服务产不出视频 buffer，回退带退避避免 1s/次的 IPC 空转）；状态回调（用户取消/系统停止→自动停采集并通知） |
| `libs/scrap/src/common/ohos.rs` | 被控 Display/Capturer（仿 `common/android.rs`，尺寸来自采集配置） |
| `libs/scrap/build.rs` / `Cargo.toml` | 链接 `libnative_avscreen_capture.so`；ohos 依赖段补 `log`/`lazy_static` |
| `src/common.rs` | `is_peer_path_allowed`（文件沙箱校验）扩至 ohos；`get_default_sound_input` ohos 返回 `"mic"`；`whoami_hostname` 解禁 |

构建：`cargo build --release` 两目标 0 error，aarch64 `librustdesk.so` 20.7MB（+3.8MB，
server 栈 + AVScreenCapture 绑定），x86_64 23.0MB。

### ArkTS/NAPI 侧

| 文件 | 改动 |
|------|------|
| `napi_init.cpp` / `index.d.ts` / `ohos_ffi.ets` | 16 个被控端函数的 typedef/NAPI 包装/TS 声明 |
| `pages/SharePage.ets`（新增） | ID 卡片（可复制）、临时密码（刷新）、服务启停（开始共享 → `ffiServerStart` + `ffiCaptureStart(显示器宽高)` + 长时任务 + `ffiServerStartService`）、连接请求接受/拒绝、已连接列表（断开） |
| `pages/Index.ets` | 「被控（共享屏幕）」入口按钮 |
| `entryability/EntryAbility.ets` | `cm_clipboard_write` 事件 → 系统 pasteboard（text/html/PixelMap） |
| `module.json5` | `backgroundModes: ["dataTransfer"]` |
| `resources/{base,zh_CN}/element/string.json` | 24 个 share_* 字符串 |

### 已知限制（M3）

- **输入注入未实现**：`inputEventClient.injectEvent` 需系统签名/受限权限
  `ohos.permission.INJECT_EVENT`，三方应用拿不到。被控端 keyboard 权限恒 false，
  控制端看到的是只读会话（文件/剪贴板/音频/观看均可用）。这是 M4 之前最大的功能缺口。
- **模拟器视频限制**：x86_64 模拟器镜像的 screen capture 服务无法产出视频
  buffer（系统侧 `NativeScreenCapture: AcquireVideoBuffer get failed`，每次 IPC 超时 1s），
  因此模拟器上被控画面恒黑、仅音频可用；连接本身稳定。真机不受影响（真机
  SetDataCallback 直接给视频 buffer，走不到回退路径）。
- 屏幕采集分辨率在启动时固定（display 宽高），**旋转/分辨率变化不跟随**
  （AVScreenCapture 需重启采集，待做 display 监听）。
- 长时任务用 `DATA_TRANSFER` 模式，系统可能在长时间低速时回收（TASK_KEEPING 需 ACL）。
  申请前必须先 `requestEnableNotification`：长时任务会挂状态栏通知，无通知权限时
  `startBackgroundRunning` 报 9800006 失败（代码里曾只打 warn 继续跑），退后台后进程
  被冻结、连接全部断开（2026-10-27 已修复并复现确认：失败时 hidumper 任务列表为空、
  `continuousTaskId: -1`）。
- **ID 注册时机**：应用启动（EntryAbility）即调用 `ffiServerStart` 注册 hbbs，
  应用打开期间 ID 始终可达（对齐 Android MainService 语义）；「开始共享」只控制
  AVScreenCapture 采集。此前只在按「开始共享」后才注册，造成桌面端连 ID 报
  「ID 不存在」（2026-10-27 已修复并在模拟器自拨验证）。
- 连接请求仅在 SharePage 打开时可见（无全局通知，Android 用前台服务通知解决）；
  后续可接 NotificationKit。
- 被控端麦克风语音通话（2026-10-29 修复）：原走 AVScreenCapture 的 mic 通道
  （`SetMicrophoneEnabled`），真机（Pura 70）实测两处问题：① 通话依赖共享在跑
  （capture 停止即上行无声）；② mic 流交付速率与配置不符（48kHz 配置下实测
  2 倍数据量，编码后对端 2 倍速变调）。现改为与主控端同款的专用 OHAudio
  VOICE_COMMUNICATION capturer（`src/ohos_voice_call.rs` `OhosMicSource`）：
  48kHz 单声道 f32 → 复制双声道进既有 opus 管线；不依赖共享；带
  `CONCURRENCY_MIX_WITH_OTHERS` 会话保证与下行播放共存；通话期间 AVScreenCapture
  mic 显式关闭避免双采。OHAudio capturer 启动失败时回退 AVScreenCapture mic 流。
- AV1 编码仍为存桩门控；H264/H265 硬编（OH_VideoEncoder）未接，编码仅 VP8/VP9 软编。

### 硬件编码补充（2026-10-27）

被控端补齐 H264/H265 硬编（OH_VideoEncoder，缓冲模式），主控硬解（M2）至此可真正
被端到端协商启用：

| 模块 | 改动 |
|------|------|
| `libs/scrap/src/ohos/ohcodec.rs` | 新增 `OhCodecEncoder`（NV12 输入，Annex-B H264/H265 输出，CBR + 运行时 SetParameter 调码率，CODEC_DATA 帧并入下一个关键帧）+ `try_get` 进程内能力探测（Create/Configure/Prepare 后即销毁）；同步修复主控硬解 `wait_output` 先 `FreeOutputBuffer` 再读缓冲的 use-after-free |
| `libs/scrap/src/common/codec.rs` | `EncoderCfg::OHOS_HW` 变体；`Encoder::new` / `update`（h264/h265 可用性）/ `supported_encoding`（上报对端）/ `use_i444` / `set_fallback` 接 ohos 臂；编码失败清探测缓存并回退 VP9 |
| `src/server/video_service.rs` | `get_encoder_config` H264/H265 臂优先 `OhCodecEncoder::try_get` |
| `libs/scrap/build.rs` | 链接 `libnative_media_venc.so` |

协商链路：被控 `supported_encoding()` 上报 h264/h265 → 主控 `Decoder::supported_decodings`
（M2 探测）回告能力 → `Encoder::update` 置 `ENCODE_CODEC_FORMAT`（auto 时 h265 > h264 > vp9）
→ `get_encoder_config` 创建 OHOS_HW 编码器。编码器连续失败 3 次走既有 `disable()` 路径
（清缓存回退 VP9 软编）。

已知限制：
- 录制 H264/H265 仍需 hwcodec feature（ohos 未启用），硬编会话的录制静默跳过（与 M2 硬解录制限制一致）。
- 模拟器上 OH_VideoEncoder 为系统软件实现（探测可通过），真机才是硬编；协商不受影响。
