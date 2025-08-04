use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(not(any(target_os = "ios")))]
use crate::ipc::CheckIfRestart;
#[cfg(not(any(target_os = "ios")))]
use crate::{ui_interface::get_builtin_option, Connection};
use hbb_common::{
    config::{
        self,
        keys::{self, KEYS_DISPLAY_SETTINGS},
        Config,
    },
    log,
    tokio::{self, sync::broadcast, time::Instant},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const TIME_HEARTBEAT: Duration = Duration::from_secs(15);
const UPLOAD_SYSINFO_TIMEOUT: Duration = Duration::from_secs(120);
const TIME_CONN: Duration = Duration::from_secs(3);

#[cfg(not(any(target_os = "ios")))]
lazy_static::lazy_static! {
    static ref SENDER : Mutex<broadcast::Sender<Vec<i32>>> = Mutex::new(start_hbbs_sync());
    static ref PRO: Arc<Mutex<bool>> = Default::default();
}

#[cfg(not(any(target_os = "ios")))]
pub fn start() {
    let _sender = SENDER.lock().unwrap();
}

#[cfg(not(target_os = "ios"))]
pub fn signal_receiver() -> broadcast::Receiver<Vec<i32>> {
    SENDER.lock().unwrap().subscribe()
}

#[cfg(not(any(target_os = "ios")))]
fn start_hbbs_sync() -> broadcast::Sender<Vec<i32>> {
    let (tx, _rx) = broadcast::channel::<Vec<i32>>(16);
    std::thread::spawn(move || start_hbbs_sync_async());
    return tx;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SelfHostStrategyOptions {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub config_options: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct PublicStrategy {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub modifiable_options: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub override_options: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub hard_options: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct PublicStrategyPayload {
    #[serde(default)]
    pub modified_at: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub options: String,
}

struct InfoUploaded {
    uploaded: bool,
    url: String,
    last_uploaded: Option<Instant>,
    id: String,
    username: Option<String>,
}

impl Default for InfoUploaded {
    fn default() -> Self {
        Self {
            uploaded: false,
            url: "".to_owned(),
            last_uploaded: None,
            id: "".to_owned(),
            username: None,
        }
    }
}

impl InfoUploaded {
    fn uploaded(url: String, id: String, username: String) -> Self {
        Self {
            uploaded: true,
            url,
            last_uploaded: None,
            id,
            username: Some(username),
        }
    }
}

#[cfg(not(any(target_os = "ios")))]
#[tokio::main(flavor = "current_thread")]
async fn start_hbbs_sync_async() {
    let mut interval = crate::rustdesk_interval(tokio::time::interval_at(
        Instant::now() + TIME_CONN,
        TIME_CONN,
    ));
    let mut last_sent: Option<Instant> = None;
    let mut info_uploaded = InfoUploaded::default();
    let mut sysinfo_ver = "".to_owned();
    let mut modified_at_public = get_public_strategy_modified_at();
    loop {
        use hbb_common::config::LocalConfig;

        tokio::select! {
            _ = interval.tick() => {
                let url = heartbeat_url();
                let id = Config::get_id();
                if url.is_empty() {
                    *PRO.lock().unwrap() = false;
                    continue;
                }
                if config::option2bool("stop-service", &Config::get_option("stop-service")) {
                    continue;
                }
                let is_public = crate::is_public(&url);
                let conns = Connection::alive_conns();
                if info_uploaded.uploaded && (url != info_uploaded.url || id != info_uploaded.id) {
                    info_uploaded.uploaded = false;
                    *PRO.lock().unwrap() = false;
                }
                // For Windows:
                // We can't skip uploading sysinfo when the username is empty, because the username may
                // always be empty before login. We also need to upload the other sysinfo info.
                //
                // https://github.com/rustdesk/rustdesk/discussions/8031
                // We still need to check the username after uploading sysinfo, because
                // 1. The username may be empty when logining in, and it can be fetched after a while.
                //    In this case, we need to upload sysinfo again.
                // 2. The username may be changed after uploading sysinfo, and we need to upload sysinfo again.
                //
                // The Windows session will switch to the last user session before the restart,
                // so it may be able to get the username before login.
                // But strangely, sometimes we can get the username before login,
                // we may not be able to get the username before login after the next restart.
                let mut v = crate::get_sysinfo();
                let sys_username = v["username"].as_str().unwrap_or_default().to_string();
                // Though the username comparison is only necessary on Windows,
                // we still keep the comparison on other platforms for consistency.
                let need_upload = (!info_uploaded.uploaded || info_uploaded.username.as_ref() != Some(&sys_username)) &&
                    info_uploaded.last_uploaded.map(|x| x.elapsed() >= UPLOAD_SYSINFO_TIMEOUT).unwrap_or(true);
                if need_upload {
                    v["version"] = json!(crate::VERSION);
                    v["id"] = json!(id);
                    v["uuid"] = json!(crate::encode64(hbb_common::get_uuid()));
                    let ab_name = Config::get_option(keys::OPTION_PRESET_ADDRESS_BOOK_NAME);
                    if !ab_name.is_empty() {
                        v[keys::OPTION_PRESET_ADDRESS_BOOK_NAME] = json!(ab_name);
                    }
                    let ab_tag = Config::get_option(keys::OPTION_PRESET_ADDRESS_BOOK_TAG);
                    if !ab_tag.is_empty() {
                        v[keys::OPTION_PRESET_ADDRESS_BOOK_TAG] = json!(ab_tag);
                    }
                    let username = get_builtin_option(keys::OPTION_PRESET_USERNAME);
                    if !username.is_empty() {
                        v[keys::OPTION_PRESET_USERNAME] = json!(username);
                    }
                    let strategy_name = get_builtin_option(keys::OPTION_PRESET_STRATEGY_NAME);
                    if !strategy_name.is_empty() {
                        v[keys::OPTION_PRESET_STRATEGY_NAME] = json!(strategy_name);
                    }
                    let device_group_name = get_builtin_option(keys::OPTION_PRESET_DEVICE_GROUP_NAME);
                    if !device_group_name.is_empty() {
                        v[keys::OPTION_PRESET_DEVICE_GROUP_NAME] = json!(device_group_name);
                    }
                    let v = v.to_string();
                    let mut hash = "".to_owned();
                    if is_public {
                        use sha2::{Digest, Sha256};
                        let mut hasher = Sha256::new();
                        hasher.update(url.as_bytes());
                        hasher.update(&v.as_bytes());
                        let res = hasher.finalize();
                        hash = hbb_common::base64::encode(&res[..]);
                        let old_hash = config::Status::get("sysinfo_hash");
                        let ver = config::Status::get("sysinfo_ver"); // sysinfo_ver is the version of sysinfo on server's side
                        if hash == old_hash {
                            // When the api doesn't exist, Ok("") will be returned in test.
                            let samever = match crate::post_request(url.replace("heartbeat", "sysinfo_ver"), "".to_owned(), "").await {
                                Ok(x)  => {
                                    sysinfo_ver = x.clone();
                                    *PRO.lock().unwrap() = true;
                                    x == ver
                                }
                                _ => {
                                    false // to make sure Pro can be assigned in below post for old
                                            // hbbs pro not supporting sysinfo_ver, use false for ensuring
                                }
                            };
                            if samever {
                                info_uploaded = InfoUploaded::uploaded(url.clone(), id.clone(), sys_username);
                                log::info!("sysinfo not changed, skip upload");
                                continue;
                            }
                        }
                    }
                    match crate::post_request(url.replace("heartbeat", "sysinfo"), v, "").await {
                        Ok(x)  => {
                            if x == "SYSINFO_UPDATED" {
                                info_uploaded = InfoUploaded::uploaded(url.clone(), id.clone(), sys_username);
                                log::info!("sysinfo updated");
                                if !hash.is_empty() {
                                    config::Status::set("sysinfo_hash", hash);
                                    config::Status::set("sysinfo_ver", sysinfo_ver.clone());
                                }
                                *PRO.lock().unwrap() = true;
                            } else if x == "ID_NOT_FOUND" {
                                info_uploaded.last_uploaded = None; // next heartbeat will upload sysinfo again
                            } else {
                                info_uploaded.last_uploaded = Some(Instant::now());
                            }
                        }
                        _ => {
                            info_uploaded.last_uploaded = Some(Instant::now());
                        }
                    }
                }
                if conns.is_empty() && last_sent.map(|x| x.elapsed() < TIME_HEARTBEAT).unwrap_or(false) {
                    continue;
                }
                last_sent = Some(Instant::now());
                let mut v = Value::default();
                v["id"] = json!(id);
                v["uuid"] = json!(crate::encode64(hbb_common::get_uuid()));
                v["ver"] = json!(hbb_common::get_version_number(crate::VERSION));
                if !conns.is_empty() {
                    v["conns"] = json!(conns);
                }
                let modified_at = if is_public {
                    modified_at_public
                } else {
                    LocalConfig::get_option("strategy_timestamp").parse::<i64>().unwrap_or(0)
                };
                if is_public {
                    v["stime"] = json!(modified_at);
                } else {
                    v["modified_at"] = json!(modified_at);

                }
                if let Ok(s) = crate::post_request(url.clone(), v.to_string(), "").await {
                    if let Ok(mut rsp) = serde_json::from_str::<HashMap::<&str, Value>>(&s) {
                        if rsp.remove("sysinfo").is_some() {
                            info_uploaded.uploaded = false;
                            config::Status::set("sysinfo_hash", "".to_owned());
                            log::info!("sysinfo required to forcely update");
                        }
                        if let Some(conns)  = rsp.remove("disconnect") {
                                if let Ok(conns) = serde_json::from_value::<Vec<i32>>(conns) {
                                    SENDER.lock().unwrap().send(conns).ok();
                                }
                        }
                        if is_public {
                            if let Some(strategy_payload) = rsp.remove("strategy") {
                                if let Ok(payload) = serde_json::from_value::<PublicStrategyPayload>(strategy_payload) {
                                    if let Ok(strategy) = serde_json::from_str::<PublicStrategy>(&payload.options) {
                                        crate::hbbs_http::sync::load_strategy(Some(strategy.clone()));
                                        save_strategy_payload(payload.clone());
                                        modified_at_public = payload.modified_at;
                                    }
                                }
                            }
                        } else {
                            if let Some(rsp_modified_at) = rsp.remove("modified_at") {
                                if let Ok(rsp_modified_at) = serde_json::from_value::<i64>(rsp_modified_at) {
                                    if rsp_modified_at != modified_at {
                                        LocalConfig::set_option("strategy_timestamp".to_string(), rsp_modified_at.to_string());
                                    }
                                }
                            }
                            if let Some(strategy) = rsp.remove("strategy") {
                                if let Ok(strategy) = serde_json::from_value::<SelfHostStrategyOptions>(strategy) {
                                    log::info!("strategy updated");
                                    handle_config_options(strategy.config_options);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn heartbeat_url() -> String {
    let url = crate::common::get_api_server(
        Config::get_option("api-server"),
        Config::get_option("custom-rendezvous-server"),
    );
    if url.is_empty() || url.contains("rustdesk.com") {
        return "".to_owned();
    }
    format!("{}/api/heartbeat", url)
}

fn handle_config_options(config_options: HashMap<String, String>) {
    let mut options = Config::get_options();
    config_options
        .iter()
        .map(|(k, v)| {
            if v.is_empty() {
                options.remove(k);
            } else {
                options.insert(k.to_string(), v.to_string());
            }
        })
        .count();
    Config::set_options(options);
}

#[allow(unused)]
#[cfg(not(any(target_os = "ios")))]
pub fn is_pro() -> bool {
    PRO.lock().unwrap().clone()
}

pub struct CheckPublicServer {
    is_public: bool,
}

impl CheckPublicServer {
    pub fn new() -> Self {
        Self {
            is_public: crate::with_public(),
        }
    }
}

impl Drop for CheckPublicServer {
    fn drop(&mut self) {
        let new_is_public = crate::with_public();
        if new_is_public != self.is_public {
            if new_is_public {
                #[cfg(not(any(target_os = "ios")))]
                load_strategy(None);
            } else {
                #[cfg(not(any(target_os = "ios")))]
                let _restart = CheckIfRestart::new();
                *config::STRATEGY_OVERRIDE_SETTINGS.write().unwrap() = HashMap::new();
                *config::STRATEGY_HARD_SETTINGS.write().unwrap() = HashMap::new();
            }
        }
    }
}

fn save_strategy_payload(payload: PublicStrategyPayload) {
    // Parse the strategy from options, clear modifiable_options, and re-serialize
    let mut processed_payload = payload.clone();
    if let Ok(mut strategy) = serde_json::from_str::<PublicStrategy>(&payload.options) {
        strategy.modifiable_options = Default::default();
        processed_payload.options = serde_json::to_string(&strategy).unwrap_or_default();
    }

    // Only encrypt the options field, keep signature in plain text
    let encrypted_options =
        match hbb_common::password_security::encrypt(processed_payload.options.as_bytes()) {
            Ok(encrypted) => encrypted,
            Err(_) => {
                log::error!("Failed to encrypt strategy options");
                return;
            }
        };

    // Create StrategyPayload with encrypted options and plain modified_at
    let storage_payload = PublicStrategyPayload {
        modified_at: processed_payload.modified_at,
        options: encrypted_options,
    };

    // Serialize the entire StrategyPayload
    let storage_data = serde_json::to_string(&storage_payload).unwrap_or_default();

    // Save to file using unified path
    if let Ok(strategy_path) = get_strategy_path(true) {
        match std::fs::write(&strategy_path, storage_data) {
            Ok(_) => {
                log::debug!("Strategy config saved to: {:?}", strategy_path);
            }
            Err(e) => {
                log::warn!("Failed to save strategy config: {}", e);
            }
        }
    } else {
        log::warn!("Failed to get strategy path");
    }
}

/// Get strategy config file path for all platforms
fn get_strategy_path(create: bool) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dir = {
        #[cfg(target_os = "windows")]
        {
            let app_name = crate::get_app_name();
            let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
            let program_data =
                std::env::var("ProgramData").unwrap_or_else(|_| format!("{}\\ProgramData", drive));
            std::path::PathBuf::from(program_data).join(&app_name)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Config::path("")
        }
    };
    if create {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir.join("strategy"))
}

/// Load strategy config from file
fn load_strategy_config_from_file() -> Result<String, Box<dyn std::error::Error>> {
    // Load from file using unified path
    let strategy_path = get_strategy_path(false)?;
    if strategy_path.exists() {
        let storage_data = std::fs::read_to_string(&strategy_path)?;
        log::debug!("Loaded strategy config from: {:?}", strategy_path);
        Ok(storage_data)
    } else {
        Err("Strategy config file not found".into())
    }
}

#[cfg(not(any(target_os = "ios")))]
pub fn load_strategy(strategy: Option<PublicStrategy>) {
    if !crate::with_public() {
        return;
    }
    let _restart = CheckIfRestart::new();
    let from_file = strategy.is_none();
    let strategy = match strategy {
        Some(strategy) => strategy,
        None => {
            // Load from strategy config file
            match crate::hbbs_http::sync::load_strategy_config_from_file() {
                Ok(strategy_store) => {
                    // Deserialize StrategyPayload with encrypted options
                    if let Ok(storage_payload) =
                        serde_json::from_str::<PublicStrategyPayload>(&strategy_store)
                    {
                        // Decrypt options
                        if let Ok(decrypted_options) = hbb_common::password_security::decrypt(
                            storage_payload.options.as_bytes(),
                        ) {
                            if let Ok(options) = String::from_utf8(decrypted_options) {
                                // Reconstruct StrategyPayload with decrypted options
                                let payload = PublicStrategyPayload {
                                    modified_at: storage_payload.modified_at,
                                    options,
                                };
                                if let Ok(strategy) =
                                    serde_json::from_str::<PublicStrategy>(&payload.options)
                                {
                                    strategy
                                } else {
                                    PublicStrategy::default()
                                }
                            } else {
                                log::warn!("Failed to decode decrypted options");
                                PublicStrategy::default()
                            }
                        } else {
                            log::warn!("Failed to decrypt strategy options");
                            PublicStrategy::default()
                        }
                    } else {
                        log::warn!("Failed to deserialize strategy payload");
                        PublicStrategy::default()
                    }
                }
                Err(e) => {
                    log::warn!("Failed to load strategy config from strategy file: {}", e);
                    PublicStrategy::default()
                }
            }
        }
    };
    if !from_file {
        let mut map_settings = HashMap::new();
        for s in keys::KEYS_SETTINGS {
            map_settings.insert(s.replace("_", "-"), s);
        }
        for (k, v) in strategy.modifiable_options {
            if let Some(k2) = map_settings.get(&k) {
                let old = config::Config::get_option(k2);
                if old != *v {
                    config::Config::set_option(k2.to_string(), v.to_owned());
                }
            }
        }
    }
    // override
    let mut settings = HashMap::new();
    for (k, v) in &strategy.override_options {
        let k2 = k.replace("-", "_");
        if KEYS_DISPLAY_SETTINGS.contains(&k2.as_str()) {
            settings.insert(k2, v.to_owned());
        } else {
            settings.insert(k.to_owned(), v.to_owned());
        }
    }
    *config::STRATEGY_OVERRIDE_SETTINGS.write().unwrap() = settings;

    // hard settings
    settings = HashMap::new();
    for (k, v) in &strategy.hard_options {
        settings.insert(k.to_owned(), v.to_owned());
    }
    *config::STRATEGY_HARD_SETTINGS.write().unwrap() = settings;
}

#[inline]
#[cfg(not(any(target_os = "ios")))]
fn get_public_strategy_modified_at() -> i64 {
    match load_strategy_config_from_file() {
        Ok(strategy_store) => {
            if let Ok(payload) = serde_json::from_str::<PublicStrategyPayload>(&strategy_store) {
                payload.modified_at
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}
