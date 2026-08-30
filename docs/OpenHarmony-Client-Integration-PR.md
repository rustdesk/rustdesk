# PR title

Unify HarmonyOS HAR and Flutter on one authoritative Core

# PR body (English)

## Summary

This PR consolidates HarmonyOS support around one authoritative RustDesk Core
shared by both frontend integrations:

- `ohos-har` for the ArkTS/HAR client;
- `ohos-flutter` for the Flutter HarmonyOS client.

The frontend features are explicit and mutually exclusive on OHOS. The thin
`native/ohos_bridge` crate owns Flutter Rust Bridge generation and packaging,
but re-exports Core's `flutter_ffi` API instead of adding another protocol or
session implementation.

## Main changes

- Make root `src/` and `libs/` the single source of truth for protocol,
  sessions, capture, input, audio, clipboard, and media behavior.
- Keep HAR/N-API glue in the external HAR project and Flutter-specific Dart,
  ArkUI, FRB, and packaging code under `flutter/` and `native/ohos_bridge/`.
- Import the Flutter-OHOS frontend and its lifecycle/security fixes into the
  unified branch.
- Retain the native OHOS AVCodec backend, while aligning it with the common
  decoder/renderer contracts and the official video timing/status pipeline.
  Decode results remain `Ok(true)` for produced output, `Ok(false)` while
  pending, and `Err` for genuine failure/watchdog conditions.
- Preserve surface and no-surface H.264/H.265 paths, decoder backpressure,
  reset/fallback, and keyframe recovery.
- Keep HarmonyOS hosting fail-closed: view-only policy, explicit input
  authorization, microphone/capture lifecycle gates, and exact-once native or
  frontend input delivery.
- Keep restricted pasteboard hosting disabled unless a matching privileged
  signing profile is available; the ordinary Flutter debug profile therefore
  does not request `READ_PASTEBOARD`.
- Add reproducible FRB generation, ABI/export checks, unified repository CI
  paths, Flutter HAP packaging coverage, and adapter documentation.

## Architecture

```text
Authoritative RustDesk Core
├── feature ohos-har
│   └── external HAR/N-API adapter
│       └── ArkTS application (top.frankhan.resk)
└── feature ohos-flutter
    └── native/ohos_bridge (thin FRB cdylib)
        └── Flutter HarmonyOS application (top.frankhan.resk.flutter)
```

## Compatibility and limitations

- OHOS file transfer remains disabled.
- HAR hosting remains fail-closed/view-only.
- The local Flutter debug profile is for device installation, not AppGallery
  publication.
- Flutter host clipboard is intentionally unavailable with the current
  non-privileged profile; no bundle identity or signing ACL is bypassed.

## Verification

Local verification completed with Flutter-OH
`3.41.10-ohos-0.0.2-beta`, FRB `1.80.1`, HarmonyOS SDK/NDK
`6.1.1.280`, and Java 17:

| Check | Result | SHA-256 |
| --- | --- | --- |
| Unified `ohos-flutter` bridge (`scripts/build-ohos-ffi.sh`) | passed; required FRB/OHOS exports verified | `c3274c67c7123dd1e989a735f5ec89c05fe61eb7a3675a352e983610ab8174a7` |
| Signed Flutter debug HAP (`scripts/build-ohos-flutter-hap.sh`) | passed; signature and code-sign blocks verified; bundle `top.frankhan.resk.flutter` | `dd41b17fe8a1151e692c1db5c4cec807dee90298ce0edfe140247ab679bf43ad` |
| HAR (`scripts/build-har.sh`) | passed; copied HAR matches the ArkTS dependency | `ae596e6d1c15eafad5ebf489077772e74aa9b9331b62604813e0178ee279b13c` |
| Signed ArkTS debug HAP | passed; signature and code-sign blocks verified; bundle `top.frankhan.resk` | `c173b3d18344c8d4504336f4b6dea6a663dc02f281a1b79970f2b7d496d4a9be` |
| Signed ArkTS debug App | passed | `d3969867a672b7d706a0f1da343789b4e19c777cd86d4535f6e4d67173c1f537` |

`flutter analyze --no-fatal-infos` reported no analyzer errors; the existing
upstream/dependency warning and deprecation backlog remains.

# PR 正文（中文）

## 概述

本 PR 将鸿蒙适配统一到唯一权威的 RustDesk Core，并由两套前端复用：

- `ohos-har`：供 ArkTS/HAR 客户端使用；
- `ohos-flutter`：供 Flutter HarmonyOS 客户端使用。

OHOS 构建必须且只能选择其中一个前端 feature。轻量的
`native/ohos_bridge` 仅负责 Flutter Rust Bridge 代码生成和打包，通过
重导出 Core 的 `flutter_ffi` 接口工作，不再维护第二套协议或会话核心。

## 主要改动

- 根目录 `src/` 与 `libs/` 成为协议、会话、采集、输入、音频、剪贴板和
  媒体行为的唯一事实来源。
- HAR/N-API 胶水继续位于外部 HAR 工程；Flutter 专属的 Dart、ArkUI、FRB
  和打包代码位于 `flutter/` 与 `native/ohos_bridge/`。
- 将 Flutter-OHOS 前端以及已完成的生命周期和安全修复合并到统一分支。
- 保留 OHOS 原生 AVCodec，并让接口、生命周期和返回语义与公共编解码器
  抽象及官方视频计时/状态管线对齐：产生输出返回 `Ok(true)`，等待输出
  返回 `Ok(false)`，真实失败或 watchdog 才返回 `Err`。
- 保留有 Surface/无 Surface 的 H.264/H.265 路径、背压、重置/回退和关键帧
  恢复逻辑。
- 鸿蒙被控端继续 fail-closed：仅观看策略、显式输入授权、麦克风/采集生命
  周期门控，以及原生输入或前端队列二选一的 exact-once 路由。
- 在没有匹配特权 Profile 时不启用受限剪贴板权限；普通 Flutter 调试
  Profile 不请求 `READ_PASTEBOARD`，不通过修改包名或绕过 ACL 解决签名。
- 增加可复现的 FRB 生成、ABI/导出检查、统一仓库 CI 路径、Flutter HAP
  打包检查和适配器文档。

## 架构

```text
权威 RustDesk Core
├── feature ohos-har
│   └── 外部 HAR/N-API 适配器
│       └── ArkTS 应用（top.frankhan.resk）
└── feature ohos-flutter
    └── native/ohos_bridge（轻量 FRB cdylib）
        └── Flutter HarmonyOS 应用（top.frankhan.resk.flutter）
```

## 兼容性与限制

- OHOS 文件传输仍保持禁用。
- HAR 被控端仍保持 fail-closed/仅观看。
- 本地 Flutter 调试 Profile 仅用于设备安装，不是 AppGallery 发布签名。
- 当前非特权 Profile 下有意禁用 Flutter 被控端剪贴板；没有绕过包名或
  签名 ACL。

## 验证

本地使用 Flutter-OH `3.41.10-ohos-0.0.2-beta`、FRB `1.80.1`、
HarmonyOS SDK/NDK `6.1.1.280` 和 Java 17 完成验证：

| 检查 | 结果 | SHA-256 |
| --- | --- | --- |
| 统一 `ohos-flutter` bridge（`scripts/build-ohos-ffi.sh`） | 通过；已核对必需 FRB/OHOS 导出符号 | `c3274c67c7123dd1e989a735f5ec89c05fe61eb7a3675a352e983610ab8174a7` |
| Flutter 调试签名 HAP（`scripts/build-ohos-flutter-hap.sh`） | 通过；签名和代码签名块验证成功；包名 `top.frankhan.resk.flutter` | `dd41b17fe8a1151e692c1db5c4cec807dee90298ce0edfe140247ab679bf43ad` |
| HAR（`scripts/build-har.sh`） | 通过；ArkTS 内 HAR 与本轮产物一致 | `ae596e6d1c15eafad5ebf489077772e74aa9b9331b62604813e0178ee279b13c` |
| ArkTS 调试签名 HAP | 通过；签名和代码签名块验证成功；包名 `top.frankhan.resk` | `c173b3d18344c8d4504336f4b6dea6a663dc02f281a1b79970f2b7d496d4a9be` |
| ArkTS 调试签名 App | 通过 | `d3969867a672b7d706a0f1da343789b4e19c777cd86d4535f6e4d67173c1f537` |

`flutter analyze --no-fatal-infos` 未发现 analyzer error；现有 upstream/依赖
warning 与 deprecated 提示仍保留。
