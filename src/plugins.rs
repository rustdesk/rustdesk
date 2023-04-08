use std::{
    collections::HashMap,
    ffi::CStr,
    path::Path,
    sync::{Arc, RwLock},
};

use hbb_common::anyhow::{anyhow, Error};
use lazy_static::lazy_static;
use libloading::Library;

lazy_static! {
    pub static ref PLUGIN_REGISTRAR: Arc<PluginRegistar<PluginImpl>> =
        Arc::new(PluginRegistar::<PluginImpl>::default());
}
// API needed to be implemented by plugins.
pub type PluginInitFunc = fn() -> i32;
// API needed to be implemented by plugins.
pub type PluginDisposeFunc = fn() -> i32;

pub trait Plugin {
    // Return: the unique ID which identifies this plugin.
    fn plugin_id(&self) -> String;
    // Return: the name which is human-readable.
    fn plugin_name(&self) -> String;
    // Return: the virtual table of the plugin.
    fn plugin_vt(&self) -> &RustDeskPluginTable;
}

#[repr(C)]
#[derive(Default, Clone)]
pub struct RustDeskPluginTable {
    pub init: Option<PluginInitFunc>,
    pub dispose: Option<PluginDisposeFunc>,
}

#[derive(Default, Clone)]
pub struct PluginImpl {
    vt: RustDeskPluginTable,
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

    fn plugin_vt(&self) -> &RustDeskPluginTable {
        &self.vt
    }
}

#[derive(Default, Clone)]
pub struct PluginRegistar<P: Plugin> {
    plugins: Arc<RwLock<HashMap<String, P>>>,
}

impl<P: Plugin> PluginRegistar<P> {
    pub fn load_plugin(&self, path: *const i8) -> i32 {
        let p = unsafe { CStr::from_ptr(path) };
        let lib_path = p.to_str().unwrap_or("").to_owned();
        let lib = unsafe { libloading::Library::new(lib_path.as_str()) };
        match lib {
            Ok(lib) => match lib.try_into() {
                Ok(plugin) => {
                    PLUGIN_REGISTRAR
                        .plugins
                        .write()
                        .unwrap()
                        .insert(lib_path, plugin);
                    return 0;
                }
                Err(err) => {
                    eprintln!("Load plugin failed: {}", err);
                }
            },
            Err(err) => {
                eprintln!("Load plugin failed: {}", err);
            }
        }
        -1
    }

    pub fn unload_plugin(&self, path: *const i8) -> i32 {
        let p = unsafe { CStr::from_ptr(path) };
        let lib_path = p.to_str().unwrap_or("").to_owned();
        match PLUGIN_REGISTRAR.plugins.write().unwrap().remove(&lib_path) {
            Some(_) => 0,
            None => -1,
        }
    }
}

impl TryFrom<Library> for PluginImpl {
    type Error = Error;

    fn try_from(library: Library) -> Result<Self, Self::Error> {
        todo!()
    }
}
