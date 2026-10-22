use super::*;

pub(super) async fn service_sync_preserves_grants_and_revocations() {
    let token = begin_enrollment()
        .await
        .finish(DeviceInfo::default())
        .await
        .unwrap();
    let other = begin_enrollment()
        .await
        .finish(DeviceInfo::default())
        .await
        .unwrap();
    let settings = Config::get();
    // Two retained user sessions downloaded this state before either revocation.
    let user_a = Config2::get_for_sync().unwrap();
    let user_b = user_a.clone();
    let user_path = Config2::file().with_extension("user.toml");
    hbb_common::config::store_path(user_path.clone(), user_a.clone()).unwrap();
    let install_path = Config2::file().with_extension("install.toml");
    super::super::service::copy_config(&user_path, &install_path).unwrap();
    let installed: Config2 = hbb_common::config::load_path(install_path.clone());
    assert!(installed == user_a);
    let devices = configuration::devices().await;
    let id = serde_json::from_value(devices[0]["record_id"].clone()).unwrap();
    authority::request(Request::Remove(vec![id])).await.unwrap();
    config::sync_config(settings.clone(), user_a).unwrap();
    super::super::service::copy_config(&user_path, &Config2::file()).unwrap();
    assert!(!is_trusted(&token).await);
    assert!(is_trusted(&other).await);
    assert!(authority::request(Request::Renew(vec![id])).await.is_err());

    authority::manage(true).await.unwrap();
    config::sync_config(settings, user_b).unwrap();
    super::super::service::copy_config(&user_path, &Config2::file()).unwrap();
    super::super::initialize().unwrap();
    assert!(!is_trusted(&token).await);
    assert!(!is_trusted(&other).await);
    assert!(configuration::devices().await.is_empty());
    fs::remove_file(user_path).unwrap();
    fs::remove_file(install_path).unwrap();
    stopped_service_password_changes().await;
}

async fn stopped_service_password_changes() {
    use super::super::service;
    let token = begin_enrollment()
        .await
        .finish(DeviceInfo::default())
        .await
        .unwrap();
    let install_path = Config2::file().with_extension("stopped-service-root.toml");
    hbb_common::config::store_path(install_path.clone(), Config2::get_for_sync().unwrap()).unwrap();
    let options = Config::get_options();
    let mut without_2fa = options.clone();
    without_2fa.remove("2fa");
    config::set_options(without_2fa).unwrap();
    Config2::set_trusted_devices_v2(String::new()).unwrap();
    assert!(
        service::update_stopped_auth(|| config::set_password("stopped-service-password")).unwrap()
    );
    config::snapshot()
        .unwrap()
        .verify_password(&proof("stopped-service-password", &[]))
        .unwrap();
    let earlier = service::pending_revocation().unwrap().unwrap();
    assert!(service::update_stopped_auth(|| config::set_password(PASSWORD)).unwrap());
    service::update_stopped_auth(|| config::set_options(options)).unwrap();
    service::acknowledge_revocation(&earlier).unwrap();
    service::copy_config(&Config2::file(), &install_path).unwrap();
    let installed: Config2 = hbb_common::config::load_path(install_path.clone());
    installed.sync_trusted_devices().unwrap();
    assert!(
        !is_trusted(&token).await,
        "Offline password changes must revoke root grants even when the original password is restored"
    );
    service::acknowledge_revocation(&service::pending_revocation().unwrap().unwrap()).unwrap();
    fs::remove_file(install_path).unwrap();
}

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
    {
        let storage = super::super::storage().unwrap();
        let mut grants = Grants::default();
        grants.revoke(None).unwrap();
        storage.write(&grants).unwrap();
    }
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
        legacy: None,
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

pub(super) async fn legacy_migration_preserves_revocation(hwid: &[u8], info: &DeviceInfo) {
    let (old, valid, _) = host::begin(legacy_proof(hwid, info, false))
        .await
        .unwrap()
        .unwrap();
    assert!(valid);
    assert!(old.unwrap().finish(info.clone()).await.unwrap().is_empty());
    let in_flight = host::begin(legacy_proof(hwid, info, true))
        .await
        .unwrap()
        .unwrap()
        .0
        .unwrap();
    let mut tokens = Vec::new();
    for _ in 0..2 {
        let (attempt, valid, _) = host::begin(legacy_proof(hwid, info, true))
            .await
            .unwrap()
            .unwrap();
        assert!(valid);
        tokens.push(attempt.unwrap().finish(info.clone()).await.unwrap());
    }
    for token in &tokens {
        assert!(is_trusted(token).await);
    }
    in_flight.check().await.unwrap();
    tokens.push(in_flight.finish(info.clone()).await.unwrap());
    let (attempt, valid, _) = host::begin(legacy_proof(hwid, info, false))
        .await
        .unwrap()
        .unwrap();
    assert!(!valid && attempt.is_none());
    let listed = configuration::devices().await;
    assert_eq!(
        listed.len(),
        2,
        "Migration must keep a single management entry"
    );
    let device = listed
        .iter()
        .find(|device| device["id"] == info.id)
        .unwrap();
    assert_eq!(device["expires_at"], 0);
    let id = serde_json::from_value(device["record_id"].clone()).unwrap();
    super::super::remove(&[id]).unwrap();
    for token in tokens {
        assert!(!is_trusted(&token).await);
    }
}
