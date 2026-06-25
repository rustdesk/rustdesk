use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Mutex, Once, RwLock},
    time::Duration,
};

use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use hbb_common::{
    anyhow::{anyhow, Result},
    config::{self, keys::*, Config},
    log,
    tokio::{net::TcpListener, select, task::JoinHandle, time},
    tokio_util::sync::CancellationToken,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

const LISTEN_PORT: u16 = 3000;
const ABILITY_ACK_INTERVAL_SECS: u64 = 35;
const PASSWORD_ROTATE_SECS: u64 = 10 * 60;
const PASSWORD_LENGTH: usize = 10;
const BUSINESS: &str = "rustdesk";
const ID_SERVER: &str = env!("RUSTDESK_ID_SERVER");
const RELAY_SERVER: &str = env!("RUSTDESK_RELAY_SERVER");
const SERVER_KEY: &str = env!("RUSTDESK_SERVER_KEY");

static START: Once = Once::new();

lazy_static::lazy_static! {
    static ref CURRENT_PASSWORD: RwLock<String> = RwLock::new(String::new());
    static ref ABILITY_ACK_TASK: Mutex<Option<AbilityAckTask>> = Mutex::new(None);
}

struct AbilityAckTask {
    cancel: CancellationToken,
    handle: JoinHandle<()>,
}

#[derive(Debug, Serialize)]
struct AccountData {
    #[serde(rename = "rdID")]
    rd_id: String,
    #[serde(rename = "rdPwd")]
    rd_pwd: String,
    #[serde(rename = "snMac")]
    sn_mac: String,
    #[serde(rename = "rdStatus")]
    rd_status: String,
    #[serde(rename = "userNum")]
    user_num: i64,
    ts: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Root<T> {
    bid: String,
    tid: String,
    ts: i64,
    data: T,
}

#[derive(Debug, Deserialize)]
struct AbilityData {
    r#type: String,
    action: Option<String>,
}

#[derive(Debug, Serialize)]
struct AbilityAckData {
    r#type: String,
    action: Option<String>,
    result: Option<serde_json::Value>,
}

struct HandlerError(hbb_common::anyhow::Error);

impl<E> From<E> for HandlerError
where
    E: Into<hbb_common::anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "ok": false,
                "error": self.0.to_string(),
            })),
        )
            .into_response()
    }
}

pub fn start() {
    START.call_once(|| {
        apply_zj_default_settings();
        apply_fixed_server_config();
        set_hostname_id();
        rotate_password();
        hbb_common::tokio::spawn(async {
            run().await;
        });
    });
}

fn apply_zj_default_settings() {
    {
        let mut defaults = config::DEFAULT_SETTINGS.write().unwrap();
        defaults
            .entry(OPTION_DIRECT_SERVER.to_owned())
            .or_insert_with(|| "Y".to_owned());
        defaults
            .entry(OPTION_ALLOW_REMOTE_CONFIG_MODIFICATION.to_owned())
            .or_insert_with(|| "Y".to_owned());
        defaults
            .entry(OPTION_ALLOW_LINUX_HEADLESS.to_owned())
            .or_insert_with(|| "Y".to_owned());
    }
    config::BUILTIN_SETTINGS
        .write()
        .unwrap()
        .insert(OPTION_REGISTER_DEVICE.to_owned(), "N".to_owned());
}

async fn run() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), LISTEN_PORT);
    let app = Router::new().route("/ability", post(ability));

    match TcpListener::bind(addr).await {
        Ok(listener) => {
            log::info!("Internal ability API listening on http://{addr}");
            hbb_common::tokio::spawn(password_rotation_loop());
            if let Err(err) = axum::serve(listener, app).await {
                log::error!("Internal ability API stopped: {err}");
            }
        }
        Err(err) => {
            log::error!("Failed to bind internal ability API on {addr}: {err}");
        }
    }
}

async fn password_rotation_loop() {
    loop {
        time::sleep(Duration::from_secs(PASSWORD_ROTATE_SECS)).await;
        rotate_password();
    }
}

async fn ability(Json(root): Json<Root<AbilityData>>) -> Result<StatusCode, HandlerError> {
    if root.data.r#type != BUSINESS {
        return Err(anyhow!("未知的 type: {}", root.data.r#type).into());
    }
    let action = root
        .data
        .action
        .ok_or_else(|| anyhow!("缺少 action 参数"))?;
    match action.as_str() {
        "start" => {
            apply_fixed_server_config();
            set_hostname_id();
            rotate_password();
            start_ability_ack_loop();
        }
        "stop" => {
            stop_ability_ack_loop();
            let data = account_data("stopped");
            let result = serde_json::to_value(&data)?;
            send_ability_ack(&action, Some(result.clone())).await?;
            return Ok(StatusCode::NO_CONTENT);
        }
        _ => return Err(anyhow!("未知的 action: {}", action).into()),
    }

    Ok(StatusCode::OK)
}

fn start_ability_ack_loop() {
    let mut task = ABILITY_ACK_TASK.lock().unwrap();
    if task
        .as_ref()
        .map_or(false, |task| !task.handle.is_finished())
    {
        return;
    }
    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();
    let handle = hbb_common::tokio::spawn(async move {
        loop {
            let result = match serde_json::to_value(account_data("running")) {
                Ok(result) => Some(result),
                Err(err) => {
                    log::warn!("Failed to serialize ability ack payload: {err}");
                    None
                }
            };
            if let Err(err) = send_ability_ack("start", result).await {
                log::warn!("Failed to send periodic ability ack: {err}");
            }
            select! {
                _ = cancel_for_task.cancelled() => break,
                _ = time::sleep(Duration::from_secs(ABILITY_ACK_INTERVAL_SECS)) => {}
            }
        }
    });
    *task = Some(AbilityAckTask { cancel, handle });
}

fn stop_ability_ack_loop() {
    if let Some(task) = ABILITY_ACK_TASK.lock().unwrap().take() {
        task.cancel.cancel();
    }
}

async fn send_ability_ack(action: &str, result: Option<serde_json::Value>) -> Result<()> {
    let Some(endpoint) = iothub_endpoint("/ability_ack") else {
        log::warn!("IOTHUB_CLIENT is not configured; skip ability ack");
        return Ok(());
    };
    let root = Root {
        bid: uuid::Uuid::new_v4().to_string(),
        tid: uuid::Uuid::new_v4().to_string(),
        ts: now_millis(),
        data: AbilityAckData {
            r#type: BUSINESS.to_owned(),
            action: Some(action.to_owned()),
            result,
        },
    };
    reqwest::Client::new()
        .post(&endpoint)
        .json(&root)
        .send()
        .await?;
    Ok(())
}

fn apply_fixed_server_config() {
    let before = (
        Config::get_option(OPTION_CUSTOM_RENDEZVOUS_SERVER),
        Config::get_option(OPTION_API_SERVER),
        Config::get_option(OPTION_RELAY_SERVER),
        Config::get_option(OPTION_KEY),
        Config::get_option(OPTION_DIRECT_SERVER),
        Config::get_option(OPTION_ALLOW_REMOTE_CONFIG_MODIFICATION),
        Config::get_option(OPTION_ALLOW_LINUX_HEADLESS),
    );

    Config::set_option(
        OPTION_CUSTOM_RENDEZVOUS_SERVER.to_owned(),
        ID_SERVER.to_owned(),
    );
    Config::set_option(OPTION_API_SERVER.to_owned(), String::new());
    Config::set_option(OPTION_RELAY_SERVER.to_owned(), RELAY_SERVER.to_owned());
    Config::set_option(OPTION_KEY.to_owned(), SERVER_KEY.to_owned());
    Config::set_option(OPTION_DIRECT_SERVER.to_owned(), "Y".to_owned());
    Config::set_option(
        OPTION_ALLOW_REMOTE_CONFIG_MODIFICATION.to_owned(),
        "Y".to_owned(),
    );
    Config::set_option(OPTION_ALLOW_LINUX_HEADLESS.to_owned(), "Y".to_owned());

    let after = (
        Config::get_option(OPTION_CUSTOM_RENDEZVOUS_SERVER),
        Config::get_option(OPTION_API_SERVER),
        Config::get_option(OPTION_RELAY_SERVER),
        Config::get_option(OPTION_KEY),
        Config::get_option(OPTION_DIRECT_SERVER),
        Config::get_option(OPTION_ALLOW_REMOTE_CONFIG_MODIFICATION),
        Config::get_option(OPTION_ALLOW_LINUX_HEADLESS),
    );
    if before != after {
        crate::RendezvousMediator::restart();
    }
}

fn set_hostname_id() {
    Config::set_key_confirmed(false);
    if let Some(id) = sanitized_hostname() {
        Config::set_id(&id);
        log::info!("RustDesk ID set to hostname: {id}");
    } else {
        Config::update_id();
        log::info!("Hostname is empty; RustDesk ID set to a random ID");
    }
}

fn sanitized_hostname() -> Option<String> {
    let id = crate::common::hostname()
        .trim()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect::<String>();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

fn rotate_password() {
    let password = Config::get_auto_password(PASSWORD_LENGTH);
    if Config::set_permanent_password(&password) {
        Config::set_option(
            OPTION_VERIFICATION_METHOD.to_owned(),
            "use-permanent-password".to_owned(),
        );
        match CURRENT_PASSWORD.write() {
            Ok(mut current) => *current = password.clone(),
            Err(err) => log::error!("Failed to cache rotated password: {err}"),
        }
        log::info!("Permanent password rotated by internal ability API: {password}");
    } else {
        log::warn!("Permanent password rotation was rejected by configuration");
    }
}

fn account_data(status: &str) -> AccountData {
    AccountData {
        rd_id: Config::get_id(),
        rd_pwd: CURRENT_PASSWORD
            .read()
            .map(|v| v.clone())
            .unwrap_or_default(),
        sn_mac: "".to_owned(),
        rd_status: status.to_owned(),
        ts: now_millis(),
        user_num: crate::server::alive_connection_count() as i64,
    }
}

fn iothub_endpoint(path: &str) -> Option<String> {
    let base = std::env::var("IOTHUB_CLIENT")
        .or_else(|_| std::env::var("RUSTDESK_IOTHUB_CLIENT"))
        .ok()?;
    if base.trim().is_empty() {
        return None;
    }
    Some(format!("{}{}", base.trim_end_matches('/'), path))
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
