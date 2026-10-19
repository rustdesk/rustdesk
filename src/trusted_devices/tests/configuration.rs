use super::*;
use hbb_common::{password_security::decrypt_str_or_original, sodiumoxide::base64};

pub(super) async fn legacy_settings_survive_saving() {
    let path = Config2::file();
    let saved = fs::read_to_string(&path).unwrap();
    let legacy = format!("trusted_devices = 'legacy-v1-records'\n{saved}");
    fs::write(&path, legacy).unwrap();
    let (_, valid, _) = host::begin(proof(PASSWORD, &[])).await.unwrap().unwrap();
    Config::set_option("trusted-device-field-test".into(), "N".into());
    let after_save = fs::read_to_string(&path).unwrap();
    let management: serde_json::Value =
        serde_json::from_str(&super::super::devices().unwrap()).unwrap();
    fs::write(&path, saved).unwrap();
    assert!(after_save.contains("legacy-v1-records"));
    assert!(after_save.contains("trusted_devices_v2"));
    assert!(!valid, "Legacy records must not bypass TOTP");
    assert_eq!(management["legacy"], true);
}

pub(super) async fn devices() -> Vec<serde_json::Value> {
    let result: serde_json::Value =
        serde_json::from_str(&authority::manage(false).await.unwrap()).unwrap();
    serde_json::from_value(result["devices"].clone()).unwrap()
}

pub(super) async fn encrypted_store_preserves_grants(token: &[u8]) {
    let stored = Config2::get_trusted_devices_v2().unwrap();
    let (plaintext, encrypted, _) = decrypt_str_or_original(&stored, "00");
    assert!(encrypted, "Host grants must be encrypted at rest");
    Config2::set_trusted_devices_v2(plaintext.clone()).unwrap();
    super::super::initialize().unwrap();
    let migrated = Config2::get_trusted_devices_v2().unwrap();
    let (decrypted, encrypted, _) = decrypt_str_or_original(&migrated, "00");
    assert!(encrypted, "Existing plaintext grants must be migrated");
    assert_eq!(decrypted, plaintext);
    let (_, valid, _) = host::begin(proof(PASSWORD, token)).await.unwrap().unwrap();
    assert!(valid, "Migration must preserve existing trusted devices");

    let mut ciphertext = base64::decode(
        migrated.strip_prefix("00").unwrap(),
        base64::Variant::Original,
    )
    .unwrap();
    *ciphertext.last_mut().unwrap() ^= 1;
    let corrupted = format!(
        "00{}",
        base64::encode(&ciphertext, base64::Variant::Original)
    );
    Config2::set_trusted_devices_v2(corrupted.clone()).unwrap();
    let login = host::begin(proof(PASSWORD, token)).await;
    let after_read = Config2::get_trusted_devices_v2();
    Config2::set_trusted_devices_v2(migrated).unwrap();
    let (attempt, valid, _) = login.unwrap().unwrap();
    assert!(attempt.is_none());
    assert!(!valid, "Damaged ciphertext must not authorize a token");
    assert_eq!(after_read.unwrap(), corrupted);
}

pub(super) async fn expiration_changes_revoke_grants() {
    let original = Config::get_options();
    for days in ["1", "7", "0", "30"] {
        let (attempt, valid, _) = host::begin(proof(PASSWORD, &[])).await.unwrap().unwrap();
        assert!(!valid);
        let mut attempt = attempt.unwrap();
        attempt.enroll = true;
        let token = attempt.finish(DeviceInfo::default()).await.unwrap();
        let (pending, _, _) = host::begin(proof(PASSWORD, &[])).await.unwrap().unwrap();
        let mut pending = pending.unwrap();
        pending.enroll = true;
        let mut options = original.clone();
        options.insert(keys::OPTION_TRUSTED_DEVICE_EXPIRATION.into(), days.into());
        let affected = config::changes_authentication(&options);
        config::update_auth(affected, move || config::set_options(options))
            .await
            .unwrap();
        assert!(
            !host::begin(proof(PASSWORD, &token))
                .await
                .unwrap()
                .unwrap()
                .1
        );
        assert!(pending.finish(DeviceInfo::default()).await.is_err());
        assert!(devices().await.is_empty());
        assert_issued_expiry(days).await;
    }
    config::set_options(original).unwrap();
    assert_eq!(devices().await.len(), 1);
    authority::manage(true).await.unwrap();
}

pub(super) async fn renewal_preserves_authorization(token: &[u8]) {
    let (attempt, _, _) = host::begin(proof(PASSWORD, &[])).await.unwrap().unwrap();
    let mut attempt = attempt.unwrap();
    attempt.enroll = true;
    let other_token = attempt.finish(DeviceInfo::default()).await.unwrap();
    let now = hbb_common::get_time();
    {
        let storage = super::super::storage().unwrap();
        let mut grants = storage.read().unwrap();
        for record in &mut grants.records {
            record.info.expires_at = Some(now + super::super::policy::DAY_MILLIS);
        }
        storage.write(&grants).unwrap();
    }
    let before = devices().await;
    let ids: Vec<[u8; TOKEN_BYTES]> = before
        .iter()
        .map(|device| serde_json::from_value(device["record_id"].clone()).unwrap())
        .collect();
    authority::renew(vec![ids[0]]).await.unwrap();
    let after = devices().await;
    let expires_at = after[0]["expires_at"].as_i64().unwrap();
    assert!(expires_at >= now + LIFETIME_MILLIS);
    assert!(expires_at <= hbb_common::get_time() + LIFETIME_MILLIS);
    assert_eq!(after[0]["time"], before[0]["time"]);
    assert_eq!(after[1], before[1]);
    for token in [token, other_token.as_slice()] {
        let (_, valid, _) = host::begin(proof(PASSWORD, token)).await.unwrap().unwrap();
        assert!(valid);
    }
    renewal_rejects_expired_and_revoked(&ids, &after[1]).await;
}

async fn renewal_rejects_expired_and_revoked(
    ids: &[[u8; TOKEN_BYTES]],
    unchanged: &serde_json::Value,
) {
    {
        let storage = super::super::storage().unwrap();
        let mut grants = storage.read().unwrap();
        grants.records[0].info.expires_at = Some(hbb_common::get_time());
        storage.write(&grants).unwrap();
    }
    assert!(authority::renew(vec![ids[1], ids[0]]).await.is_err());
    assert_eq!(devices().await, vec![unchanged.clone()]);
    authority::manage(true).await.unwrap();
    assert!(authority::renew(vec![ids[1]]).await.is_err());
    assert!(devices().await.is_empty());
}

async fn assert_issued_expiry(days: &str) {
    let (attempt, _, _) = host::begin(proof(PASSWORD, &[])).await.unwrap().unwrap();
    let mut attempt = attempt.unwrap();
    attempt.enroll = true;
    attempt
        .finish(DeviceInfo {
            time: 1,
            expires_at: Some(i64::MAX),
            ..Default::default()
        })
        .await
        .unwrap();
    let listed = devices().await;
    let time = listed[0]["time"].as_i64().unwrap();
    let days: i64 = days.parse().unwrap();
    let expected = if days == 0 {
        0
    } else {
        time + days * super::super::policy::DAY_MILLIS
    };
    assert_ne!(time, 1);
    assert_eq!(listed[0]["expires_at"], expected);
    if days == 0 {
        let id = serde_json::from_value(listed[0]["record_id"].clone()).unwrap();
        assert!(authority::renew(vec![id]).await.is_err());
        assert_eq!(devices().await, listed);
    }
}

#[test]
fn explicit_expiry_is_enforced_and_never_expiring_grants_can_be_revoked() {
    for days in [0, 1, 7, 30] {
        let mut grants = Grants::default();
        grants.revoke(Some(SCOPE)).unwrap();
        let ticket = grants.begin(&SCOPE).unwrap();
        let verifier = [9; TOKEN_BYTES];
        let expires_at = super::super::policy::expires_at(NOW, days);
        grants
            .enroll_digest(
                &ticket,
                verifier,
                DeviceInfo {
                    time: NOW,
                    expires_at: Some(expires_at),
                    ..Default::default()
                },
            )
            .unwrap();
        if days == 0 {
            assert!(grants
                .verify_digest(&ticket, Some(&verifier), i64::MAX)
                .unwrap());
        } else {
            assert!(grants
                .verify_digest(&ticket, Some(&verifier), expires_at - 1)
                .unwrap());
            assert!(!grants
                .verify_digest(&ticket, Some(&verifier), expires_at)
                .unwrap());
        }
        grants.revoke(Some(SCOPE)).unwrap();
        assert!(grants.verify_digest(&ticket, Some(&verifier), NOW).is_err());
    }
}

pub(super) fn malformed_host_field_preserves_authentication() {
    let path = Config2::file();
    let saved = fs::read_to_string(&path).unwrap();
    let mut value: hbb_common::toml::Value = hbb_common::toml::from_str(&saved).unwrap();
    value.as_table_mut().unwrap().insert(
        "trusted_devices_v2".into(),
        hbb_common::toml::Value::Integer(123),
    );
    let malformed = hbb_common::toml::to_string(&value).unwrap();
    fs::write(&path, &malformed).unwrap();

    let loaded: Config2 = hbb_common::toml::from_str(&malformed).unwrap();
    let credential = Config2::get_trusted_devices_v2();
    Config::set_option("trusted-device-field-test".into(), "Y".into());
    let after_save = fs::read_to_string(&path).unwrap();
    let cleared = super::super::clear();
    let recovered = super::super::devices();
    fs::write(&path, saved).unwrap();

    assert_eq!(loaded.options.get("2fa"), Config::get_options().get("2fa"));
    assert!(credential.is_err());
    let after_save: Config2 = hbb_common::toml::from_str(&after_save).unwrap();
    assert_eq!(after_save.options["trusted-device-field-test"], "Y");
    assert_eq!(after_save.options.get("2fa"), loaded.options.get("2fa"));
    assert!(serde_json::to_value(&after_save)
        .unwrap()
        .get("trusted_devices_v2")
        .is_none());
    cleared.unwrap();
    let recovered: serde_json::Value = serde_json::from_str(&recovered.unwrap()).unwrap();
    assert_eq!(recovered["devices"], serde_json::json!([]));
}
