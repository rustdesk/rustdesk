//! SOS 客户端专用常量定义
//!
//! 从 `hbb_common::config` 中提取 SOS 需要的常量，避免依赖完整 Config 结构。

/// 信令服务器注册间隔（毫秒）
/// 与 hbb_common::config::REG_INTERVAL 一致
pub const REG_INTERVAL: u64 = 15_000;

/// TCP 连接超时（毫秒）
pub const CONNECT_TIMEOUT: u64 = 18_000;

/// 默认信令服务器 — 引用 hbb_common 的配置，与 RustDesk 主项目保持一致
pub fn default_rendezvous_server() -> String {
    hbb_common::config::RENDEZVOUS_SERVERS
        .first()
        .copied()
        .unwrap_or("rs-ny.rustdesk.com")
        .to_string()
}

/// 信令服务器默认端口
pub use hbb_common::config::RENDEZVOUS_PORT;

/// 注册表路径
pub const REGISTRY_PATH: &str = "SOFTWARE\\RustDeskSOS";

/// 应用名称
pub const APP_NAME: &str = "RustDesk SOS";

/// 上报给信令服务器与控制端的协议版本。
///
/// 唯一来源：`Cargo.toml` 的 `version`。控制端拿它与 `1.2.4` / `1.3.0` / `1.4.5`
/// 等阈值做功能门控，所以**必须跟上上游协议基线**，不能填自建版本号。
/// 字段分散在 `LocalAddr`、`PunchHoleSent`、`RelayResponse`、`PeerInfo` 四处，
/// 以前各自硬编码字符串，曾经出现信令侧报 1.4.4、控制端侧报 0.1.0 的不一致。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 输入管道名（Main → System 输入事件）
///
/// 命名管道本来就在全局命名空间（`\\.\pipe\`），跨会话可用，无需改动。
pub const INPUT_PIPE: &str = r"\\.\pipe\sos_input";

/// 输入管道消息类型
pub const MSG_MOUSE: u32 = 1;
pub const MSG_KEY: u32 = 2;
pub const MSG_SHUTDOWN: u32 = 3;

// ── 命名事件（跨会话）──
//
// SYSTEM 子进程现在优先运行在**控制台会话**（session 1），而主进程在用户的
// RDP 会话（可能是 session 2）。**默认命名空间按会话隔离**，不带 `Global\`
// 前缀的话子进程 `OpenEventW` 会一直失败（`Failed to open capture
// start/stop events`），整个安全桌面采集链路就断了。
//
// 创建方（主进程）需要 `SeCreateGlobalPrivilege`，见
// `sos_shmem::enable_create_global_privilege`；SYSTEM 侧默认具备该特权。

/// 捕获就绪事件（捕获线程写完首帧 → SetEvent，主进程 WaitForSingleObject）
pub const CAPTURE_READY_EVENT: &str = r"Global\SosCaptureReady";

/// 捕获启停事件（主进程 SetEvent → 子进程 WaitForMultipleObjects）
pub const CAPTURE_START_EVENT: &str = r"Global\SosCaptureStart";
pub const CAPTURE_STOP_EVENT: &str = r"Global\SosCaptureStop";
