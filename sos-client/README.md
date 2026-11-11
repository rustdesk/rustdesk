# RustDesk SOS 精简受控端

RustDesk 的裁剪版**受控端**（被控方）。仅支持 Windows，单一可执行文件，
托盘交互，可在 UAC 安全桌面上继续操作。

与上游 RustDesk 的差异：

|          | 上游 RustDesk                      | 本项目                                              |
| -------- | ---------------------------------- | --------------------------------------------------- |
| 视频编码 | 平台硬件编码 + VP8/VP9 软件编码    | **仅自建 VP8/VP9**（`libvpx`），不启用硬件编码      |
| 视频链路 | `scrap::Encoder` + `video_service` | 自建 `sos_video` 采集→编码→发送链路                 |
| QoS      | 全局标量                           | **按显示器独立**（每屏独立采集器/编码器/帧率/画质） |
| 界面     | Flutter / Sciter                   | 仅托盘                                              |
| 音频     | 支持                               | **不支持**                                          |
| 端口转发 | 支持                               | **不支持**                                          |
| 部署形态 | 安装为服务                         | 便携单文件，需管理员启动                            |

## 功能

- ✅ 屏幕捕获：DXGI（首选，约 5ms）→ GDI（回退，约 50ms）
- ✅ 编码：VP9（默认）/ VP8，可被控制端的编码偏好覆盖
- ✅ 鼠标键盘输入注入
- ✅ 剪贴板同步（文本 + 文件列表）
- ✅ 文件传输
- ✅ UAC 安全桌面：SYSTEM 子进程通过共享内存送帧，可在提权对话框上操作
- ✅ 多显示器：每块屏独立的采集管线与 QoS
- ✅ 编码成本自适应：编不进帧预算时逐级降 `cpu-used`，再不行才试 VP8（实测不划算会自行回退）

## 构建

需要 vcpkg 提供静态 `libvpx`（`VCPKG_ROOT` 指向 vcpkg 安装目录）。

```powershell
cd sos-client

# 发布构建：cargo build --release + 复制到 dist + UPX 压缩 + 校验（推荐）
cargo xtask
```

产物有两个，用途不同：

| 路径                              | 说明                                                    |
| --------------------------------- | ------------------------------------------------------- |
| `target\release\rustdesk-sos.exe` | cargo 原样输出，**不压缩**，供增量编译使用              |
| `dist\rustdesk-sos.exe`           | 交付物，UPX 压缩后（实测 **8.75 MB → 2.82 MB，32.3%**） |

`build.rs` 用 `bindgen` 生成 libvpx 绑定。视频链路**强依赖**它：没有 libvpx 时
`build.rs` 不会置 `sos_vpx`，`main.rs` 里对应的 `compile_error!` 会直接说明原因
（早先那条「回退到上游 `video_service`」的分支其实编译不过，已删除）。

### IDE 接入（rust-analyzer）

本仓库里有 **三个互相独立** 的 Cargo 项目：

| 项目                          | 说明                                                          |
| ----------------------------- | ------------------------------------------------------------- |
| `Cargo.toml`                  | 根 workspace（`rustdesk` + `libs/*`）                         |
| `sos-client/Cargo.toml`       | 独立项目，被根 workspace 的 `exclude` 排除，自带 `Cargo.lock` |
| `sos-client/xtask/Cargo.toml` | 又一层独立 workspace（自带 `[workspace]`），`cargo xtask` 用  |

`cargo metadata` 对 `sos-client/Cargo.toml` 报告的 `workspace_root` 就是
`sos-client` 自己，所以三者在 Cargo 眼里完全平等。但 rust-analyzer 默认只做
「自动发现」，被上层 workspace `exclude` 掉的项目容易落在发现结果之外 ——
表现是 `sos-client` 下的文件不属于任何已加载项目（没有补全/跳转，并弹出
「以下文件未包含在任何项目中」）。

仓库根的 `.vscode/settings.json` 用 `rust-analyzer.linkedProjects` 把三个项目
显式列了出来（注意 `.vscode` 在根 `.gitignore` 里，该文件是**本机配置**，
换机器需要重新建）。若不想改配置，把 `sos-client` 作为第二个工作区文件夹
加进 VS Code 也能达到同样效果。

### 为什么不直接 `cargo build --release`

直接构建也能用，但产物是未压缩的 8.75 MB。`cargo xtask` 额外做三件事：
编译 → 复制到 `dist\` → 压缩并校验。

**压缩的是副本，不是 `target\release` 里的原件。** 这一点很关键：cargo 用产物
mtime 判断是否需要重新链接，如果原地改写产物，每次 `cargo build` 都会重新
链接（多花几十秒）。因此 `target\release` 永远保持 cargo 出厂的原始状态，
`dist\` 每次覆盖重来，不存在「压过没有」的判断分支。

`xtask` 是一个零依赖的小 crate（`xtask/`），通过 `.cargo\config.toml` 里的
cargo 别名调用。

### 参数

```powershell
cargo xtask                              # 默认：best + lzma，输出到 dist\
cargo xtask --no-upx                     # 只复制不压缩
cargo xtask --out-dir out                # 换个输出目录
cargo xtask --level fast                 # 压缩等级 best|normal|fast
cargo xtask --no-lzma                    # 不用 LZMA（体积略大、解压稍快）
cargo xtask --upx-path C:\upx\upx.exe    # 指定 upx.exe
cargo xtask -- --offline                 # `--` 之后的参数原样透传给 cargo build
cargo xtask --help                       # 完整帮助
```

### UPX 查找顺序

`--upx-path` → 环境变量 `UPX_PATH` → 本机 `tools\upx\**\upx.exe`
→ `PATH` → 常见安装位置（Program Files / scoop / chocolatey）→ **自动下载**。

**`tools\` 目录不入库**（见 `.gitignore`）。里面的东西都是本机工具、随用随放：
`tools\upx\` 放 UPX 副本（xtask 会自动发现，免去设环境变量），
`tools\diag-wmi.*` 是排查 WMI CPU 占用的现场诊断脚本。两者都不是交付源码，
所以不跟着仓库走。

### 自动补全 UPX

前面都找不到时，xtask 会向 GitHub API 查询 UPX **最新 release**，下载 win64 zip
到 `tools\upx\`，用该 release 的官方 sha256 强制校验后解压使用。之后就一直复用
这份本地副本，不会重复下载。

**版本与哈希都不在本项目里钉死**——这样 UPX 发布安全修复后，下一次构建就能用上，
不需要改代码、也不需要等宿主项目发版。

| 场景            | 做法                                                                        |
| --------------- | --------------------------------------------------------------------------- |
| 正常联网        | 什么都不用做                                                                |
| 内网镜像 / 代理 | `--upx-url <URL>`（仍用 GitHub 公布的 sha256 校验，确保与官方资产字节一致） |
| 完全离线        | `--upx-path` 指定本地 `upx.exe`，或先 `winget install UPX.UPX`              |
| 禁止联网下载    | `--no-upx-fetch` 或环境变量 `SOS_NO_UPX_FETCH=1`                            |
| 不压缩          | `--no-upx`                                                                  |

下载失败或校验不符时，`dist\` 里仍留着一份**可用的未压缩 exe**，只是退出码为 1。

### 三条注意事项

- **签名必须在压缩之后**：UPX 会改写 PE 布局，先签名会被破坏。
- **杀软误报**：UPX 壳 + 远程控制类程序是启发式规则的经典组合，部分 EDR
  可能拦截。部署环境严格时用 `--no-upx` 出未压缩版本。
- 已有运行中的 `rustdesk-sos.exe` 会被 `build.rs` 先 `taskkill` 掉（否则链接失败）。

### 不带 UPX 的普通构建

```powershell
cargo build --release --manifest-path sos-client\Cargo.toml
# 或
cargo xtask --no-upx
```

## 运行

```powershell
# 默认管理服务器
rustdesk-sos.exe

# 指定管理服务器与公钥
rustdesk-sos.exe --manager hbbs.example.com --key <base64-pubkey>

# 指定中继服务器
rustdesk-sos.exe --relay hbbr.example.com

# 固定密码（不传则每次连接断开后自动换新密码）
rustdesk-sos.exe --password mypassword

# 输出详细日志到控制台与 rustdesk-sos.log
rustdesk-sos.exe --debug
```

每个参数都有短选项，等价写法：

```powershell
rustdesk-sos.exe -m hbbs.example.com -r hbbr.example.com -k <base64-pubkey> -p mypassword -d
```

### 参数

| 长选项             | 短选项 | 说明                                                                 |
| ------------------ | ------ | -------------------------------------------------------------------- |
| `--manager <HOST>` | `-m`   | 管理（信令/ID）服务器地址，如 `hbbs.example.com`                     |
| `--relay <HOST>`   | `-r`   | 中继服务器地址，不指定则用管理服务器下发的；本机可逗号分隔配多台后备 |
| `--key <BASE64>`   | `-k`   | 管理服务器公钥，自定义服务器时必填                                   |
| `--password <PWD>` | `-p`   | 固定密码；不传则每次断开后自动换新密码                               |
| `--debug`          | `-d`   | 分配控制台并输出详细日志                                             |
| `--service`        | `-s`   | 内部使用：主进程以 SYSTEM 身份拉起自身时传入                         |

`--debug` 会打开大量诊断日志（信令收包、QoS 采样、分阶段耗时等），
生产环境不要开。

### 中继选择

候选来源优先级：**`--relay` > 管理服务器下发 > 管理服务器端口 +1**。
本地配置优先，与上游 `get_relay_server()` 一致（内网部署或临时切换时，
本机比服务器更清楚该走哪台）。

**多中继是服务器侧的配置概念，不是协议字段的做法。** 要区分清楚：

| 位置                                                    | 形态               |
| ------------------------------------------------------- | ------------------ |
| hbbs 配置（`relay-servers=` / `RELAY=` 环境变量）       | 逗号分隔的**多台** |
| hbbs 内部 `relay_servers: Vec<String>`                  | 列表               |
| hbbs `get_relay_server()` 选一台后下发                  | **单个**主机名     |
| 协议 `relay_server` 字段（PunchHole / RequestRelay 等） | **单个**主机名     |

hbbs 每连接自行选一台：开源版轮询（`ROTATION_RELAY_SERVER`），
Pro 版按 GeoLite2 选地理最近的（见官方文档 _Configure Relay Servers_）。
客户端拿到的永远是单值，**没有在多台之间挑选的余地**。

`--relay` 支持逗号分隔，是为了让**本机**也能配后备（服务器只给一台，
而那一台可能正好挂了，或 hbbs 配置有误）：

```powershell
rustdesk-sos.exe -m hbbs.example.com -r relay1.example.com,relay2.example.com
```

按顺序逐个尝试取首个连通的那台，总耗时以一次 `CONNECT_TIMEOUT` 为预算封顶
（不劣于上游单次尝试）。`RelayResponse.relay_server` 上报的是**实际连上的那台**
—— 该字段会被 hbbs 转发给对端，对端据此决定连哪台，报错就会配不上。
全部失败时错误信息会列出每台的失败原因。

### 单实例

程序持有 `Global\RustDeskSOS_main` 互斥体，重复启动会提示后退出。

## 服务器端

依赖 RustDesk 服务端组件（[rustdesk/rustdesk-server](https://github.com/rustdesk/rustdesk-server)）：

- 信令服务器 `hbbs`（UDP 21116）
- 中继服务器 `hbbr`（TCP 21117）

## 要求

- Windows 10+ x86_64
- 必须以管理员权限运行（否则直接弹窗退出）
- DXGI 捕获需要 DirectX 11 兼容 GPU；远程桌面会话下 DXGI 不可用，自动降级到 GDI

## 代码结构

`src/` 全部平铺并用 `sos_` 前缀，是为了让 `tests/*.rs` 能用
`#[path = "../src/sos_xxx.rs"] mod sos_xxx;` 单独复用某个模块做单测
（测试里的 `crate::sos_xxx` 因此也能解析）。`main.rs` 顶部有同一份模块地图。

```
src/
  main.rs            入口、模式分发、SYSTEM 子进程看门狗
  ── 启动 / 进程 ──
  sos_bootstrap.rs   提权检查、单实例、日志
  sos_config.rs      注册表配置、ID/密码
  sos_constants.rs   全局常量与 Windows 对象名
  sos_system.rs      SYSTEM 子进程入口与令牌提升
  sos_tray.rs        托盘
  sos_cpu.rs         CPU 占用采样（GetSystemTimes，不用 PDH）
  ── 捕获 ──
  sos_capture.rs     SYSTEM 子进程的采集线程（写共享内存）
  sos_dxgi.rs        DXGI Desktop Duplication 捕获
  sos_gdi.rs         GDI 捕获（安全桌面自检/降级/状态图）
  sos_display.rs     显示器枚举与几何查询
  sos_uac.rs         UAC 安全桌面 / 会话连接状态检测
  ── 视频 ──
  sos_video.rs       采集→编码→发送链路 + QoS 编排
  sos_vpx.rs         libvpx 封装、编码格式决策、成本判定（纯函数可单测）
  sos_qos.rs         按显示器的帧率/画质/码率状态机（纯逻辑可单测）
  ── 网络 ──
  sos_connection.rs  单连接生命周期（鉴权、消息泵、订阅）
  sos_rendezvous.rs  信令（UDP 注册、打洞、中继）
  sos_relay.rs       中继候选解析与优先级（纯函数可单测）
  sos_file_transfer.rs  文件传输
  sos_pipe.rs        命名管道（Main → System）
  sos_shmem.rs       共享内存（帧数据通道）
  ── 输入 ──
  sos_input.rs       输入事件分派
platform/windows.cc  SYSTEM 令牌提升、桌面切换钩子
.cargo/config.toml  cargo 别名（xtask）
xtask/              发布构建助手（编译 → 复制到 dist → UPX 压缩 → 校验）
tools/              本机工具（不入库）：WMI 诊断脚本、可选的 UPX 副本
dist/               交付物输出目录（不入库）
tests/              编码/捕获/QoS/CPU 的实测与回归测试
```

`libs/` 与仓库根 `src/`（上游 RustDesk）**不做任何修改**，所有改动都在 `sos-client/` 内。
