use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(not(any(target_os = "ios")))]
use crate::{ui_interface::get_builtin_option, Connection};
use hbb_common::{
    config::{self, keys, Config, LocalConfig},
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
pub struct StrategyOptions {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub config_options: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

#[cfg(not(any(target_os = "ios")))]
#[tokio::main(flavor = "current_thread")]
async fn start_hbbs_sync_async() {
    let mut interval = crate::rustdesk_interval(tokio::time::interval_at(
        Instant::now() + TIME_CONN,
        TIME_CONN,
    ));
    let mut last_sent: Option<Instant> = None;
    let mut info_uploaded: (bool, String, Option<Instant>, String) =
        (false, "".to_owned(), None, "".to_owned());
    let mut sysinfo_ver = "".to_owned();
    loop {
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
                let conns = Connection::alive_conns();
                if info_uploaded.0 && (url != info_uploaded.1 || id != info_uploaded.3) {
                    info_uploaded.0 = false;
                    *PRO.lock().unwrap() = false;
                }
                if !info_uploaded.0 && info_uploaded.2.map(|x| x.elapsed() >= UPLOAD_SYSINFO_TIMEOUT).unwrap_or(true) {
                    let mut v = crate::get_sysinfo();
                    // username is empty in login screen of windows, but here we only upload sysinfo once, causing
                    // real user name not uploaded after login screen. https://github.com/rustdesk/rustdesk/discussions/8031
                    if !cfg!(windows) || !v["username"].as_str().unwrap_or_default().is_empty() {
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
                        use sha2::{Digest, Sha256};
                        let mut hasher = Sha256::new();
                        hasher.update(url.as_bytes());
                        hasher.update(&v.as_bytes());
                        let res = hasher.finalize();
                        let hash = hbb_common::base64::encode(&res[..]);
                        let old_hash = config::Status::get("sysinfo_hash");
                        let ver = config::Status::get("sysinfo_ver"); // sysinfo_ver is the version of sysinfo on server's side
                        if hash == old_hash {
                            let samever = match crate::post_request(url.replace("heartbeat", "sysinfo_ver"), "".to_owned(), "").await {
                                Ok(x)  => {
                                    sysinfo_ver = x.clone();
                                    x == ver
                                }
                                _ => {
                                    true // if failed to get sysinfo_ver, we assume it's the same version
                                }
                            };
                            if samever {
                                info_uploaded = (true, url.clone(), None, id.clone());
                                log::info!("sysinfo not changed, skip upload");
                                continue;
                            }
                        }
                        match crate::post_request(url.replace("heartbeat", "sysinfo"), v, "").await {
                            Ok(x)  => {
                                if x == "SYSINFO_UPDATED" {
                                    info_uploaded = (true, url.clone(), None, id.clone());
                                    log::info!("sysinfo updated");
                                    config::Status::set("sysinfo_hash", hash);
                                    config::Status::set("sysinfo_ver", sysinfo_ver.clone());
                                    *PRO.lock().unwrap() = true;
                                } else if x == "ID_NOT_FOUND" {
                                    info_uploaded.2 = None; // next heartbeat will upload sysinfo again
                                } else {
                                    info_uploaded.2 = Some(Instant::now());
                                }
                            }
                            _ => {
                                info_uploaded.2 = Some(Instant::now());
                            }
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
                let modified_at = LocalConfig::get_option("strategy_timestamp").parse::<i64>().unwrap_or(0);
                v["modified_at"] = json!(modified_at);
                if let Ok(s) = crate::post_request(url.clone(), v.to_string(), "").await {
                    if let Ok(mut rsp) = serde_json::from_str::<HashMap::<&str, Value>>(&s) {
                        if rsp.remove("sysinfo").is_some() {
                            info_uploaded.0 = false;
                            config::Status::set("sysinfo_hash", "".to_owned());
                            log::info!("sysinfo required to forcely update");
                        }
                        if let Some(conns)  = rsp.remove("disconnect") {
                                if let Ok(conns) = serde_json::from_value::<Vec<i32>>(conns) {
                                    SENDER.lock().unwrap().send(conns).ok();
                                }
                        }
                        if let Some(rsp_modified_at) = rsp.remove("modified_at") {
                            if let Ok(rsp_modified_at) = serde_json::from_value::<i64>(rsp_modified_at) {
                                if rsp_modified_at != modified_at {
                                    LocalConfig::set_option("strategy_timestamp".to_string(), rsp_modified_at.to_string());
                                }
                            }
                        }
                        if let Some(strategy) = rsp.remove("strategy") {
                            if let Ok(strategy) = serde_json::from_value::<StrategyOptions>(strategy) {
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
