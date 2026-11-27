use super::*;

const RECORD: &str = "reg_recovery";
const SETTING: &str = "ipc-recovery-test-setting";
const TIMEOUT_MS: u64 = 1_000;

fn assert_synced(record: Option<&str>) {
    for cfg in [Config2::get(), config::load_path(Config2::file())] {
        assert_eq!(cfg.options.get(RECORD).map(String::as_str), record);
        assert_eq!(cfg.options[SETTING], "gui");
    }
}

async fn check_writes(client: &mut ConnectionTmpl<ConnClient>, base: &Config2) -> ResultType<()> {
    for supplied in [None, Some(""), Some("active"), Some("old")] {
        let mut cfg = base.clone();
        if let Some(value) = supplied {
            cfg.options.insert(RECORD.into(), value.into());
        }
        let options = Data::Options(Some(cfg.options.clone()));
        let full = Data::SyncConfig(Some((Config::get(), cfg).into()));
        for host in [Some("active"), None] {
            Config::set_option(RECORD.into(), host.unwrap_or_default().into());
            for request in [&options, &full] {
                Config::set_option(SETTING.into(), "local".into());
                client.send(request).await?;
                let reply = client.next_timeout(TIMEOUT_MS).await?;
                assert!(matches!(
                    (request, reply),
                    (Data::Options(_), Some(Data::Options(None)))
                        | (Data::SyncConfig(_), Some(Data::SyncConfig(None)))
                ));
                assert_synced(host);
            }
        }
    }
    Ok(())
}

pub(crate) async fn ipc_preserves_reg_recovery() -> ResultType<()> {
    let listener = tokio::spawn(start_with_listener("", new_listener("").await?));
    let mut client = connect(TIMEOUT_MS, "").await?;
    crate::common::set_server_running(true);
    Config::set_option(RECORD.into(), "active".into());
    let mut options = get_options_(TIMEOUT_MS).await?;
    options.insert(SETTING.into(), "gui".into());
    tokio::task::spawn_blocking(move || set_options(options)).await??;
    client.send(&Data::SyncConfig(None)).await?;
    let Some(Data::SyncConfig(Some(configs))) = client.next_timeout(TIMEOUT_MS).await? else {
        bail!("expected full configuration");
    };
    let (config, snapshot) = *configs;
    assert!(!snapshot.options.contains_key(RECORD));
    for hosting in [true, false] {
        crate::common::set_server_running(hosting);
        let expected = (hosting || crate::platform::is_root()).then_some("active");
        assert!(!get_options_(TIMEOUT_MS).await?.contains_key(RECORD));
        assert_synced(expected);
        Config::set_option(RECORD.into(), "active".into());
        Config::set_option(SETTING.into(), "local".into());
        crate::trusted_devices::config::sync_config(config.clone(), snapshot.clone())?;
        assert_synced(expected);
    }
    check_writes(&mut client, &snapshot).await?;
    Config::set_option(RECORD.into(), String::new());
    Config::set_option(SETTING.into(), String::new());
    listener.abort();
    Ok(())
}
