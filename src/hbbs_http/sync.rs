use std::{collections::HashMap, sync::Mutex, time::Duration};

use hbb_common::{
    config::{Config, LocalConfig},
    tokio::{self, sync::broadcast, time::Instant},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::Connection;

const TIME_HEARTBEAT: Duration = Duration::from_secs(30);
const TIME_CONN: Duration = Duration::from_secs(3);

lazy_static::lazy_static! {
    static ref SENDER : Mutex<broadcast::Sender<Vec<i32>>> = Mutex::new(start_hbbs_sync());
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn start() {
    let _sender = SENDER.lock().unwrap();
}

pub fn signal_receiver() -> broadcast::Receiver<Vec<i32>> {
    SENDER.lock().unwrap().subscribe()
}

fn start_hbbs_sync() -> broadcast::Sender<Vec<i32>> {
    let (tx, _rx) = broadcast::channel::<Vec<i32>>(16);
    std::thread::spawn(move || start_hbbs_sync_async());
    return tx;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StrategyOptions {
    pub config_options: HashMap<String, String>,
    pub extra: HashMap<String, String>,
}

#[tokio::main(flavor = "current_thread")]
async fn start_hbbs_sync_async() {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval_at(Instant::now() + TIME_CONN, TIME_CONN);
        let mut last_send = Instant::now();
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let url = heartbeat_url();
                    let modified_at = LocalConfig::get_option("strategy_timestamp").parse::<i64>().unwrap_or(0);
                    if !url.is_empty() {
                        let conns = Connection::alive_conns();
                        if conns.is_empty() && last_send.elapsed() < TIME_HEARTBEAT {
                            continue;
                        }
                        last_send = Instant::now();
                        let mut v = Value::default();
                        v["id"] = json!(Config::get_id());
                        v["ver"] = json!(hbb_common::get_version_number(crate::VERSION));
                        if !conns.is_empty() {
                            v["conns"] = json!(conns);
                        }
                        v["modified_at"] = json!(modified_at);
                        if let Ok(s) = crate::post_request(url.clone(), v.to_string(), "").await {
                            if let Ok(mut rsp) = serde_json::from_str::<HashMap::<&str, Value>>(&s) {
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
                                        handle_config_options(strategy.config_options);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
    .await
    .ok();
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
