use super::HbbHttpResponse;
use crate::hbbs_http::create_http_client_with_url;
use hbb_common::{config::LocalConfig, log, ResultType};
use reqwest::blocking::Client;
use serde_derive::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use url::Url;

lazy_static::lazy_static! {
    static ref OIDC_SESSION: Arc<RwLock<OidcSession>> = Arc::new(RwLock::new(OidcSession::new()));
}

const QUERY_INTERVAL_SECS: f32 = 1.0;
const QUERY_TIMEOUT_SECS: u64 = 60 * 3;
const REQUESTING_ACCOUNT_AUTH: &str = "Requesting account auth";
const WAITING_ACCOUNT_AUTH: &str = "Waiting account auth";
const LOGIN_ACCOUNT_AUTH: &str = "Login account auth";

#[derive(Deserialize, Clone, Debug)]
pub struct OidcAuthUrl {
    code: String,
    url: Url,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct DeviceInfo {
    /// Linux , Windows , Android ...
    #[serde(default)]
    pub os: String,

    /// `browser` or `client`
    #[serde(default)]
    pub r#type: String,

    /// device name from rustdesk client,
    /// browser info(name + version) from browser
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WhitelistItem {
    data: String, // ip / device uuid
    info: DeviceInfo,
    exp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserInfo {
    #[serde(default, flatten)]
    pub settings: UserSettings,
    #[serde(default)]
    pub login_device_whitelist: Vec<WhitelistItem>,
    #[serde(default)]
    pub other: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserSettings {
    #[serde(default)]
    pub email_verification: bool,
    #[serde(default)]
    pub email_alarm_notification: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(i64)]
pub enum UserStatus {
    Disabled = 0,
    Normal = 1,
    Unverified = -1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPayload {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub status: UserStatus,
    pub info: UserInfo,
    #[serde(default)]
    pub is_admin: bool,
    #[serde(default)]
    pub third_auth_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthBody {
    pub access_token: String,
    pub r#type: String,
    #[serde(default)]
    pub tfa_type: String,
    #[serde(default)]
    pub secret: String,
    pub user: UserPayload,
}

pub struct OidcSession {
    client: Option<Client>,
    state_msg: &'static str,
    failed_msg: String,
    code_url: Option<OidcAuthUrl>,
    auth_body: Option<AuthBody>,
    keep_querying: bool,
    running: bool,
    query_timeout: Duration,
}

#[derive(Serialize)]
pub struct AuthResult {
    pub state_msg: String,
    pub failed_msg: String,
    pub url: Option<String>,
    pub auth_body: Option<AuthBody>,
}

impl Default for UserStatus {
    fn default() -> Self {
        UserStatus::Normal
    }
}

impl OidcSession {
    fn new() -> Self {
        Self {
            client: None,
            state_msg: REQUESTING_ACCOUNT_AUTH,
            failed_msg: "".to_owned(),
            code_url: None,
            auth_body: None,
            keep_querying: false,
            running: false,
            query_timeout: Duration::from_secs(QUERY_TIMEOUT_SECS),
        }
    }

    fn ensure_client(api_server: &str) {
        let mut write_guard = OIDC_SESSION.write().unwrap();
        if write_guard.client.is_none() {
            // This URL is used to detect the appropriate TLS implementation for the server.
            let login_option_url = format!("{}/api/login-options", &api_server);
            let client = create_http_client_with_url(&login_option_url);
            write_guard.client = Some(client);
        }
    }

    fn auth(
        api_server: &str,
        op: &str,
        id: &str,
        uuid: &str,
    ) -> ResultType<HbbHttpResponse<OidcAuthUrl>> {
        Self::ensure_client(api_server);
        let resp = if let Some(client) = &OIDC_SESSION.read().unwrap().client {
            client
                .post(format!("{}/api/oidc/auth", api_server))
                .json(&serde_json::json!({
                    "op": op,
                    "id": id,
                    "uuid": uuid,
                    "deviceInfo": crate::ui_interface::get_login_device_info(),
                }))
                .send()?
        } else {
            hbb_common::bail!("http client not initialized");
        };
        let status = resp.status();
        match resp.try_into() {
            Ok(v) => Ok(v),
            Err(err) => {
                hbb_common::bail!("Http status: {}, err: {}", status, err);
            }
        }
    }

    fn query(
        api_server: &str,
        code: &str,
        id: &str,
        uuid: &str,
    ) -> ResultType<HbbHttpResponse<AuthBody>> {
        let url = Url::parse_with_params(
            &format!("{}/api/oidc/auth-query", api_server),
            &[("code", code), ("id", id), ("uuid", uuid)],
        )?;
        Self::ensure_client(api_server);
        if let Some(client) = &OIDC_SESSION.read().unwrap().client {
            Ok(client.get(url).send()?.try_into()?)
        } else {
            hbb_common::bail!("http client not initialized")
        }
    }

    fn reset(&mut self) {
        self.state_msg = REQUESTING_ACCOUNT_AUTH;
        self.failed_msg = "".to_owned();
        self.keep_querying = true;
        self.running = false;
        self.code_url = None;
        self.auth_body = None;
    }

    fn before_task(&mut self) {
        self.reset();
        self.running = true;
    }

    fn after_task(&mut self) {
        self.running = false;
    }

    fn sleep(secs: f32) {
        std::thread::sleep(std::time::Duration::from_secs_f32(secs));
    }

    fn auth_task(api_server: String, op: String, id: String, uuid: String, remember_me: bool) {
        let auth_request_res = Self::auth(&api_server, &op, &id, &uuid);
        log::info!("Request oidc auth result: {:?}", &auth_request_res);
        let code_url = match auth_request_res {
            Ok(HbbHttpResponse::<_>::Data(code_url)) => code_url,
            Ok(HbbHttpResponse::<_>::Error(err)) => {
                OIDC_SESSION
                    .write()
                    .unwrap()
                    .set_state(REQUESTING_ACCOUNT_AUTH, err);
                return;
            }
            Ok(_) => {
                OIDC_SESSION
                    .write()
                    .unwrap()
                    .set_state(REQUESTING_ACCOUNT_AUTH, "Invalid auth response".to_owned());
                return;
            }
            Err(err) => {
                OIDC_SESSION
                    .write()
                    .unwrap()
                    .set_state(REQUESTING_ACCOUNT_AUTH, err.to_string());
                return;
            }
        };

        OIDC_SESSION
            .write()
            .unwrap()
            .set_state(WAITING_ACCOUNT_AUTH, "".to_owned());
        OIDC_SESSION.write().unwrap().code_url = Some(code_url.clone());

        let begin = Instant::now();
        let query_timeout = OIDC_SESSION.read().unwrap().query_timeout;
        while OIDC_SESSION.read().unwrap().keep_querying && begin.elapsed() < query_timeout {
            match Self::query(&api_server, &code_url.code, &id, &uuid) {
                Ok(HbbHttpResponse::<_>::Data(auth_body)) => {
                    if auth_body.r#type == "access_token" {
                        if remember_me {
                            LocalConfig::set_option(
                                "access_token".to_owned(),
                                auth_body.access_token.clone(),
                            );
                            LocalConfig::set_option(
                                "user_info".to_owned(),
                                serde_json::json!({ "name": auth_body.user.name, "status": auth_body.user.status }).to_string(),
                            );
                        }
                    }
                    OIDC_SESSION
                        .write()
                        .unwrap()
                        .set_state(LOGIN_ACCOUNT_AUTH, "".to_owned());
                    OIDC_SESSION.write().unwrap().auth_body = Some(auth_body);
                    return;
                }
                Ok(HbbHttpResponse::<_>::Error(err)) => {
                    if err.contains("No authed oidc is found") {
                        // ignore, keep querying
                    } else {
                        OIDC_SESSION
                            .write()
                            .unwrap()
                            .set_state(WAITING_ACCOUNT_AUTH, err);
                        return;
                    }
                }
                Ok(_) => {
                    // ignore
                }
                Err(err) => {
                    log::trace!("Failed query oidc {}", err);
                    // ignore
                }
            }
            Self::sleep(QUERY_INTERVAL_SECS);
        }

        if begin.elapsed() >= query_timeout {
            OIDC_SESSION
                .write()
                .unwrap()
                .set_state(WAITING_ACCOUNT_AUTH, "timeout".to_owned());
        }

        // no need to handle "keep_querying == false"
    }

    fn set_state(&mut self, state_msg: &'static str, failed_msg: String) {
        self.state_msg = state_msg;
        self.failed_msg = failed_msg;
    }

    fn wait_stop_querying() {
        let wait_secs = 0.3;
        while OIDC_SESSION.read().unwrap().running {
            Self::sleep(wait_secs);
        }
    }

    pub fn account_auth(
        api_server: String,
        op: String,
        id: String,
        uuid: String,
        remember_me: bool,
    ) {
        Self::auth_cancel();
        Self::wait_stop_querying();
        OIDC_SESSION.write().unwrap().before_task();
        std::thread::spawn(move || {
            Self::auth_task(api_server, op, id, uuid, remember_me);
            OIDC_SESSION.write().unwrap().after_task();
        });
    }

    fn get_result_(&self) -> AuthResult {
        AuthResult {
            state_msg: self.state_msg.to_string(),
            failed_msg: self.failed_msg.clone(),
            url: self.code_url.as_ref().map(|x| x.url.to_string()),
            auth_body: self.auth_body.clone(),
        }
    }

    pub fn auth_cancel() {
        OIDC_SESSION.write().unwrap().keep_querying = false;
    }

    pub fn get_result() -> AuthResult {
        OIDC_SESSION.read().unwrap().get_result_()
    }
}
