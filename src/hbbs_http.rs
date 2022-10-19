use hbb_common::{
    anyhow::{self, bail},
    tokio, ResultType,
};
use reqwest::Response;
use serde_derive::Deserialize;
use serde_json::{Map, Value};
use serde::de::DeserializeOwned;

pub mod account;

pub enum HbbHttpResponse<T> {
    ErrorFormat,
    Error(String),
    DataTypeFormat,
    Data(T),
}

#[tokio::main(flavor = "current_thread")]
async fn resp_to_serde_map(resp: Response) -> reqwest::Result<Map<String, Value>> {
    resp.json().await
}

impl<T: DeserializeOwned> TryFrom<Response> for HbbHttpResponse<T> {
    type Error = reqwest::Error;

    fn try_from(resp: Response) -> Result<Self, <Self as TryFrom<Response>>::Error> {
        let map = resp_to_serde_map(resp)?;
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
