use super::*;

const RECORD: &str = "reg_recovery";
const SETTING: &str = "ipc-recovery-test-setting";
const TIMEOUT_MS: u64 = 1000;

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

    gui_save_after_recovery(incoming).await;
    Config::set_option(SETTING.into(), String::new());
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
    for saved in [Config2::get(), persisted] {
        assert_eq!(
            saved.options.get(RECORD).map(String::as_str),
            record,
            "{edit}"
        );
        assert_eq!(saved.options[SETTING], edit);
    }
}
