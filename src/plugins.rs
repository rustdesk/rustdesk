use std::{collections::HashMap, path::Path, sync::Arc, ffi::CStr};

use hbb_common::anyhow::{anyhow, Error};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref PLUGIN_REGISTRAR: Arc<PluginRegistar<PluginImpl>> =
        Arc::new(PluginRegistar::<PluginImpl>::default());
}

pub trait Plugin {
    // Return: the unique ID which identifies this plugin.
    fn plugin_id(&self) -> String;
    // Return: the name which is human-readable.
    fn plugin_name(&self) -> String;
}

#[derive(Default, Clone)]
pub struct PluginImpl {
    id: String,
    name: String,
}

impl Plugin for PluginImpl {
    fn plugin_id(&self) -> String {
        self.id.to_owned()
    }

    fn plugin_name(&self) -> String {
        self.name.to_owned()
    }
}

#[derive(Default, Clone)]
pub struct PluginRegistar<P: Plugin> {
    plugins: HashMap<String, P>,
}

impl<P: Plugin> PluginRegistar<P> {
    pub fn load_plugin(&self, path: *const i8) -> i32 {
        let p = unsafe { CStr::from_ptr(path) };
        0
    }

    pub fn unload_plugin(&self, path: *const i8) -> i32 {
        let p = unsafe { CStr::from_ptr(path) };
        0
    }
}

impl TryFrom<&Path> for PluginImpl {
    type Error = Error;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        Err(anyhow!("Not implemented yet."))
    }
}
