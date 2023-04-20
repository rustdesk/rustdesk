use hbb_common::ResultType;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::ffi::{c_char, CStr};

#[derive(Debug, Serialize, Deserialize)]
pub struct UiButton {
    key: String,
    text: String,
    icon: String,   // icon can be int in flutter, but string in other ui framework. And it is flexible to use string.
    tooltip: String,
    action: String, // The action to be triggered when the button is clicked.
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UiCheckbox {
    key: String,
    text: String,
    tooltip: String,
    action: String, // The action to be triggered when the checkbox is checked or unchecked.
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t", content = "c")]
pub enum UiType {
    Button(UiButton),
    Checkbox(UiCheckbox),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Location {
    pub ui: HashMap<String, UiType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigItem {
    pub key: String,
    pub value: String,
    pub default: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub local: Vec<ConfigItem>,
    pub peer: Vec<ConfigItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Desc {
    id: String,
    name: String,
    version: String,
    description: String,
    author: String,
    home: String,
    license: String,
    published: String,
    released: String,
    github: String,
    location: Location,
    config: Config,
}

impl Desc {
    pub fn from_cstr(s: *const c_char) -> ResultType<Self> {
        let s = unsafe { CStr::from_ptr(s) };
        Ok(serde_json::from_str(s.to_str()?)?)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn home(&self) -> &str {
        &self.home
    }

    pub fn license(&self) -> &str {
        &self.license
    }

    pub fn published(&self) -> &str {
        &self.published
    }

    pub fn released(&self) -> &str {
        &self.released
    }

    pub fn github(&self) -> &str {
        &self.github
    }

    pub fn location(&self) -> &Location {
        &self.location
    }

    pub fn config(&self) -> &Config {
        &self.config
    }
}
