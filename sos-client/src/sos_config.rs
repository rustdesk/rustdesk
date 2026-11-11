//! SOS 注册表配置模块
//!
//! 替代 `hbb_common::config::Config`，所有配置存储在
//! `HKLM\SOFTWARE\RustDeskSOS` 注册表键下。
//!
//! 优先级：CLI 参数 > 注册表 > 硬编码默认值

use crate::sos_constants;
use hbb_common::ResultType;
use std::sync::Mutex;
use winreg::enums::*;
use winreg::RegKey;

// ── 内存中的临时密码（不写入注册表，每次启动/连接断开后重新生成） ──

static CURRENT_PASSWORD: std::sync::OnceLock<Mutex<String>> = std::sync::OnceLock::new();

/// 获取当前内存中的临时密码
pub fn get_current_password() -> String {
    CURRENT_PASSWORD
        .get()
        .and_then(|m| m.lock().ok().map(|g| g.clone()))
        .unwrap_or_default()
}

/// 设置当前内存中的临时密码
pub fn set_current_password(pwd: String) {
    let lock = CURRENT_PASSWORD.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut g) = lock.lock() {
        *g = pwd;
    }
    // 通知托盘刷新 tooltip（事件驱动，零轮询开销）
    crate::sos_tray::notify_password_changed();
}

/// 生成随机临时密码（8 位字母数字）
pub fn generate_temp_password() -> String {
    use hbb_common::rand::Rng;
    (0..6)
        .map(|_| hbb_common::rand::thread_rng().gen_range(0..10).to_string())
        .collect()
}

// ── 内存中的 NAT 类型（不写入注册表） ──

static CURRENT_NAT_TYPE: std::sync::OnceLock<Mutex<i32>> = std::sync::OnceLock::new();

/// 获取缓存的 NAT 类型（0=UNKNOWN, 1=ASYMMETRIC, 2=SYMMETRIC）
pub fn get_nat_type() -> i32 {
    CURRENT_NAT_TYPE
        .get()
        .and_then(|m| m.lock().ok().map(|g| *g))
        .unwrap_or(0)
}

/// 存储 NAT 类型
pub fn set_nat_type(t: i32) {
    let lock = CURRENT_NAT_TYPE.get_or_init(|| Mutex::new(0));
    if let Ok(mut g) = lock.lock() {
        *g = t;
    }
}

// ── 注册表配置 ──

/// 注册表配置管理
pub struct RegistryConfig;

impl RegistryConfig {
    // ── 底层注册表读写 ──

    fn open_key() -> ResultType<RegKey> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let (key, _) = hklm.create_subkey(sos_constants::REGISTRY_PATH)?;
        Ok(key)
    }

    pub fn get_string(name: &str, default: &str) -> String {
        Self::open_key()
            .and_then(|k| k.get_value(name).map_err(Into::into))
            .unwrap_or_else(|_| default.to_string())
    }

    pub fn get_binary(name: &str) -> ResultType<Vec<u8>> {
        // winreg 不支持直接读 REG_BINARY；改用 hex 编码存储
        let s = Self::get_string(name, "");
        if s.is_empty() {
            return Ok(vec![]);
        }
        hex::decode(&s).map_err(|e| anyhow::anyhow!("Failed to decode hex value: {}", e))
    }

    /// 检查注册表键是否存在（首次运行检测）
    pub fn is_initialized() -> bool {
        Self::open_key()
            .and_then(|k| k.get_value::<String, _>("ID").map_err(Into::into))
            .is_ok()
    }

    // ── 配置读取接口（与 hbb_common::Config 兼容） ──

    pub fn get_id() -> String {
        Self::get_string("ID", "")
    }

    /// 获取 Ed25519 密钥对：优先注册表（兼容旧数据），否则在内存中生成一份
    pub fn get_key_pair() -> (Vec<u8>, Vec<u8>) {
        // 先检查注册表
        if let Ok(v) = Self::get_binary("KeyPair") {
            if v.len() == 64 {
                let sk = v[..32].to_vec();
                let pk = v[32..].to_vec();
                return (sk, pk);
            }
        }
        // 注册表没有 → 在内存中生成（仅本次会话有效）
        use std::sync::Mutex;
        static MEM_KEYPAIR: std::sync::OnceLock<Mutex<(Vec<u8>, Vec<u8>)>> =
            std::sync::OnceLock::new();
        let lock = MEM_KEYPAIR.get_or_init(|| {
            let (pk, sk) = hbb_common::sodiumoxide::crypto::sign::gen_keypair();
            Mutex::new((sk.0.to_vec(), pk.0.to_vec()))
        });
        lock.lock().unwrap().clone()
    }

    /// 记录公钥是否已在信令服务器确认（`RegisterPkResponse` 成功后置位）
    ///
    /// `UUID_MISMATCH` 时会被清回 `false`，随后换 ID 重新注册。
    pub fn set_key_confirmed(v: bool) {
        if let Ok(key) = Self::open_key() {
            let _ = key.set_value("KeyConfirmed", &u32::from(v));
        }
    }

    /// 获取持久化的密码盐值（不存在时自动生成并存储）
    /// 用于跨连接一致的密码哈希，使 token 认证在文件传输等场景下有效
    pub fn get_password_salt() -> String {
        let salt = Self::get_string("Salt", "");
        if !salt.is_empty() {
            return salt;
        }
        // 生成新的随机盐值
        use hbb_common::rand::Rng;
        let salt: String = (0..12)
            .map(|_| {
                hbb_common::rand::thread_rng().sample(hbb_common::rand::distributions::Alphanumeric)
                    as char
            })
            .collect();
        if let Ok(key) = Self::open_key() {
            let _ = key.set_value("Salt", &salt);
        }
        salt
    }

    // ── 首次运行初始化 ──

    /// 清理旧版遗留的注册表键（SOS 现仅存设备 ID，其余均内存管理）
    pub fn clean_legacy_keys() {
        const LEGACY_KEYS: &[&str] = &[
            "KeyPair",
            "Password",
            "TemporaryPassword",
            "RendezvousServer",
            "KeyConfirmed",
            "EncID",
            "NatType",
        ];
        if let Ok(key) = Self::open_key() {
            for k in LEGACY_KEYS {
                let _ = key.delete_value(k);
            }
        }
    }

    /// 首次运行时生成设备 ID 并写入注册表
    ///
    /// 注册表仅存储设备 ID，用于保持跨启动的设备身份一致性。
    /// 密钥对、密码、信令服务器等均为内存管理（SOS 为一次性工具）。
    pub fn init_first_run(cli_id: &str, _cli_manager: &str) -> ResultType<()> {
        // 清理旧版遗留键
        Self::clean_legacy_keys();

        let key = Self::open_key()?;

        // 仅存储设备 ID
        let id = if !cli_id.is_empty() {
            cli_id.to_string()
        } else {
            Self::generate_auto_id()
        };
        key.set_value("ID", &id)?;
        log::info!("Device ID: {}", id);

        Ok(())
    }

    /// 生成新的随机 ID（用于 UUID_MISMATCH 时更换身份）
    /// 范围 1_000_000_000..2_000_000_000，与上游 Config::update_id() 一致
    pub fn update_id() -> String {
        use hbb_common::rand::Rng;
        let old_id = Self::get_id();
        let new_id = hbb_common::rand::thread_rng()
            .gen_range(1_000_000_000u64..2_000_000_000u64)
            .to_string();
        if let Ok(key) = Self::open_key() {
            let _ = key.set_value("ID", &new_id);
        }
        log::info!("id updated from {} to {}", old_id, new_id);
        new_id
    }

    /// 基于本机 MAC 地址生成 ID（与 RustDesk 原版一致）
    fn generate_auto_id() -> String {
        let mut id = 0u32;
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            if let Ok(Some(ma)) = hbb_common::mac_address::get_mac_address() {
                for x in &ma.bytes()[2..] {
                    id = (id << 8) | (*x as u32);
                }
                id &= 0x1FFFFFFF;
            }
        }
        if id == 0 {
            use hbb_common::rand::Rng;
            id = hbb_common::rand::thread_rng().gen_range(1_000_000u32..2_000_000_000u32);
        }
        id.to_string()
    }
}

/// SOS 运行时配置（从注册表和 CLI 合并）
#[derive(Clone)]
pub struct SosConfig {
    pub id: String,
    pub rendezvous_server: String,
    /// 中继服务器（来自 `--relay`）。空串表示交给信令服务器下发。
    /// 支持逗号分隔多个地址，由 `sos_rendezvous::resolve_relay_candidates`
    /// 逐个尝试。
    pub relay_server: String,
    pub password: String,       // CLI 传入的临时密码
    pub server_pub_key: String, // 信令服务器公钥（Base64）
}

impl SosConfig {
    /// 从 CLI 参数和注册表构建运行时配置
    pub fn from_cli_and_registry(
        cli_manager: &str,
        cli_relay: &str,
        cli_key: &str,
        cli_password: &str,
    ) -> ResultType<Self> {
        // 确保注册表已初始化
        if !RegistryConfig::is_initialized() {
            log::info!("First run detected, initializing registry...");
            RegistryConfig::init_first_run("", cli_manager)?;
        }

        // ID: 始终从注册表读取（自动生成或首次运行生成）
        let id = {
            let rid = RegistryConfig::get_id();
            if rid.is_empty() {
                RegistryConfig::init_first_run("", cli_manager)?;
                RegistryConfig::get_id()
            } else {
                rid
            }
        };

        // 信令服务器: CLI > 默认（不读注册表，SOS 为一次性工具）
        let rendezvous_server = if !cli_manager.is_empty() {
            cli_manager.to_string()
        } else {
            sos_constants::default_rendezvous_server()
        };

        // 信令服务器公钥：CLI > RustDesk 官方默认
        let server_pub_key = if !cli_key.is_empty() {
            cli_key.to_string()
        } else {
            hbb_common::config::RS_PUB_KEY.to_string()
        };

        // 中继服务器：CLI > 空字符串（由信令服务器自动下发）
        let relay_server = if !cli_relay.is_empty() {
            cli_relay.to_string()
        } else {
            String::new()
        };

        Ok(Self {
            id,
            rendezvous_server,
            relay_server,
            password: cli_password.to_string(),
            server_pub_key,
        })
    }
}

// ── 上游剪贴板服务全局实例 ──

use librustdesk::service::GenericService;

/// 上游 clipboard_service 全局实例（由 main 初始化，连接循环订阅）
pub static CLIPBOARD_SVC: std::sync::OnceLock<GenericService> = std::sync::OnceLock::new();

/// 设置剪贴板服务实例
pub fn set_clipboard_service(svc: GenericService) {
    let _ = CLIPBOARD_SVC.set(svc);
}
