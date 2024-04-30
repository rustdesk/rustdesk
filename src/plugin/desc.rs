use hbb_common::ResultType;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::ffi::{c_char, CStr};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiButton {
    key: String,
    text: String,
    icon: String, // icon can be int in flutter, but string in other ui framework. And it is flexible to use string.
    tooltip: String,
    action: String, // The action to be triggered when the button is clicked.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiCheckbox {
    key: String,
    text: String,
    tooltip: String,
    action: String, // The action to be triggered when the checkbox is checked or unchecked.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", content = "c")]
pub enum UiType {
    Button(UiButton),
    Checkbox(UiCheckbox),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub ui: HashMap<String, Vec<UiType>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigItem {
    pub key: String,
    pub default: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub shared: Vec<ConfigItem>,
    pub peer: Vec<ConfigItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishInfo {
    pub published: String,
    pub last_released: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub platforms: String,
    pub author: String,
    pub home: String,
    pub license: String,
    pub source: String,
    pub publish_info: PublishInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Desc {
    meta: Meta,
    need_reboot: bool,
    location: Location,
    config: Config,
    listen_events: Vec<String>,
}

impl Desc {
    pub fn from_cstr(s: *const c_char) -> ResultType<Self> {
        let s = unsafe { CStr::from_ptr(s) };
        Ok(serde_json::from_str(s.to_str()?)?)
    }

    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    pub fn location(&self) -> &Location {
        &self.location
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn listen_events(&self) -> &Vec<String> {
        &self.listen_events
    }
}
