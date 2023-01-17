use std::{
    collections::HashMap,
    fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::SystemTime,
};

use anyhow::Result;
use rand::Rng;
use regex::Regex;
use serde as de;
use serde_derive::{Deserialize, Serialize};
use sodiumoxide::base64;
use sodiumoxide::crypto::sign;

use crate::{
    log,
    password_security::{
        decrypt_str_or_original, decrypt_vec_or_original, encrypt_str_or_original,
        encrypt_vec_or_original,
    },
};

pub const RENDEZVOUS_TIMEOUT: u64 = 12_000;
pub const CONNECT_TIMEOUT: u64 = 18_000;
pub const READ_TIMEOUT: u64 = 30_000;
pub const REG_INTERVAL: i64 = 12_000;
pub const COMPRESS_LEVEL: i32 = 3;
const SERIAL: i32 = 3;
const PASSWORD_ENC_VERSION: &'static str = "00";
// 128x128
#[cfg(target_os = "macos")] // 128x128 on 160x160 canvas, then shrink to 128, mac looks better with padding
pub const ICON: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAMAAAD04JH5AAAAyVBMVEUAAAAAcf8Acf8Acf8Acv8Acf8Acf8Acf8Acf8AcP8Acf8Ab/8AcP8Acf////8AaP/z+f/o8v/k7v/5/v/T5f8AYP/u9v/X6f+hx/+Kuv95pP8Aef/B1/+TwP9xoP8BdP/g6P+Irv9ZmP8Bgf/E3f98q/9sn/+01f+Es/9nm/9Jif8hhv8off/M4P+syP+avP86iP/c7f+xy/9yqf9Om/9hk/9Rjv+60P99tv9fpf88lv8yjf8Tgf8deP+kvP8BiP8NeP8hkP80gP8oj2VLAAAADXRSTlMA7o7qLvnaxZ1FOxYPjH9HWgAABHJJREFUeNrtm+tW4jAQgBfwuu7MtIUWsOUiCCioIIgLiqvr+z/UHq/LJKVkmwTcc/r9E2nzlU4mSTP9lpGRkZGR8VX5cZjfL+yCEXYL+/nDH//U/Pd8DgyTy39Xbv7oIAcWyB0cqbW/sweW2NtRaj8H1sgpGOwUIAH7Bkd7YJW9dXFwAJY5WNP/cmCZQnJvzIN18on5LwfWySXlxEPYAIcad8D6PdiHDbCfIFCADVBIENiFDbCbIACKPPXrZ+cP8E6/0znvP4EymgIEravIRcTxu8HxNSJ60a8W0AYECKrlAN+YwAthCd9wm1Ug6wKzIn5SgRduXfwkqDasCjx0XFzi9PV6zwNcIuhcWBOg+ikySq8C9UD4dEKWBCoOcspvAuLHTo9sCDQiFPHotRM48j8G5gVur1FdAN2uaYEuiz7xFsgEJ2RUoMUakXuBTHHoGxQYOBhHjeUBAefEnMAowFhaLBOKuOemBBbxLRQrH2PBCgMvNCPQGMeevTb9zLrPxz2Mo+QbEaijzPUcOOHMQZkKGRAIPem39+bypREMPTkQW/oCfk866zAkiIFG4yIKRE/aAnfiSd0WrORY6pFdXQEqi9mvAQm0RIOSnoCcZ8vJoz3diCnjRk+g8VP4/fuQDJ2Lxr6WwG0gXs9aTpDzW0vgDBlVUpixR8gYk44AD8FrUKHr8JQJGgIDnoDqoALxmWPQSi9AVVzm8gKUuEPGr/QCvptwJkbSYT/TC4S8C96DGjTj86aHtAI0x2WaBIq0eSYYpRa4EsdWVVwWu9O0Aj6f6dyBMnwEraeOgSYu0wZlauzA47QCbT7DgAQSE+hZWoEBF/BBmWOewNMK3BsSqKUW4MGcWqCSVmDkbvkXGKQOwg6PAUO9oL3xXhA20yaiCjuwYygRVQlUOTWTCf2SuNJTxeFjgaHByGuAIvd8ItdPLTDhS7IuqEE1YSKVOgbayLhSFQhMzYh8hwfBs1r7c505YVIQYEdNoKwxK06MJiyrpUFHiF0NAfCQUVHoiRclIXJIR6C2fqG37pBHvcWpgwzvAtYwkR5UGV2e42UISdBJETl3mg8ouo54Rcnti1/vaT+iuUQBt500Cgo4U10BeHSkk57FB0JjWkKRMWgLUA0lLodtImAQdaMiiri3+gIAPZQoutHNsgKF1aaDMhMyIdBf8Th+Bh8MTjGWCpl5Wv43tDmnF+IUVMrcZgRoiAxhtrloYizNkZaAnF5leglbNhj0wYCAbCDvGb0mP4nib7O7ZlcYQ2m1gPtIZgVgGNNMeaVAaWR+57TrqgtUnm3sHQ+kYeE6fufUubG1ez50FXbPnWgBlgSABmN3TTcsRl2yWkHRrwbiunvk/W2+Mg1hPZplPDeXRbZzStFH15s1QIVd3UImP5z/bHpeeQLvRJ7XLFUffQIlCvqlXETQbgN9/rlYABGosv+Vi9m2Xs639YLGrZd0br+odetlvdsvbN56abfd4vbCzv9Q3v/ygoOV21A4OPpfXvH4Ai+5ZGRkZGRkbJA/t/I0QMzoMiEAAAAASUVORK5CYII=
";
#[cfg(not(target_os = "macos"))] // 128x128 no padding
pub const ICON: &str = "data:image/png;base64,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
";
#[cfg(target_os = "macos")]
lazy_static::lazy_static! {
    pub static ref ORG: Arc<RwLock<String>> = Arc::new(RwLock::new("com.carriez".to_owned()));
}

type Size = (i32, i32, i32, i32);

lazy_static::lazy_static! {
    static ref CONFIG: Arc<RwLock<Config>> = Arc::new(RwLock::new(Config::load()));
    static ref CONFIG2: Arc<RwLock<Config2>> = Arc::new(RwLock::new(Config2::load()));
    static ref LOCAL_CONFIG: Arc<RwLock<LocalConfig>> = Arc::new(RwLock::new(LocalConfig::load()));
    pub static ref ONLINE: Arc<Mutex<HashMap<String, i64>>> = Default::default();
    pub static ref PROD_RENDEZVOUS_SERVER: Arc<RwLock<String>> = Default::default();
    pub static ref APP_NAME: Arc<RwLock<String>> = Arc::new(RwLock::new("RustDesk".to_owned()));
    static ref KEY_PAIR: Arc<Mutex<Option<(Vec<u8>, Vec<u8>)>>> = Default::default();
    static ref HW_CODEC_CONFIG: Arc<RwLock<HwCodecConfig>> = Arc::new(RwLock::new(HwCodecConfig::load()));
}

lazy_static::lazy_static! {
    pub static ref APP_DIR: Arc<RwLock<String>> = Default::default();
}

#[cfg(any(target_os = "android", target_os = "ios"))]
lazy_static::lazy_static! {
    pub static ref APP_HOME_DIR: Arc<RwLock<String>> = Default::default();
}

// #[cfg(any(target_os = "android", target_os = "ios"))]
lazy_static::lazy_static! {
    pub static ref HELPER_URL: HashMap<&'static str, &'static str> = HashMap::from([
        ("rustdesk docs home", "https://rustdesk.com/docs/en/"),
        ("rustdesk docs x11-required", "https://rustdesk.com/docs/en/manual/linux/#x11-required"),
        ]);
}

const CHARS: &'static [char] = &[
    '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k',
    'm', 'n', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

//check for env variable RENDEZVOUS_SERVER if not use the default
pub const RENDEZVOUS_SERVERS: [&'static str;3] = 
    match option_env!("RENDEZVOUS_SERVER") {
        Some(key) => [key,key,key],
        None => ["rs-ny.rustdesk.com","rs-sg.rustdesk.com","rs-cn.rustdesk.com"],
    };


//check for env variable RS_PUB_KEY if not use default
pub const RS_PUB_KEY: &'static str = match option_env!("RS_PUB_KEY_VAL") {
    Some(key) => key,
    None => "OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=",
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
            let s: &str = de::Deserialize::deserialize(deserializer)?;
            Ok(if s.is_empty() {
                Self::$default_func()
            } else {
                s.to_owned()
            })
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
    #[serde(default)]
    pub id: String, // use
    #[serde(default)]
    enc_id: String, // store
    #[serde(default)]
    password: String,
    #[serde(default)]
    salt: String,
    #[serde(default)]
    key_pair: (Vec<u8>, Vec<u8>), // sk, pk
    #[serde(default)]
    key_confirmed: bool,
    #[serde(default)]
    keys_confirmed: HashMap<String, bool>,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone)]
pub struct Socks5Server {
    #[serde(default)]
    pub proxy: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

// more variable configs
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Config2 {
    #[serde(default)]
    rendezvous_server: String,
    #[serde(default)]
    nat_type: i32,
    #[serde(default)]
    serial: i32,

    #[serde(default)]
    socks: Option<Socks5Server>,

    // the other scalar value must before this
    #[serde(default)]
    pub options: HashMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct PeerConfig {
    #[serde(default)]
    pub password: Vec<u8>,
    #[serde(default)]
    pub size: Size,
    #[serde(default)]
    pub size_ft: Size,
    #[serde(default)]
    pub size_pf: Size,
    #[serde(
        default = "PeerConfig::default_view_style",
        deserialize_with = "PeerConfig::deserialize_view_style"
    )]
    pub view_style: String,
    #[serde(
        default = "PeerConfig::default_scroll_style",
        deserialize_with = "PeerConfig::deserialize_scroll_style"
    )]
    pub scroll_style: String,
    #[serde(
        default = "PeerConfig::default_image_quality",
        deserialize_with = "PeerConfig::deserialize_image_quality"
    )]
    pub image_quality: String,
    #[serde(default)]
    pub custom_image_quality: Vec<i32>,
    #[serde(default)]
    pub show_remote_cursor: bool,
    #[serde(default)]
    pub lock_after_session_end: bool,
    #[serde(default)]
    pub privacy_mode: bool,
    #[serde(default)]
    pub port_forwards: Vec<(i32, String, i32)>,
    #[serde(default)]
    pub direct_failures: i32,
    #[serde(default)]
    pub disable_audio: bool,
    #[serde(default)]
    pub disable_clipboard: bool,
    #[serde(default)]
    pub enable_file_transfer: bool,
    #[serde(default)]
    pub show_quality_monitor: bool,
    #[serde(default)]
    pub keyboard_mode: String,

    // The other scalar value must before this
    #[serde(default, deserialize_with = "PeerConfig::deserialize_options")]
    pub options: HashMap<String, String>,
    // Various data for flutter ui
    #[serde(default)]
    pub ui_flutter: HashMap<String, String>,
    #[serde(default)]
    pub info: PeerInfoSerde,
    #[serde(default)]
    pub transfer: TransferSerde,
}

#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone)]
pub struct PeerInfoSerde {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub platform: String,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct TransferSerde {
    #[serde(default)]
    pub write_jobs: Vec<String>,
    #[serde(default)]
    pub read_jobs: Vec<String>,
}

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
                if let Ok(output) = std::process::Command::new("whoami").output() {
                    let user = String::from_utf8_lossy(&output.stdout)
                        .to_string()
                        .trim()
                        .to_owned();
                    if user != "root" {
                        return format!("/home/{}", user).into();
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
        if let Some(mut socks) = config.socks {
            let (password, _, store) =
                decrypt_str_or_original(&socks.password, PASSWORD_ENC_VERSION);
            socks.password = password;
            config.socks = Some(socks);
            if store {
                config.store();
            }
        }
        config
    }

    pub fn file() -> PathBuf {
        Config::file_("2")
    }

    fn store(&self) {
        let mut config = self.clone();
        if let Some(mut socks) = config.socks {
            socks.password = encrypt_str_or_original(&socks.password, PASSWORD_ENC_VERSION);
            config.socks = Some(socks);
        }
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
            log::error!("Failed to load config: {}", err);
            T::default()
        }
    };
    cfg
}

#[inline]
pub fn store_path<T: serde::Serialize>(path: PathBuf, cfg: T) -> crate::ResultType<()> {
    Ok(confy::store_path(path, cfg)?)
}

impl Config {
    fn load_<T: serde::Serialize + serde::de::DeserializeOwned + Default + std::fmt::Debug>(
        suffix: &str,
    ) -> T {
        let file = Self::file_(suffix);
        log::debug!("Configuration path: {}", file.display());
        let cfg = load_path(file);
        if suffix.is_empty() {
            log::trace!("{:?}", cfg);
        }
        cfg
    }

    fn store_<T: serde::Serialize>(config: &T, suffix: &str) {
        let file = Self::file_(suffix);
        if let Err(err) = store_path(file, config) {
            log::error!("Failed to store config: {}", err);
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
        } else {
            if crate::get_modified_time(&Self::file_(""))
                .checked_sub(std::time::Duration::from_secs(30)) // allow modification during installation
                .unwrap_or(crate::get_exe_time())
                < crate::get_exe_time()
            {
                if !config.id.is_empty()
                    && config.enc_id.is_empty()
                    && !decrypt_str_or_original(&config.id, PASSWORD_ENC_VERSION).1
                {
                    id_valid = true;
                    store = true;
                }
            }
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
        config.password = encrypt_str_or_original(&config.password, PASSWORD_ENC_VERSION);
        config.enc_id = encrypt_str_or_original(&config.id, PASSWORD_ENC_VERSION);
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
        return Self::path(APP_HOME_DIR.read().unwrap().as_str());
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
            let org = "";
            #[cfg(target_os = "macos")]
            let org = ORG.read().unwrap().clone();
            // /var/root for root
            if let Some(project) =
                directories_next::ProjectDirs::from("", &org, &*APP_NAME.read().unwrap())
            {
                let mut path = patch(project.config_dir().to_path_buf());
                path.push(p);
                return path;
            }
            return "".into();
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
            path.push(format!("ipc{}", postfix));
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
        let mut rendezvous_server = Self::get_option("custom-rendezvous-server");
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
                .unwrap_or("".to_owned());
        }
        if !rendezvous_server.contains(":") {
            rendezvous_server = format!("{}:{}", rendezvous_server, RENDEZVOUS_PORT);
        }
        rendezvous_server
    }

    pub fn get_rendezvous_servers() -> Vec<String> {
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
                .split(",")
                .filter(|x| x.contains("."))
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
                delay = tmp_delay.clone();
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
                id = id & 0x1FFFFFFF;
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
        if let Some(true) = CONFIG.read().unwrap().keys_confirmed.get(host) {
            true
        } else {
            false
        }
    }

    pub fn set_host_key_confirmed(host: &str, v: bool) {
        if Self::get_host_key_confirmed(host) == v {
            return;
        }
        let mut config = CONFIG.write().unwrap();
        config.keys_confirmed.insert(host.to_owned(), v);
        config.store();
    }

    pub fn get_key_pair() -> (Vec<u8>, Vec<u8>) {
        // lock here to make sure no gen_keypair more than once
        // no use of CONFIG directly here to ensure no recursive calling in Config::load because of password dec which calling this function
        let mut lock = KEY_PAIR.lock().unwrap();
        if let Some(p) = lock.as_ref() {
            return p.clone();
        }
        let mut config = Config::load_::<Config>("");
        if config.key_pair.0.is_empty() {
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
        return config.key_pair;
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
        CONFIG2.read().unwrap().options.clone()
    }

    pub fn set_options(v: HashMap<String, String>) {
        let mut config = CONFIG2.write().unwrap();
        if config.options == v {
            return;
        }
        config.options = v;
        config.store();
    }

    pub fn get_option(k: &str) -> String {
        if let Some(v) = CONFIG2.read().unwrap().options.get(k) {
            v.clone()
        } else {
            "".to_owned()
        }
    }

    pub fn set_option(k: String, v: String) {
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
        let mut config = CONFIG.write().unwrap();
        if password == config.password {
            return;
        }
        config.password = password.into();
        config.store();
    }

    pub fn get_permanent_password() -> String {
        CONFIG.read().unwrap().password.clone()
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

    pub fn get_socks() -> Option<Socks5Server> {
        CONFIG2.read().unwrap().socks.clone()
    }

    pub fn get_network_type() -> NetworkType {
        match &CONFIG2.read().unwrap().socks {
            None => NetworkType::Direct,
            Some(_) => NetworkType::ProxySocks,
        }
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
            path.with_extension(&ext)
        } else {
            path.with_extension("toml")
        }
    }
}

const PEERS: &str = "peers";

impl PeerConfig {
    pub fn load(id: &str) -> PeerConfig {
        let _lock = CONFIG.read().unwrap();
        match confy::load_path(&Self::path(id)) {
            Ok(config) => {
                let mut config: PeerConfig = config;
                let mut store = false;
                let (password, _, store2) =
                    decrypt_vec_or_original(&config.password, PASSWORD_ENC_VERSION);
                config.password = password;
                store = store || store2;
                config.options.get_mut("rdp_password").map(|v| {
                    let (password, _, store2) = decrypt_str_or_original(v, PASSWORD_ENC_VERSION);
                    *v = password;
                    store = store || store2;
                });
                config.options.get_mut("os-password").map(|v| {
                    let (password, _, store2) = decrypt_str_or_original(v, PASSWORD_ENC_VERSION);
                    *v = password;
                    store = store || store2;
                });
                if store {
                    config.store(id);
                }
                config
            }
            Err(err) => {
                log::error!("Failed to load config: {}", err);
                Default::default()
            }
        }
    }

    pub fn store(&self, id: &str) {
        let _lock = CONFIG.read().unwrap();
        let mut config = self.clone();
        config.password = encrypt_vec_or_original(&config.password, PASSWORD_ENC_VERSION);
        config
            .options
            .get_mut("rdp_password")
            .map(|v| *v = encrypt_str_or_original(v, PASSWORD_ENC_VERSION));
        config
            .options
            .get_mut("os-password")
            .map(|v| *v = encrypt_str_or_original(v, PASSWORD_ENC_VERSION));
        if let Err(err) = store_path(Self::path(id), config) {
            log::error!("Failed to store config: {}", err);
        }
    }

    pub fn remove(id: &str) {
        fs::remove_file(&Self::path(id)).ok();
    }

    fn path(id: &str) -> PathBuf {
        let id_encoded: String;

        //If the id contains invalid chars, encode it
        let forbidden_paths = Regex::new(r".*[<>:/\\|\?\*].*").unwrap();
        if forbidden_paths.is_match(id) {
            id_encoded =
                "base64_".to_string() + base64::encode(id, base64::Variant::Original).as_str();
        } else {
            id_encoded = id.to_string();
        }
        let path: PathBuf = [PEERS, id_encoded.as_str()].iter().collect();
        Config::with_extension(Config::path(path))
    }

    pub fn peers() -> Vec<(String, SystemTime, PeerConfig)> {
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
                        let t = crate::get_modified_time(&p);
                        let id = p
                            .file_stem()
                            .map(|p| p.to_str().unwrap_or(""))
                            .unwrap_or("")
                            .to_owned();

                        let id_decoded_string: String;
                        if id.starts_with("base64_") && id.len() != 7 {
                            let id_decoded = base64::decode(&id[7..], base64::Variant::Original)
                                .unwrap_or(Vec::new());
                            id_decoded_string =
                                String::from_utf8_lossy(&id_decoded).as_ref().to_owned();
                        } else {
                            id_decoded_string = id;
                        }

                        let c = PeerConfig::load(&id_decoded_string);
                        if c.info.platform.is_empty() {
                            fs::remove_file(&p).ok();
                        }
                        (id_decoded_string, t, c)
                    })
                    .filter(|p| !p.2.info.platform.is_empty())
                    .collect();
                peers.sort_unstable_by(|a, b| b.1.cmp(&a.1));
                return peers;
            }
        }
        Default::default()
    }

    serde_field_string!(
        default_view_style,
        deserialize_view_style,
        "original".to_owned()
    );
    serde_field_string!(
        default_scroll_style,
        deserialize_scroll_style,
        "scrollauto".to_owned()
    );
    serde_field_string!(
        default_image_quality,
        deserialize_image_quality,
        "balanced".to_owned()
    );

    fn deserialize_options<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let mut mp: HashMap<String, String> = de::Deserialize::deserialize(deserializer)?;
        if !mp.contains_key("codec-preference") {
            mp.insert("codec-preference".to_owned(), "auto".to_owned());
        }
        Ok(mp)
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct LocalConfig {
    #[serde(default)]
    remote_id: String, // latest used one
    #[serde(default)]
    kb_layout_type: String,
    #[serde(default)]
    size: Size,
    #[serde(default)]
    pub fav: Vec<String>,
    #[serde(default)]
    options: HashMap<String, String>,
    // Various data for flutter ui
    #[serde(default)]
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
        if let Some(v) = LOCAL_CONFIG.read().unwrap().options.get(k) {
            v.clone()
        } else {
            "".to_owned()
        }
    }

    pub fn set_option(k: String, v: String) {
        let mut config = LOCAL_CONFIG.write().unwrap();
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

    pub fn get_flutter_config(k: &str) -> String {
        if let Some(v) = LOCAL_CONFIG.read().unwrap().ui_flutter.get(k) {
            v.clone()
        } else {
            "".to_owned()
        }
    }

    pub fn set_flutter_config(k: String, v: String) {
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
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub ip_mac: HashMap<String, String>,
}

impl DiscoveryPeer {
    pub fn is_same_peer(&self, other: &DiscoveryPeer) -> bool {
        self.id == other.id && self.username == other.username
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct LanPeers {
    pub peers: Vec<DiscoveryPeer>,
}

impl LanPeers {
    pub fn load() -> LanPeers {
        let _lock = CONFIG.read().unwrap();
        match confy::load_path(&Config::file_("_lan_peers")) {
            Ok(peers) => peers,
            Err(err) => {
                log::error!("Failed to load lan peers: {}", err);
                Default::default()
            }
        }
    }

    pub fn store(peers: &Vec<DiscoveryPeer>) {
        let f = LanPeers {
            peers: peers.clone(),
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
pub struct HwCodecConfig {
    #[serde(default)]
    pub options: HashMap<String, String>,
}

impl HwCodecConfig {
    pub fn load() -> HwCodecConfig {
        Config::load_::<HwCodecConfig>("_hwcodec")
    }

    pub fn store(&self) {
        Config::store_(self, "_hwcodec");
    }

    pub fn remove() {
        std::fs::remove_file(Config::file_("_hwcodec")).ok();
    }

    /// refresh current global HW_CODEC_CONFIG, usually uesd after HwCodecConfig::remove()
    pub fn refresh() {
        *HW_CODEC_CONFIG.write().unwrap() = HwCodecConfig::load();
        log::debug!("HW_CODEC_CONFIG refreshed successfully");
    }

    pub fn get() -> HwCodecConfig {
        return HW_CODEC_CONFIG.read().unwrap().clone();
    }
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
}
