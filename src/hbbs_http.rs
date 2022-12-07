use reqwest::blocking::Response;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

#[cfg(feature = "flutter")]
pub mod account;
pub mod record_upload;

#[derive(Debug)]
pub enum HbbHttpResponse<T> {
    ErrorFormat,
    Error(String),
    DataTypeFormat,
    Data(T),
}

impl<T: DeserializeOwned> TryFrom<Response> for HbbHttpResponse<T> {
    type Error = reqwest::Error;

    fn try_from(resp: Response) -> Result<Self, <Self as TryFrom<Response>>::Error> {
        let map = resp.json::<Map<String, Value>>()?;
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
