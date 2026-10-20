use super::*;

pub(super) async fn unknown_expiration_preserves_login_and_settings() {
    let original = Config::get_options();
    let key = keys::OPTION_TRUSTED_DEVICE_EXPIRATION;
    for layer in [
        None,
        Some(&*hbb_common::config::DEFAULT_SETTINGS),
        Some(&*hbb_common::config::OVERWRITE_SETTINGS),
    ] {
        if let Some(layer) = layer {
            layer.write().unwrap().insert(key.into(), "14".into());
        } else {
            Config::set_option(key.into(), "14".into());
        }
        let mut options = Config::get_options();
        options.insert("trusted-device-expiration-test".into(), "Y".into());
        let saved = config::set_options(options);
        let synced = config::set_config2(Config2::get());
        let snapshot = config::snapshot();
        let login = host::begin(proof(PASSWORD, &[])).await;
        if let Some(layer) = layer {
            layer.write().unwrap().remove(key);
        }
        saved.unwrap();
        synced.unwrap();
        assert_eq!(
            snapshot.unwrap().validity_days,
            super::super::policy::DEFAULT_DAYS
        );
        let stored: Config2 =
            hbb_common::toml::from_str(&fs::read_to_string(Config2::file()).unwrap()).unwrap();
        assert_eq!(stored.options["trusted-device-expiration-test"], "Y");
        let (_, valid, _) = login.unwrap().unwrap();
        assert!(!valid);
        config::set_options(original.clone()).unwrap();
    }
}

pub(super) async fn unusable_grant_scope_keeps_current_password_totp_available() {
    let attempt = begin_enrollment().await;
    let token = attempt.finish(DeviceInfo::default()).await.unwrap();
    let pending = begin_enrollment().await;
    config::import(|| Ok(())).unwrap();
    assert!(host::begin(proof("wrong-password", &token)).await.is_err());
    let (attempt, valid, totp) = host::begin(proof(PASSWORD, &token)).await.unwrap().unwrap();
    assert!(attempt.is_none());
    assert!(!valid);
    assert_eq!(totp.secret, vec![1; 20]);
    assert!(pending.finish(DeviceInfo::default()).await.is_err());

    let mut wrong_scope = config::snapshot().unwrap().scope;
    wrong_scope[0] ^= 1;
    let error = authority::handle(authority::Request::Begin {
        scope: wrong_scope,
        verifier: None,
    })
    .err()
    .unwrap();
    assert!(!error.is::<authority::Unavailable>());

    let path = Config2::file();
    let saved = fs::read_to_string(&path).unwrap();
    let mut changed = Config2::get();
    changed
        .options
        .insert("2fa".into(), "changed-secret".into());
    hbb_common::config::store_path(path.clone(), changed).unwrap();
    let login = host::begin(proof(PASSWORD, &token)).await;
    fs::write(&path, saved).unwrap();
    assert!(login.is_err());
    authority::manage(true).await.unwrap();
}
