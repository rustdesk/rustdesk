use super::*;

const RECORD: &str = "reg_recovery";
const SETTING: &str = "ipc-recovery-test-setting";
const TIMEOUT_MS: u64 = 1000;
const CHILD_TIMEOUT_MS: u64 = 10_000;
const CHILD_PROFILE: &str = "RUSTDESK_TEST_SHARED_IPC_PROFILE";
const HOST_TEST: &str =
    "trusted_devices::tests::password_and_configuration_revocation_gate_real_persisted_grants";

// Called from the serialized configuration tests, after they select a fresh app profile.
pub(crate) async fn ipc_preserves_reg_recovery() {
    let mut incoming = new_listener("").await.unwrap();
    let mut replies = connect(TIMEOUT_MS, "").await.unwrap();
    let mut receiver = Connection::new(incoming.next().await.unwrap().unwrap());
    for full_config in [false, true] {
        for (stored, returned, edit) in [
            ("current", Some("current"), "identical"),
            ("current", Some("previous"), "changed"),
            ("current", None, "omitted"),
            ("current", Some(""), "empty"),
            ("", Some("previous"), "absent"),
        ] {
            Config::set_option(RECORD.into(), stored.into());
            let mut snapshot = Config2::get();
            if let Some(value) = returned {
                snapshot.options.insert(RECORD.into(), value.into());
            } else {
                snapshot.options.remove(RECORD);
            }
            snapshot.options.insert(SETTING.into(), edit.into());
            let update = if full_config {
                Data::SyncConfig(Some((Config::get(), snapshot).into()))
            } else {
                Data::Options(Some(snapshot.options))
            };
            handle(update, &mut receiver).await;
            let reply = replies.next_timeout(TIMEOUT_MS).await.unwrap();
            assert!(matches!(
                (full_config, reply),
                (true, Some(Data::SyncConfig(None))) | (false, Some(Data::Options(None)))
            ));
            assert_saved((!stored.is_empty()).then_some(stored), edit);
        }
    }

    shared_profile_save(&mut incoming).await;
    gui_save_after_recovery(incoming).await;
    sync_profile_options().await;
}

async fn sync_profile_options() {
    let mut service = Config2::get();
    let mut stale = service.clone();
    stale.options.insert(RECORD.into(), "previous".into());
    ipc_windows::ConfigPeer::SharedProfile
        .sync_config(Config::get(), stale)
        .unwrap();
    assert_saved(None, "gui");
    service.options.insert(RECORD.into(), "host".into());
    service
        .options
        .insert(SETTING.into(), "options-sync".into());
    Config::set_option(RECORD.into(), "previous".into());
    let mut options = service.options.clone();
    ipc_windows::ConfigPeer::SeparateProfile
        .mirror_options(&mut options)
        .await
        .unwrap();
    assert_saved(None, "options-sync");
    assert!(!options.contains_key(RECORD));
    Config::set_option(RECORD.into(), "previous".into());
    service.options.insert(SETTING.into(), "full-sync".into());
    ipc_windows::ConfigPeer::SeparateProfile
        .sync_config(Config::get(), service)
        .unwrap();
    assert_saved(None, "full-sync");
    let cleared = std::fs::read(Config2::file()).unwrap();
    let mut options = Config::get_options();
    options.insert(SETTING.into(), "offline".into());
    Config::set_option(RECORD.into(), "previous".into());
    // Another process cleared recovery after this process last cached its options.
    std::fs::write(Config2::file(), cleared).unwrap();
    tokio::task::spawn_blocking(move || set_options(options))
        .await
        .unwrap()
        .unwrap();
    assert_saved(None, "offline");
    Config::set_option(SETTING.into(), String::new());
}

#[test]
fn separate_profile_config_replies_omit_only_recovery() {
    let mut config = Config2::default();
    config.options = HashMap::from([
        (RECORD.into(), "host".into()),
        (SETTING.into(), "edit".into()),
    ]);
    for reply in [
        Data::Options(Some(config.options.clone())),
        Data::SyncConfig(Some((Config::default(), config).into())),
    ] {
        assert!(config_reply(reply.clone(), || bail!("unknown client profile")).is_err());
        let filtered = match config_reply(reply, || Ok(false)).unwrap() {
            Data::Options(Some(options)) => options,
            Data::SyncConfig(Some(configs)) => configs.1.options,
            _ => panic!("expected a configuration snapshot"),
        };
        assert_eq!(filtered, HashMap::from([(SETTING.into(), "edit".into())]));
    }
}

pub(crate) async fn shared_profile_client() -> bool {
    let Ok(profile) = std::env::var(CHILD_PROFILE) else {
        return false;
    };
    assert!(profile.starts_with("RustDeskV2Test"));
    *config::APP_NAME.write().unwrap() = profile;
    assert!(!Config2::get().options.contains_key(RECORD));
    for (record, edit) in [(Some("current"), "shared-on"), (None, "shared-off")] {
        let mut options = get_options_(TIMEOUT_MS).await.unwrap();
        assert_eq!(options.get(RECORD).map(String::as_str), record);
        options.insert(SETTING.into(), edit.into());
        tokio::task::spawn_blocking(move || set_options(options))
            .await
            .unwrap()
            .unwrap();
        assert_saved(record, edit);
    }
    true
}

async fn shared_profile_save(incoming: &mut Incoming) {
    let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", HOST_TEST])
        .env(CHILD_PROFILE, config::APP_NAME.read().unwrap().as_str())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    for record in ["current", ""] {
        for read in [true, false] {
            let socket = timeout(CHILD_TIMEOUT_MS, incoming.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let mut conn = Connection::new(socket);
            let request = conn.next_timeout(TIMEOUT_MS).await.unwrap().unwrap();
            assert_eq!(matches!(request, Data::Options(None)), read);
            if read {
                Config::set_option(RECORD.into(), record.into());
            }
            handle(request, &mut conn).await;
        }
    }
    assert!(timeout(CHILD_TIMEOUT_MS, child.wait())
        .await
        .unwrap()
        .unwrap()
        .success());
    assert_saved(None, "shared-off");
}

async fn gui_save_after_recovery(incoming: Incoming) {
    let listener = tokio::spawn(start_with_listener("", incoming));
    Config::set_option(RECORD.into(), "current".into());
    let mut cached = get_options_(TIMEOUT_MS).await.unwrap();
    assert_eq!(cached[RECORD], "current");
    Config::set_option(RECORD.into(), String::new());
    cached.insert(SETTING.into(), "gui".into());
    tokio::task::spawn_blocking(move || set_options(cached))
        .await
        .unwrap()
        .unwrap();
    assert_saved(None, "gui");
    listener.abort();
}

fn assert_saved(record: Option<&str>, edit: &str) {
    let file = std::fs::read_to_string(Config2::file()).unwrap();
    let persisted: Config2 = hbb_common::toml::from_str(&file).unwrap();
    for saved in [persisted, Config2::get()] {
        assert_eq!(
            saved.options.get(RECORD).map(String::as_str),
            record,
            "{edit}"
        );
        assert_eq!(saved.options[SETTING], edit);
    }
}
