# feat(ohos): add OpenHarmony client integration and native media backends

> [!IMPORTANT]
> ## Merge dependency — please do not merge this PR yet
>
> This PR depends on [rustdesk/hbb_common#577](https://github.com/rustdesk/hbb_common/pull/577).
>
> Please merge the `hbb_common` PR first. This PR should only be merged after the RustDesk main repository has updated and completed its compatibility work for `hbb_common@c30d817`, or for the equivalent final revision produced when hbb_common PR #577 is merged.
>
> The current development branch temporarily pins `hbb_common` to `a4dc4d9`, which is based on the older upstream revision `5591761`. This temporary pin exists only to keep the current RustDesk Actions buildable while the upstream dependency transition is pending. It must not be retained in the final merged version.
>
> Before merging this PR:
>
> 1. Merge `rustdesk/hbb_common#577`.
> 2. Update the RustDesk main repository to the merged hbb_common revision.
> 3. Complete the RustDesk-side API compatibility work required by `hbb_common@c30d817`, including the aligned-buffer API changes.
> 4. Rebase this PR onto the updated RustDesk master branch.
> 5. Replace the temporary `hbb_common@a4dc4d9` pin with the official merged revision.
> 6. Rerun the complete CI matrix.
>
> Until these steps are complete, this PR should remain a draft.

## Summary

This PR adds controller-side OpenHarmony support to the RustDesk core for use by the native ArkTS HarmonyOS client.

The implementation is kept platform-scoped with `target_env = "ohos"` wherever possible. Existing Flutter clients and other platforms are intended to retain their current build paths and runtime behavior.

The changes are organized into three functional areas:

1. Isolate OpenHarmony from desktop Linux dependencies and platform paths.
2. Add native OpenHarmony video decoding and audio playback backends.
3. Expose a headless session event bridge for the ArkTS/HAR client.

## Motivation

The Rust OpenHarmony target, such as `aarch64-unknown-linux-ohos`, reports:

- `target_os = "linux"`
- `target_env = "ohos"`

OpenHarmony is therefore accidentally included by many existing `target_os = "linux"` conditions, even though it does not provide the desktop Linux environment expected by RustDesk.

In particular, OpenHarmony applications do not provide the same:

- X11 or Wayland desktop stack;
- PulseAudio and desktop audio environment;
- DBus services;
- tray, wallpaper, windowing, and desktop input dependencies;
- filesystem and IPC layout;
- native TLS implementation;
- Flutter runtime integration.

Without explicit `target_env = "ohos"` handling, the OpenHarmony target attempts to compile unavailable desktop Linux dependencies and cannot integrate cleanly with a native ArkTS client.

## Why ArkTS instead of Flutter

The official RustDesk client is implemented with Flutter, and retaining that client would normally be the preferred way to preserve upstream UI behavior and reduce long-term maintenance work.

However, Flutter support on the OHOS platform is still progressing too slowly for RustDesk's current requirements. Several third-party Flutter packages and platform integrations required by RustDesk are not yet fully available or sufficiently mature on OHOS. After evaluating a direct Flutter port, I was unable to complete a reliable and maintainable migration within a reasonable time.

The current OHOS client therefore uses ArkTS as a practical platform-native alternative. The ArkTS layer is limited to the HarmonyOS UI, system integration, and HAR bindings. Session behavior, protocol handling, codec negotiation, input, clipboard, and other core semantics continue to come from the RustDesk core rather than being reimplemented in ArkTS.

This is not intended to reject or permanently replace the official Flutter client. If the Flutter ecosystem on OHOS matures and the third-party packages required by RustDesk become fully supported in the future, I would be very willing to port and maintain the original RustDesk Flutter client on OHOS instead.

## Changes

### 1. OpenHarmony platform isolation

- Exclude OpenHarmony from desktop Linux-only dependencies and code paths.
- Prevent desktop components such as X11, Wayland, PulseAudio, DBus, Sciter, tray, wallpaper, and desktop capture helpers from being selected for OHOS builds.
- Add OHOS-specific build conditions without changing the existing conditions for Android, iOS, Linux desktop, macOS, or Windows.
- Use an OHOS-compatible `rdev` branch while keeping the official `rustdesk-org/rdev` dependency for other platforms.
- Use rustls-backed HTTP networking on OpenHarmony instead of relying on unavailable native TLS facilities.
- Keep configuration, IPC, and platform-directory behavior coordinated with the accompanying hbb_common PR.

### 2. Native video decoding

- Add an OpenHarmony decoder backend based on the native AVCodec APIs.
- Advertise H.264 and H.265 decoding only when the corresponding platform decoder capability is available.
- Support both NativeWindow surface rendering and decoded-buffer output.
- Integrate the OHOS decoder into RustDesk's existing codec negotiation and decoder lifecycle.
- Preserve the existing VP8, VP9, AV1, MediaCodec, hwcodec, and software-decoder paths on other platforms.
- Keep codec negotiation in the RustDesk core rather than duplicating protocol logic in ArkTS.

### 3. Native audio playback

- Add an OpenHarmony audio output backend using the native OHAudio renderer APIs.
- Integrate the renderer with the existing RustDesk audio decoding pipeline.
- Handle renderer startup, buffering, flushing, shutdown, and callback lifetime on the OHOS-specific path.
- Leave the existing CPAL and platform audio implementations unchanged for non-OHOS targets.

### 4. Headless session bridge

- Allow an OpenHarmony client to start and manage RustDesk sessions without requiring a Flutter `StreamSink`.
- Forward existing `EventToUI` session events through an OHOS callback bridge.
- Forward decoded frames and session lifecycle events to the native HAR/ArkTS integration layer.
- Reuse the existing RustDesk session state, protocol, error handling, input, clipboard, and connection logic.
- Preserve the existing Flutter event-stream implementation on all Flutter targets.

## Scope

This PR adds the core capabilities required by the native ArkTS controller client.

It does not:

- replace or redesign the existing Flutter client;
- change the RustDesk protocol;
- add OpenHarmony controlled-side screen capture or hosting;
- duplicate session or codec-negotiation logic in the ArkTS layer;
- intentionally change behavior on Android, iOS, desktop Linux, macOS, or Windows.

Controlled-side support is intentionally outside the scope of this PR.

## Commit organization

The functional changes are separated into reviewable commits:

1. `build(ohos): isolate desktop Linux paths`
2. `feat(ohos): add native video and audio backends`
3. `feat(ohos): expose headless session event bridge`

The current dependency-pin commits are temporary integration commits. They should be rewritten or removed after the RustDesk repository adopts the merged hbb_common revision.

## Validation

Current validation includes:

- `cargo check --locked --lib`
- GitHub Actions main CI
- Linux x86_64 and AArch64 builds
- Android AArch64, ARMv7, and x86_64 builds
- OpenHarmony HAR integration builds
- Manual remote-session testing with the ArkTS client
- H.264 and H.265 decoder negotiation and playback
- Native audio playback
- Session events, mouse/touch input, keyboard input, and clipboard integration

Current CI runs using the temporary hbb_common compatibility pin:

- Main CI: https://github.com/FrankHan052176/rustdesk4ohos/actions/runs/30195890807
- Full Flutter CI: https://github.com/FrankHan052176/rustdesk4ohos/actions/runs/30195891032

The full CI matrix must be rerun after replacing the temporary pin with the official merged hbb_common revision.

## Related repositories

- hbb_common prerequisite PR: https://github.com/rustdesk/hbb_common/pull/577
- OpenHarmony Core integration branch: https://github.com/FrankHan052176/rustdesk4ohos
- Native ArkTS client: https://github.com/FrankHan052176/RustDesk-ArkTS
- HarmonyOS HAR bridge: https://github.com/FrankHan052176/RustDeskHar

I am an individual HarmonyOS developer, and my Rust experience is limited. I have tried to keep these changes minimal, platform-gated, and aligned with the existing RustDesk architecture. Feedback and corrections are very welcome, and I am happy to revise the implementation as requested.

---

> [!IMPORTANT]
> ## 合并依赖——请暂时不要合入本 PR
>
> 本 PR 依赖 [rustdesk/hbb_common#577](https://github.com/rustdesk/hbb_common/pull/577)。
>
> 请先合并 `hbb_common` PR。只有当 RustDesk 主仓库更新并完成对 `hbb_common@c30d817` 的兼容适配，或完成对 hbb_common PR #577 合并后等效最终版本的适配后，才能合入本 PR。
>
> 当前开发分支暂时将 `hbb_common` 固定在 `a4dc4d9`。该提交基于较旧的上游版本 `5591761`，其作用仅是在上游依赖迁移完成前，让目前的 RustDesk Actions 能够正常构建。最终合入时不能保留这个临时依赖版本。
>
> 合入本 PR 前，需要依次完成：
>
> 1. 合并 `rustdesk/hbb_common#577`。
> 2. 将 RustDesk 主仓库更新到合并后的 hbb_common 版本。
> 3. 完成 RustDesk 对 `hbb_common@c30d817` 所需的 API 兼容修改，包括对齐缓冲区 API 的变化。
> 4. 将本 PR rebase 到更新后的 RustDesk master。
> 5. 将临时的 `hbb_common@a4dc4d9` 引用替换为官方合并后的版本。
> 6. 重新运行完整 CI 矩阵。
>
> 在这些步骤完成前，建议将本 PR 保持为 Draft 状态。

## 概述

本 PR 为 RustDesk Core 添加控制端 OpenHarmony 支持，用于原生 ArkTS HarmonyOS 客户端。

相关实现尽可能使用 `target_env = "ohos"` 限定在 OpenHarmony 平台。现有 Flutter 客户端及其他平台应继续使用原有构建路径和运行行为。

改动分为三个主要部分：

1. 将 OpenHarmony 与桌面 Linux 的依赖及平台路径隔离。
2. 添加 OpenHarmony 原生视频解码和音频播放后端。
3. 为 ArkTS/HAR 客户端导出无 Flutter 依赖的会话事件桥接。

## 背景

Rust 的 OpenHarmony 目标，例如 `aarch64-unknown-linux-ohos`，会报告：

- `target_os = "linux"`
- `target_env = "ohos"`

因此，仅使用 `target_os = "linux"` 判断的现有代码会错误地将 OpenHarmony 当作桌面 Linux 环境。

OpenHarmony 应用并不提供与桌面 Linux 相同的以下环境：

- X11 或 Wayland 桌面栈；
- PulseAudio 及桌面音频环境；
- DBus 服务；
- 托盘、壁纸、窗口和桌面输入依赖；
- 文件系统及 IPC 目录结构；
- 原生 TLS 实现；
- Flutter 运行时集成。

如果不显式识别 `target_env = "ohos"`，OpenHarmony 构建会尝试编译不可用的桌面 Linux 依赖，也无法与原生 ArkTS 客户端正确集成。

## 为什么使用 ArkTS 而不是 Flutter

RustDesk 官方客户端使用 Flutter 实现。通常而言，继续沿用官方 Flutter 客户端更有利于保持上游 UI 行为一致，也能够减少长期维护成本。

但是，目前 Flutter 在 OHOS 平台上的适配进度对于 RustDesk 的实际需求而言仍然较为缓慢。RustDesk 所依赖的部分 Flutter 第三方库和平台集成能力尚未在 OHOS 上得到完整适配，或成熟度仍不足以支撑稳定客户端。经过对直接迁移 Flutter 客户端的评估和尝试后，我无法在合理时间内完成一套可靠且易于维护的整体移植。

因此，当前 OHOS 客户端采用 ArkTS 作为务实的平台原生替代方案。ArkTS 层只负责 HarmonyOS 界面、系统集成和 HAR 接口调用；会话行为、协议处理、编解码器协商、输入、剪贴板及其他核心语义仍然由 RustDesk Core 提供，而不是在 ArkTS 中重新实现。

这并不意味着否定或永久替代官方 Flutter 客户端。如果未来 Flutter 在 OHOS 上的生态逐渐成熟，并且 RustDesk 所需的第三方库都得到完整适配，我也非常愿意将 RustDesk 原版 Flutter 客户端移植并维护到 OHOS 平台。

## 主要修改

### 1. OpenHarmony 平台隔离

- 从桌面 Linux 专用依赖和代码路径中排除 OpenHarmony。
- 防止 OHOS 构建选择 X11、Wayland、PulseAudio、DBus、Sciter、托盘、壁纸及桌面采集相关实现。
- 添加 OHOS 专用构建条件，同时保持 Android、iOS、桌面 Linux、macOS 和 Windows 的现有条件不变。
- OpenHarmony 使用兼容的 `rdev` 分支，其他平台继续使用官方 `rustdesk-org/rdev`。
- OpenHarmony 的 HTTP 网络请求使用 rustls，不依赖平台当前无法提供的原生 TLS。
- 配置、IPC 和平台目录行为与配套的 hbb_common PR 保持一致。

### 2. 原生视频解码

- 添加基于 OpenHarmony 原生 AVCodec API 的视频解码后端。
- 仅在系统报告相应解码能力时声明支持 H.264 和 H.265。
- 支持 NativeWindow Surface 直出和解码缓冲区输出两种路径。
- 将 OHOS 解码器接入 RustDesk 现有的编解码器协商和解码器生命周期。
- 保持其他平台现有的 VP8、VP9、AV1、MediaCodec、hwcodec 和软件解码路径不变。
- 编解码器协商继续由 RustDesk Core 负责，不在 ArkTS 层重复实现协议逻辑。

### 3. 原生音频播放

- 使用 OpenHarmony 原生 OHAudio Renderer API 添加音频输出后端。
- 将其接入 RustDesk 现有音频解码管线。
- 在 OHOS 专用路径中处理播放启动、缓冲、刷新、停止和回调生命周期。
- 非 OHOS 平台继续使用原有 CPAL 及各平台音频实现。

### 4. 无 Flutter 依赖的会话桥接

- 允许 OpenHarmony 客户端在不提供 Flutter `StreamSink` 的情况下启动和管理 RustDesk 会话。
- 通过 OHOS 回调桥接转发已有的 `EventToUI` 会话事件。
- 向原生 HAR/ArkTS 集成层转发解码帧和会话生命周期事件。
- 复用 RustDesk 现有的会话状态、协议、错误处理、输入、剪贴板和连接逻辑。
- 所有 Flutter 平台继续使用原有 Flutter 事件流实现。

## 范围说明

本 PR 添加的是原生 ArkTS 控制端客户端所需的 Core 能力。

本 PR 不会：

- 替换或重新设计现有 Flutter 客户端；
- 修改 RustDesk 协议；
- 添加 OpenHarmony 被控端的屏幕采集或主机服务；
- 在 ArkTS 层重复实现会话或编解码器协商逻辑；
- 有意改变 Android、iOS、桌面 Linux、macOS 或 Windows 的行为。

OpenHarmony 被控端支持不在本 PR 的范围内。

## Commit 组织

功能改动被拆分为便于审阅的提交：

1. `build(ohos): isolate desktop Linux paths`
2. `feat(ohos): add native video and audio backends`
3. `feat(ohos): expose headless session event bridge`

当前与依赖版本固定有关的提交只是临时集成提交。RustDesk 主仓库采用合并后的 hbb_common 版本后，应重写或移除这些临时提交。

## 验证

目前已经完成：

- `cargo check --locked --lib`
- GitHub Actions 主 CI
- Linux x86_64 和 AArch64 构建
- Android AArch64、ARMv7 和 x86_64 构建
- OpenHarmony HAR 集成构建
- ArkTS 客户端远程会话实机测试
- H.264 和 H.265 解码器协商及画面播放
- 原生音频播放
- 会话事件、鼠标/触摸输入、键盘输入和剪贴板集成

当前 CI 使用临时 hbb_common 兼容版本：

- 主 CI：https://github.com/FrankHan052176/rustdesk4ohos/actions/runs/30195890807
- 完整 Flutter CI：https://github.com/FrankHan052176/rustdesk4ohos/actions/runs/30195891032

替换为官方合并后的 hbb_common 版本后，仍需要重新运行完整 CI 矩阵。

## 相关仓库

- hbb_common 前置 PR：https://github.com/rustdesk/hbb_common/pull/577
- OpenHarmony Core 集成分支：https://github.com/FrankHan052176/rustdesk4ohos
- 原生 ArkTS 客户端：https://github.com/FrankHan052176/RustDesk-ArkTS
- HarmonyOS HAR 桥接：https://github.com/FrankHan052176/RustDeskHar

我是 HarmonyOS 个人开发者，Rust 方面的经验有限。我已经尽量让相关改动保持最小范围、仅在 OpenHarmony 平台生效，并与 RustDesk 现有架构保持一致。如果实现中存在不正确或需要调整的地方，欢迎指正，我也很愿意按照审阅意见继续修改。
