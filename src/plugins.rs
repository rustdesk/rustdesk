use std::{
    collections::HashMap,
    ffi::{c_char, CStr},
    sync::{Arc, RwLock},
};

use hbb_common::{
    anyhow::Error,
    log::{debug, error},
};
use lazy_static::lazy_static;
use libloading::{Library, Symbol};

lazy_static! {
    pub static ref PLUGIN_REGISTRAR: Arc<PluginRegistar<PluginImpl>> =
        Arc::new(PluginRegistar::<PluginImpl>::default());
}
// API needed to be implemented by plugins.
pub type PluginInitFunc = fn() -> i32;
// API needed to be implemented by plugins.
pub type PluginIdFunc = fn() -> *const c_char;
// API needed to be implemented by plugins.
pub type PluginNameFunc = fn() -> *const c_char;
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
    pub init: Option<PluginInitFunc>, // NonNull
    pub dispose: Option<PluginDisposeFunc>, // NonNull
}

pub struct PluginImpl {
    vt: RustDeskPluginTable,
    pub id: String,
    pub name: String,
    _inner: Option<Library>,
}

impl Default for PluginImpl {
    fn default() -> Self {
        Self {
            _inner: None,
            vt: Default::default(),
            id: Default::default(),
            name: Default::default(),
        }
    }
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
    pub fn load_plugin(&self, path: *const c_char) -> i32 {
        let p = unsafe { CStr::from_ptr(path) };
        let lib_path = p.to_str().unwrap_or("").to_owned();
        let lib = unsafe { libloading::Library::new(lib_path.as_str()) };
        match lib {
            Ok(lib) => match lib.try_into() {
                Ok(plugin) => {
                    let plugin: PluginImpl = plugin;
                    // try to initialize this plugin
                    if let Some(init) = plugin.plugin_vt().init {
                        let init_ret = init();
                        if init_ret != 0 {
                            error!(
                                "Error when initializing the plugin {} with error code {}.",
                                plugin.name, init_ret
                            );
                            return init_ret;
                        }
                    }
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

    pub fn unload_plugin(&self, path: *const c_char) -> i32 {
        let p = unsafe { CStr::from_ptr(path) };
        let lib_path = p.to_str().unwrap_or("").to_owned();
        match PLUGIN_REGISTRAR.plugins.write().unwrap().remove(&lib_path) {
            Some(plugin) => {
                if let Some(dispose) = plugin.plugin_vt().dispose {
                    return dispose();
                }
                0
            }
            None => -1,
        }
    }
}

impl TryFrom<Library> for PluginImpl {
    type Error = Error;

    fn try_from(library: Library) -> Result<Self, Self::Error> {
        let init: Symbol<PluginInitFunc> = unsafe { library.get(b"plugin_init")? };
        let dispose: Symbol<PluginDisposeFunc> = unsafe { library.get(b"plugin_dispose")? };
        let id_func: Symbol<PluginIdFunc> = unsafe { library.get(b"plugin_id")? };
        let id_string = unsafe {
            std::ffi::CStr::from_ptr(id_func())
                .to_str()
                .unwrap_or("")
                .to_owned()
        };
        let name_func: Symbol<PluginNameFunc> = unsafe { library.get(b"plugin_name")? };
        let name_string = unsafe {
            std::ffi::CStr::from_ptr(name_func())
                .to_str()
                .unwrap_or("")
                .to_owned()
        };
        debug!(
            "Successfully loaded the plugin called {} with id {}.",
            name_string, id_string
        );
        Ok(Self {
            vt: RustDeskPluginTable {
                init: Some(*init),
                dispose: Some(*dispose),
            },
            id: id_string,
            name: name_string,
            _inner: Some(library),
        })
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_plugin() {
    use std::io::Write;
    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir("./examples/custom_plugin");
    // Strip this shared library.
    cmd.env("RUSTFLAGS", "-C link-arg=-s");
    cmd.arg("build");
    // Spawn the compiler process.
    let mut child = cmd.spawn().unwrap();
    // Wait for the compiler to finish.
    let status = child.wait().unwrap();
    assert!(status.success());
    // Load the library.
    let lib = unsafe {
        Library::new("./examples/custom_plugin/target/debug/libcustom_plugin.so").unwrap()
    };
    let plugin: PluginImpl = lib.try_into().unwrap();
    assert!(plugin._inner.is_some());
    assert!(plugin.name == "A Template Rust Plugin");
    assert!(plugin.id == "TemplatePlugin");
    println!(
        "plugin vt size: {}",
        std::mem::size_of::<RustDeskPluginTable>()
    );
    assert!(PLUGIN_REGISTRAR
        .plugins
        .write()
        .unwrap()
        .insert("test".to_owned(), plugin)
        .is_none());
}
