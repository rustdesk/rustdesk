use super::*;

const RECORD: &str = "reg_recovery";
const SETTING: &str = "ipc-recovery-test-setting";
const TIMEOUT_MS: u64 = 1_000;

async fn check_writes(client: &mut ConnectionTmpl<ConnClient>, full: bool) {
    for (host, supplied, edit) in [
        ("active", Some("active"), "identical"),
        ("active", Some("old"), "replace"),
        ("active", Some(""), "empty"),
        ("active", None, "omit"),
        ("", Some("active"), "replay"),
    ] {
        Config::set_option(RECORD.into(), host.into());
        let mut snapshot = Config2::get();
        snapshot.options.remove(RECORD);
        if let Some(value) = supplied {
            snapshot.options.insert(RECORD.into(), value.into());
        }
        snapshot.options.insert(SETTING.into(), edit.into());
        let request = if full {
            Data::SyncConfig(Some((Config::get(), snapshot).into()))
        } else {
            Data::Options(Some(snapshot.options))
        };
        client.send(&request).await.unwrap();
        let reply = client.next_timeout(TIMEOUT_MS).await.unwrap();
        assert!(matches!(
            (full, reply),
            (true, Some(Data::SyncConfig(None))) | (false, Some(Data::Options(None)))
        ));
        let saved = Config2::get().options;
        assert_eq!(
            saved.get(RECORD).map(String::as_str),
            (!host.is_empty()).then_some(host)
        );
        assert_eq!(saved[SETTING], edit);
    }
}

pub(crate) async fn ipc_preserves_reg_recovery() {
    let listener = tokio::spawn(start_with_listener("", new_listener("").await.unwrap()));
    let mut client = connect(TIMEOUT_MS, "").await.unwrap();
    for full in [false, true] {
        check_writes(&mut client, full).await;
    }
    Config::set_option(RECORD.into(), "active".into());
    crate::common::set_server_running(true);
    let mut options = get_options_(TIMEOUT_MS).await.unwrap();
    assert!(!options.contains_key(RECORD));
    options.insert(SETTING.into(), "gui".into());
    tokio::task::spawn_blocking(move || set_options(options))
        .await
        .unwrap()
        .unwrap();
    client.send(&Data::SyncConfig(None)).await.unwrap();
    let Some(Data::SyncConfig(Some(configs))) = client.next_timeout(TIMEOUT_MS).await.unwrap()
    else {
        panic!("expected full configuration");
    };
    let (config, settings) = *configs;
    assert!(!settings.options.contains_key(RECORD));
    assert_eq!(settings.options[SETTING], "gui");
    crate::trusted_devices::config::sync_config(config.clone(), settings.clone()).unwrap();
    assert_eq!(Config::get_option(RECORD), "active");
    crate::common::set_server_running(false);
    crate::trusted_devices::config::sync_config(config, settings).unwrap();
    assert!(!Config2::get().options.contains_key(RECORD));
    let stored: Config2 = config::load_path(Config2::file());
    assert!(!stored.options.contains_key(RECORD));
    assert_eq!(stored.options[SETTING], "gui");
    Config::set_option(SETTING.into(), String::new());
    listener.abort();
}
