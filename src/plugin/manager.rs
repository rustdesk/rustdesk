// 1. Check update.
// 2. Install or uninstall.

use super::{desc::Meta as PluginMeta, ipc::InstallStatus, *};
use crate::flutter;
use hbb_common::{allow_err, bail, log, tokio, toml};
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::{
    collections::{HashMap, HashSet},
    fs::{read_to_string, remove_dir_all, OpenOptions},
    io::Write,
    sync::{Arc, Mutex},
};

const MSG_TO_UI_PLUGIN_MANAGER_LIST: &str = "plugin_list";
const MSG_TO_UI_PLUGIN_MANAGER_INSTALL: &str = "plugin_install";
const MSG_TO_UI_PLUGIN_MANAGER_UNINSTALL: &str = "plugin_uninstall";

const IPC_PLUGIN_POSTFIX: &str = "_plugin";

#[cfg(target_os = "windows")]
const PLUGIN_PLATFORM: &str = "windows";
#[cfg(target_os = "linux")]
const PLUGIN_PLATFORM: &str = "linux";
#[cfg(target_os = "macos")]
const PLUGIN_PLATFORM: &str = "macos";

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
    pub meta: PluginMeta,
    pub installed_version: String,
    pub invalid_reason: String,
}

static PLUGIN_SOURCE_LOCAL: &str = "local";

fn get_plugin_source_list() -> Vec<PluginSource> {
    // Only one source for now.
    vec![PluginSource {
        name: "rustdesk".to_string(),
        url: "https://raw.githubusercontent.com/fufesou/rustdesk-plugins/main".to_string(),
        description: "".to_string(),
    }]
}

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
                if let Ok(text) = resp.text() {
                    match toml::from_str::<ManagerMeta>(&text) {
                        Ok(manager_meta) => {
                            for meta in manager_meta.plugins.iter() {
                                if !meta
                                    .platforms
                                    .to_uppercase()
                                    .contains(&PLUGIN_PLATFORM.to_uppercase())
                                {
                                    continue;
                                }
                                plugins.insert(
                                    meta.id.clone(),
                                    PluginInfo {
                                        source: source.clone(),
                                        meta: meta.clone(),
                                        installed_version: "".to_string(),
                                        invalid_reason: "".to_string(),
                                    },
                                );
                            }
                        }
                        Err(e) => log::error!("Failed to parse plugin list from '{}', {}", url, e),
                    }
                }
            }
            Err(e) => log::error!("Failed to get plugin list from '{}', {}", url, e),
        }
    }
    plugins
}

fn send_plugin_list_event(plugins: &HashMap<String, PluginInfo>) {
    let mut plugin_list = plugins.values().collect::<Vec<_>>();
    plugin_list.sort_by(|a, b| a.meta.name.cmp(&b.meta.name));
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

    // A big read lock is needed to prevent race conditions.
    // Loading plugin list may be slow.
    // Users may call uninstall plugin in the middle.
    let plugin_infos = super::plugins::get_plugin_infos();
    let plugin_infos_read_lock = plugin_infos.read().unwrap();
    for (id, info) in plugin_infos_read_lock.iter() {
        if info.uninstalled {
            continue;
        }

        if let Some(p) = plugins.get_mut(id) {
            p.installed_version = info.desc.meta().version.clone();
            p.invalid_reason = "".to_string();
        } else {
            plugins.insert(
                id.to_string(),
                PluginInfo {
                    source: PluginSource {
                        name: PLUGIN_SOURCE_LOCAL.to_string(),
                        url: PLUGIN_SOURCE_LOCAL_DIR.to_string(),
                        description: "".to_string(),
                    },
                    meta: info.desc.meta().clone(),
                    installed_version: info.desc.meta().version.clone(),
                    invalid_reason: "".to_string(),
                },
            );
        }
    }
    send_plugin_list_event(&plugins);
    *plugin_info_lock = plugins;
}

#[cfg(target_os = "windows")]
fn elevate_install(
    plugin_id: &str,
    plugin_url: &str,
    same_plugin_exists: bool,
) -> ResultType<bool> {
    // to-do: Support args with space in quotes. 'arg 1' and "arg 2"
    let args = if same_plugin_exists {
        format!("--plugin-install {}", plugin_id)
    } else {
        format!("--plugin-install {} {}", plugin_id, plugin_url)
    };
    crate::platform::elevate(&args)
}

#[cfg(target_os = "linux")]
fn elevate_install(
    plugin_id: &str,
    plugin_url: &str,
    same_plugin_exists: bool,
) -> ResultType<bool> {
    let mut args = vec!["--plugin-install", plugin_id];
    if !same_plugin_exists {
        args.push(&plugin_url);
    }
    crate::platform::elevate(args)
}

#[cfg(target_os = "macos")]
fn elevate_install(
    plugin_id: &str,
    plugin_url: &str,
    same_plugin_exists: bool,
) -> ResultType<bool> {
    let mut args = vec!["--plugin-install", plugin_id];
    if !same_plugin_exists {
        args.push(&plugin_url);
    }
    crate::platform::elevate(args, "RustDesk wants to install then plugin")
}

#[inline]
#[cfg(target_os = "windows")]
fn elevate_uninstall(plugin_id: &str) -> ResultType<bool> {
    crate::platform::elevate(&format!("--plugin-uninstall {}", plugin_id))
}

#[inline]
#[cfg(target_os = "linux")]
fn elevate_uninstall(plugin_id: &str) -> ResultType<bool> {
    crate::platform::elevate(vec!["--plugin-uninstall", plugin_id])
}

#[inline]
#[cfg(target_os = "macos")]
fn elevate_uninstall(plugin_id: &str) -> ResultType<bool> {
    crate::platform::elevate(
        vec!["--plugin-uninstall", plugin_id],
        "RustDesk wants to uninstall the plugin",
    )
}

pub fn install_plugin(id: &str) -> ResultType<()> {
    match PLUGIN_INFO.lock().unwrap().get(id) {
        Some(plugin) => {
            let mut same_plugin_exists = false;
            if let Some(version) = super::plugins::get_version(id) {
                if version == plugin.meta.version {
                    same_plugin_exists = true;
                }
            }
            let plugin_url = format!(
                "{}/plugins/{}/{}/{}_{}.zip",
                plugin.source.url,
                plugin.meta.id,
                PLUGIN_PLATFORM,
                plugin.meta.id,
                plugin.meta.version
            );
            let allowed_install = elevate_install(id, &plugin_url, same_plugin_exists)?;
            if allowed_install && same_plugin_exists {
                super::ipc::load_plugin(id)?;
                super::plugins::load_plugin(id)?;
                super::plugins::mark_uninstalled(id, false);
                push_install_event(id, "finished");
            }
            Ok(())
        }
        None => {
            bail!("Plugin not found: {}", id);
        }
    }
}

fn get_uninstalled_plugins(uninstalled_plugin_set: &HashSet<String>) -> ResultType<Vec<String>> {
    let plugins_dir = super::get_plugins_dir()?;
    let mut plugins = Vec::new();
    if plugins_dir.exists() {
        for entry in std::fs::read_dir(plugins_dir)? {
            match entry {
                Ok(entry) => {
                    let plugin_dir = entry.path();
                    if plugin_dir.is_dir() {
                        if let Some(id) = plugin_dir.file_name().and_then(|n| n.to_str()) {
                            if uninstalled_plugin_set.contains(id) {
                                plugins.push(id.to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to read plugins dir entry, {}", e);
                }
            }
        }
    }
    Ok(plugins)
}

pub fn remove_uninstalled() -> ResultType<()> {
    let mut uninstalled_plugin_set = get_uninstall_id_set()?;
    for id in get_uninstalled_plugins(&uninstalled_plugin_set)?.iter() {
        super::config::remove(id as _);
        if let Ok(dir) = super::get_plugin_dir(id as _) {
            allow_err!(remove_dir_all(dir.clone()));
            if !dir.exists() {
                uninstalled_plugin_set.remove(id);
            }
        }
    }
    allow_err!(update_uninstall_id_set(uninstalled_plugin_set));
    Ok(())
}

pub fn uninstall_plugin(id: &str, called_by_ui: bool) {
    if called_by_ui {
        match elevate_uninstall(id) {
            Ok(true) => {
                if let Err(e) = super::ipc::uninstall_plugin(id) {
                    log::error!("Failed to uninstall plugin '{}': {}", id, e);
                    push_uninstall_event(id, "failed");
                    return;
                }
                super::plugins::unload_plugin(id);
                super::plugins::mark_uninstalled(id, true);
                super::config::remove(id);
                push_uninstall_event(id, "");
            }
            Ok(false) => {
                return;
            }
            Err(e) => {
                log::error!(
                    "Failed to uninstall plugin '{}', check permission error: {}",
                    id,
                    e
                );
                push_uninstall_event(id, "failed");
                return;
            }
        }
    }

    if super::is_server_running() {
        super::plugins::unload_plugin(&id);
    }
}

fn push_event(id: &str, r#type: &str, msg: &str) {
    let mut m = HashMap::new();
    m.insert("name", MSG_TO_UI_TYPE_PLUGIN_MANAGER);
    m.insert("id", id);
    m.insert(r#type, msg);
    if let Ok(event) = serde_json::to_string(&m) {
        let _res = flutter::push_global_event(flutter::APP_TYPE_MAIN, event.clone());
    }
}

#[inline]
fn push_uninstall_event(id: &str, msg: &str) {
    push_event(id, MSG_TO_UI_PLUGIN_MANAGER_UNINSTALL, msg);
}

#[inline]
fn push_install_event(id: &str, msg: &str) {
    push_event(id, MSG_TO_UI_PLUGIN_MANAGER_INSTALL, msg);
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
                                        allow_err!(super::ipc::load_plugin_async(id).await);
                                        std::thread::spawn(load_plugin_list);
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

pub(super) fn get_uninstall_id_set() -> ResultType<HashSet<String>> {
    let uninstall_file_path = super::get_uninstall_file_path()?;
    if !uninstall_file_path.exists() {
        std::fs::create_dir_all(&super::get_plugins_dir()?)?;
        return Ok(HashSet::new());
    }
    let s = read_to_string(uninstall_file_path)?;
    Ok(serde_json::from_str::<HashSet<String>>(&s)?)
}

fn update_uninstall_id_set(set: HashSet<String>) -> ResultType<()> {
    let content = serde_json::to_string(&set)?;
    let file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(super::get_uninstall_file_path()?)?;
    let mut writer = std::io::BufWriter::new(file);
    writer.write_all(content.as_bytes())?;
    Ok(())
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

    pub fn change_uninstall_plugin(id: &str, add: bool) {
        match super::get_uninstall_id_set() {
            Ok(mut set) => {
                if add {
                    set.insert(id.to_string());
                } else {
                    set.remove(id);
                }
                if let Err(e) = super::update_uninstall_id_set(set) {
                    log::error!("Failed to write uninstall list, {}", e);
                }
            }
            Err(e) => log::error!(
                "Failed to get plugins dir, unable to read uninstall list, {}",
                e
            ),
        }
    }

    pub fn install_plugin_with_url(id: &str, url: &str) {
        log::info!("Installing plugin '{}', url: {}", id, url);
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

        let filename = match url.rsplit('/').next() {
            Some(filename) => plugin_dir.join(filename),
            None => {
                send_install_status(id, InstallStatus::FailedDownloading);
                log::error!("Failed to download plugin file, invalid url: {}", url);
                return;
            }
        };

        let filename_to_remove = filename.clone();
        let _call_on_ret = crate::common::SimpleCallOnReturn {
            b: true,
            f: Box::new(move || {
                if let Err(e) = std::fs::remove_file(&filename_to_remove) {
                    log::error!("Failed to remove plugin file: {}", e);
                }
            }),
        };

        // download
        if !download_file(id, url, &filename) {
            return;
        }

        // install
        send_install_status(id, InstallStatus::Installing);
        if let Err(e) = do_install_file(&filename, &plugin_dir) {
            log::error!("Failed to install plugin: {}", e);
            send_install_status(id, InstallStatus::FailedInstalling);
            return;
        }

        // finished
        send_install_status(id, InstallStatus::Finished);
    }
}
