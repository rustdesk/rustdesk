use super::HbbHttpResponse;
use hbb_common::{config::Config, log, sleep, tokio, tokio::sync::RwLock, ResultType};
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use url::Url;

lazy_static::lazy_static! {
    static ref API_SERVER: String = crate::get_api_server(
        Config::get_option("api-server"), Config::get_option("custom-rendezvous-server"));
    static ref OIDC_SESSION: Arc<RwLock<OidcSession>> = Arc::new(RwLock::new(OidcSession::new()));
}

const QUERY_INTERVAL_SECS: f32 = 1.0;
const QUERY_TIMEOUT_SECS: u64 = 60;
const REQUESTING_ACCOUNT_AUTH: &str = "Requesting account auth";
const WAITING_ACCOUNT_AUTH: &str = "Waiting account auth";
const LOGIN_ACCOUNT_AUTH: &str = "Login account auth";

#[derive(Deserialize, Clone)]
pub struct OidcAuthUrl {
    code: String,
    url: Url,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct UserPayload {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub note: Option<String>,
    pub status: Option<i64>,
    pub grp: Option<String>,
    pub is_admin: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthBody {
    pub access_token: String,
    pub token_type: String,
    pub user: UserPayload,
}

pub struct OidcSession {
    client: reqwest::Client,
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

impl OidcSession {
    fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            state_msg: REQUESTING_ACCOUNT_AUTH,
            failed_msg: "".to_owned(),
            code_url: None,
            auth_body: None,
            keep_querying: false,
            running: false,
            query_timeout: Duration::from_secs(QUERY_TIMEOUT_SECS),
        }
    }

    async fn auth(op: &str, id: &str, uuid: &str) -> ResultType<HbbHttpResponse<OidcAuthUrl>> {
        Ok(OIDC_SESSION
            .read()
            .await
            .client
            .post(format!("{}/api/oidc/auth", *API_SERVER))
            .json(&HashMap::from([("op", op), ("id", id), ("uuid", uuid)]))
            .send()
            .await?
            .try_into()?)
    }

    async fn query(code: &str, id: &str, uuid: &str) -> ResultType<HbbHttpResponse<AuthBody>> {
        let url = reqwest::Url::parse_with_params(
            &format!("{}/api/oidc/auth-query", *API_SERVER),
            &[("code", code), ("id", id), ("uuid", uuid)],
        )?;
        Ok(OIDC_SESSION
            .read()
            .await
            .client
            .get(url)
            .send()
            .await?
            .try_into()?)
    }

    fn reset(&mut self) {
        self.state_msg = REQUESTING_ACCOUNT_AUTH;
        self.failed_msg = "".to_owned();
        self.keep_querying = true;
        self.running = false;
        self.code_url = None;
        self.auth_body = None;
    }

    async fn before_task(&mut self) {
        self.reset();
        self.running = true;
    }

    async fn after_task(&mut self) {
        self.running = false;
    }

    async fn auth_task(op: String, id: String, uuid: String) {
        let code_url = match Self::auth(&op, &id, &uuid).await {
            Ok(HbbHttpResponse::<_>::Data(code_url)) => code_url,
            Ok(HbbHttpResponse::<_>::Error(err)) => {
                OIDC_SESSION
                    .write()
                    .await
                    .set_state(REQUESTING_ACCOUNT_AUTH, err);
                return;
            }
            Ok(_) => {
                OIDC_SESSION
                    .write()
                    .await
                    .set_state(REQUESTING_ACCOUNT_AUTH, "Invalid auth response".to_owned());
                return;
            }
            Err(err) => {
                OIDC_SESSION
                    .write()
                    .await
                    .set_state(REQUESTING_ACCOUNT_AUTH, err.to_string());
                return;
            }
        };

        OIDC_SESSION
            .write()
            .await
            .set_state(WAITING_ACCOUNT_AUTH, "".to_owned());
        OIDC_SESSION.write().await.code_url = Some(code_url.clone());

        let begin = Instant::now();
        let query_timeout = OIDC_SESSION.read().await.query_timeout;
        while OIDC_SESSION.read().await.keep_querying && begin.elapsed() < query_timeout {
            match Self::query(&code_url.code, &id, &uuid).await {
                Ok(HbbHttpResponse::<_>::Data(auth_body)) => {
                    OIDC_SESSION
                        .write()
                        .await
                        .set_state(LOGIN_ACCOUNT_AUTH, "".to_owned());
                    OIDC_SESSION.write().await.auth_body = Some(auth_body);
                    return;
                    // to-do, set access-token
                }
                Ok(HbbHttpResponse::<_>::Error(err)) => {
                    if err.contains("No authed oidc is found") {
                        // ignore, keep querying
                    } else {
                        OIDC_SESSION
                            .write()
                            .await
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
            sleep(QUERY_INTERVAL_SECS).await;
        }

        if begin.elapsed() >= query_timeout {
            OIDC_SESSION
                .write()
                .await
                .set_state(WAITING_ACCOUNT_AUTH, "timeout".to_owned());
        }

        // no need to handle "keep_querying == false"
    }

    fn set_state(&mut self, state_msg: &'static str, failed_msg: String) {
        self.state_msg = state_msg;
        self.failed_msg = failed_msg;
    }

    pub async fn account_auth(op: String, id: String, uuid: String) {
        if OIDC_SESSION.read().await.running {
            OIDC_SESSION.write().await.keep_querying = false;
        }
        let wait_secs = 0.3;
        sleep(wait_secs).await;
        while OIDC_SESSION.read().await.running {
            sleep(wait_secs).await;
        }

        tokio::spawn(async move {
            OIDC_SESSION.write().await.before_task().await;
            Self::auth_task(op, id, uuid).await;
            OIDC_SESSION.write().await.after_task().await;
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

    pub async fn get_result() -> AuthResult {
        OIDC_SESSION.read().await.get_result_()
    }
}
