use super::{cstr_to_string, str_to_cstr_ret};
use hbb_common::{allow_err, bail, config::Config as HbbConfig, lazy_static, log, ResultType};
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::c_char,
    fs,
    ops::{Deref, DerefMut},
    path::PathBuf,
    ptr,
    str::FromStr,
    sync::{Arc, Mutex},
};

lazy_static::lazy_static! {
    static ref CONFIG_SHARED: Arc<Mutex<HashMap<String, SharedConfig>>> = Default::default();
    static ref CONFIG_PEERS: Arc<Mutex<HashMap<String, PeersConfig>>> = Default::default();
    static ref CONFIG_MANAGER: Arc<Mutex<ManagerConfig>> = {
        let conf = hbb_common::config::load_path::<ManagerConfig>(ManagerConfig::path());
        Arc::new(Mutex::new(conf))
    };
}
use crate::ui_interface::get_id;

pub(super) const CONFIG_TYPE_SHARED: &str = "shared";
pub(super) const CONFIG_TYPE_PEER: &str = "peer";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SharedConfig(HashMap<String, String>);
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PeerConfig(HashMap<String, String>);
type PeersConfig = HashMap<String, PeerConfig>;

#[inline]
fn path_plugins(id: &str) -> PathBuf {
    HbbConfig::path("plugins").join(id)
}

impl Deref for SharedConfig {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SharedConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for PeerConfig {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PeerConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl SharedConfig {
    #[inline]
    fn path(id: &str) -> PathBuf {
        path_plugins(id).join("shared.toml")
    }

    #[inline]
    fn load(id: &str) {
        let mut lock = CONFIG_SHARED.lock().unwrap();
        if lock.contains_key(id) {
            return;
        }
        let conf = hbb_common::config::load_path::<HashMap<String, String>>(Self::path(id));
        let mut conf = SharedConfig(conf);
        if let Some(desc_conf) = super::plugins::get_desc_conf(id) {
            for item in desc_conf.shared.iter() {
                if !conf.contains_key(&item.key) {
                    conf.insert(item.key.to_owned(), item.default.to_owned());
                }
            }
        }
        lock.insert(id.to_owned(), conf);
    }

    #[inline]
    fn load_if_not_exists(id: &str) {
        if CONFIG_SHARED.lock().unwrap().contains_key(id) {
            return;
        }
        Self::load(id);
    }

    #[inline]
    pub fn get(id: &str, key: &str) -> Option<String> {
        Self::load_if_not_exists(id);
        CONFIG_SHARED
            .lock()
            .unwrap()
            .get(id)?
            .get(key)
            .map(|s| s.to_owned())
    }

    #[inline]
    pub fn set(id: &str, key: &str, value: &str) -> ResultType<()> {
        Self::load_if_not_exists(id);
        match CONFIG_SHARED.lock().unwrap().get_mut(id) {
            Some(config) => {
                config.insert(key.to_owned(), value.to_owned());
                hbb_common::config::store_path(Self::path(id), config)
            }
            None => {
                // unreachable
                bail!("No such plugin {}", id)
            }
        }
    }
}

impl PeerConfig {
    #[inline]
    fn path(id: &str, peer: &str) -> PathBuf {
        path_plugins(id)
            .join("peers")
            .join(format!("{}.toml", peer))
    }

    #[inline]
    fn load(id: &str, peer: &str) {
        let mut lock = CONFIG_PEERS.lock().unwrap();
        if let Some(peers) = lock.get(id) {
            if peers.contains_key(peer) {
                return;
            }
        }

        let conf = hbb_common::config::load_path::<HashMap<String, String>>(Self::path(id, peer));
        let mut conf = PeerConfig(conf);
        if let Some(desc_conf) = super::plugins::get_desc_conf(id) {
            for item in desc_conf.peer.iter() {
                if !conf.contains_key(&item.key) {
                    conf.insert(item.key.to_owned(), item.default.to_owned());
                }
            }
        }

        if let Some(peers) = lock.get_mut(id) {
            peers.insert(peer.to_owned(), conf);
            return;
        }

        let mut peers = HashMap::new();
        peers.insert(peer.to_owned(), conf);
        lock.insert(id.to_owned(), peers);
    }

    #[inline]
    fn load_if_not_exists(id: &str, peer: &str) {
        if let Some(peers) = CONFIG_PEERS.lock().unwrap().get(id) {
            if peers.contains_key(peer) {
                return;
            }
        }
        Self::load(id, peer);
    }

    #[inline]
    pub fn get(id: &str, peer: &str, key: &str) -> Option<String> {
        Self::load_if_not_exists(id, peer);
        CONFIG_PEERS
            .lock()
            .unwrap()
            .get(id)?
            .get(peer)?
            .get(key)
            .map(|s| s.to_owned())
    }

    #[inline]
    pub fn set(id: &str, peer: &str, key: &str, value: &str) -> ResultType<()> {
        Self::load_if_not_exists(id, peer);
        match CONFIG_PEERS.lock().unwrap().get_mut(id) {
            Some(peers) => match peers.get_mut(peer) {
                Some(config) => {
                    config.insert(key.to_owned(), value.to_owned());
                    hbb_common::config::store_path(Self::path(id, peer), config)
                }
                None => {
                    // unreachable
                    bail!("No such peer {}", peer)
                }
            },
            None => {
                // unreachable
                bail!("No such plugin {}", id)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginStatus {
    pub enabled: bool,
}

const MANAGER_VERSION: &str = "0.1.0";

#[derive(Debug, Serialize, Deserialize)]
pub struct ManagerConfig {
    pub version: String,
    pub enabled: bool,
    #[serde(default)]
    pub options: HashMap<String, String>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginStatus>,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            version: MANAGER_VERSION.to_owned(),
            enabled: true,
            options: HashMap::new(),
            plugins: HashMap::new(),
        }
    }
}

// Do not care about the `store_path` error, no need to store the old value and restore if failed.
impl ManagerConfig {
    #[inline]
    fn path() -> PathBuf {
        HbbConfig::path("plugins").join("manager.toml")
    }

    #[inline]
    pub fn get_option(key: &str) -> Option<String> {
        if key == "enabled" {
            Some(CONFIG_MANAGER.lock().unwrap().enabled.to_string())
        } else {
            CONFIG_MANAGER
                .lock()
                .unwrap()
                .options
                .get(key)
                .map(|s| s.to_owned())
        }
    }

    fn set_option_enabled(enabled: bool) -> ResultType<()> {
        let mut lock = CONFIG_MANAGER.lock().unwrap();
        lock.enabled = enabled;
        hbb_common::config::store_path(Self::path(), &*lock)
    }

    fn set_option_not_enabled(key: &str, value: &str) -> ResultType<()> {
        let mut lock = CONFIG_MANAGER.lock().unwrap();
        lock.options.insert(key.to_owned(), value.to_owned());
        hbb_common::config::store_path(Self::path(), &*lock)
    }

    #[inline]
    pub fn set_option(key: &str, value: &str) {
        if key == "enabled" {
            let enabled = bool::from_str(value).unwrap_or(false);
            allow_err!(Self::set_option_enabled(enabled));
            if enabled {
                allow_err!(super::load_plugins());
            } else {
                super::unload_plugins();
            }
        } else {
            allow_err!(Self::set_option_not_enabled(key, value));
        }
    }

    #[inline]
    pub fn get_plugin_option(id: &str, key: &str) -> Option<String> {
        let lock = CONFIG_MANAGER.lock().unwrap();
        let status = lock.plugins.get(id)?;
        match key {
            "enabled" => Some(status.enabled.to_string()),
            _ => None,
        }
    }

    fn set_plugin_option_enabled(id: &str, enabled: bool) -> ResultType<()> {
        let mut lock = CONFIG_MANAGER.lock().unwrap();
        if let Some(status) = lock.plugins.get_mut(id) {
            status.enabled = enabled;
        } else {
            lock.plugins.insert(id.to_owned(), PluginStatus { enabled });
        }
        hbb_common::config::store_path(Self::path(), &*lock)
    }

    pub fn set_plugin_option(id: &str, key: &str, value: &str) {
        match key {
            "enabled" => {
                let enabled = bool::from_str(value).unwrap_or(false);
                allow_err!(Self::set_plugin_option_enabled(id, enabled));
                if enabled {
                    allow_err!(super::load_plugin(None, Some(id)));
                } else {
                    super::unload_plugin(id);
                }
            }
            _ => log::error!("No such option {}", key),
        }
    }

    #[inline]
    pub fn add_plugin(id: &str) -> ResultType<()> {
        let mut lock = CONFIG_MANAGER.lock().unwrap();
        lock.plugins
            .insert(id.to_owned(), PluginStatus { enabled: true });
        hbb_common::config::store_path(Self::path(), &*lock)
    }

    #[inline]
    pub fn remove_plugin(id: &str, uninstall: bool) -> ResultType<()> {
        let mut lock = CONFIG_MANAGER.lock().unwrap();
        lock.plugins.remove(id);
        hbb_common::config::store_path(Self::path(), &*lock)?;
        if uninstall {
            allow_err!(fs::remove_dir_all(path_plugins(id)));
        }
        Ok(())
    }

    pub fn remove_plugins(uninstall: bool) {
        let mut lock = CONFIG_MANAGER.lock().unwrap();
        lock.plugins.clear();
        allow_err!(hbb_common::config::store_path(Self::path(), &*lock));
        if uninstall {
            allow_err!(fs::remove_dir_all(HbbConfig::path("plugins")));
        }
    }
}

pub(super) extern "C" fn cb_get_local_peer_id() -> *const c_char {
    str_to_cstr_ret(&get_id())
}

// Return shared config if peer is nullptr.
pub(super) extern "C" fn cb_get_conf(
    peer: *const c_char,
    id: *const c_char,
    key: *const c_char,
) -> *const c_char {
    match (cstr_to_string(id), cstr_to_string(key)) {
        (Ok(id), Ok(key)) => {
            if peer.is_null() {
                SharedConfig::load_if_not_exists(&id);
                if let Some(conf) = CONFIG_SHARED.lock().unwrap().get(&id) {
                    if let Some(value) = conf.get(&key) {
                        return str_to_cstr_ret(value);
                    }
                }
            } else {
                match cstr_to_string(peer) {
                    Ok(peer) => {
                        PeerConfig::load_if_not_exists(&id, &peer);
                        if let Some(conf) = CONFIG_PEERS.lock().unwrap().get(&id) {
                            if let Some(conf) = conf.get(&peer) {
                                if let Some(value) = conf.get(&key) {
                                    return str_to_cstr_ret(value);
                                }
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }
        _ => {}
    }
    ptr::null()
}
