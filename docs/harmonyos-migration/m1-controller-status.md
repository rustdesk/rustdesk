# M1 主控端迁移实施记录（Android → HarmonyOS）

> 里程碑对应 `android-feature-analysis.md` §4.3 的 **M1 主控端先行**：
> 纯客户端、不碰系统权限——远程观看（软解）、触控/键盘输入、文件传输、剪贴板（客户端方向）。
>
> **状态：构建链路全部打通**（2026-10-24）。
> `cargo check --target aarch64-unknown-linux-ohos` 0 error；
> `librustdesk.so`（aarch64 ELF，~16MB）产出；
> `hvigor assembleHap` 成功，HAP 内含 `libentry.so`（NAPI 壳）+ `librustdesk.so`（Rust 核心）。

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
| `libs/libsodium-sys/`（vendored patch） | 增加 target 作用域的 `SODIUM_LIB_DIR_<target>` 查找（Windows 宿主交叉编译时 aarch64 libsodium 不得泄漏进宿主 build 脚本链接） |

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
| libsodium-sys 的 `cfg!(target_env="msvc")` 在 build.rs 里判的是宿主 | vendored patch：`SODIUM_LIB_DIR_<target>` 目标作用域 + 双名库文件 |
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

## 五、已知限制（M1 收尾后再评估）

| 项 | 状态 | 后续 |
|----|------|------|
| 音频播放 | 未实现（`start_audio` 报不支持，帧丢弃） | OHAudio（C API，可被 Rust 直接链接） |
| 硬件解码 | 未实现，仅 VP8/VP9 软解 | AVCodec（OH_VideoDecoder）Rust 绑定 |
| AV1/H264/H265 解码 | AV1 已门控（aom 存根）；H264/H265 需 hwcodec/ffmpeg | 按需启用 |
| 远端光标渲染 | **已实现**：`cursor_data/id/position` 事件 → PixelMap 叠加层，与视频共享缩放变换，工具栏可开关（对端 Android / `cursor_embedded` 时不绘制） | — |
| 剪贴板自动推送 | 本机→远端为**手动同步**（工具栏按钮 → 粘贴弹窗发送，零权限；`pasteboard.getData()` 需 ACL 受限权限 READ_PASTEBOARD，正式签名可申请后改为直读+回前台自动推） | 按需接 |
| 会话亮屏 | **已实现**：远程页/文件传输页 `setWindowKeepScreenOn` | — |
| 最近会话持久化 | **已实现**：`@ohos.data.preferences`（重启保留最近 10 条） | — |
| 沙箱外文件导入/导出 | **已实现**：文件传输页本地面板接 `@ohos.file.picker`。导入 = 文件选择器（仅文件：手机上 FOLDER/MIXED 模式不可用，仅平板/折叠屏/PC）；导出 = `DocumentViewPicker.save`（文件直拷，文件夹先 `zlib.compressFile` 打 zip 再保存） | 平板上可再启用文件夹导入 |
| 语音通话/录制/终端 | 未实现 | M4 里程碑 |
| 真机联调 | 未做（需签名 HAP + hdc） | DevEco 签名后 `hdc install` |
