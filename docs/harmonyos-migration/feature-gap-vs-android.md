# RustDesk 鸿蒙主控端 vs Android 端 —— 功能差距比对（M1 后）

> 比对基准：Android 客户端（rustdesk 1.5.1，`flutter/lib/mobile/` + Kotlin 原生层）
> 比对对象：`harmonyos/`（ArkTS UI + NAPI + librustdesk.so），M1 主控端里程碑完成态
> 上游分析：`android-feature-analysis.md`（Android 功能清单，编号 #1-#44 沿用）
> 日期：2026-10-25

## 一、总览

M1 目标是"纯客户端主控"，不碰被控端系统权限，因此被控端功能（#1-#22）整体未开工。
主控侧（#36-#44）Rust 会话引擎全量复用，ArkTS UI 已覆盖日常远控主路径；
差距集中在 **音频、硬件解码、部分编码格式、被控端全部能力**。

| 维度 | Android | OHOS M1 | 差距评级 |
|------|---------|---------|---------|
| 架构 | Flutter UI + JNI | ArkTS UI + NAPI/dlopen | 等价（技术路线不同） |
| 会话引擎 | Rust `Session<T>` | 同一份 Rust 代码 | 无差距 |
| UI 代码形态 | Flutter（移动+桌面共用） | ArkTS（手写移动端三页） | 功能面窄（见下） |
| 被控端（Share Screen） | 完整 | 无 | **整体缺失（M2+）** |

## 二、主控端功能逐项比对

| Android 功能 | OHOS M1 状态 | 说明 / 差距来源 |
|------|------|------|
| #36 远程控制（观看+触控+键盘） | ✅ 已实现 | XComponent 直渲 RGBA；Tap/长按/拖动/双指滚轮/指针模式；软键盘差量文本 + 修饰键 + 快捷键行；触控光标叠加 |
| #36a 多显示器切换 | ✅ 已实现 | 工具栏显示器切换行（`ohos_switch_display`）；Android 端在显示菜单里，鸿蒙为平铺按钮 |
| #36b 双指缩放/平移 | ✅ 已实现 | pinch 以适屏中心缩放 + 拖动平移 + 视口钳制（Android 为 InteractiveViewer 等价物） |
| #36c 远端光标渲染 | ✅ 已实现 | `cursor_data/id/position` → PixelMap 叠加层；工具栏开关；对端 Android/`cursor_embedded` 不绘制 |
| #36d 快捷操作 | ✅ 已实现 | Ctrl+Alt+Del / Alt+F4 / Win / Win+R / Win+D / Win+L / 锁屏 / 任务管理器（三指呼出或工具栏） |
| #37 触控手势模式 | ✅ 已实现 | 指针模式（移动即移动远端光标，点按落在光标处）；Android 端为对端 Android 强制 touch mode，鸿蒙同语义由 UI 侧提供 |
| #38 软键盘输入 | ✅ 已实现 | `TextInput` 差量 → `ohos_input_string`，与 `remote_page.dart` 同算法 |
| #39 横竖屏适配 | ✅ 已实现 | `onAreaChange` 检测方向；横屏工具栏自动隐藏 + 角标呼回 |
| #40 硬件解码 | ❌ 未实现 | 仅 libvpx 软解 VP8/VP9；AV1 已门控（aom 存桩）；H264/H265 未接。Android 有 MediaCodec 硬解。后续 OH_VideoDecoder（M4） |
| #36e 音频（听被控端声音） | ❌ 未实现 | `start_audio` 明确报不支持、解码帧丢弃（`src/client.rs` ohos 分支）。Android 完整。后续 OHAudio（M4） |
| #36f 语音通话（麦克风双向） | ❌ 未实现 | `io_loop` voice_call 走 iOS 模板（空实现）。Android 有 AudioRecord 双向通路（M4） |
| #41 文件传输 | ✅ 已实现（有边界） | 双面板 + 上传/下载 + 进度 + 覆盖确认 + 取消；本地面板沙箱目录；**沙箱外导入仅文件**（手机上系统 picker 不支持选文件夹，平板/PC 可）、**导出文件夹打包为 zip**（系统保存 picker 无树形写出）。Android SAF 可原路径递归写出 |
| #42 剪贴板（客户端方向） | ⚠️ 半实现 | 远端→本机：自动（`clipboard_cb` → `pasteboard.setData`）；本机→远端：**手动**（工具栏按钮 → 粘贴弹窗 → `ohos_send_clipboard`）。直读剪贴板需 ACL 受限权限 `READ_PASTEBOARD`，正式签名可申请后改为回前台自动推。Android 双向自动。仅文本，无图片/HTML |
| #43 会话亮屏保活 | ✅ 已实现 | `setWindowKeepScreenOn`（远程页+文件传输页）；Android 用 Wakelock 双通道 |
| #44 最近会话持久化 | ✅ 已实现 | `@ohos.data.preferences`，保留最近 10 条（Android 为 peer 记录 + ab 全量） |
| #44a 连接选项 | ⚠️ 精简 | 有强制中继（连接时勾选）、密码记忆；**无**：ID/中继/API 服务器设置 UI 之外的完整选项面、画质选择、录制、隧道/端口反向、CLI/deep link |
| #44b 服务器配置 | ✅ 已实现 | 自定义 ID/中继/API 服务器 + Key 写入 `RustDesk2.toml`（`ohos_set_option`），下次连接生效 |
| #44c 会话录制 | ❌ 未实现 | Android 可录制会话到 MP4（M4+） |
| #44d 摄像头查看 | ❌ 未实现 | 依赖对端能力 + 解码面，未接（M4+） |
| #44e 终端/端口转发 | ❌ 未实现 | Android 移动端亦无终端；端口转发为桌面功能，两端一致 |
| — 局域网发现 | ✅ 已实现 | `lan.rs` 复用，mDNS/广播同 Android |
| — 2FA 登录 | ✅ 已实现 | `msgbox` 2FA 事件 → 验证码弹窗 → `ohos_session_send2fa` |
| — 密码登录/重连 | ✅ 已实现 | 密码弹窗、`session_login`、重连（含强制中继重连） |
| — 多语言 | ✅ 中/英 | resources `base` + `zh_CN`；Android 全量语言包。按需增补 |

## 三、被控端能力（Android 有、OHOS 无 —— M2/M3 范围）

| Android 功能 | OHOS 状态 | 鸿蒙路径（`android-feature-analysis.md` 已评估） |
|------|------|------|
| #1 屏幕采集 | ❌ | AVScreenCapture（ContinuousTask + 授权流） |
| #4/#5 音频采集/语音通话（被控） | ❌ | AVScreenCapture 内录 / AudioCapturer |
| #6 前台服务+常驻通知 | ❌ | ContinuousTask + NotificationKit |
| #7 连接请求通知/授权 | ❌ | 通知 + emitter，或 UI 弹窗 |
| #11-#15 输入注入（无障碍） | ❌ | `inputEventClient.injectEvent` 需系统签名/受限权限，**最大风险项**（M3） |
| #17-#19 被控剪贴板 | ❌ | pasteboard 直译 |
| #20-#22 悬浮窗 | ❌ | systemWindow（M4） |
| #9 开机自启 / #35 电池白名单 | ❌ | 平台限制，首版砍掉/降级 |
| #23-#26 被控文件沙箱 | ❌（主控方向已实现） | home 目录 = filesDir，Rust 校验逻辑复用 |
| #28 硬编解码能力上报 | ❌ | AVCodecList JSON 同构喂给 Rust |

## 四、差距清单（按优先级排序的后续工作）

1. **M2 被控端**：AVScreenCapture + ContinuousTask + 被控剪贴板/文件沙箱 + 连接授权弹窗。这是与 Android 差距的主体。
2. **M3 输入注入**：权限方案验证（`INJECT_EVENT` 系统签名/受限开放权限），鼠标/触摸/键盘映射。
3. **M4 体验补全**（主控侧当前已知缺口）：
   - 音频播放（OHAudio C API，Rust 可直接链接）
   - 硬件解码 H264/H265/AV1（OH_VideoDecoder 绑定）
   - 语音通话麦克风（AudioCapturer + IO 分支去 iOS 模板）
   - 会话录制、摄像头查看、画质选项、图片/HTML 剪贴板
4. **签名解锁项**：正式签名后 `READ_PASTEBOARD` 可申请 → 本机→远端剪贴板改自动；平板上启用文件夹导入 picker。
5. **AV1**：aom 源获取（gstreamer 镜像/vendored 压缩包）后去掉存桩门控。

## 五、结论

主控端日常远控主路径（观看 VP8/VP9、输入、文件传输、剪贴板、多显示器、2FA、服务器配置）
已对齐 Android 移动端的核心体验；**明确落后于 Android 的主控功能是音频、硬件解码、
会话录制、语音通话、图片剪贴板**。被控端能力（Android 的另一半）尚未开始，
是 M2/M3 的全部内容。整体功能覆盖度约为主控路径 ~85%、全功能 ~40%。
