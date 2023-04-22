use crate::plugins::Plugin;

use super::desc::ConfigItem;
use hbb_common::{bail, config::Config as HbbConfig, lazy_static, ResultType};
use serde_derive::{Deserialize, Serialize};
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
    {collections::HashMap, path::PathBuf},
};

lazy_static::lazy_static! {
    static ref CONFIG_LOCAL: Arc<Mutex<HashMap<String, LocalConfig>>> = Default::default();
    static ref CONFIG_LOCAL_ITEMS: Arc<Mutex<HashMap<String, Vec<ConfigItem>>>> = Default::default();
    static ref CONFIG_PEERS: Arc<Mutex<HashMap<String, PeersConfig>>> = Default::default();
    static ref CONFIG_PEER_ITEMS: Arc<Mutex<HashMap<String, Vec<ConfigItem>>>> = Default::default();
    static ref CONFIG_MANAGER: Arc<Mutex<ManagerConfig>> = {
        let conf = hbb_common::config::load_path::<ManagerConfig>(ManagerConfig::path());
        Arc::new(Mutex::new(conf))
    };
}

pub(super) const CONFIG_TYPE_LOCAL: &str = "local";
pub(super) const CONFIG_TYPE_PEER: &str = "peer";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LocalConfig(HashMap<String, String>);
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PeerConfig(HashMap<String, String>);
type PeersConfig = HashMap<String, PeerConfig>;

#[inline]
fn path_plugins(id: &str) -> PathBuf {
    HbbConfig::path("plugins").join(id)
}

impl Deref for LocalConfig {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LocalConfig {
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

impl LocalConfig {
    #[inline]
    fn path(id: &str) -> PathBuf {
        path_plugins(id).join("local.toml")
    }

    #[inline]
    pub fn load(id: &str) {
        let mut conf = hbb_common::config::load_path::<LocalConfig>(Self::path(id));
        if let Some(items) = CONFIG_LOCAL_ITEMS.lock().unwrap().get(id) {
            for item in items {
                if !conf.contains_key(&item.key) {
                    conf.insert(item.key.to_owned(), item.default.to_owned());
                }
            }
        }
        CONFIG_LOCAL.lock().unwrap().insert(id.to_owned(), conf);
    }

    #[inline]
    pub fn get(id: &str, key: &str) -> Option<String> {
        if let Some(conf) = CONFIG_LOCAL.lock().unwrap().get(id) {
            return conf.get(key).map(|s| s.to_owned());
        }
        Self::load(id);
        CONFIG_LOCAL
            .lock()
            .unwrap()
            .get(id)?
            .get(key)
            .map(|s| s.to_owned())
    }

    #[inline]
    pub fn set(id: &str, key: &str, value: &str) -> ResultType<()> {
        match CONFIG_LOCAL.lock().unwrap().get_mut(id) {
            Some(config) => {
                config.insert(key.to_owned(), value.to_owned());
                hbb_common::config::store_path(Self::path(id), config)
            }
            None => bail!("No such plugin {}", id),
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
    pub fn load(id: &str, peer: &str) {
        let mut conf = hbb_common::config::load_path::<PeerConfig>(Self::path(id, peer));
        if let Some(items) = CONFIG_PEER_ITEMS.lock().unwrap().get(id) {
            for item in items {
                if !conf.contains_key(&item.key) {
                    conf.insert(item.key.to_owned(), item.default.to_owned());
                }
            }
        }
        match CONFIG_PEERS.lock().unwrap().get_mut(id) {
            Some(peers) => {
                peers.insert(peer.to_owned(), conf);
            }
            None => {
                let mut peers = HashMap::new();
                peers.insert(peer.to_owned(), conf);
                CONFIG_PEERS.lock().unwrap().insert(id.to_owned(), peers);
            }
        }
    }

    #[inline]
    pub fn get(id: &str, peer: &str, key: &str) -> Option<String> {
        if let Some(peers) = CONFIG_PEERS.lock().unwrap().get(id) {
            if let Some(conf) = peers.get(peer) {
                return conf.get(key).map(|s| s.to_owned());
            }
        }
        Self::load(id, peer);
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
        match CONFIG_PEERS.lock().unwrap().get_mut(id) {
            Some(peers) => match peers.get_mut(peer) {
                Some(config) => {
                    config.insert(key.to_owned(), value.to_owned());
                    hbb_common::config::store_path(Self::path(id, peer), config)
                }
                None => bail!("No such peer {}", peer),
            },
            None => bail!("No such plugin {}", id),
        }
    }
}

#[inline]
pub(super) fn set_local_items(id: &str, items: &Vec<ConfigItem>) {
    CONFIG_LOCAL_ITEMS
        .lock()
        .unwrap()
        .insert(id.to_owned(), items.clone());
}

#[inline]
pub(super) fn set_peer_items(id: &str, items: &Vec<ConfigItem>) {
    CONFIG_PEER_ITEMS
        .lock()
        .unwrap()
        .insert(id.to_owned(), items.clone());
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
    pub plugins: HashMap<String, PluginStatus>,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            version: "0.1.0".to_owned(),
            enabled: true,
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
    pub fn is_enabled() -> bool {
        CONFIG_MANAGER.lock().unwrap().enabled
    }

    #[inline]
    pub fn set_enabled(enabled: bool) -> ResultType<()> {
        let mut lock = CONFIG_MANAGER.lock().unwrap();
        lock.enabled = enabled;
        hbb_common::config::store_path(Self::path(), &*lock)
    }

    #[inline]
    pub fn get_plugin_status<T>(id: &str, f: fn(&PluginStatus) -> T) -> Option<T> {
        let lock = CONFIG_MANAGER.lock().unwrap();
        lock.plugins.get(id).map(f)
    }

    pub fn set_plugin_enabled(id: &str, enabled: bool) -> ResultType<()> {
        let mut lock = CONFIG_MANAGER.lock().unwrap();
        if let Some(status) = lock.plugins.get_mut(id) {
            status.enabled = enabled;
            hbb_common::config::store_path(Self::path(), &*lock)
        } else {
            bail!("No such plugin {}", id)
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
    pub fn remove_plugin(id: &str) -> ResultType<()> {
        let mut lock = CONFIG_MANAGER.lock().unwrap();
        lock.plugins.remove(id);
        hbb_common::config::store_path(Self::path(), &*lock)
    }
}
