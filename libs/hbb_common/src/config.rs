use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::{Mutex, RwLock},
    time::{Duration, Instant, SystemTime},
};

use anyhow::Result;
use bytes::Bytes;
use rand::Rng;
use regex::Regex;
use serde as de;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use sodiumoxide::base64;
use sodiumoxide::crypto::sign;

use crate::{
    compress::{compress, decompress},
    log,
    password_security::{
        decrypt_str_or_original, decrypt_vec_or_original, encrypt_str_or_original,
        encrypt_vec_or_original, symmetric_crypt,
    },
};

pub const RENDEZVOUS_TIMEOUT: u64 = 12_000;
pub const CONNECT_TIMEOUT: u64 = 18_000;
pub const READ_TIMEOUT: u64 = 18_000;
// https://github.com/quic-go/quic-go/issues/525#issuecomment-294531351
// https://datatracker.ietf.org/doc/html/draft-hamilton-early-deployment-quic-00#section-6.10
// 15 seconds is recommended by quic, though oneSIP recommend 25 seconds,
// https://www.onsip.com/voip-resources/voip-fundamentals/what-is-nat-keepalive
pub const REG_INTERVAL: i64 = 15_000;
pub const COMPRESS_LEVEL: i32 = 3;
const SERIAL: i32 = 3;
const PASSWORD_ENC_VERSION: &str = "00";
pub const ENCRYPT_MAX_LEN: usize = 128; // used for password, pin, etc, not for all

#[cfg(target_os = "macos")]
lazy_static::lazy_static! {
    pub static ref ORG: RwLock<String> = RwLock::new("com.carriez".to_owned());
}

type Size = (i32, i32, i32, i32);
type KeyPair = (Vec<u8>, Vec<u8>);

lazy_static::lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new(Config::load());
    static ref CONFIG2: RwLock<Config2> = RwLock::new(Config2::load());
    static ref LOCAL_CONFIG: RwLock<LocalConfig> = RwLock::new(LocalConfig::load());
    static ref TRUSTED_DEVICES: RwLock<(Vec<TrustedDevice>, bool)> = Default::default();
    static ref ONLINE: Mutex<HashMap<String, i64>> = Default::default();
    pub static ref PROD_RENDEZVOUS_SERVER: RwLock<String> = RwLock::new(match option_env!("RENDEZVOUS_SERVER") {
        Some(key) if !key.is_empty() => key,
        _ => "",
    }.to_owned());
    pub static ref EXE_RENDEZVOUS_SERVER: RwLock<String> = Default::default();
    pub static ref APP_NAME: RwLock<String> = RwLock::new("RustDesk".to_owned());
    static ref KEY_PAIR: Mutex<Option<KeyPair>> = Default::default();
    static ref USER_DEFAULT_CONFIG: RwLock<(UserDefaultConfig, Instant)> = RwLock::new((UserDefaultConfig::load(), Instant::now()));
    pub static ref NEW_STORED_PEER_CONFIG: Mutex<HashSet<String>> = Default::default();
    pub static ref DEFAULT_SETTINGS: RwLock<HashMap<String, String>> = Default::default();
    pub static ref OVERWRITE_SETTINGS: RwLock<HashMap<String, String>> = Default::default();
    pub static ref DEFAULT_DISPLAY_SETTINGS: RwLock<HashMap<String, String>> = Default::default();
    pub static ref OVERWRITE_DISPLAY_SETTINGS: RwLock<HashMap<String, String>> = Default::default();
    pub static ref DEFAULT_LOCAL_SETTINGS: RwLock<HashMap<String, String>> = Default::default();
    pub static ref OVERWRITE_LOCAL_SETTINGS: RwLock<HashMap<String, String>> = Default::default();
    pub static ref HARD_SETTINGS: RwLock<HashMap<String, String>> = Default::default();
    pub static ref BUILTIN_SETTINGS: RwLock<HashMap<String, String>> = Default::default();
}

lazy_static::lazy_static! {
    pub static ref APP_DIR: RwLock<String> = Default::default();
}

#[cfg(any(target_os = "android", target_os = "ios"))]
lazy_static::lazy_static! {
    pub static ref APP_HOME_DIR: RwLock<String> = Default::default();
}

pub const LINK_DOCS_HOME: &str = "https://rustdesk.com/docs/en/";
pub const LINK_DOCS_X11_REQUIRED: &str = "https://rustdesk.com/docs/en/manual/linux/#x11-required";
pub const LINK_HEADLESS_LINUX_SUPPORT: &str =
    "https://github.com/rustdesk/rustdesk/wiki/Headless-Linux-Support";
lazy_static::lazy_static! {
    pub static ref HELPER_URL: HashMap<&'static str, &'static str> = HashMap::from([
        ("rustdesk docs home", LINK_DOCS_HOME),
        ("rustdesk docs x11-required", LINK_DOCS_X11_REQUIRED),
        ("rustdesk x11 headless", LINK_HEADLESS_LINUX_SUPPORT),
        ]);
}

const CHARS: &[char] = &[
    '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k',
    'm', 'n', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

pub const RENDEZVOUS_SERVERS: &[&str] = &["rs-ny.rustdesk.com"];
pub const PUBLIC_RS_PUB_KEY: &str = "OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=";

pub const RS_PUB_KEY: &str = match option_env!("RS_PUB_KEY") {
    Some(key) if !key.is_empty() => key,
    _ => PUBLIC_RS_PUB_KEY,
};

pub const RENDEZVOUS_PORT: i32 = 21116;
pub const RELAY_PORT: i32 = 21117;

macro_rules! serde_field_string {
    ($default_func:ident, $de_func:ident, $default_expr:expr) => {
        fn $default_func() -> String {
            $default_expr
        }

        fn $de_func<'de, D>(deserializer: D) -> Result<String, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            let s: String =
                de::Deserialize::deserialize(deserializer).unwrap_or(Self::$default_func());
            if s.is_empty() {
                return Ok(Self::$default_func());
            }
            Ok(s)
        }
    };
}

macro_rules! serde_field_bool {
    ($struct_name: ident, $field_name: literal, $func: ident, $default: literal) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct $struct_name {
            #[serde(default = $default, rename = $field_name, deserialize_with = "deserialize_bool")]
            pub v: bool,
        }
        impl Default for $struct_name {
            fn default() -> Self {
                Self { v: Self::$func() }
            }
        }
        impl $struct_name {
            pub fn $func() -> bool {
                UserDefaultConfig::read($field_name) == "Y"
            }
        }
        impl Deref for $struct_name {
            type Target = bool;

            fn deref(&self) -> &Self::Target {
                &self.v
            }
        }
        impl DerefMut for $struct_name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.v
            }
        }
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetworkType {
    Direct,
    ProxySocks,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Config {
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        deserialize_with = "deserialize_string"
    )]
    pub id: String, // use
    #[serde(default, deserialize_with = "deserialize_string")]
    enc_id: String, // store
    #[serde(default, deserialize_with = "deserialize_string")]
    password: String,
    #[serde(default, deserialize_with = "deserialize_string")]
    salt: String,
    #[serde(default, deserialize_with = "deserialize_keypair")]
    key_pair: KeyPair, // sk, pk
    #[serde(default, deserialize_with = "deserialize_bool")]
    key_confirmed: bool,
    #[serde(default, deserialize_with = "deserialize_hashmap_string_bool")]
    keys_confirmed: HashMap<String, bool>,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone)]
pub struct Socks5Server {
    #[serde(default, deserialize_with = "deserialize_string")]
    pub proxy: String,
    #[serde(default, deserialize_with = "deserialize_string")]
    pub username: String,
    #[serde(default, deserialize_with = "deserialize_string")]
    pub password: String,
}

// more variable configs
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Config2 {
    #[serde(default, deserialize_with = "deserialize_string")]
    rendezvous_server: String,
    #[serde(default, deserialize_with = "deserialize_i32")]
    nat_type: i32,
    #[serde(default, deserialize_with = "deserialize_i32")]
    serial: i32,
    #[serde(default, deserialize_with = "deserialize_string")]
    unlock_pin: String,
    #[serde(default, deserialize_with = "deserialize_string")]
    trusted_devices: String,

    #[serde(default)]
    socks: Option<Socks5Server>,

    // the other scalar value must before this
    #[serde(default, deserialize_with = "deserialize_hashmap_string_string")]
    pub options: HashMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Resolution {
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PeerConfig {
    #[serde(default, deserialize_with = "deserialize_vec_u8")]
    pub password: Vec<u8>,
    #[serde(default, deserialize_with = "deserialize_size")]
    pub size: Size,
    #[serde(default, deserialize_with = "deserialize_size")]
    pub size_ft: Size,
    #[serde(default, deserialize_with = "deserialize_size")]
    pub size_pf: Size,
    #[serde(
        default = "PeerConfig::default_view_style",
        deserialize_with = "PeerConfig::deserialize_view_style",
        skip_serializing_if = "String::is_empty"
    )]
    pub view_style: String,
    // Image scroll style, scrollbar or scroll auto
    #[serde(
        default = "PeerConfig::default_scroll_style",
        deserialize_with = "PeerConfig::deserialize_scroll_style",
        skip_serializing_if = "String::is_empty"
    )]
    pub scroll_style: String,
    #[serde(
        default = "PeerConfig::default_image_quality",
        deserialize_with = "PeerConfig::deserialize_image_quality",
        skip_serializing_if = "String::is_empty"
    )]
    pub image_quality: String,
    #[serde(
        default = "PeerConfig::default_custom_image_quality",
        deserialize_with = "PeerConfig::deserialize_custom_image_quality",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub custom_image_quality: Vec<i32>,
    #[serde(flatten)]
    pub show_remote_cursor: ShowRemoteCursor,
    #[serde(flatten)]
    pub lock_after_session_end: LockAfterSessionEnd,
    #[serde(flatten)]
    pub privacy_mode: PrivacyMode,
    #[serde(flatten)]
    pub allow_swap_key: AllowSwapKey,
    #[serde(default, deserialize_with = "deserialize_vec_i32_string_i32")]
    pub port_forwards: Vec<(i32, String, i32)>,
    #[serde(default, deserialize_with = "deserialize_i32")]
    pub direct_failures: i32,
    #[serde(flatten)]
    pub disable_audio: DisableAudio,
    #[serde(flatten)]
    pub disable_clipboard: DisableClipboard,
    #[serde(flatten)]
    pub enable_file_copy_paste: EnableFileCopyPaste,
    #[serde(flatten)]
    pub show_quality_monitor: ShowQualityMonitor,
    #[serde(flatten)]
    pub follow_remote_cursor: FollowRemoteCursor,
    #[serde(flatten)]
    pub follow_remote_window: FollowRemoteWindow,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub keyboard_mode: String,
    #[serde(flatten)]
    pub view_only: ViewOnly,
    #[serde(flatten)]
    pub sync_init_clipboard: SyncInitClipboard,
    // Mouse wheel or touchpad scroll mode
    #[serde(
        default = "PeerConfig::default_reverse_mouse_wheel",
        deserialize_with = "PeerConfig::deserialize_reverse_mouse_wheel",
        skip_serializing_if = "String::is_empty"
    )]
    pub reverse_mouse_wheel: String,
    #[serde(
        default = "PeerConfig::default_displays_as_individual_windows",
        deserialize_with = "PeerConfig::deserialize_displays_as_individual_windows",
        skip_serializing_if = "String::is_empty"
    )]
    pub displays_as_individual_windows: String,
    #[serde(
        default = "PeerConfig::default_use_all_my_displays_for_the_remote_session",
        deserialize_with = "PeerConfig::deserialize_use_all_my_displays_for_the_remote_session",
        skip_serializing_if = "String::is_empty"
    )]
    pub use_all_my_displays_for_the_remote_session: String,

    #[serde(
        default,
        deserialize_with = "deserialize_hashmap_resolutions",
        skip_serializing_if = "HashMap::is_empty"
    )]
    pub custom_resolutions: HashMap<String, Resolution>,

    // The other scalar value must before this
    #[serde(
        default,
        deserialize_with = "deserialize_hashmap_string_string",
        skip_serializing_if = "HashMap::is_empty"
    )]
    pub options: HashMap<String, String>, // not use delete to represent default values
    // Various data for flutter ui
    #[serde(default, deserialize_with = "deserialize_hashmap_string_string")]
    pub ui_flutter: HashMap<String, String>,
    #[serde(default)]
    pub info: PeerInfoSerde,
    #[serde(default)]
    pub transfer: TransferSerde,
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            password: Default::default(),
            size: Default::default(),
            size_ft: Default::default(),
            size_pf: Default::default(),
            view_style: Self::default_view_style(),
            scroll_style: Self::default_scroll_style(),
            image_quality: Self::default_image_quality(),
            custom_image_quality: Self::default_custom_image_quality(),
            show_remote_cursor: Default::default(),
            lock_after_session_end: Default::default(),
            privacy_mode: Default::default(),
            allow_swap_key: Default::default(),
            port_forwards: Default::default(),
            direct_failures: Default::default(),
            disable_audio: Default::default(),
            disable_clipboard: Default::default(),
            enable_file_copy_paste: Default::default(),
            show_quality_monitor: Default::default(),
            follow_remote_cursor: Default::default(),
            follow_remote_window: Default::default(),
            keyboard_mode: Default::default(),
            view_only: Default::default(),
            reverse_mouse_wheel: Self::default_reverse_mouse_wheel(),
            displays_as_individual_windows: Self::default_displays_as_individual_windows(),
            use_all_my_displays_for_the_remote_session:
                Self::default_use_all_my_displays_for_the_remote_session(),
            custom_resolutions: Default::default(),
            options: Self::default_options(),
            ui_flutter: Default::default(),
            info: Default::default(),
            transfer: Default::default(),
            sync_init_clipboard: Default::default(),
        }
    }
}

#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone)]
pub struct PeerInfoSerde {
    #[serde(default, deserialize_with = "deserialize_string")]
    pub username: String,
    #[serde(default, deserialize_with = "deserialize_string")]
    pub hostname: String,
    #[serde(default, deserialize_with = "deserialize_string")]
    pub platform: String,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct TransferSerde {
    #[serde(default, deserialize_with = "deserialize_vec_string")]
    pub write_jobs: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_vec_string")]
    pub read_jobs: Vec<String>,
}

#[inline]
pub fn get_online_state() -> i64 {
    *ONLINE.lock().unwrap().values().max().unwrap_or(&0)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn patch(path: PathBuf) -> PathBuf {
    if let Some(_tmp) = path.to_str() {
        #[cfg(windows)]
        return _tmp
            .replace(
                "system32\\config\\systemprofile",
                "ServiceProfiles\\LocalService",
            )
            .into();
        #[cfg(target_os = "macos")]
        return _tmp.replace("Application Support", "Preferences").into();
        #[cfg(target_os = "linux")]
        {
            if _tmp == "/root" {
                if let Ok(user) = crate::platform::linux::run_cmds_trim_newline("whoami") {
                    if user != "root" {
                        let cmd = format!("getent passwd '{}' | awk -F':' '{{print $6}}'", user);
                        if let Ok(output) = crate::platform::linux::run_cmds_trim_newline(&cmd) {
                            return output.into();
                        }
                        return format!("/home/{user}").into();
                    }
                }
            }
        }
    }
    path
}

impl Config2 {
    fn load() -> Config2 {
        let mut config = Config::load_::<Config2>("2");
        let mut store = false;
        if let Some(mut socks) = config.socks {
            let (password, _, store2) =
                decrypt_str_or_original(&socks.password, PASSWORD_ENC_VERSION);
            socks.password = password;
            config.socks = Some(socks);
            store |= store2;
        }
        let (unlock_pin, _, store2) =
            decrypt_str_or_original(&config.unlock_pin, PASSWORD_ENC_VERSION);
        config.unlock_pin = unlock_pin;
        store |= store2;
        if store {
            config.store();
        }
        config
    }

    pub fn file() -> PathBuf {
        Config::file_("2")
    }

    fn store(&self) {
        let mut config = self.clone();
        if let Some(mut socks) = config.socks {
            socks.password =
                encrypt_str_or_original(&socks.password, PASSWORD_ENC_VERSION, ENCRYPT_MAX_LEN);
            config.socks = Some(socks);
        }
        config.unlock_pin =
            encrypt_str_or_original(&config.unlock_pin, PASSWORD_ENC_VERSION, ENCRYPT_MAX_LEN);
        Config::store_(&config, "2");
    }

    pub fn get() -> Config2 {
        return CONFIG2.read().unwrap().clone();
    }

    pub fn set(cfg: Config2) -> bool {
        let mut lock = CONFIG2.write().unwrap();
        if *lock == cfg {
            return false;
        }
        *lock = cfg;
        lock.store();
        true
    }
}

pub fn load_path<T: serde::Serialize + serde::de::DeserializeOwned + Default + std::fmt::Debug>(
    file: PathBuf,
) -> T {
    let cfg = match confy::load_path(&file) {
        Ok(config) => config,
        Err(err) => {
            if let confy::ConfyError::GeneralLoadError(err) = &err {
                if err.kind() == std::io::ErrorKind::NotFound {
                    return T::default();
                }
            }
            log::error!("Failed to load config '{}': {}", file.display(), err);
            T::default()
        }
    };
    cfg
}

#[inline]
pub fn store_path<T: serde::Serialize>(path: PathBuf, cfg: T) -> crate::ResultType<()> {
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(confy::store_path_perms(
            path,
            cfg,
            fs::Permissions::from_mode(0o600),
        )?)
    }
    #[cfg(windows)]
    {
        Ok(confy::store_path(path, cfg)?)
    }
}

impl Config {
    fn load_<T: serde::Serialize + serde::de::DeserializeOwned + Default + std::fmt::Debug>(
        suffix: &str,
    ) -> T {
        let file = Self::file_(suffix);
        let cfg = load_path(file);
        if suffix.is_empty() {
            log::trace!("{:?}", cfg);
        }
        cfg
    }

    fn store_<T: serde::Serialize>(config: &T, suffix: &str) {
        let file = Self::file_(suffix);
        if let Err(err) = store_path(file, config) {
            log::error!("Failed to store {suffix} config: {err}");
        }
    }

    fn load() -> Config {
        let mut config = Config::load_::<Config>("");
        let mut store = false;
        let (password, _, store1) = decrypt_str_or_original(&config.password, PASSWORD_ENC_VERSION);
        config.password = password;
        store |= store1;
        let mut id_valid = false;
        let (id, encrypted, store2) = decrypt_str_or_original(&config.enc_id, PASSWORD_ENC_VERSION);
        if encrypted {
            config.id = id;
            id_valid = true;
            store |= store2;
        } else if
        // Comment out for forward compatible
        // crate::get_modified_time(&Self::file_(""))
        // .checked_sub(std::time::Duration::from_secs(30)) // allow modification during installation
        // .unwrap_or_else(crate::get_exe_time)
        // < crate::get_exe_time()
        // &&
        !config.id.is_empty()
            && config.enc_id.is_empty()
            && !decrypt_str_or_original(&config.id, PASSWORD_ENC_VERSION).1
        {
            id_valid = true;
            store = true;
        }
        if !id_valid {
            for _ in 0..3 {
                if let Some(id) = Config::get_auto_id() {
                    config.id = id;
                    store = true;
                    break;
                } else {
                    log::error!("Failed to generate new id");
                }
            }
        }
        if store {
            config.store();
        }
        config
    }

    fn store(&self) {
        let mut config = self.clone();
        config.password =
            encrypt_str_or_original(&config.password, PASSWORD_ENC_VERSION, ENCRYPT_MAX_LEN);
        config.enc_id = encrypt_str_or_original(&config.id, PASSWORD_ENC_VERSION, ENCRYPT_MAX_LEN);
        config.id = "".to_owned();
        Config::store_(&config, "");
    }

    pub fn file() -> PathBuf {
        Self::file_("")
    }

    fn file_(suffix: &str) -> PathBuf {
        let name = format!("{}{}", *APP_NAME.read().unwrap(), suffix);
        Config::with_extension(Self::path(name))
    }

    pub fn is_empty(&self) -> bool {
        (self.id.is_empty() && self.enc_id.is_empty()) || self.key_pair.0.is_empty()
    }

    pub fn get_home() -> PathBuf {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        return PathBuf::from(APP_HOME_DIR.read().unwrap().as_str());
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            if let Some(path) = dirs_next::home_dir() {
                patch(path)
            } else if let Ok(path) = std::env::current_dir() {
                path
            } else {
                std::env::temp_dir()
            }
        }
    }

    pub fn path<P: AsRef<Path>>(p: P) -> PathBuf {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            let mut path: PathBuf = APP_DIR.read().unwrap().clone().into();
            path.push(p);
            return path;
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            #[cfg(not(target_os = "macos"))]
            let org = "".to_owned();
            #[cfg(target_os = "macos")]
            let org = ORG.read().unwrap().clone();
            // /var/root for root
            if let Some(project) =
                directories_next::ProjectDirs::from("", &org, &APP_NAME.read().unwrap())
            {
                let mut path = patch(project.config_dir().to_path_buf());
                path.push(p);
                return path;
            }
            "".into()
        }
    }

    #[allow(unreachable_code)]
    pub fn log_path() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            if let Some(path) = dirs_next::home_dir().as_mut() {
                path.push(format!("Library/Logs/{}", *APP_NAME.read().unwrap()));
                return path.clone();
            }
        }
        #[cfg(target_os = "linux")]
        {
            let mut path = Self::get_home();
            path.push(format!(".local/share/logs/{}", *APP_NAME.read().unwrap()));
            std::fs::create_dir_all(&path).ok();
            return path;
        }
        #[cfg(target_os = "android")]
        {
            let mut path = Self::get_home();
            path.push(format!("{}/Logs", *APP_NAME.read().unwrap()));
            std::fs::create_dir_all(&path).ok();
            return path;
        }
        if let Some(path) = Self::path("").parent() {
            let mut path: PathBuf = path.into();
            path.push("log");
            return path;
        }
        "".into()
    }

    pub fn ipc_path(postfix: &str) -> String {
        #[cfg(windows)]
        {
            // \\ServerName\pipe\PipeName
            // where ServerName is either the name of a remote computer or a period, to specify the local computer.
            // https://docs.microsoft.com/en-us/windows/win32/ipc/pipe-names
            format!(
                "\\\\.\\pipe\\{}\\query{}",
                *APP_NAME.read().unwrap(),
                postfix
            )
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            #[cfg(target_os = "android")]
            let mut path: PathBuf =
                format!("{}/{}", *APP_DIR.read().unwrap(), *APP_NAME.read().unwrap()).into();
            #[cfg(not(target_os = "android"))]
            let mut path: PathBuf = format!("/tmp/{}", *APP_NAME.read().unwrap()).into();
            fs::create_dir(&path).ok();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o0777)).ok();
            path.push(format!("ipc{postfix}"));
            path.to_str().unwrap_or("").to_owned()
        }
    }

    pub fn icon_path() -> PathBuf {
        let mut path = Self::path("icons");
        if fs::create_dir_all(&path).is_err() {
            path = std::env::temp_dir();
        }
        path
    }

    #[inline]
    pub fn get_any_listen_addr(is_ipv4: bool) -> SocketAddr {
        if is_ipv4 {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
        } else {
            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)
        }
    }

    pub fn get_rendezvous_server() -> String {
        let mut rendezvous_server = EXE_RENDEZVOUS_SERVER.read().unwrap().clone();
        if rendezvous_server.is_empty() {
            rendezvous_server = Self::get_option("custom-rendezvous-server");
        }
        if rendezvous_server.is_empty() {
            rendezvous_server = PROD_RENDEZVOUS_SERVER.read().unwrap().clone();
        }
        if rendezvous_server.is_empty() {
            rendezvous_server = CONFIG2.read().unwrap().rendezvous_server.clone();
        }
        if rendezvous_server.is_empty() {
            rendezvous_server = Self::get_rendezvous_servers()
                .drain(..)
                .next()
                .unwrap_or_default();
        }
        if !rendezvous_server.contains(':') {
            rendezvous_server = format!("{rendezvous_server}:{RENDEZVOUS_PORT}");
        }
        rendezvous_server
    }

    pub fn get_rendezvous_servers() -> Vec<String> {
        let s = EXE_RENDEZVOUS_SERVER.read().unwrap().clone();
        if !s.is_empty() {
            return vec![s];
        }
        let s = Self::get_option("custom-rendezvous-server");
        if !s.is_empty() {
            return vec![s];
        }
        let s = PROD_RENDEZVOUS_SERVER.read().unwrap().clone();
        if !s.is_empty() {
            return vec![s];
        }
        let serial_obsolute = CONFIG2.read().unwrap().serial > SERIAL;
        if serial_obsolute {
            let ss: Vec<String> = Self::get_option("rendezvous-servers")
                .split(',')
                .filter(|x| x.contains('.'))
                .map(|x| x.to_owned())
                .collect();
            if !ss.is_empty() {
                return ss;
            }
        }
        return RENDEZVOUS_SERVERS.iter().map(|x| x.to_string()).collect();
    }

    pub fn reset_online() {
        *ONLINE.lock().unwrap() = Default::default();
    }

    pub fn update_latency(host: &str, latency: i64) {
        ONLINE.lock().unwrap().insert(host.to_owned(), latency);
        let mut host = "".to_owned();
        let mut delay = i64::MAX;
        for (tmp_host, tmp_delay) in ONLINE.lock().unwrap().iter() {
            if tmp_delay > &0 && tmp_delay < &delay {
                delay = *tmp_delay;
                host = tmp_host.to_string();
            }
        }
        if !host.is_empty() {
            let mut config = CONFIG2.write().unwrap();
            if host != config.rendezvous_server {
                log::debug!("Update rendezvous_server in config to {}", host);
                log::debug!("{:?}", *ONLINE.lock().unwrap());
                config.rendezvous_server = host;
                config.store();
            }
        }
    }

    pub fn set_id(id: &str) {
        let mut config = CONFIG.write().unwrap();
        if id == config.id {
            return;
        }
        config.id = id.into();
        config.store();
    }

    pub fn set_nat_type(nat_type: i32) {
        let mut config = CONFIG2.write().unwrap();
        if nat_type == config.nat_type {
            return;
        }
        config.nat_type = nat_type;
        config.store();
    }

    pub fn get_nat_type() -> i32 {
        CONFIG2.read().unwrap().nat_type
    }

    pub fn set_serial(serial: i32) {
        let mut config = CONFIG2.write().unwrap();
        if serial == config.serial {
            return;
        }
        config.serial = serial;
        config.store();
    }

    pub fn get_serial() -> i32 {
        std::cmp::max(CONFIG2.read().unwrap().serial, SERIAL)
    }

    fn get_auto_id() -> Option<String> {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            return Some(
                rand::thread_rng()
                    .gen_range(1_000_000_000..2_000_000_000)
                    .to_string(),
            );
        }

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let mut id = 0u32;
            if let Ok(Some(ma)) = mac_address::get_mac_address() {
                for x in &ma.bytes()[2..] {
                    id = (id << 8) | (*x as u32);
                }
                id &= 0x1FFFFFFF;
                Some(id.to_string())
            } else {
                None
            }
        }
    }

    pub fn get_auto_password(length: usize) -> String {
        let mut rng = rand::thread_rng();
        (0..length)
            .map(|_| CHARS[rng.gen::<usize>() % CHARS.len()])
            .collect()
    }

    pub fn get_key_confirmed() -> bool {
        CONFIG.read().unwrap().key_confirmed
    }

    pub fn set_key_confirmed(v: bool) {
        let mut config = CONFIG.write().unwrap();
        if config.key_confirmed == v {
            return;
        }
        config.key_confirmed = v;
        if !v {
            config.keys_confirmed = Default::default();
        }
        config.store();
    }

    pub fn get_host_key_confirmed(host: &str) -> bool {
        matches!(CONFIG.read().unwrap().keys_confirmed.get(host), Some(true))
    }

    pub fn set_host_key_confirmed(host: &str, v: bool) {
        if Self::get_host_key_confirmed(host) == v {
            return;
        }
        let mut config = CONFIG.write().unwrap();
        config.keys_confirmed.insert(host.to_owned(), v);
        config.store();
    }

    pub fn get_key_pair() -> KeyPair {
        // lock here to make sure no gen_keypair more than once
        // no use of CONFIG directly here to ensure no recursive calling in Config::load because of password dec which calling this function
        let mut lock = KEY_PAIR.lock().unwrap();
        if let Some(p) = lock.as_ref() {
            return p.clone();
        }
        let mut config = Config::load_::<Config>("");
        if config.key_pair.0.is_empty() {
            log::info!("Generated new keypair for id: {}", config.id);
            let (pk, sk) = sign::gen_keypair();
            let key_pair = (sk.0.to_vec(), pk.0.into());
            config.key_pair = key_pair.clone();
            std::thread::spawn(|| {
                let mut config = CONFIG.write().unwrap();
                config.key_pair = key_pair;
                config.store();
            });
        }
        *lock = Some(config.key_pair.clone());
        config.key_pair
    }

    pub fn get_id() -> String {
        let mut id = CONFIG.read().unwrap().id.clone();
        if id.is_empty() {
            if let Some(tmp) = Config::get_auto_id() {
                id = tmp;
                Config::set_id(&id);
            }
        }
        id
    }

    pub fn get_id_or(b: String) -> String {
        let a = CONFIG.read().unwrap().id.clone();
        if a.is_empty() {
            b
        } else {
            a
        }
    }

    pub fn get_options() -> HashMap<String, String> {
        let mut res = DEFAULT_SETTINGS.read().unwrap().clone();
        res.extend(CONFIG2.read().unwrap().options.clone());
        res.extend(OVERWRITE_SETTINGS.read().unwrap().clone());
        res
    }

    #[inline]
    fn purify_options(v: &mut HashMap<String, String>) {
        v.retain(|k, v| is_option_can_save(&OVERWRITE_SETTINGS, k, &DEFAULT_SETTINGS, v));
    }

    pub fn set_options(mut v: HashMap<String, String>) {
        Self::purify_options(&mut v);
        let mut config = CONFIG2.write().unwrap();
        if config.options == v {
            return;
        }
        config.options = v;
        config.store();
    }

    pub fn get_option(k: &str) -> String {
        get_or(
            &OVERWRITE_SETTINGS,
            &CONFIG2.read().unwrap().options,
            &DEFAULT_SETTINGS,
            k,
        )
        .unwrap_or_default()
    }

    pub fn get_bool_option(k: &str) -> bool {
        option2bool(k, &Self::get_option(k))
    }

    pub fn set_option(k: String, v: String) {
        if !is_option_can_save(&OVERWRITE_SETTINGS, &k, &DEFAULT_SETTINGS, &v) {
            return;
        }
        let mut config = CONFIG2.write().unwrap();
        let v2 = if v.is_empty() { None } else { Some(&v) };
        if v2 != config.options.get(&k) {
            if v2.is_none() {
                config.options.remove(&k);
            } else {
                config.options.insert(k, v);
            }
            config.store();
        }
    }

    pub fn update_id() {
        // to-do: how about if one ip register a lot of ids?
        let id = Self::get_id();
        let mut rng = rand::thread_rng();
        let new_id = rng.gen_range(1_000_000_000..2_000_000_000).to_string();
        Config::set_id(&new_id);
        log::info!("id updated from {} to {}", id, new_id);
    }

    pub fn set_permanent_password(password: &str) {
        if HARD_SETTINGS
            .read()
            .unwrap()
            .get("password")
            .map_or(false, |v| v == password)
        {
            return;
        }
        let mut config = CONFIG.write().unwrap();
        if password == config.password {
            return;
        }
        config.password = password.into();
        config.store();
        Self::clear_trusted_devices();
    }

    pub fn get_permanent_password() -> String {
        let mut password = CONFIG.read().unwrap().password.clone();
        if password.is_empty() {
            if let Some(v) = HARD_SETTINGS.read().unwrap().get("password") {
                password = v.to_owned();
            }
        }
        password
    }

    pub fn set_salt(salt: &str) {
        let mut config = CONFIG.write().unwrap();
        if salt == config.salt {
            return;
        }
        config.salt = salt.into();
        config.store();
    }

    pub fn get_salt() -> String {
        let mut salt = CONFIG.read().unwrap().salt.clone();
        if salt.is_empty() {
            salt = Config::get_auto_password(6);
            Config::set_salt(&salt);
        }
        salt
    }

    pub fn set_socks(socks: Option<Socks5Server>) {
        let mut config = CONFIG2.write().unwrap();
        if config.socks == socks {
            return;
        }
        config.socks = socks;
        config.store();
    }

    #[inline]
    fn get_socks_from_custom_client_advanced_settings(
        settings: &HashMap<String, String>,
    ) -> Option<Socks5Server> {
        let url = settings.get(keys::OPTION_PROXY_URL)?;
        Some(Socks5Server {
            proxy: url.to_owned(),
            username: settings
                .get(keys::OPTION_PROXY_USERNAME)
                .map(|x| x.to_string())
                .unwrap_or_default(),
            password: settings
                .get(keys::OPTION_PROXY_PASSWORD)
                .map(|x| x.to_string())
                .unwrap_or_default(),
        })
    }

    pub fn get_socks() -> Option<Socks5Server> {
        Self::get_socks_from_custom_client_advanced_settings(&OVERWRITE_SETTINGS.read().unwrap())
            .or(CONFIG2.read().unwrap().socks.clone())
            .or(Self::get_socks_from_custom_client_advanced_settings(
                &DEFAULT_SETTINGS.read().unwrap(),
            ))
    }

    #[inline]
    pub fn is_proxy() -> bool {
        Self::get_network_type() != NetworkType::Direct
    }

    pub fn get_network_type() -> NetworkType {
        if OVERWRITE_SETTINGS
            .read()
            .unwrap()
            .get(keys::OPTION_PROXY_URL)
            .is_some()
        {
            return NetworkType::ProxySocks;
        }
        if CONFIG2.read().unwrap().socks.is_some() {
            return NetworkType::ProxySocks;
        }
        if DEFAULT_SETTINGS
            .read()
            .unwrap()
            .get(keys::OPTION_PROXY_URL)
            .is_some()
        {
            return NetworkType::ProxySocks;
        }
        NetworkType::Direct
    }

    pub fn get_unlock_pin() -> String {
        CONFIG2.read().unwrap().unlock_pin.clone()
    }

    pub fn set_unlock_pin(pin: &str) {
        let mut config = CONFIG2.write().unwrap();
        if pin == config.unlock_pin {
            return;
        }
        config.unlock_pin = pin.to_string();
        config.store();
    }

    pub fn get_trusted_devices_json() -> String {
        serde_json::to_string(&Self::get_trusted_devices()).unwrap_or_default()
    }

    pub fn get_trusted_devices() -> Vec<TrustedDevice> {
        let (devices, synced) = TRUSTED_DEVICES.read().unwrap().clone();
        if synced {
            return devices;
        }
        let devices = CONFIG2.read().unwrap().trusted_devices.clone();
        let (devices, succ, store) = decrypt_str_or_original(&devices, PASSWORD_ENC_VERSION);
        if succ {
            let mut devices: Vec<TrustedDevice> =
                serde_json::from_str(&devices).unwrap_or_default();
            let len = devices.len();
            devices.retain(|d| !d.outdate());
            if store || devices.len() != len {
                Self::set_trusted_devices(devices.clone());
            }
            *TRUSTED_DEVICES.write().unwrap() = (devices.clone(), true);
            devices
        } else {
            Default::default()
        }
    }

    fn set_trusted_devices(mut trusted_devices: Vec<TrustedDevice>) {
        trusted_devices.retain(|d| !d.outdate());
        let devices = serde_json::to_string(&trusted_devices).unwrap_or_default();
        let max_len = 1024 * 1024;
        if devices.bytes().len() > max_len {
            log::error!("Trusted devices too large: {}", devices.bytes().len());
            return;
        }
        let devices = encrypt_str_or_original(&devices, PASSWORD_ENC_VERSION, max_len);
        let mut config = CONFIG2.write().unwrap();
        config.trusted_devices = devices;
        config.store();
        *TRUSTED_DEVICES.write().unwrap() = (trusted_devices, true);
    }

    pub fn add_trusted_device(device: TrustedDevice) {
        let mut devices = Self::get_trusted_devices();
        devices.retain(|d| d.hwid != device.hwid);
        devices.push(device);
        Self::set_trusted_devices(devices);
    }

    pub fn remove_trusted_devices(hwids: &Vec<Bytes>) {
        let mut devices = Self::get_trusted_devices();
        devices.retain(|d| !hwids.contains(&d.hwid));
        Self::set_trusted_devices(devices);
    }

    pub fn clear_trusted_devices() {
        Self::set_trusted_devices(Default::default());
    }

    pub fn get() -> Config {
        return CONFIG.read().unwrap().clone();
    }

    pub fn set(cfg: Config) -> bool {
        let mut lock = CONFIG.write().unwrap();
        if *lock == cfg {
            return false;
        }
        *lock = cfg;
        lock.store();
        true
    }

    fn with_extension(path: PathBuf) -> PathBuf {
        let ext = path.extension();
        if let Some(ext) = ext {
            let ext = format!("{}.toml", ext.to_string_lossy());
            path.with_extension(ext)
        } else {
            path.with_extension("toml")
        }
    }
}

const PEERS: &str = "peers";

impl PeerConfig {
    pub fn load(id: &str) -> PeerConfig {
        let _lock = CONFIG.read().unwrap();
        match confy::load_path(Self::path(id)) {
            Ok(config) => {
                let mut config: PeerConfig = config;
                let mut store = false;
                let (password, _, store2) =
                    decrypt_vec_or_original(&config.password, PASSWORD_ENC_VERSION);
                config.password = password;
                store = store || store2;
                for opt in ["rdp_password", "os-username", "os-password"] {
                    if let Some(v) = config.options.get_mut(opt) {
                        let (encrypted, _, store2) =
                            decrypt_str_or_original(v, PASSWORD_ENC_VERSION);
                        *v = encrypted;
                        store = store || store2;
                    }
                }
                if store {
                    config.store(id);
                }
                config
            }
            Err(err) => {
                if let confy::ConfyError::GeneralLoadError(err) = &err {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        return Default::default();
                    }
                }
                log::error!("Failed to load peer config '{}': {}", id, err);
                Default::default()
            }
        }
    }

    pub fn store(&self, id: &str) {
        let _lock = CONFIG.read().unwrap();
        let mut config = self.clone();
        config.password =
            encrypt_vec_or_original(&config.password, PASSWORD_ENC_VERSION, ENCRYPT_MAX_LEN);
        for opt in ["rdp_password", "os-username", "os-password"] {
            if let Some(v) = config.options.get_mut(opt) {
                *v = encrypt_str_or_original(v, PASSWORD_ENC_VERSION, ENCRYPT_MAX_LEN)
            }
        }
        if let Err(err) = store_path(Self::path(id), config) {
            log::error!("Failed to store config: {}", err);
        }
        NEW_STORED_PEER_CONFIG.lock().unwrap().insert(id.to_owned());
    }

    pub fn remove(id: &str) {
        fs::remove_file(Self::path(id)).ok();
    }

    fn path(id: &str) -> PathBuf {
        //If the id contains invalid chars, encode it
        let forbidden_paths = Regex::new(r".*[<>:/\\|\?\*].*");
        let path: PathBuf;
        if let Ok(forbidden_paths) = forbidden_paths {
            let id_encoded = if forbidden_paths.is_match(id) {
                "base64_".to_string() + base64::encode(id, base64::Variant::Original).as_str()
            } else {
                id.to_string()
            };
            path = [PEERS, id_encoded.as_str()].iter().collect();
        } else {
            log::warn!("Regex create failed: {:?}", forbidden_paths.err());
            // fallback for failing to create this regex.
            path = [PEERS, id.replace(":", "_").as_str()].iter().collect();
        }
        Config::with_extension(Config::path(path))
    }

    pub fn peers(id_filters: Option<Vec<String>>) -> Vec<(String, SystemTime, PeerConfig)> {
        if let Ok(peers) = Config::path(PEERS).read_dir() {
            if let Ok(peers) = peers
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, _>>()
            {
                let mut peers: Vec<_> = peers
                    .iter()
                    .filter(|p| {
                        p.is_file()
                            && p.extension().map(|p| p.to_str().unwrap_or("")) == Some("toml")
                    })
                    .map(|p| {
                        let id = p
                            .file_stem()
                            .map(|p| p.to_str().unwrap_or(""))
                            .unwrap_or("")
                            .to_owned();

                        let id_decoded_string = if id.starts_with("base64_") && id.len() != 7 {
                            let id_decoded = base64::decode(&id[7..], base64::Variant::Original)
                                .unwrap_or_default();
                            String::from_utf8_lossy(&id_decoded).as_ref().to_owned()
                        } else {
                            id
                        };
                        (id_decoded_string, p)
                    })
                    .filter(|(id, _)| {
                        let Some(filters) = &id_filters else {
                            return true;
                        };
                        filters.contains(id)
                    })
                    .map(|(id, p)| {
                        let t = crate::get_modified_time(p);
                        let c = PeerConfig::load(&id);
                        if c.info.platform.is_empty() {
                            fs::remove_file(p).ok();
                        }
                        (id, t, c)
                    })
                    .filter(|p| !p.2.info.platform.is_empty())
                    .collect();
                peers.sort_unstable_by(|a, b| b.1.cmp(&a.1));
                return peers;
            }
        }
        Default::default()
    }

    pub fn exists(id: &str) -> bool {
        Self::path(id).exists()
    }

    serde_field_string!(
        default_view_style,
        deserialize_view_style,
        UserDefaultConfig::read(keys::OPTION_VIEW_STYLE)
    );
    serde_field_string!(
        default_scroll_style,
        deserialize_scroll_style,
        UserDefaultConfig::read(keys::OPTION_SCROLL_STYLE)
    );
    serde_field_string!(
        default_image_quality,
        deserialize_image_quality,
        UserDefaultConfig::read(keys::OPTION_IMAGE_QUALITY)
    );
    serde_field_string!(
        default_reverse_mouse_wheel,
        deserialize_reverse_mouse_wheel,
        UserDefaultConfig::read(keys::OPTION_REVERSE_MOUSE_WHEEL)
    );
    serde_field_string!(
        default_displays_as_individual_windows,
        deserialize_displays_as_individual_windows,
        UserDefaultConfig::read(keys::OPTION_DISPLAYS_AS_INDIVIDUAL_WINDOWS)
    );
    serde_field_string!(
        default_use_all_my_displays_for_the_remote_session,
        deserialize_use_all_my_displays_for_the_remote_session,
        UserDefaultConfig::read(keys::OPTION_USE_ALL_MY_DISPLAYS_FOR_THE_REMOTE_SESSION)
    );

    fn default_custom_image_quality() -> Vec<i32> {
        let f: f64 = UserDefaultConfig::read(keys::OPTION_CUSTOM_IMAGE_QUALITY)
            .parse()
            .unwrap_or(50.0);
        vec![f as _]
    }

    fn deserialize_custom_image_quality<'de, D>(deserializer: D) -> Result<Vec<i32>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let v: Vec<i32> = de::Deserialize::deserialize(deserializer)?;
        if v.len() == 1 && v[0] >= 10 && v[0] <= 0xFFF {
            Ok(v)
        } else {
            Ok(Self::default_custom_image_quality())
        }
    }

    fn default_options() -> HashMap<String, String> {
        let mut mp: HashMap<String, String> = Default::default();
        [
            keys::OPTION_CODEC_PREFERENCE,
            keys::OPTION_CUSTOM_FPS,
            keys::OPTION_ZOOM_CURSOR,
            keys::OPTION_TOUCH_MODE,
            keys::OPTION_I444,
            keys::OPTION_SWAP_LEFT_RIGHT_MOUSE,
            keys::OPTION_COLLAPSE_TOOLBAR,
        ]
        .map(|key| {
            mp.insert(key.to_owned(), UserDefaultConfig::read(key));
        });
        mp
    }
}

serde_field_bool!(
    ShowRemoteCursor,
    "show_remote_cursor",
    default_show_remote_cursor,
    "ShowRemoteCursor::default_show_remote_cursor"
);
serde_field_bool!(
    FollowRemoteCursor,
    "follow_remote_cursor",
    default_follow_remote_cursor,
    "FollowRemoteCursor::default_follow_remote_cursor"
);

serde_field_bool!(
    FollowRemoteWindow,
    "follow_remote_window",
    default_follow_remote_window,
    "FollowRemoteWindow::default_follow_remote_window"
);
serde_field_bool!(
    ShowQualityMonitor,
    "show_quality_monitor",
    default_show_quality_monitor,
    "ShowQualityMonitor::default_show_quality_monitor"
);
serde_field_bool!(
    DisableAudio,
    "disable_audio",
    default_disable_audio,
    "DisableAudio::default_disable_audio"
);
serde_field_bool!(
    EnableFileCopyPaste,
    "enable-file-copy-paste",
    default_enable_file_copy_paste,
    "EnableFileCopyPaste::default_enable_file_copy_paste"
);
serde_field_bool!(
    DisableClipboard,
    "disable_clipboard",
    default_disable_clipboard,
    "DisableClipboard::default_disable_clipboard"
);
serde_field_bool!(
    LockAfterSessionEnd,
    "lock_after_session_end",
    default_lock_after_session_end,
    "LockAfterSessionEnd::default_lock_after_session_end"
);
serde_field_bool!(
    PrivacyMode,
    "privacy_mode",
    default_privacy_mode,
    "PrivacyMode::default_privacy_mode"
);

serde_field_bool!(
    AllowSwapKey,
    "allow_swap_key",
    default_allow_swap_key,
    "AllowSwapKey::default_allow_swap_key"
);

serde_field_bool!(
    ViewOnly,
    "view_only",
    default_view_only,
    "ViewOnly::default_view_only"
);

serde_field_bool!(
    SyncInitClipboard,
    "sync-init-clipboard",
    default_sync_init_clipboard,
    "SyncInitClipboard::default_sync_init_clipboard"
);

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct LocalConfig {
    #[serde(default, deserialize_with = "deserialize_string")]
    remote_id: String, // latest used one
    #[serde(default, deserialize_with = "deserialize_string")]
    kb_layout_type: String,
    #[serde(default, deserialize_with = "deserialize_size")]
    size: Size,
    #[serde(default, deserialize_with = "deserialize_vec_string")]
    pub fav: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_hashmap_string_string")]
    options: HashMap<String, String>,
    // Various data for flutter ui
    #[serde(default, deserialize_with = "deserialize_hashmap_string_string")]
    ui_flutter: HashMap<String, String>,
}

impl LocalConfig {
    fn load() -> LocalConfig {
        Config::load_::<LocalConfig>("_local")
    }

    fn store(&self) {
        Config::store_(self, "_local");
    }

    pub fn get_kb_layout_type() -> String {
        LOCAL_CONFIG.read().unwrap().kb_layout_type.clone()
    }

    pub fn set_kb_layout_type(kb_layout_type: String) {
        let mut config = LOCAL_CONFIG.write().unwrap();
        config.kb_layout_type = kb_layout_type;
        config.store();
    }

    pub fn get_size() -> Size {
        LOCAL_CONFIG.read().unwrap().size
    }

    pub fn set_size(x: i32, y: i32, w: i32, h: i32) {
        let mut config = LOCAL_CONFIG.write().unwrap();
        let size = (x, y, w, h);
        if size == config.size || size.2 < 300 || size.3 < 300 {
            return;
        }
        config.size = size;
        config.store();
    }

    pub fn set_remote_id(remote_id: &str) {
        let mut config = LOCAL_CONFIG.write().unwrap();
        if remote_id == config.remote_id {
            return;
        }
        config.remote_id = remote_id.into();
        config.store();
    }

    pub fn get_remote_id() -> String {
        LOCAL_CONFIG.read().unwrap().remote_id.clone()
    }

    pub fn set_fav(fav: Vec<String>) {
        let mut lock = LOCAL_CONFIG.write().unwrap();
        if lock.fav == fav {
            return;
        }
        lock.fav = fav;
        lock.store();
    }

    pub fn get_fav() -> Vec<String> {
        LOCAL_CONFIG.read().unwrap().fav.clone()
    }

    pub fn get_option(k: &str) -> String {
        get_or(
            &OVERWRITE_LOCAL_SETTINGS,
            &LOCAL_CONFIG.read().unwrap().options,
            &DEFAULT_LOCAL_SETTINGS,
            k,
        )
        .unwrap_or_default()
    }

    // Usually get_option should be used.
    pub fn get_option_from_file(k: &str) -> String {
        get_or(
            &OVERWRITE_LOCAL_SETTINGS,
            &Self::load().options,
            &DEFAULT_LOCAL_SETTINGS,
            k,
        )
        .unwrap_or_default()
    }

    pub fn get_bool_option(k: &str) -> bool {
        option2bool(k, &Self::get_option(k))
    }

    pub fn set_option(k: String, v: String) {
        if !is_option_can_save(&OVERWRITE_LOCAL_SETTINGS, &k, &DEFAULT_LOCAL_SETTINGS, &v) {
            return;
        }
        let mut config = LOCAL_CONFIG.write().unwrap();
        // The custom client will explictly set "default" as the default language.
        let is_custom_client_default_lang = k == keys::OPTION_LANGUAGE && v == "default";
        if is_custom_client_default_lang {
            config.options.insert(k, "".to_owned());
            config.store();
            return;
        }
        let v2 = if v.is_empty() { None } else { Some(&v) };
        if v2 != config.options.get(&k) {
            if v2.is_none() {
                config.options.remove(&k);
            } else {
                config.options.insert(k, v);
            }
            config.store();
        }
    }

    pub fn get_flutter_option(k: &str) -> String {
        get_or(
            &OVERWRITE_LOCAL_SETTINGS,
            &LOCAL_CONFIG.read().unwrap().ui_flutter,
            &DEFAULT_LOCAL_SETTINGS,
            k,
        )
        .unwrap_or_default()
    }

    pub fn set_flutter_option(k: String, v: String) {
        let mut config = LOCAL_CONFIG.write().unwrap();
        let v2 = if v.is_empty() { None } else { Some(&v) };
        if v2 != config.ui_flutter.get(&k) {
            if v2.is_none() {
                config.ui_flutter.remove(&k);
            } else {
                config.ui_flutter.insert(k, v);
            }
            config.store();
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct DiscoveryPeer {
    #[serde(default, deserialize_with = "deserialize_string")]
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_string")]
    pub username: String,
    #[serde(default, deserialize_with = "deserialize_string")]
    pub hostname: String,
    #[serde(default, deserialize_with = "deserialize_string")]
    pub platform: String,
    #[serde(default, deserialize_with = "deserialize_bool")]
    pub online: bool,
    #[serde(default, deserialize_with = "deserialize_hashmap_string_string")]
    pub ip_mac: HashMap<String, String>,
}

impl DiscoveryPeer {
    pub fn is_same_peer(&self, other: &DiscoveryPeer) -> bool {
        self.id == other.id && self.username == other.username
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct LanPeers {
    #[serde(default, deserialize_with = "deserialize_vec_discoverypeer")]
    pub peers: Vec<DiscoveryPeer>,
}

impl LanPeers {
    pub fn load() -> LanPeers {
        let _lock = CONFIG.read().unwrap();
        match confy::load_path(Config::file_("_lan_peers")) {
            Ok(peers) => peers,
            Err(err) => {
                log::error!("Failed to load lan peers: {}", err);
                Default::default()
            }
        }
    }

    pub fn store(peers: &[DiscoveryPeer]) {
        let f = LanPeers {
            peers: peers.to_owned(),
        };
        if let Err(err) = store_path(Config::file_("_lan_peers"), f) {
            log::error!("Failed to store lan peers: {}", err);
        }
    }

    pub fn modify_time() -> crate::ResultType<u64> {
        let p = Config::file_("_lan_peers");
        Ok(fs::metadata(p)?
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as _)
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct UserDefaultConfig {
    #[serde(default, deserialize_with = "deserialize_hashmap_string_string")]
    options: HashMap<String, String>,
}

impl UserDefaultConfig {
    fn read(key: &str) -> String {
        let mut cfg = USER_DEFAULT_CONFIG.write().unwrap();
        // we do so, because default config may changed in another process, but we don't sync it
        // but no need to read every time, give a small interval to avoid too many redundant read waste
        if cfg.1.elapsed() > Duration::from_secs(1) {
            *cfg = (Self::load(), Instant::now());
        }
        cfg.0.get(key)
    }

    pub fn load() -> UserDefaultConfig {
        Config::load_::<UserDefaultConfig>("_default")
    }

    #[inline]
    fn store(&self) {
        Config::store_(self, "_default");
    }

    pub fn get(&self, key: &str) -> String {
        match key {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            keys::OPTION_VIEW_STYLE => self.get_string(key, "adaptive", vec!["original"]),
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            keys::OPTION_VIEW_STYLE => self.get_string(key, "original", vec!["adaptive"]),
            keys::OPTION_SCROLL_STYLE => self.get_string(key, "scrollauto", vec!["scrollbar"]),
            keys::OPTION_IMAGE_QUALITY => {
                self.get_string(key, "balanced", vec!["best", "low", "custom"])
            }
            keys::OPTION_CODEC_PREFERENCE => {
                self.get_string(key, "auto", vec!["vp8", "vp9", "av1", "h264", "h265"])
            }
            keys::OPTION_CUSTOM_IMAGE_QUALITY => {
                self.get_double_string(key, 50.0, 10.0, 0xFFF as f64)
            }
            keys::OPTION_CUSTOM_FPS => self.get_double_string(key, 30.0, 5.0, 120.0),
            keys::OPTION_ENABLE_FILE_COPY_PASTE => self.get_string(key, "Y", vec!["", "N"]),
            _ => self
                .get_after(key)
                .map(|v| v.to_string())
                .unwrap_or_default(),
        }
    }

    pub fn set(&mut self, key: String, value: String) {
        if !is_option_can_save(
            &OVERWRITE_DISPLAY_SETTINGS,
            &key,
            &DEFAULT_DISPLAY_SETTINGS,
            &value,
        ) {
            return;
        }
        if value.is_empty() {
            self.options.remove(&key);
        } else {
            self.options.insert(key, value);
        }
        self.store();
    }

    #[inline]
    fn get_string(&self, key: &str, default: &str, others: Vec<&str>) -> String {
        match self.get_after(key) {
            Some(option) => {
                if others.contains(&option.as_str()) {
                    option.to_owned()
                } else {
                    default.to_owned()
                }
            }
            None => default.to_owned(),
        }
    }

    #[inline]
    fn get_double_string(&self, key: &str, default: f64, min: f64, max: f64) -> String {
        match self.get_after(key) {
            Some(option) => {
                let v: f64 = option.parse().unwrap_or(default);
                if v >= min && v <= max {
                    v.to_string()
                } else {
                    default.to_string()
                }
            }
            None => default.to_string(),
        }
    }

    fn get_after(&self, k: &str) -> Option<String> {
        get_or(
            &OVERWRITE_DISPLAY_SETTINGS,
            &self.options,
            &DEFAULT_DISPLAY_SETTINGS,
            k,
        )
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AbPeer {
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub id: String,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub hash: String,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub username: String,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub hostname: String,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub platform: String,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub alias: String,
    #[serde(default, deserialize_with = "deserialize_vec_string")]
    pub tags: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AbEntry {
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub guid: String,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_vec_abpeer")]
    pub peers: Vec<AbPeer>,
    #[serde(default, deserialize_with = "deserialize_vec_string")]
    pub tags: Vec<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub tag_colors: String,
}

impl AbEntry {
    pub fn personal(&self) -> bool {
        self.name == "My address book" || self.name == "Legacy address book"
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Ab {
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub access_token: String,
    #[serde(default, deserialize_with = "deserialize_vec_abentry")]
    pub ab_entries: Vec<AbEntry>,
}

impl Ab {
    fn path() -> PathBuf {
        let filename = format!("{}_ab", APP_NAME.read().unwrap().clone());
        Config::path(filename)
    }

    pub fn store(json: String) {
        if let Ok(mut file) = std::fs::File::create(Self::path()) {
            let data = compress(json.as_bytes());
            let max_len = 64 * 1024 * 1024;
            if data.len() > max_len {
                // maxlen of function decompress
                log::error!("ab data too large, {} > {}", data.len(), max_len);
                return;
            }
            if let Ok(data) = symmetric_crypt(&data, true) {
                file.write_all(&data).ok();
            }
        };
    }

    pub fn load() -> Ab {
        if let Ok(mut file) = std::fs::File::open(Self::path()) {
            let mut data = vec![];
            if file.read_to_end(&mut data).is_ok() {
                if let Ok(data) = symmetric_crypt(&data, false) {
                    let data = decompress(&data);
                    if let Ok(ab) = serde_json::from_str::<Ab>(&String::from_utf8_lossy(&data)) {
                        return ab;
                    }
                }
            }
        };
        Self::remove();
        Ab::default()
    }

    pub fn remove() {
        std::fs::remove_file(Self::path()).ok();
    }
}

// use default value when field type is wrong
macro_rules! deserialize_default {
    ($func_name:ident, $return_type:ty) => {
        fn $func_name<'de, D>(deserializer: D) -> Result<$return_type, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            Ok(de::Deserialize::deserialize(deserializer).unwrap_or_default())
        }
    };
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct GroupPeer {
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub id: String,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub username: String,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub hostname: String,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub platform: String,
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub login_name: String,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct GroupUser {
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub name: String,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Group {
    #[serde(
        default,
        deserialize_with = "deserialize_string",
        skip_serializing_if = "String::is_empty"
    )]
    pub access_token: String,
    #[serde(default, deserialize_with = "deserialize_vec_groupuser")]
    pub users: Vec<GroupUser>,
    #[serde(default, deserialize_with = "deserialize_vec_grouppeer")]
    pub peers: Vec<GroupPeer>,
}

impl Group {
    fn path() -> PathBuf {
        let filename = format!("{}_group", APP_NAME.read().unwrap().clone());
        Config::path(filename)
    }

    pub fn store(json: String) {
        if let Ok(mut file) = std::fs::File::create(Self::path()) {
            let data = compress(json.as_bytes());
            let max_len = 64 * 1024 * 1024;
            if data.len() > max_len {
                // maxlen of function decompress
                return;
            }
            if let Ok(data) = symmetric_crypt(&data, true) {
                file.write_all(&data).ok();
            }
        };
    }

    pub fn load() -> Self {
        if let Ok(mut file) = std::fs::File::open(Self::path()) {
            let mut data = vec![];
            if file.read_to_end(&mut data).is_ok() {
                if let Ok(data) = symmetric_crypt(&data, false) {
                    let data = decompress(&data);
                    if let Ok(group) = serde_json::from_str::<Self>(&String::from_utf8_lossy(&data))
                    {
                        return group;
                    }
                }
            }
        };
        Self::remove();
        Self::default()
    }

    pub fn remove() {
        std::fs::remove_file(Self::path()).ok();
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct TrustedDevice {
    pub hwid: Bytes,
    pub time: i64,
    pub id: String,
    pub name: String,
    pub platform: String,
}

impl TrustedDevice {
    pub fn outdate(&self) -> bool {
        const DAYS_90: i64 = 90 * 24 * 60 * 60 * 1000;
        self.time + DAYS_90 < crate::get_time()
    }
}

deserialize_default!(deserialize_string, String);
deserialize_default!(deserialize_bool, bool);
deserialize_default!(deserialize_i32, i32);
deserialize_default!(deserialize_vec_u8, Vec<u8>);
deserialize_default!(deserialize_vec_string, Vec<String>);
deserialize_default!(deserialize_vec_i32_string_i32, Vec<(i32, String, i32)>);
deserialize_default!(deserialize_vec_discoverypeer, Vec<DiscoveryPeer>);
deserialize_default!(deserialize_vec_abpeer, Vec<AbPeer>);
deserialize_default!(deserialize_vec_abentry, Vec<AbEntry>);
deserialize_default!(deserialize_vec_groupuser, Vec<GroupUser>);
deserialize_default!(deserialize_vec_grouppeer, Vec<GroupPeer>);
deserialize_default!(deserialize_keypair, KeyPair);
deserialize_default!(deserialize_size, Size);
deserialize_default!(deserialize_hashmap_string_string, HashMap<String, String>);
deserialize_default!(deserialize_hashmap_string_bool,  HashMap<String, bool>);
deserialize_default!(deserialize_hashmap_resolutions, HashMap<String, Resolution>);

#[inline]
fn get_or(
    a: &RwLock<HashMap<String, String>>,
    b: &HashMap<String, String>,
    c: &RwLock<HashMap<String, String>>,
    k: &str,
) -> Option<String> {
    a.read()
        .unwrap()
        .get(k)
        .or(b.get(k))
        .or(c.read().unwrap().get(k))
        .cloned()
}

#[inline]
fn is_option_can_save(
    overwrite: &RwLock<HashMap<String, String>>,
    k: &str,
    defaults: &RwLock<HashMap<String, String>>,
    v: &str,
) -> bool {
    if overwrite.read().unwrap().contains_key(k)
        || defaults.read().unwrap().get(k).map_or(false, |x| x == v)
    {
        return false;
    }
    true
}

#[inline]
pub fn is_incoming_only() -> bool {
    HARD_SETTINGS
        .read()
        .unwrap()
        .get("conn-type")
        .map_or(false, |x| x == ("incoming"))
}

#[inline]
pub fn is_outgoing_only() -> bool {
    HARD_SETTINGS
        .read()
        .unwrap()
        .get("conn-type")
        .map_or(false, |x| x == ("outgoing"))
}

#[inline]
fn is_some_hard_opton(name: &str) -> bool {
    HARD_SETTINGS
        .read()
        .unwrap()
        .get(name)
        .map_or(false, |x| x == ("Y"))
}

#[inline]
pub fn is_disable_tcp_listen() -> bool {
    is_some_hard_opton("disable-tcp-listen")
}

#[inline]
pub fn is_disable_settings() -> bool {
    is_some_hard_opton("disable-settings")
}

#[inline]
pub fn is_disable_ab() -> bool {
    is_some_hard_opton("disable-ab")
}

#[inline]
pub fn is_disable_account() -> bool {
    is_some_hard_opton("disable-account")
}

#[inline]
pub fn is_disable_installation() -> bool {
    is_some_hard_opton("disable-installation")
}

// This function must be kept the same as the one in flutter and sciter code.
// flutter: flutter/lib/common.dart -> option2bool()
// sciter: Does not have the function, but it should be kept the same.
pub fn option2bool(option: &str, value: &str) -> bool {
    if option.starts_with("enable-") {
        value != "N"
    } else if option.starts_with("allow-")
        || option == "stop-service"
        || option == keys::OPTION_DIRECT_SERVER
        || option == "force-always-relay"
    {
        value == "Y"
    } else {
        value != "N"
    }
}

pub mod keys {
    pub const OPTION_VIEW_ONLY: &str = "view_only";
    pub const OPTION_SHOW_MONITORS_TOOLBAR: &str = "show_monitors_toolbar";
    pub const OPTION_COLLAPSE_TOOLBAR: &str = "collapse_toolbar";
    pub const OPTION_SHOW_REMOTE_CURSOR: &str = "show_remote_cursor";
    pub const OPTION_FOLLOW_REMOTE_CURSOR: &str = "follow_remote_cursor";
    pub const OPTION_FOLLOW_REMOTE_WINDOW: &str = "follow_remote_window";
    pub const OPTION_ZOOM_CURSOR: &str = "zoom-cursor";
    pub const OPTION_SHOW_QUALITY_MONITOR: &str = "show_quality_monitor";
    pub const OPTION_DISABLE_AUDIO: &str = "disable_audio";
    pub const OPTION_ENABLE_FILE_COPY_PASTE: &str = "enable-file-copy-paste";
    pub const OPTION_DISABLE_CLIPBOARD: &str = "disable_clipboard";
    pub const OPTION_LOCK_AFTER_SESSION_END: &str = "lock_after_session_end";
    pub const OPTION_PRIVACY_MODE: &str = "privacy_mode";
    pub const OPTION_TOUCH_MODE: &str = "touch-mode";
    pub const OPTION_I444: &str = "i444";
    pub const OPTION_REVERSE_MOUSE_WHEEL: &str = "reverse_mouse_wheel";
    pub const OPTION_SWAP_LEFT_RIGHT_MOUSE: &str = "swap-left-right-mouse";
    pub const OPTION_DISPLAYS_AS_INDIVIDUAL_WINDOWS: &str = "displays_as_individual_windows";
    pub const OPTION_USE_ALL_MY_DISPLAYS_FOR_THE_REMOTE_SESSION: &str =
        "use_all_my_displays_for_the_remote_session";
    pub const OPTION_VIEW_STYLE: &str = "view_style";
    pub const OPTION_SCROLL_STYLE: &str = "scroll_style";
    pub const OPTION_IMAGE_QUALITY: &str = "image_quality";
    pub const OPTION_CUSTOM_IMAGE_QUALITY: &str = "custom_image_quality";
    pub const OPTION_CUSTOM_FPS: &str = "custom-fps";
    pub const OPTION_CODEC_PREFERENCE: &str = "codec-preference";
    pub const OPTION_SYNC_INIT_CLIPBOARD: &str = "sync-init-clipboard";
    pub const OPTION_THEME: &str = "theme";
    pub const OPTION_LANGUAGE: &str = "lang";
    pub const OPTION_REMOTE_MENUBAR_DRAG_LEFT: &str = "remote-menubar-drag-left";
    pub const OPTION_REMOTE_MENUBAR_DRAG_RIGHT: &str = "remote-menubar-drag-right";
    pub const OPTION_HIDE_AB_TAGS_PANEL: &str = "hideAbTagsPanel";
    pub const OPTION_ENABLE_CONFIRM_CLOSING_TABS: &str = "enable-confirm-closing-tabs";
    pub const OPTION_ENABLE_OPEN_NEW_CONNECTIONS_IN_TABS: &str =
        "enable-open-new-connections-in-tabs";
    pub const OPTION_TEXTURE_RENDER: &str = "use-texture-render";
    pub const OPTION_ENABLE_CHECK_UPDATE: &str = "enable-check-update";
    pub const OPTION_SYNC_AB_WITH_RECENT_SESSIONS: &str = "sync-ab-with-recent-sessions";
    pub const OPTION_SYNC_AB_TAGS: &str = "sync-ab-tags";
    pub const OPTION_FILTER_AB_BY_INTERSECTION: &str = "filter-ab-by-intersection";
    pub const OPTION_ACCESS_MODE: &str = "access-mode";
    pub const OPTION_ENABLE_KEYBOARD: &str = "enable-keyboard";
    pub const OPTION_ENABLE_CLIPBOARD: &str = "enable-clipboard";
    pub const OPTION_ENABLE_FILE_TRANSFER: &str = "enable-file-transfer";
    pub const OPTION_ENABLE_AUDIO: &str = "enable-audio";
    pub const OPTION_ENABLE_TUNNEL: &str = "enable-tunnel";
    pub const OPTION_ENABLE_REMOTE_RESTART: &str = "enable-remote-restart";
    pub const OPTION_ENABLE_RECORD_SESSION: &str = "enable-record-session";
    pub const OPTION_ENABLE_BLOCK_INPUT: &str = "enable-block-input";
    pub const OPTION_ALLOW_REMOTE_CONFIG_MODIFICATION: &str = "allow-remote-config-modification";
    pub const OPTION_ENABLE_LAN_DISCOVERY: &str = "enable-lan-discovery";
    pub const OPTION_DIRECT_SERVER: &str = "direct-server";
    pub const OPTION_DIRECT_ACCESS_PORT: &str = "direct-access-port";
    pub const OPTION_WHITELIST: &str = "whitelist";
    pub const OPTION_ALLOW_AUTO_DISCONNECT: &str = "allow-auto-disconnect";
    pub const OPTION_AUTO_DISCONNECT_TIMEOUT: &str = "auto-disconnect-timeout";
    pub const OPTION_ALLOW_ONLY_CONN_WINDOW_OPEN: &str = "allow-only-conn-window-open";
    pub const OPTION_ALLOW_AUTO_RECORD_INCOMING: &str = "allow-auto-record-incoming";
    pub const OPTION_ALLOW_AUTO_RECORD_OUTGOING: &str = "allow-auto-record-outgoing";
    pub const OPTION_VIDEO_SAVE_DIRECTORY: &str = "video-save-directory";
    pub const OPTION_ENABLE_ABR: &str = "enable-abr";
    pub const OPTION_ALLOW_REMOVE_WALLPAPER: &str = "allow-remove-wallpaper";
    pub const OPTION_ALLOW_ALWAYS_SOFTWARE_RENDER: &str = "allow-always-software-render";
    pub const OPTION_ALLOW_LINUX_HEADLESS: &str = "allow-linux-headless";
    pub const OPTION_ENABLE_HWCODEC: &str = "enable-hwcodec";
    pub const OPTION_APPROVE_MODE: &str = "approve-mode";
    pub const OPTION_VERIFICATION_METHOD: &str = "verification-method";
    pub const OPTION_CUSTOM_RENDEZVOUS_SERVER: &str = "custom-rendezvous-server";
    pub const OPTION_API_SERVER: &str = "api-server";
    pub const OPTION_KEY: &str = "key";
    pub const OPTION_PRESET_ADDRESS_BOOK_NAME: &str = "preset-address-book-name";
    pub const OPTION_PRESET_ADDRESS_BOOK_TAG: &str = "preset-address-book-tag";
    pub const OPTION_ENABLE_DIRECTX_CAPTURE: &str = "enable-directx-capture";
    pub const OPTION_ENABLE_ANDROID_SOFTWARE_ENCODING_HALF_SCALE: &str =
        "enable-android-software-encoding-half-scale";
    pub const OPTION_ENABLE_TRUSTED_DEVICES: &str = "enable-trusted-devices";

    // buildin options
    pub const OPTION_DISPLAY_NAME: &str = "display-name";
    pub const OPTION_DISABLE_UDP: &str = "disable-udp";
    pub const OPTION_PRESET_USERNAME: &str = "preset-user-name";
    pub const OPTION_PRESET_STRATEGY_NAME: &str = "preset-strategy-name";
    pub const OPTION_REMOVE_PRESET_PASSWORD_WARNING: &str = "remove-preset-password-warning";
    pub const OPTION_HIDE_SECURITY_SETTINGS: &str = "hide-security-settings";
    pub const OPTION_HIDE_NETWORK_SETTINGS: &str = "hide-network-settings";
    pub const OPTION_HIDE_SERVER_SETTINGS: &str = "hide-server-settings";
    pub const OPTION_HIDE_PROXY_SETTINGS: &str = "hide-proxy-settings";
    pub const OPTION_HIDE_USERNAME_ON_CARD: &str = "hide-username-on-card";
    pub const OPTION_HIDE_HELP_CARDS: &str = "hide-help-cards";
    pub const OPTION_DEFAULT_CONNECT_PASSWORD: &str = "default-connect-password";
    pub const OPTION_HIDE_TRAY: &str = "hide-tray";
    pub const OPTION_ONE_WAY_CLIPBOARD_REDIRECTION: &str = "one-way-clipboard-redirection";
    pub const OPTION_ALLOW_LOGON_SCREEN_PASSWORD: &str = "allow-logon-screen-password";
    pub const OPTION_ONE_WAY_FILE_TRANSFER: &str = "one-way-file-transfer";

    // flutter local options
    pub const OPTION_FLUTTER_REMOTE_MENUBAR_STATE: &str = "remoteMenubarState";
    pub const OPTION_FLUTTER_PEER_SORTING: &str = "peer-sorting";
    pub const OPTION_FLUTTER_PEER_TAB_INDEX: &str = "peer-tab-index";
    pub const OPTION_FLUTTER_PEER_TAB_ORDER: &str = "peer-tab-order";
    pub const OPTION_FLUTTER_PEER_TAB_VISIBLE: &str = "peer-tab-visible";
    pub const OPTION_FLUTTER_PEER_CARD_UI_TYLE: &str = "peer-card-ui-type";
    pub const OPTION_FLUTTER_CURRENT_AB_NAME: &str = "current-ab-name";
    pub const OPTION_ALLOW_REMOTE_CM_MODIFICATION: &str = "allow-remote-cm-modification";

    // android floating window options
    pub const OPTION_DISABLE_FLOATING_WINDOW: &str = "disable-floating-window";
    pub const OPTION_FLOATING_WINDOW_SIZE: &str = "floating-window-size";
    pub const OPTION_FLOATING_WINDOW_UNTOUCHABLE: &str = "floating-window-untouchable";
    pub const OPTION_FLOATING_WINDOW_TRANSPARENCY: &str = "floating-window-transparency";
    pub const OPTION_FLOATING_WINDOW_SVG: &str = "floating-window-svg";

    // android keep screen on
    pub const OPTION_KEEP_SCREEN_ON: &str = "keep-screen-on";

    pub const OPTION_DISABLE_GROUP_PANEL: &str = "disable-group-panel";
    pub const OPTION_PRE_ELEVATE_SERVICE: &str = "pre-elevate-service";

    // proxy settings
    // The following options are not real keys, they are just used for custom client advanced settings.
    // The real keys are in Config2::socks.
    pub const OPTION_PROXY_URL: &str = "proxy-url";
    pub const OPTION_PROXY_USERNAME: &str = "proxy-username";
    pub const OPTION_PROXY_PASSWORD: &str = "proxy-password";

    // DEFAULT_DISPLAY_SETTINGS, OVERWRITE_DISPLAY_SETTINGS
    pub const KEYS_DISPLAY_SETTINGS: &[&str] = &[
        OPTION_VIEW_ONLY,
        OPTION_SHOW_MONITORS_TOOLBAR,
        OPTION_COLLAPSE_TOOLBAR,
        OPTION_SHOW_REMOTE_CURSOR,
        OPTION_FOLLOW_REMOTE_CURSOR,
        OPTION_FOLLOW_REMOTE_WINDOW,
        OPTION_ZOOM_CURSOR,
        OPTION_SHOW_QUALITY_MONITOR,
        OPTION_DISABLE_AUDIO,
        OPTION_ENABLE_FILE_COPY_PASTE,
        OPTION_DISABLE_CLIPBOARD,
        OPTION_LOCK_AFTER_SESSION_END,
        OPTION_PRIVACY_MODE,
        OPTION_TOUCH_MODE,
        OPTION_I444,
        OPTION_REVERSE_MOUSE_WHEEL,
        OPTION_SWAP_LEFT_RIGHT_MOUSE,
        OPTION_DISPLAYS_AS_INDIVIDUAL_WINDOWS,
        OPTION_USE_ALL_MY_DISPLAYS_FOR_THE_REMOTE_SESSION,
        OPTION_VIEW_STYLE,
        OPTION_SCROLL_STYLE,
        OPTION_IMAGE_QUALITY,
        OPTION_CUSTOM_IMAGE_QUALITY,
        OPTION_CUSTOM_FPS,
        OPTION_CODEC_PREFERENCE,
        OPTION_SYNC_INIT_CLIPBOARD,
    ];
    // DEFAULT_LOCAL_SETTINGS, OVERWRITE_LOCAL_SETTINGS
    pub const KEYS_LOCAL_SETTINGS: &[&str] = &[
        OPTION_THEME,
        OPTION_LANGUAGE,
        OPTION_ENABLE_CONFIRM_CLOSING_TABS,
        OPTION_ENABLE_OPEN_NEW_CONNECTIONS_IN_TABS,
        OPTION_TEXTURE_RENDER,
        OPTION_SYNC_AB_WITH_RECENT_SESSIONS,
        OPTION_SYNC_AB_TAGS,
        OPTION_FILTER_AB_BY_INTERSECTION,
        OPTION_REMOTE_MENUBAR_DRAG_LEFT,
        OPTION_REMOTE_MENUBAR_DRAG_RIGHT,
        OPTION_HIDE_AB_TAGS_PANEL,
        OPTION_FLUTTER_REMOTE_MENUBAR_STATE,
        OPTION_FLUTTER_PEER_SORTING,
        OPTION_FLUTTER_PEER_TAB_INDEX,
        OPTION_FLUTTER_PEER_TAB_ORDER,
        OPTION_FLUTTER_PEER_TAB_VISIBLE,
        OPTION_FLUTTER_PEER_CARD_UI_TYLE,
        OPTION_FLUTTER_CURRENT_AB_NAME,
        OPTION_DISABLE_FLOATING_WINDOW,
        OPTION_FLOATING_WINDOW_SIZE,
        OPTION_FLOATING_WINDOW_UNTOUCHABLE,
        OPTION_FLOATING_WINDOW_TRANSPARENCY,
        OPTION_FLOATING_WINDOW_SVG,
        OPTION_KEEP_SCREEN_ON,
        OPTION_DISABLE_GROUP_PANEL,
        OPTION_PRE_ELEVATE_SERVICE,
        OPTION_ALLOW_REMOTE_CM_MODIFICATION,
        OPTION_ALLOW_AUTO_RECORD_OUTGOING,
        OPTION_VIDEO_SAVE_DIRECTORY,
    ];
    // DEFAULT_SETTINGS, OVERWRITE_SETTINGS
    pub const KEYS_SETTINGS: &[&str] = &[
        OPTION_ACCESS_MODE,
        OPTION_ENABLE_KEYBOARD,
        OPTION_ENABLE_CLIPBOARD,
        OPTION_ENABLE_FILE_TRANSFER,
        OPTION_ENABLE_AUDIO,
        OPTION_ENABLE_TUNNEL,
        OPTION_ENABLE_REMOTE_RESTART,
        OPTION_ENABLE_RECORD_SESSION,
        OPTION_ENABLE_BLOCK_INPUT,
        OPTION_ALLOW_REMOTE_CONFIG_MODIFICATION,
        OPTION_ENABLE_LAN_DISCOVERY,
        OPTION_DIRECT_SERVER,
        OPTION_DIRECT_ACCESS_PORT,
        OPTION_WHITELIST,
        OPTION_ALLOW_AUTO_DISCONNECT,
        OPTION_AUTO_DISCONNECT_TIMEOUT,
        OPTION_ALLOW_ONLY_CONN_WINDOW_OPEN,
        OPTION_ALLOW_AUTO_RECORD_INCOMING,
        OPTION_ENABLE_ABR,
        OPTION_ALLOW_REMOVE_WALLPAPER,
        OPTION_ALLOW_ALWAYS_SOFTWARE_RENDER,
        OPTION_ALLOW_LINUX_HEADLESS,
        OPTION_ENABLE_HWCODEC,
        OPTION_APPROVE_MODE,
        OPTION_VERIFICATION_METHOD,
        OPTION_PROXY_URL,
        OPTION_PROXY_USERNAME,
        OPTION_PROXY_PASSWORD,
        OPTION_CUSTOM_RENDEZVOUS_SERVER,
        OPTION_API_SERVER,
        OPTION_KEY,
        OPTION_PRESET_ADDRESS_BOOK_NAME,
        OPTION_PRESET_ADDRESS_BOOK_TAG,
        OPTION_ENABLE_DIRECTX_CAPTURE,
        OPTION_ENABLE_ANDROID_SOFTWARE_ENCODING_HALF_SCALE,
        OPTION_ENABLE_TRUSTED_DEVICES,
    ];

    // BUILDIN_SETTINGS
    pub const KEYS_BUILDIN_SETTINGS: &[&str] = &[
        OPTION_DISPLAY_NAME,
        OPTION_DISABLE_UDP,
        OPTION_PRESET_USERNAME,
        OPTION_PRESET_STRATEGY_NAME,
        OPTION_REMOVE_PRESET_PASSWORD_WARNING,
        OPTION_HIDE_SECURITY_SETTINGS,
        OPTION_HIDE_NETWORK_SETTINGS,
        OPTION_HIDE_SERVER_SETTINGS,
        OPTION_HIDE_PROXY_SETTINGS,
        OPTION_HIDE_USERNAME_ON_CARD,
        OPTION_HIDE_HELP_CARDS,
        OPTION_DEFAULT_CONNECT_PASSWORD,
        OPTION_HIDE_TRAY,
        OPTION_ONE_WAY_CLIPBOARD_REDIRECTION,
        OPTION_ALLOW_LOGON_SCREEN_PASSWORD,
        OPTION_ONE_WAY_FILE_TRANSFER,
    ];
}

pub fn common_load<
    T: serde::Serialize + serde::de::DeserializeOwned + Default + std::fmt::Debug,
>(
    suffix: &str,
) -> T {
    Config::load_::<T>(suffix)
}

pub fn common_store<T: serde::Serialize>(config: &T, suffix: &str) {
    Config::store_(config, suffix);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize() {
        let cfg: Config = Default::default();
        let res = toml::to_string_pretty(&cfg);
        assert!(res.is_ok());
        let cfg: PeerConfig = Default::default();
        let res = toml::to_string_pretty(&cfg);
        assert!(res.is_ok());
    }

    #[test]
    fn test_overwrite_settings() {
        DEFAULT_SETTINGS
            .write()
            .unwrap()
            .insert("b".to_string(), "a".to_string());
        DEFAULT_SETTINGS
            .write()
            .unwrap()
            .insert("c".to_string(), "a".to_string());
        CONFIG2
            .write()
            .unwrap()
            .options
            .insert("a".to_string(), "b".to_string());
        CONFIG2
            .write()
            .unwrap()
            .options
            .insert("b".to_string(), "b".to_string());
        OVERWRITE_SETTINGS
            .write()
            .unwrap()
            .insert("b".to_string(), "c".to_string());
        OVERWRITE_SETTINGS
            .write()
            .unwrap()
            .insert("c".to_string(), "f".to_string());
        OVERWRITE_SETTINGS
            .write()
            .unwrap()
            .insert("d".to_string(), "c".to_string());
        let mut res: HashMap<String, String> = Default::default();
        res.insert("b".to_owned(), "c".to_string());
        res.insert("d".to_owned(), "c".to_string());
        res.insert("c".to_owned(), "a".to_string());
        Config::purify_options(&mut res);
        assert!(res.len() == 0);
        res.insert("b".to_owned(), "c".to_string());
        res.insert("d".to_owned(), "c".to_string());
        res.insert("c".to_owned(), "a".to_string());
        res.insert("f".to_owned(), "a".to_string());
        Config::purify_options(&mut res);
        assert!(res.len() == 1);
        res.insert("b".to_owned(), "c".to_string());
        res.insert("d".to_owned(), "c".to_string());
        res.insert("c".to_owned(), "a".to_string());
        res.insert("f".to_owned(), "a".to_string());
        res.insert("e".to_owned(), "d".to_string());
        Config::purify_options(&mut res);
        assert!(res.len() == 2);
        res.insert("b".to_owned(), "c".to_string());
        res.insert("d".to_owned(), "c".to_string());
        res.insert("c".to_owned(), "a".to_string());
        res.insert("f".to_owned(), "a".to_string());
        res.insert("c".to_owned(), "d".to_string());
        res.insert("d".to_owned(), "cc".to_string());
        Config::purify_options(&mut res);
        DEFAULT_SETTINGS
            .write()
            .unwrap()
            .insert("f".to_string(), "c".to_string());
        Config::purify_options(&mut res);
        assert!(res.len() == 2);
        DEFAULT_SETTINGS
            .write()
            .unwrap()
            .insert("f".to_string(), "a".to_string());
        Config::purify_options(&mut res);
        assert!(res.len() == 1);
        let res = Config::get_options();
        assert!(res["a"] == "b");
        assert!(res["c"] == "f");
        assert!(res["b"] == "c");
        assert!(res["d"] == "c");
        assert!(Config::get_option("a") == "b");
        assert!(Config::get_option("c") == "f");
        assert!(Config::get_option("b") == "c");
        assert!(Config::get_option("d") == "c");
        DEFAULT_SETTINGS.write().unwrap().clear();
        OVERWRITE_SETTINGS.write().unwrap().clear();
        CONFIG2.write().unwrap().options.clear();

        DEFAULT_LOCAL_SETTINGS
            .write()
            .unwrap()
            .insert("b".to_string(), "a".to_string());
        DEFAULT_LOCAL_SETTINGS
            .write()
            .unwrap()
            .insert("c".to_string(), "a".to_string());
        LOCAL_CONFIG
            .write()
            .unwrap()
            .options
            .insert("a".to_string(), "b".to_string());
        LOCAL_CONFIG
            .write()
            .unwrap()
            .options
            .insert("b".to_string(), "b".to_string());
        OVERWRITE_LOCAL_SETTINGS
            .write()
            .unwrap()
            .insert("b".to_string(), "c".to_string());
        OVERWRITE_LOCAL_SETTINGS
            .write()
            .unwrap()
            .insert("d".to_string(), "c".to_string());
        assert!(LocalConfig::get_option("a") == "b");
        assert!(LocalConfig::get_option("c") == "a");
        assert!(LocalConfig::get_option("b") == "c");
        assert!(LocalConfig::get_option("d") == "c");
        DEFAULT_LOCAL_SETTINGS.write().unwrap().clear();
        OVERWRITE_LOCAL_SETTINGS.write().unwrap().clear();
        LOCAL_CONFIG.write().unwrap().options.clear();

        DEFAULT_DISPLAY_SETTINGS
            .write()
            .unwrap()
            .insert("b".to_string(), "a".to_string());
        DEFAULT_DISPLAY_SETTINGS
            .write()
            .unwrap()
            .insert("c".to_string(), "a".to_string());
        USER_DEFAULT_CONFIG
            .write()
            .unwrap()
            .0
            .options
            .insert("a".to_string(), "b".to_string());
        USER_DEFAULT_CONFIG
            .write()
            .unwrap()
            .0
            .options
            .insert("b".to_string(), "b".to_string());
        OVERWRITE_DISPLAY_SETTINGS
            .write()
            .unwrap()
            .insert("b".to_string(), "c".to_string());
        OVERWRITE_DISPLAY_SETTINGS
            .write()
            .unwrap()
            .insert("d".to_string(), "c".to_string());
        assert!(UserDefaultConfig::read("a") == "b");
        assert!(UserDefaultConfig::read("c") == "a");
        assert!(UserDefaultConfig::read("b") == "c");
        assert!(UserDefaultConfig::read("d") == "c");
        DEFAULT_DISPLAY_SETTINGS.write().unwrap().clear();
        OVERWRITE_DISPLAY_SETTINGS.write().unwrap().clear();
        LOCAL_CONFIG.write().unwrap().options.clear();
    }

    #[test]
    fn test_config_deserialize() {
        let wrong_type_str = r#"
        id = true
        enc_id = []
        password = 1
        salt = "123456"
        key_pair = {}
        key_confirmed = "1"
        keys_confirmed = 1
        "#;
        let cfg = toml::from_str::<Config>(wrong_type_str);
        assert_eq!(
            cfg,
            Ok(Config {
                salt: "123456".to_string(),
                ..Default::default()
            })
        );

        let wrong_field_str = r#"
        hello = "world"
        key_confirmed = true
        "#;
        let cfg = toml::from_str::<Config>(wrong_field_str);
        assert_eq!(
            cfg,
            Ok(Config {
                key_confirmed: true,
                ..Default::default()
            })
        );
    }

    #[test]
    fn test_peer_config_deserialize() {
        let default_peer_config = toml::from_str::<PeerConfig>("").unwrap();
        // test custom_resolution
        {
            let wrong_type_str = r#"
            view_style = "adaptive"
            scroll_style = "scrollbar"
            custom_resolutions = true
            "#;
            let mut cfg_to_compare = default_peer_config.clone();
            cfg_to_compare.view_style = "adaptive".to_string();
            cfg_to_compare.scroll_style = "scrollbar".to_string();
            let cfg = toml::from_str::<PeerConfig>(wrong_type_str);
            assert_eq!(cfg, Ok(cfg_to_compare), "Failed to test wrong_type_str");

            let wrong_type_str = r#"
            view_style = "adaptive"
            scroll_style = "scrollbar"
            [custom_resolutions.0]
            w = "1920"
            h = 1080
            "#;
            let mut cfg_to_compare = default_peer_config.clone();
            cfg_to_compare.view_style = "adaptive".to_string();
            cfg_to_compare.scroll_style = "scrollbar".to_string();
            let cfg = toml::from_str::<PeerConfig>(wrong_type_str);
            assert_eq!(cfg, Ok(cfg_to_compare), "Failed to test wrong_type_str");

            let wrong_field_str = r#"
            [custom_resolutions.0]
            w = 1920
            h = 1080
            hello = "world"
            [ui_flutter]
            "#;
            let mut cfg_to_compare = default_peer_config.clone();
            cfg_to_compare.custom_resolutions =
                HashMap::from([("0".to_string(), Resolution { w: 1920, h: 1080 })]);
            let cfg = toml::from_str::<PeerConfig>(wrong_field_str);
            assert_eq!(cfg, Ok(cfg_to_compare), "Failed to test wrong_field_str");
        }
    }

    #[test]
    fn test_store_load() {
        let peerconfig_id = "123456789";
        let cfg: PeerConfig = Default::default();
        cfg.store(&peerconfig_id);
        assert_eq!(PeerConfig::load(&peerconfig_id), cfg);

        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                // ignore file type information by masking with 0o777 (see https://stackoverflow.com/a/50045872)
                fs::metadata(PeerConfig::path(&peerconfig_id))
                    .expect("reading metadata failed")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
