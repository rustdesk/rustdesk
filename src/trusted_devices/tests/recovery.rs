use super::*;

pub(super) fn corrupt_controller_credentials_allow_reenrollment(
    credentials: &Credentials,
    peer_id: &str,
) {
    let token = credentials.load().unwrap().unwrap();
    let info = PeerConfig::load(peer_id).info;
    let mut saved: serde_json::Value =
        serde_json::from_str(&PeerConfig::get_trusted_device_v2(peer_id).unwrap()).unwrap();
    saved["ciphertext"] = serde_json::json!([]);
    let undecryptable = saved.to_string();
    saved["ciphertext"] = serde_json::json!(hbb_common::password_security::symmetric_crypt(
        &token[..TOKEN_BYTES - 1],
        true
    )
    .unwrap());
    for corrupt in ["{".to_owned(), undecryptable, saved.to_string()] {
        PeerConfig::set_trusted_device_v2(peer_id, corrupt, info.clone()).unwrap();
        assert_eq!(credentials.load().unwrap(), None);
        credentials.save(&token, info.clone()).unwrap();
        assert_eq!(credentials.load().unwrap(), Some(token.clone()));
    }
}

pub(super) async fn service_sync_preserves_grants_and_revocations() {
    let removed = begin_enrollment()
        .await
        .finish(DeviceInfo::default())
        .await
        .unwrap();
    let retained = begin_enrollment()
        .await
        .finish(DeviceInfo::default())
        .await
        .unwrap();
    let settings = Config::get();
    let previous = Config2::get_for_sync().unwrap();
    let id =
        serde_json::from_value(configuration::devices().await[0]["record_id"].clone()).unwrap();
    authority::request(Request::Remove(vec![id])).await.unwrap();
    let added = begin_enrollment()
        .await
        .finish(DeviceInfo::default())
        .await
        .unwrap();
    let current = Config2::get_for_sync().unwrap();

    // Explicit service sync replaces the older copy, including removals and enrollment.
    previous.sync_trusted_devices().unwrap();
    config::sync_config(settings.clone(), current.clone()).unwrap();
    assert!(!is_trusted(&removed).await);
    assert!(is_trusted(&retained).await);
    assert!(is_trusted(&added).await);
    // Installation imports this file, including the original grant deadlines.
    let imported: Config2 = hbb_common::config::load_path(Config2::file());
    assert_eq!(
        imported, current,
        "The service importer must receive current grants"
    );

    Config2::set_trusted_devices_v2(String::new()).unwrap();
    config::sync_config(settings.clone(), current).unwrap();
    super::super::initialize().unwrap();
    assert!(is_trusted(&retained).await);
    assert!(is_trusted(&added).await);

    authority::request(Request::Clear).await.unwrap();
    let cleared = Config2::get_for_sync().unwrap();
    previous.sync_trusted_devices().unwrap();
    config::sync_config(settings, cleared).unwrap();
    assert!(configuration::devices().await.is_empty());
    stopped_service_password_changes().await;
    #[cfg(windows)]
    unavailable_host_clears_local_trust_before_removal().await;
}

#[cfg(windows)]
async fn unavailable_host_clears_local_trust_before_removal() {
    let token = begin_enrollment()
        .await
        .finish(DeviceInfo::default())
        .await
        .unwrap();
    let previous = Config2::get_for_sync().unwrap();
    let settings = Config::get();
    let options = Config::get_options();
    config::refresh_from_server().await.unwrap();
    let imported: Config2 = hbb_common::config::load_path(Config2::file());
    assert_eq!(Config::get(), settings);
    assert_eq!(imported.options, options);
    assert!(configuration::devices().await.is_empty());
    previous.sync_trusted_devices().unwrap();
    config::sync_config(settings, imported).unwrap();
    assert!(!is_trusted(&token).await);
}

async fn stopped_service_password_changes() {
    let token = begin_enrollment()
        .await
        .finish(DeviceInfo::default())
        .await
        .unwrap();
    let previous = Config2::get_for_sync().unwrap();
    let options = Config::get_options();
    Config::set_option("stop-service".into(), "Y".into());
    assert!(
        config::update_auth(true, || config::set_password("stopped-service-password"))
            .await
            .unwrap()
    );
    config::snapshot()
        .unwrap()
        .verify_password(&proof("stopped-service-password", &[]))
        .unwrap();
    assert!(config::update_auth(true, || config::set_password(PASSWORD))
        .await
        .unwrap());
    let mut without_2fa = options.clone();
    without_2fa.remove("2fa");
    config::update_options(without_2fa).await.unwrap();
    config::update_options(options).await.unwrap();
    let current = Config2::get_for_sync().unwrap();
    previous.sync_trusted_devices().unwrap();
    config::sync_config(Config::get(), current).unwrap();
    assert!(
        !is_trusted(&token).await,
        "Synced offline edits must revoke trust even when the original password is restored"
    );
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
    let error = authority::begin(wrong_scope, None, None).err().unwrap();
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
    authority::request(Request::Clear).await.unwrap();
}

pub(super) async fn legacy_migration_preserves_revocation(hwid: &[u8], info: &DeviceInfo) {
    legacy_migration_at_capacity_preserves_expiry(hwid, info).await;
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

async fn legacy_migration_at_capacity_preserves_expiry(hwid: &[u8], info: &DeviceInfo) {
    let original = Config2::get_for_sync().unwrap();
    let mut options = Config::get_options();
    options.insert(keys::OPTION_TRUSTED_DEVICE_EXPIRATION.into(), "1".into());
    config::set_options(options).unwrap();
    let device = configuration::devices()
        .await
        .into_iter()
        .find(|device| device["id"] == info.id)
        .unwrap();
    let expiry = device["expires_at"].clone();
    for _ in 1..super::super::MAX_RECORDS {
        begin_enrollment()
            .await
            .finish(DeviceInfo::default())
            .await
            .unwrap();
    }
    let (attempt, valid, _) = host::begin(legacy_proof(hwid, info, true))
        .await
        .unwrap()
        .unwrap();
    assert!(valid);
    let token = attempt.unwrap().finish(info.clone()).await.unwrap();
    assert!(
        is_trusted(&token).await,
        "The issued migration token must survive eviction"
    );
    let devices = configuration::devices().await;
    assert_eq!(devices.len(), super::super::MAX_RECORDS);
    assert_eq!(
        devices
            .iter()
            .find(|device| device["id"] == info.id)
            .unwrap()["expires_at"],
        expiry
    );
    config::sync_config(Config::get(), original).unwrap();
}
