use crate::log;
use directories_next::ProjectDirs;
use rand::Rng;
use serde_derive::{Deserialize, Serialize};
use sodiumoxide::crypto::sign;
use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::SystemTime,
};

pub const APP_NAME: &str = "RustDesk";
pub const BIND_INTERFACE: &str = "0.0.0.0";
pub const RENDEZVOUS_TIMEOUT: u64 = 12_000;
pub const CONNECT_TIMEOUT: u64 = 18_000;
pub const COMPRESS_LEVEL: i32 = 3;
const SERIAL: i32 = 0;
// 128x128
#[cfg(target_os = "macos")] // 128x128 on 160x160 canvas, then shrink to 128, mac looks better with padding
pub const ICON: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAMAAAD04JH5AAAAyVBMVEUAAAAAcf8Acf8Acf8Acv8Acf8Acf8Acf8Acf8AcP8Acf8Ab/8AcP8Acf////8AaP/z+f/o8v/k7v/5/v/T5f8AYP/u9v/X6f+hx/+Kuv95pP8Aef/B1/+TwP9xoP8BdP/g6P+Irv9ZmP8Bgf/E3f98q/9sn/+01f+Es/9nm/9Jif8hhv8off/M4P+syP+avP86iP/c7f+xy/9yqf9Om/9hk/9Rjv+60P99tv9fpf88lv8yjf8Tgf8deP+kvP8BiP8NeP8hkP80gP8oj2VLAAAADXRSTlMA7o7qLvnaxZ1FOxYPjH9HWgAABHJJREFUeNrtm+tW4jAQgBfwuu7MtIUWsOUiCCioIIgLiqvr+z/UHq/LJKVkmwTcc/r9E2nzlU4mSTP9lpGRkZGR8VX5cZjfL+yCEXYL+/nDH//U/Pd8DgyTy39Xbv7oIAcWyB0cqbW/sweW2NtRaj8H1sgpGOwUIAH7Bkd7YJW9dXFwAJY5WNP/cmCZQnJvzIN18on5LwfWySXlxEPYAIcad8D6PdiHDbCfIFCADVBIENiFDbCbIACKPPXrZ+cP8E6/0znvP4EymgIEravIRcTxu8HxNSJ60a8W0AYECKrlAN+YwAthCd9wm1Ug6wKzIn5SgRduXfwkqDasCjx0XFzi9PV6zwNcIuhcWBOg+ikySq8C9UD4dEKWBCoOcspvAuLHTo9sCDQiFPHotRM48j8G5gVur1FdAN2uaYEuiz7xFsgEJ2RUoMUakXuBTHHoGxQYOBhHjeUBAefEnMAowFhaLBOKuOemBBbxLRQrH2PBCgMvNCPQGMeevTb9zLrPxz2Mo+QbEaijzPUcOOHMQZkKGRAIPem39+bypREMPTkQW/oCfk866zAkiIFG4yIKRE/aAnfiSd0WrORY6pFdXQEqi9mvAQm0RIOSnoCcZ8vJoz3diCnjRk+g8VP4/fuQDJ2Lxr6WwG0gXs9aTpDzW0vgDBlVUpixR8gYk44AD8FrUKHr8JQJGgIDnoDqoALxmWPQSi9AVVzm8gKUuEPGr/QCvptwJkbSYT/TC4S8C96DGjTj86aHtAI0x2WaBIq0eSYYpRa4EsdWVVwWu9O0Aj6f6dyBMnwEraeOgSYu0wZlauzA47QCbT7DgAQSE+hZWoEBF/BBmWOewNMK3BsSqKUW4MGcWqCSVmDkbvkXGKQOwg6PAUO9oL3xXhA20yaiCjuwYygRVQlUOTWTCf2SuNJTxeFjgaHByGuAIvd8ItdPLTDhS7IuqEE1YSKVOgbayLhSFQhMzYh8hwfBs1r7c505YVIQYEdNoKwxK06MJiyrpUFHiF0NAfCQUVHoiRclIXJIR6C2fqG37pBHvcWpgwzvAtYwkR5UGV2e42UISdBJETl3mg8ouo54Rcnti1/vaT+iuUQBt500Cgo4U10BeHSkk57FB0JjWkKRMWgLUA0lLodtImAQdaMiiri3+gIAPZQoutHNsgKF1aaDMhMyIdBf8Th+Bh8MTjGWCpl5Wv43tDmnF+IUVMrcZgRoiAxhtrloYizNkZaAnF5leglbNhj0wYCAbCDvGb0mP4nib7O7ZlcYQ2m1gPtIZgVgGNNMeaVAaWR+57TrqgtUnm3sHQ+kYeE6fufUubG1ez50FXbPnWgBlgSABmN3TTcsRl2yWkHRrwbiunvk/W2+Mg1hPZplPDeXRbZzStFH15s1QIVd3UImP5z/bHpeeQLvRJ7XLFUffQIlCvqlXETQbgN9/rlYABGosv+Vi9m2Xs639YLGrZd0br+odetlvdsvbN56abfd4vbCzv9Q3v/ygoOV21A4OPpfXvH4Ai+5ZGRkZGRkbJA/t/I0QMzoMiEAAAAASUVORK5CYII=
";
#[cfg(windows)] // windows, 32x32, bigger very ugly after shrink
pub const ICON: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAolBMVEUAAAAAcf8Acf8Acf8AcP8Acf8Acf8Acf8AcP8Acf/////9/v/7/f+gyv9wr/8Ld/8GdP/A3P+nzv+Qwf9mqv8lhv8UfP+Kvv88k/8zjv/x+P/Y6f/U5//J4f/F3/+x1P+pz/+izP+Cuv9Zov9Qnv9FmP8gg//f7f+42P96tv9fpv8ui//1+f/q8//o8v/Q5f+ax/+Zxv+VxP+Fu/9rrf8rif+x1o3FAAAACXRSTlMAv/RPTPKHioRsIqhAAAABNklEQVQ4y4WT6XaCQAyFB7S2GUD2RaxSKGDV7sv7v1on4djEluL9Ncn5biaZk1FKzSwbRmVbM2V0DRNaGD9Maq6sacBS9jRwpUTw1Ww7SEOvkwQDaeVofQeJ1nrXjgD3pTaqCDCg/xs4OBrlwUGTHvxzoHAx69Y9+Hk5oGdAsCNXSoHfEN1JYEme4KcfvO9WAmtMBNx6jIaCgSPGLbAyvKRhAEcrAxAKTSZiACu+gNSbyWwZeKdQKj91yRXWIOWZTMgATun6EtjgczNQaKMEWD0+xJ6B4AnvECVqdKwYoJ50hIPyw25AANkzpsLB00cYOIUEoHU0uVaQVcMxBwlgVZJ3Orz+3ahH6gP2tBgfYzsZmdo1fGIzRx5Irn2WxKaHeJnKtb/4cS5/PbWACd0oo/n/39/4vwGFYSxtSYV4OAAAAABJRU5ErkJggg==
";
#[cfg(target_os = "linux")] // 128x128 no padding
pub const ICON: &str = "data:image/png;base64,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
";
#[cfg(target_os = "macos")]
pub const ORG: &str = "com.carriez";

type Size = (i32, i32, i32, i32);

lazy_static::lazy_static! {
    static ref CONFIG: Arc<RwLock<Config>> = Arc::new(RwLock::new(Config::load()));
    static ref CONFIG2: Arc<RwLock<Config2>> = Arc::new(RwLock::new(Config2::load()));
    pub static ref ONLINE: Arc<Mutex<HashMap<String, i64>>> = Default::default();
}
#[cfg(any(target_os = "android", target_os = "ios"))]
lazy_static::lazy_static! {
    pub static ref APP_DIR: Arc<RwLock<String>> = Default::default();
}
const CHARS: &'static [char] = &[
    '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k',
    'm', 'n', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

pub const RENDEZVOUS_SERVERS: &'static [&'static str] = &[
    "rs-sg.rustdesk.com",
    "rs-cn.rustdesk.com",
];
pub const RENDEZVOUS_PORT: i32 = 21116;
pub const RELAY_PORT: i32 = 21117;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    id: String,
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

// more variable configs
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Config2 {
    #[serde(default)]
    remote_id: String, // latest used one
    #[serde(default)]
    size: Size,
    #[serde(default)]
    rendezvous_server: String,
    #[serde(default)]
    nat_type: i32,
    #[serde(default)]
    serial: i32,

    // the other scalar value must before this
    #[serde(default)]
    pub options: HashMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct PeerConfig {
    #[serde(default)]
    pub password: Vec<u8>,
    #[serde(default)]
    pub size: Size,
    #[serde(default)]
    pub size_ft: Size,
    #[serde(default)]
    pub size_pf: Size,
    #[serde(default)]
    pub view_style: String, // original (default), scale
    #[serde(default)]
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

    // the other scalar value must before this
    #[serde(default)]
    pub options: HashMap<String, String>,
    #[serde(default)]
    pub info: PeerInfoSerde,
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
                    let user = String::from_utf8_lossy(&output.stdout).to_string().trim().to_owned();
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
        Config::load_::<Config2>("2")
    }

    fn store(&self) {
        Config::store_(self, "2");
    }
}

impl Config {
    fn load_<T: serde::Serialize + serde::de::DeserializeOwned + Default + std::fmt::Debug>(
        suffix: &str,
    ) -> T {
        let file = Self::file_(suffix);
        log::debug!("Configuration path: {}", file.display());
        let cfg = match confy::load_path(&file) {
            Ok(config) => config,
            Err(err) => {
                log::error!("Failed to load config: {}", err);
                T::default()
            }
        };
        if suffix.is_empty() {
            log::debug!("{:?}", cfg);
        }
        cfg
    }

    fn store_<T: serde::Serialize>(config: &T, suffix: &str) {
        let file = Self::file_(suffix);
        if let Err(err) = confy::store_path(file, config) {
            log::error!("Failed to store config: {}", err);
        }
    }

    fn load() -> Config {
        Config::load_::<Config>("")
    }

    fn store(&self) {
        Config::store_(self, "");
    }

    pub fn file() -> PathBuf {
        Self::file_("")
    }

    pub fn import(from: &str) {
        log::info!("import {}", from);
        // load first to create path
        Self::load();
        crate::allow_err!(std::fs::copy(from, Self::file()));
        crate::allow_err!(std::fs::copy(
            from.replace(".toml", "2.toml"),
            Self::file_("2")
        ));
    }

    pub fn save_tmp() -> String {
        let _lock = CONFIG.read().unwrap(); // do not use let _, which will be dropped immediately
        let path = Self::file_("2").to_str().unwrap_or("").to_owned();
        let path2 = format!("{}_tmp", path);
        crate::allow_err!(std::fs::copy(&path, &path2));
        let path = Self::file().to_str().unwrap_or("").to_owned();
        let path2 = format!("{}_tmp", path);
        crate::allow_err!(std::fs::copy(&path, &path2));
        path2
    }

    fn file_(suffix: &str) -> PathBuf {
        let name = format!("{}{}", APP_NAME, suffix);
        Self::path(name).with_extension("toml")
    }

    pub fn get_home() -> PathBuf {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        return Self::path("");
        if let Some(path) = dirs_next::home_dir() {
            patch(path)
        } else if let Ok(path) = std::env::current_dir() {
            path
        } else {
            std::env::temp_dir()
        }
    }

    fn path<P: AsRef<Path>>(p: P) -> PathBuf {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            let mut path: PathBuf = APP_DIR.read().unwrap().clone().into();
            path.push(p);
            return path;
        }
        #[cfg(not(target_os = "macos"))]
        let org = "";
        #[cfg(target_os = "macos")]
        let org = ORG;
        // /var/root for root
        if let Some(project) = ProjectDirs::from("", org, APP_NAME) {
            let mut path = patch(project.config_dir().to_path_buf());
            path.push(p);
            return path;
        }
        return "".into();
    }

    pub fn log_path() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            if let Some(path) = dirs_next::home_dir().as_mut() {
                path.push(format!("Library/Logs/{}", APP_NAME));
                return path.clone();
            }
        }
        #[cfg(target_os = "linux")]
        {
            let mut path = Self::get_home();
            path.push(format!(".local/share/logs/{}", APP_NAME));
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
            format!("\\\\.\\pipe\\{}\\query{}", APP_NAME, postfix)
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut path: PathBuf = format!("/tmp/{}", APP_NAME).into();
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
    pub fn get_any_listen_addr() -> SocketAddr {
        format!("{}:0", BIND_INTERFACE).parse().unwrap()
    }

    pub fn get_rendezvous_server() -> SocketAddr {
        let mut rendezvous_server = Self::get_option("custom-rendezvous-server");
        if rendezvous_server.is_empty() {
            rendezvous_server = CONFIG2.write().unwrap().rendezvous_server.clone();
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
        if let Ok(addr) = crate::to_socket_addr(&rendezvous_server) {
            addr
        } else {
            Self::get_any_listen_addr()
        }
    }

    pub fn get_rendezvous_servers() -> Vec<String> {
        let s = Self::get_option("custom-rendezvous-server");
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
        return None;
        let mut id = 0u32;
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
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

    pub fn get_auto_password() -> String {
        let mut rng = rand::thread_rng();
        (0..6)
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

    pub fn set_key_pair(pair: (Vec<u8>, Vec<u8>)) {
        let mut config = CONFIG.write().unwrap();
        config.key_pair = pair;
        config.store();
    }

    pub fn get_key_pair() -> (Vec<u8>, Vec<u8>) {
        // lock here to make sure no gen_keypair more than once
        let mut config = CONFIG.write().unwrap();
        if config.key_pair.0.is_empty() {
            let (pk, sk) = sign::gen_keypair();
            config.key_pair = (sk.0.to_vec(), pk.0.into());
            config.store();
        }
        config.key_pair.clone()
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

    pub fn get_options() -> HashMap<String, String> {
        CONFIG2.read().unwrap().options.clone()
    }

    pub fn set_options(v: HashMap<String, String>) {
        let mut config = CONFIG2.write().unwrap();
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
        if k == "custom-rendezvous-server" {
            config.rendezvous_server = "".to_owned();
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

    pub fn update_id() {
        // to-do: how about if one ip register a lot of ids?
        let id = Self::get_id();
        let mut rng = rand::thread_rng();
        let new_id = rng.gen_range(1_000_000_000, 2_000_000_000).to_string();
        Config::set_id(&new_id);
        log::info!("id updated from {} to {}", id, new_id);
    }

    pub fn set_password(password: &str) {
        let mut config = CONFIG.write().unwrap();
        if password == config.password {
            return;
        }
        config.password = password.into();
        config.store();
    }

    pub fn get_password() -> String {
        let mut password = CONFIG.read().unwrap().password.clone();
        if password.is_empty() {
            password = Config::get_auto_password();
            Config::set_password(&password);
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
            salt = Config::get_auto_password();
            Config::set_salt(&salt);
        }
        salt
    }

    pub fn get_size() -> Size {
        CONFIG2.read().unwrap().size
    }

    pub fn set_size(x: i32, y: i32, w: i32, h: i32) {
        let mut config = CONFIG2.write().unwrap();
        let size = (x, y, w, h);
        if size == config.size || size.2 < 300 || size.3 < 300 {
            return;
        }
        config.size = size;
        config.store();
    }

    pub fn set_remote_id(remote_id: &str) {
        let mut config = CONFIG2.write().unwrap();
        if remote_id == config.remote_id {
            return;
        }
        config.remote_id = remote_id.into();
        config.store();
    }

    pub fn get_remote_id() -> String {
        CONFIG2.read().unwrap().remote_id.clone()
    }
}

const PEERS: &str = "peers";

impl PeerConfig {
    pub fn load(id: &str) -> PeerConfig {
        let _ = CONFIG.read().unwrap(); // for lock
        match confy::load_path(&Self::path(id)) {
            Ok(config) => config,
            Err(err) => {
                log::error!("Failed to load config: {}", err);
                Default::default()
            }
        }
    }

    pub fn store(&self, id: &str) {
        let _ = CONFIG.read().unwrap(); // for lock
        if let Err(err) = confy::store_path(Self::path(id), self) {
            log::error!("Failed to store config: {}", err);
        }
    }

    pub fn remove(id: &str) {
        fs::remove_file(&Self::path(id)).ok();
    }

    fn path(id: &str) -> PathBuf {
        let path: PathBuf = [PEERS, id].iter().collect();
        Config::path(path).with_extension("toml")
    }

    pub fn peers() -> Vec<(String, SystemTime, PeerInfoSerde)> {
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
                        let t = fs::metadata(p)
                            .map(|m| m.modified().unwrap_or(SystemTime::UNIX_EPOCH))
                            .unwrap_or(SystemTime::UNIX_EPOCH);
                        let id = p
                            .file_stem()
                            .map(|p| p.to_str().unwrap_or(""))
                            .unwrap_or("")
                            .to_owned();
                        let info = PeerConfig::load(&id).info;
                        if info.platform.is_empty() {
                            fs::remove_file(&p).ok();
                        }
                        (id, t, info)
                    })
                    .filter(|p| !p.2.platform.is_empty())
                    .collect();
                peers.sort_unstable_by(|a, b| b.1.cmp(&a.1));
                return peers;
            }
        }
        Default::default()
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
