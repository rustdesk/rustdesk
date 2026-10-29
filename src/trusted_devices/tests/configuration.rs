use super::*;
use hbb_common::{
    password_security::{decrypt_str_or_original, encrypt_str_or_original},
    sodiumoxide::base64,
};

pub(super) async fn legacy_settings_survive_saving() {
    let existing = begin_enrollment()
        .await
        .finish(DeviceInfo::default())
        .await
        .unwrap();
    let now = hbb_common::get_time();
    let day = super::super::policy::DAY_MILLIS;
    let info = DeviceInfo {
        time: now - 7 * day,
        id: "legacy-controller".into(),
        ..Default::default()
    };
    let hwid = b"legacy-hwid";
    let encrypted = legacy_fixture(hwid, &info);
    let path = Config2::file();
    let saved = format!(
        "trusted_devices = '{encrypted}'\n{}",
        fs::read_to_string(&path).unwrap()
    );
    fs::write(&path, saved).unwrap();
    Config::set_option("trusted-device-field-test".into(), "N".into());
    assert!(fs::read_to_string(&path).unwrap().contains(&encrypted));
    let listed = devices().await;
    assert_eq!(listed.len(), 2);
    let device = listed
        .iter()
        .find(|device| device["id"] == info.id)
        .unwrap();
    let expiry = device["expires_at"].as_i64().unwrap();
    assert_eq!(expiry, info.time + 90 * day);
    assert!(Config2::get_trusted_device_stores().unwrap().1.is_empty());
    let original = Config::get_options();
    let mut options = original.clone();
    options.insert(keys::OPTION_TRUSTED_DEVICE_EXPIRATION.into(), "0".into());
    config::set_options(options).unwrap();
    super::super::initialize().unwrap();
    assert!(devices()
        .await
        .iter()
        .all(|device| device["expires_at"] == 0));
    recovery::legacy_migration_preserves_revocation(hwid, &info).await;
    assert!(is_trusted(&existing).await);
    authority::request(Request::Clear).await.unwrap();
    config::set_options(original).unwrap();
}

fn legacy_fixture(hwid: &[u8], info: &DeviceInfo) -> String {
    let mut legacy = serde_json::to_value(info).unwrap();
    legacy["hwid"] = serde_json::json!(hwid);
    let mut expired = legacy.clone();
    expired["time"] = serde_json::json!(info.time - 90 * super::super::policy::DAY_MILLIS);
    expired["id"] = serde_json::json!("expired-controller");
    let plaintext = serde_json::to_string(&vec![legacy, expired]).unwrap();
    encrypt_str_or_original(&plaintext, "00", plaintext.len())
}

pub(super) async fn devices() -> Vec<serde_json::Value> {
    let Response::Devices(devices) = authority::request(Request::List).await.unwrap() else {
        panic!("Expected a trusted-device list");
    };
    let result: serde_json::Value = serde_json::from_str(&devices).unwrap();
    serde_json::from_value(result["devices"].clone()).unwrap()
}

pub(super) async fn encrypted_store_preserves_grants(token: &[u8]) {
    let stored = Config2::get_trusted_devices_v2().unwrap();
    let (plaintext, encrypted, _) = decrypt_str_or_original(&stored, "00");
    assert!(encrypted, "Host grants must be encrypted at rest");
    Config2::set_trusted_devices_v2(plaintext).unwrap();
    super::super::initialize().unwrap();
    assert!(!is_trusted(token).await);
    Config2::set_trusted_devices_v2(stored.clone()).unwrap();
    assert!(is_trusted(token).await);

    let mut ciphertext = base64::decode(
        stored.strip_prefix("00").unwrap(),
        base64::Variant::Original,
    )
    .unwrap();
    *ciphertext.last_mut().unwrap() ^= 1;
    let corrupted = format!(
        "00{}",
        base64::encode(&ciphertext, base64::Variant::Original)
    );
    Config2::set_trusted_devices_v2(corrupted).unwrap();
    let (attempt, valid, _) = host::begin(proof(PASSWORD, token)).await.unwrap().unwrap();
    assert!(!valid, "Damaged ciphertext must not authorize a token");
    let mut attempt = attempt.expect("A damaged grant store must allow fresh enrollment");
    attempt.enroll = true;
    let replacement = attempt.finish(DeviceInfo::default()).await.unwrap();
    assert!(is_trusted(&replacement).await);
    assert!(!is_trusted(token).await);
    Config2::set_trusted_devices_v2(stored).unwrap();
}

pub(super) async fn expiration_changes_preserve_grants() {
    use super::super::policy::expires_at;
    let original = Config::get_options();
    for (days, remaining) in [
        ("7", 21),
        ("30", 3),
        ("0", 3),
        ("30", 0),
        ("90", 3),
        ("1", 21),
    ] {
        let attempt = begin_enrollment().await;
        let token = attempt.finish(DeviceInfo::default()).await.unwrap();
        let pending = begin_enrollment().await;
        let now = hbb_common::get_time();
        let previous = expires_at(now, remaining);
        {
            let storage = super::super::storage().unwrap();
            let mut grants = storage.read().unwrap();
            grants.records[0].info.expires_at = Some(previous);
            storage.write(&grants).unwrap();
        }
        let mut options = original.clone();
        options.insert(keys::OPTION_TRUSTED_DEVICE_EXPIRATION.into(), days.into());
        if days == "90" {
            let mut value = Config2::get();
            value.options = options;
            config::set_config2(value).unwrap();
        } else {
            config::update_options(options).await.unwrap();
        }
        assert!(is_trusted(&token).await);
        let expiry = devices().await[0]["expires_at"].as_i64().unwrap();
        if days == "0" {
            assert_eq!(expiry, 0);
        } else if remaining != 0 && remaining <= days.parse::<u32>().unwrap() {
            assert_eq!(expiry, previous);
        } else {
            assert!(expiry >= expires_at(now, days.parse().unwrap()));
            assert!(expiry <= expires_at(hbb_common::get_time(), days.parse().unwrap()));
        }
        pending.finish(DeviceInfo::default()).await.unwrap();
        authority::request(Request::Clear).await.unwrap();
        assert_issued_expiry(days).await;
        authority::request(Request::Clear).await.unwrap();
    }
    config::set_options(original).unwrap();
}

pub(super) async fn renewal_preserves_authorization(token: &[u8]) {
    let attempt = begin_enrollment().await;
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
    assert!(matches!(
        authority::request(Request::Renew(vec![ids[0]]))
            .await
            .unwrap(),
        Response::Done
    ));
    let after = devices().await;
    let expires_at = after[0]["expires_at"].as_i64().unwrap();
    assert!(expires_at >= super::super::policy::expires_at(now, 90));
    assert!(expires_at <= super::super::policy::expires_at(hbb_common::get_time(), 90));
    assert_eq!(after[0]["time"], before[0]["time"]);
    assert_eq!(after[1], before[1]);
    for token in [token, other_token.as_slice()] {
        assert!(is_trusted(token).await);
    }
    authority::request(Request::Clear).await.unwrap();
    assert!(authority::request(Request::Renew(vec![ids[1]]))
        .await
        .is_err());
    assert!(devices().await.is_empty());
}

async fn assert_issued_expiry(days: &str) {
    let attempt = begin_enrollment().await;
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
        assert!(authority::request(Request::Renew(vec![id])).await.is_err());
        assert_eq!(devices().await, listed);
    }
}

#[test]
fn explicit_expiry_is_enforced_and_never_expiring_grants_can_be_revoked() {
    for days in [0, 1, 7, 30, 90] {
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

    let credential = Config2::get_trusted_devices_v2();
    Config::set_option("trusted-device-field-test".into(), "Y".into());
    let after_save = fs::read_to_string(&path).unwrap();
    fs::write(&path, saved).unwrap();

    assert!(credential.is_err());
    let after_save: Config2 = hbb_common::toml::from_str(&after_save).unwrap();
    assert_eq!(after_save.options["trusted-device-field-test"], "Y");
    assert_eq!(
        after_save.options.get("2fa"),
        Config::get_options().get("2fa")
    );
    assert!(serde_json::to_value(&after_save)
        .unwrap()
        .get("trusted_devices_v2")
        .is_none());
}
