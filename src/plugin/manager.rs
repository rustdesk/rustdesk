// 1. Check update.
// 2. Install or uninstall.

use super::{desc::Meta as PluginMeta, ipc::InstallStatus, *};
use crate::{common::is_server, flutter};
use hbb_common::{allow_err, bail, config::load_path, log, tokio, toml};
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const MSG_TO_UI_PLUGIN_MANAGER_LIST: &str = "plugin_list";
const MSG_TO_UI_PLUGIN_MANAGER_UPDATE: &str = "plugin_update";
const MSG_TO_UI_PLUGIN_MANAGER_INSTALL: &str = "plugin_install";

const IPC_PLUGIN_POSTFIX: &str = "_plugin";

lazy_static::lazy_static! {
    static ref PLUGIN_INFO: Arc<Mutex<HashMap<String, PluginInfo>>> = Arc::new(Mutex::new(HashMap::new()));
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ManagerMeta {
    pub version: String,
    pub description: String,
    pub plugins: Vec<PluginMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSource {
    pub name: String,
    pub url: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct PluginInfo {
    pub source: PluginSource,
    pub plugin: PluginMeta,
    pub installed_version: String,
    pub install_time: String,
    pub invalid_reason: String,
}

static PLUGIN_SOURCE_LOCAL: &str = "local";

#[cfg(not(debug_assertions))]
fn get_plugin_source_list() -> Vec<PluginSource> {
    // Only one source for now.
    vec![PluginSource {
        name: "rustdesk".to_string(),
        url: "https://github.com/fufesou/rustdesk-plugins".to_string(),
        description: "".to_string(),
    }]
}

#[cfg(debug_assertions)]
fn get_source_plugins() -> HashMap<String, PluginInfo> {
    let meta_file = super::get_plugins_dir().unwrap().join("meta.toml");
    let mut plugins = HashMap::new();
    let meta = load_path::<ManagerMeta>(meta_file);
    let source = PluginSource {
        name: "rustdesk".to_string(),
        url: "https://github.com/fufesou/rustdesk-plugins".to_string(),
        description: "".to_string(),
    };
    for plugin in meta.plugins.iter() {
        plugins.insert(
            plugin.id.clone(),
            PluginInfo {
                source: source.clone(),
                plugin: plugin.clone(),
                installed_version: "".to_string(),
                install_time: "".to_string(),
                invalid_reason: "".to_string(),
            },
        );
    }
    plugins
}

#[cfg(not(debug_assertions))]
fn get_source_plugins() -> HashMap<String, PluginInfo> {
    let mut plugins = HashMap::new();
    for source in get_plugin_source_list().into_iter() {
        let url = format!("{}/meta.toml", source.url);
        match reqwest::blocking::get(&url) {
            Ok(resp) => {
                if !resp.status().is_success() {
                    log::error!(
                        "Failed to get plugin list from '{}', status code: {}",
                        url,
                        resp.status()
                    );
                }
                match resp.json::<ManagerMeta>() {
                    Ok(meta) => {
                        for plugin in meta.plugins.iter() {
                            plugins.insert(
                                plugin.id.clone(),
                                PluginInfo {
                                    source: source.clone(),
                                    plugin: plugin.clone(),
                                    installed_version: "".to_string(),
                                    install_time: "".to_string(),
                                    invalid_reason: "".to_string(),
                                },
                            );
                        }
                    }
                    Err(e) => log::error!("Failed to parse plugin list from '{}', {}", url, e),
                }
            }
            Err(e) => log::error!("Failed to get plugin list from '{}', {}", url, e),
        }
    }
    plugins
}

fn send_plugin_list_event(plugins: &HashMap<String, PluginInfo>) {
    let mut plugin_list = plugins.values().collect::<Vec<_>>();
    plugin_list.sort_by(|a, b| a.plugin.name.cmp(&b.plugin.name));
    if let Ok(plugin_list) = serde_json::to_string(&plugin_list) {
        let mut m = HashMap::new();
        m.insert("name", MSG_TO_UI_TYPE_PLUGIN_MANAGER);
        m.insert(MSG_TO_UI_PLUGIN_MANAGER_LIST, &plugin_list);
        if let Ok(event) = serde_json::to_string(&m) {
            let _res = flutter::push_global_event(flutter::APP_TYPE_MAIN, event.clone());
        }
    }
}

pub fn load_plugin_list() {
    let mut plugin_info_lock = PLUGIN_INFO.lock().unwrap();
    let mut plugins = get_source_plugins();
    for (id, info) in super::plugins::get_plugin_infos().read().unwrap().iter() {
        if let Some(p) = plugins.get_mut(id) {
            p.install_time = info.install_time.clone();
            p.invalid_reason = info.desc.meta().version.clone();
        } else {
            plugins.insert(
                id.to_string(),
                PluginInfo {
                    source: PluginSource {
                        name: PLUGIN_SOURCE_LOCAL.to_string(),
                        url: PLUGIN_SOURCE_LOCAL_DIR.to_string(),
                        description: "".to_string(),
                    },
                    plugin: info.desc.meta().clone(),
                    installed_version: info.desc.meta().version.clone(),
                    install_time: info.install_time.clone(),
                    invalid_reason: "".to_string(),
                },
            );
        }
    }
    send_plugin_list_event(&plugins);
    *plugin_info_lock = plugins;
}

pub fn install_plugin(id: &str) -> ResultType<()> {
    match PLUGIN_INFO.lock().unwrap().get(id) {
        Some(plugin) => {
            let _plugin_url = format!(
                "{}/plugins/{}/{}_{}.zip",
                plugin.source.url, plugin.plugin.id, plugin.plugin.id, plugin.plugin.version
            );
            #[cfg(windows)]
            let _res =
                crate::platform::elevate(&format!("--plugin-install '{}' '{}'", id, _plugin_url))?;
            Ok(())
        }
        None => {
            bail!("Plugin not found: {}", id);
        }
    }
}

pub(super) fn remove_plugins() {}

// 1. Add to uninstall list.
// 2. Try remove.
// 2. Remove on the next start.
pub fn uninstall_plugin(id: &str) {
    // to-do: add to uninstall list.
    super::plugins::unload_plugin(id);
}

fn push_install_event(id: &str, msg: &str) {
    let mut m = HashMap::new();
    m.insert("name", MSG_TO_UI_TYPE_PLUGIN_MANAGER);
    m.insert("id", id);
    m.insert(MSG_TO_UI_PLUGIN_MANAGER_INSTALL, msg);
    if let Ok(event) = serde_json::to_string(&m) {
        let _res = flutter::push_global_event(flutter::APP_TYPE_MAIN, event.clone());
    }
}

async fn handle_conn(mut stream: crate::ipc::Connection) {
    loop {
        tokio::select! {
            res = stream.next() => {
                match res {
                    Err(err) => {
                        log::trace!("plugin ipc connection closed: {}", err);
                        break;
                    }
                    Ok(Some(data)) => {
                        match &data {
                            crate::ipc::Data::Plugin(super::ipc::Plugin::InstallStatus((id, status))) => {
                                match status {
                                    InstallStatus::Downloading(n) => {
                                        push_install_event(&id, &format!("downloading-{}", n));
                                    },
                                    InstallStatus::Installing => {
                                        push_install_event(&id, "installing");
                                    }
                                    InstallStatus::Finished => {
                                        allow_err!(super::plugins::load_plugin(&id));
                                        allow_err!(super::ipc::load_plugin(id));
                                        push_install_event(&id, "finished");
                                    }
                                    InstallStatus::FailedCreating => {
                                        push_install_event(&id, "failed-creating");
                                    }
                                    InstallStatus::FailedDownloading => {
                                        push_install_event(&id, "failed-downloading");
                                    }
                                    InstallStatus::FailedInstalling => {
                                        push_install_event(&id, "failed-installing");
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {
                    }
                }
            }
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::main]
pub async fn start_ipc() {
    match crate::ipc::new_listener(IPC_PLUGIN_POSTFIX).await {
        Ok(mut incoming) => {
            while let Some(result) = incoming.next().await {
                match result {
                    Ok(stream) => {
                        log::debug!("Got new connection");
                        tokio::spawn(handle_conn(crate::ipc::Connection::new(stream)));
                    }
                    Err(err) => {
                        log::error!("Couldn't get plugin client: {:?}", err);
                    }
                }
            }
        }
        Err(err) => {
            log::error!("Failed to start plugin ipc server: {}", err);
        }
    }
}

// install process
pub(super) mod install {
    use super::IPC_PLUGIN_POSTFIX;
    use crate::{
        ipc::{connect, Data},
        plugin::ipc::{InstallStatus, Plugin},
    };
    use hbb_common::{allow_err, bail, log, tokio, ResultType};
    use std::{
        fs::File,
        io::{BufReader, BufWriter, Write},
        path::PathBuf,
    };
    use zip::ZipArchive;

    #[tokio::main(flavor = "current_thread")]
    async fn send_install_status(id: &str, status: InstallStatus) {
        allow_err!(_send_install_status(id, status).await);
    }

    async fn _send_install_status(id: &str, status: InstallStatus) -> ResultType<()> {
        let mut c = connect(1_000, IPC_PLUGIN_POSTFIX).await?;
        c.send(&Data::Plugin(Plugin::InstallStatus((
            id.to_string(),
            status,
        ))))
        .await?;
        Ok(())
    }

    fn download_to_file(url: &str, file: File) -> ResultType<()> {
        let resp = match reqwest::blocking::get(url) {
            Ok(resp) => resp,
            Err(e) => {
                bail!("get plugin from '{}', {}", url, e);
            }
        };

        if !resp.status().is_success() {
            bail!("get plugin from '{}', status code: {}", url, resp.status());
        }

        let mut writer = BufWriter::new(file);
        writer.write_all(resp.bytes()?.as_ref())?;
        Ok(())
    }

    fn download_file(id: &str, url: &str, filename: &PathBuf) -> bool {
        let file = match File::create(filename) {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to create plugin file: {}", e);
                send_install_status(id, InstallStatus::FailedCreating);
                return false;
            }
        };
        if let Err(e) = download_to_file(url, file) {
            log::error!("Failed to download plugin '{}', {}", id, e);
            send_install_status(id, InstallStatus::FailedDownloading);
            return false;
        }
        true
    }

    fn do_install_file(filename: &PathBuf, target_dir: &PathBuf) -> ResultType<()> {
        let mut zip = ZipArchive::new(BufReader::new(File::open(filename)?))?;
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            let file_path = target_dir.join(file.name());
            if file.name().ends_with("/") {
                std::fs::create_dir_all(&file_path)?;
            } else {
                if let Some(p) = file_path.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(&p)?;
                    }
                }
                let mut outfile = File::create(&file_path)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
        Ok(())
    }

    pub fn install_plugin(id: &str, url: &str) {
        let plugin_dir = match super::super::get_plugin_dir(id) {
            Ok(d) => d,
            Err(e) => {
                send_install_status(id, InstallStatus::FailedCreating);
                log::error!("Failed to get plugin dir: {}", e);
                return;
            }
        };
        if !plugin_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&plugin_dir) {
                send_install_status(id, InstallStatus::FailedCreating);
                log::error!("Failed to create plugin dir: {}", e);
                return;
            }
        }

        let filename = plugin_dir.join(format!("{}.zip", id));
        if !download_file(id, url, &filename) {
            return;
        }
        send_install_status(id, InstallStatus::Installing);
        if let Err(e) = do_install_file(&filename, &plugin_dir) {
            log::error!("Failed to install plugin: {}", e);
            send_install_status(id, InstallStatus::FailedInstalling);
            return;
        }
        send_install_status(id, InstallStatus::Finished);
    }
}
