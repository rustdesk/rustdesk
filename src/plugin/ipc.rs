// to-do: Interdependence(This mod and crate::ipc) is not good practice here.
use crate::ipc::{connect, Connection, Data};
use hbb_common::{allow_err, log, tokio, ResultType};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InstallStatus {
    Downloading(u8),
    Installing,
    Finished,
    FailedCreating,
    FailedDownloading,
    FailedInstalling,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum Plugin {
    Config(String, String, Option<String>),
    ManagerConfig(String, Option<String>),
    ManagerPluginConfig(String, String, Option<String>),
    Load(String),
    Reload(String),
    InstallStatus((String, InstallStatus)),
    Uninstall(String),
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_config(id: &str, name: &str) -> ResultType<Option<String>> {
    get_config_async(id, name, 1_000).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_config(id: &str, name: &str, value: String) -> ResultType<()> {
    set_config_async(id, name, value).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_manager_config(name: &str) -> ResultType<Option<String>> {
    get_manager_config_async(name, 1_000).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_manager_config(name: &str, value: String) -> ResultType<()> {
    set_manager_config_async(name, value).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_manager_plugin_config(id: &str, name: &str) -> ResultType<Option<String>> {
    get_manager_plugin_config_async(id, name, 1_000).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_manager_plugin_config(id: &str, name: &str, value: String) -> ResultType<()> {
    set_manager_plugin_config_async(id, name, value).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn load_plugin(id: &str) -> ResultType<()> {
    load_plugin_async(id).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn reload_plugin(id: &str) -> ResultType<()> {
    reload_plugin_async(id).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn uninstall_plugin(id: &str) -> ResultType<()> {
    uninstall_plugin_async(id).await
}

async fn get_config_async(id: &str, name: &str, ms_timeout: u64) -> ResultType<Option<String>> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::Plugin(Plugin::Config(
        id.to_owned(),
        name.to_owned(),
        None,
    )))
    .await?;
    if let Some(Data::Plugin(Plugin::Config(id2, name2, value))) =
        c.next_timeout(ms_timeout).await?
    {
        if id == id2 && name == name2 {
            return Ok(value);
        }
    }
    return Ok(None);
}

async fn set_config_async(id: &str, name: &str, value: String) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(&Data::Plugin(Plugin::Config(
        id.to_owned(),
        name.to_owned(),
        Some(value),
    )))
    .await?;
    Ok(())
}

async fn get_manager_config_async(name: &str, ms_timeout: u64) -> ResultType<Option<String>> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::Plugin(Plugin::ManagerConfig(name.to_owned(), None)))
        .await?;
    if let Some(Data::Plugin(Plugin::ManagerConfig(name2, value))) =
        c.next_timeout(ms_timeout).await?
    {
        if name == name2 {
            return Ok(value);
        }
    }
    return Ok(None);
}

async fn set_manager_config_async(name: &str, value: String) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(&Data::Plugin(Plugin::ManagerConfig(
        name.to_owned(),
        Some(value),
    )))
    .await?;
    Ok(())
}

async fn get_manager_plugin_config_async(
    id: &str,
    name: &str,
    ms_timeout: u64,
) -> ResultType<Option<String>> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::Plugin(Plugin::ManagerPluginConfig(
        id.to_owned(),
        name.to_owned(),
        None,
    )))
    .await?;
    if let Some(Data::Plugin(Plugin::ManagerPluginConfig(id2, name2, value))) =
        c.next_timeout(ms_timeout).await?
    {
        if id == id2 && name == name2 {
            return Ok(value);
        }
    }
    return Ok(None);
}

async fn set_manager_plugin_config_async(id: &str, name: &str, value: String) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(&Data::Plugin(Plugin::ManagerPluginConfig(
        id.to_owned(),
        name.to_owned(),
        Some(value),
    )))
    .await?;
    Ok(())
}

pub async fn load_plugin_async(id: &str) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(&Data::Plugin(Plugin::Load(id.to_owned()))).await?;
    Ok(())
}

async fn reload_plugin_async(id: &str) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(&Data::Plugin(Plugin::Reload(id.to_owned()))).await?;
    Ok(())
}

async fn uninstall_plugin_async(id: &str) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(&Data::Plugin(Plugin::Uninstall(id.to_owned())))
        .await?;
    Ok(())
}

pub async fn handle_plugin(plugin: Plugin, stream: &mut Connection) {
    match plugin {
        Plugin::Config(id, name, value) => match value {
            None => {
                let value = super::SharedConfig::get(&id, &name);
                allow_err!(
                    stream
                        .send(&Data::Plugin(Plugin::Config(id, name, value)))
                        .await
                );
            }
            Some(value) => {
                allow_err!(super::SharedConfig::set(&id, &name, &value));
            }
        },
        Plugin::ManagerConfig(name, value) => match value {
            None => {
                let value = super::ManagerConfig::get_option(&name);
                allow_err!(
                    stream
                        .send(&Data::Plugin(Plugin::ManagerConfig(name, value)))
                        .await
                );
            }
            Some(value) => {
                super::ManagerConfig::set_option(&name, &value);
            }
        },
        Plugin::ManagerPluginConfig(id, name, value) => match value {
            None => {
                let value = super::ManagerConfig::get_plugin_option(&id, &name);
                allow_err!(
                    stream
                        .send(&Data::Plugin(Plugin::ManagerPluginConfig(id, name, value)))
                        .await
                );
            }
            Some(value) => {
                super::ManagerConfig::set_plugin_option(&id, &name, &value);
            }
        },
        Plugin::Load(id) => {
            allow_err!(super::load_plugin(&id));
        }
        Plugin::Reload(id) => {
            allow_err!(super::reload_plugin(&id));
        }
        Plugin::Uninstall(id) => {
            super::manager::uninstall_plugin(&id, false);
        }
        _ => {}
    }
}
