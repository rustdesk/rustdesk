use hbb_common::{bail, ResultType};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::time::Duration;

#[cfg(feature = "flutter")]
pub mod account;
pub mod downloader;
mod http_client;
pub mod record_upload;
#[cfg(not(target_env = "ohos"))]
pub mod sync;
pub use http_client::{
    create_http_client_async, create_http_client_async_with_url_strict,
    create_http_client_with_url, create_http_client_with_url_strict, get_url_for_tls,
};

pub fn validate_rustdesk_api_server(api_server: &str, use_proxy: bool) -> ResultType<()> {
    let url = format!("{}/api/login-options", api_server.trim_end_matches('/'));
    let response = if use_proxy {
        create_http_client_with_url(&url)
            .get(&url)
            .timeout(Duration::from_millis(2_500))
            .send()?
    } else {
        reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(2_500))
            .build()?
            .get(&url)
            .send()?
    };
    if !response.status().is_success() {
        bail!(
            "RustDesk API /api/login-options returned HTTP {}",
            response.status().as_u16()
        );
    }
    let _: Vec<String> = response.json()?;
    Ok(())
}

#[derive(Debug)]
pub enum HbbHttpResponse<T> {
    ErrorFormat,
    Error(String),
    DataTypeFormat,
    Data(T),
}

impl<T: DeserializeOwned> HbbHttpResponse<T> {
    pub fn parse(body: &str) -> ResultType<Self> {
        let map = serde_json::from_str::<Map<String, Value>>(body)?;
        if let Some(error) = map.get("error") {
            if let Some(err) = error.as_str() {
                Ok(Self::Error(err.to_owned()))
            } else {
                Ok(Self::ErrorFormat)
            }
        } else {
            match serde_json::from_value(Value::Object(map)) {
                Ok(v) => Ok(Self::Data(v)),
                Err(_) => Ok(Self::DataTypeFormat),
            }
        }
    }
}
