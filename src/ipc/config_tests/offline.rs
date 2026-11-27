use super::*;

pub(super) async fn save() {
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

    let mut offline = Config2::get();
    offline
        .options
        .insert(SETTING.into(), "other-process".into());
    config::store_path(Config2::file(), offline).unwrap();
    const SINGLE: &str = "offline-single-setting";
    for value in ["", "saved", ""] {
        let saved = tokio::task::spawn_blocking(move || try_set_option(SINGLE, value))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved[SETTING], "other-process");
        assert_eq!(
            saved.get(SINGLE).map(String::as_str),
            (!value.is_empty()).then_some(value)
        );
        assert_saved(None, "other-process");
    }
    Config::set_stored_option(RECORD, "new".into()).unwrap();
    tokio::task::spawn_blocking(|| try_set_option(RECORD, "previous"))
        .await
        .unwrap()
        .unwrap();
    assert_saved(Some("new"), "other-process");
    let expiration = keys::OPTION_TRUSTED_DEVICE_EXPIRATION;
    assert!(
        tokio::task::spawn_blocking(move || try_set_option(expiration, "7"))
            .await
            .unwrap()
            .is_err()
    );
    assert!(Config::get_option(expiration).is_empty());
    repair().await;
    Config::set_option(SETTING.into(), String::new());
}

async fn repair() {
    Config::set_nat_type(2);
    const PIN: &str = "ipc-test-pin";
    Config::set_unlock_pin(PIN);
    std::fs::write(Config2::file(), "[").unwrap();
    let mut options = Config::get_options();
    options.insert(SETTING.into(), "offline-repaired".into());
    tokio::task::spawn_blocking(move || set_options(options))
        .await
        .unwrap()
        .unwrap();
    assert_saved(None, "offline-repaired");
    let text = std::fs::read_to_string(Config2::file()).unwrap();
    let stored: hbb_common::toml::Value = hbb_common::toml::from_str(&text).unwrap();
    assert_eq!(stored["nat_type"].as_integer(), Some(2));
    let pin = stored["unlock_pin"].as_str().unwrap();
    assert_ne!(pin, PIN);
    assert_eq!(
        hbb_common::password_security::decrypt_str_or_original(pin, "00").0,
        PIN
    );

    Config::set_stored_option(RECORD, "current".into()).unwrap();
    std::fs::write(Config2::file(), [0xff]).unwrap();
    Config::set_stored_option(RECORD, String::new()).unwrap();
    assert_saved(None, "offline-repaired");
    Config::set_unlock_pin("");
}
