use super::{
    authority, client::Credentials, config, host, random_token, token_verifier, DeviceInfo, Grants,
    LIFETIME_MILLIS, TOKEN_BYTES,
};
use base::config::keys;
use hbb_common::{config::Config, sodiumoxide::crypto::hash::sha256};
use std::{fs, path::PathBuf};

const PASSWORD: &str = "permanent-test-password";

pub(crate) fn initialize_config() {
    static INITIALIZED: std::sync::Once = std::sync::Once::new();
    INITIALIZED.call_once(|| {
        *hbb_common::config::APP_NAME.write().unwrap() =
            format!("RustDeskV2Test{}", uuid::Uuid::new_v4());
    });
}

fn proof(password: &str, token: &[u8]) -> host::LoginProof {
    let salt = Config::get_effective_permanent_password_salt();
    let challenge = "trusted-device-test".to_owned();
    let mut material = hbb_common::config::compute_permanent_password_h1(password, &salt).to_vec();
    material.extend_from_slice(challenge.as_bytes());
    host::LoginProof {
        password: sha256::hash(&material).0.to_vec(),
        salt,
        challenge,
        token: token.to_vec(),
    }
}

fn configure() -> std::collections::HashMap<String, String> {
    initialize_config();
    let totp = crate::auth_2fa::TOTPInfo {
        name: "test".into(),
        secret: vec![1; 20],
        digits: 6,
        created_at: 1,
    };
    assert!(config::set_password(PASSWORD).unwrap());
    let mut options = Config::get_options();
    options.insert("2fa".into(), totp.into_string().unwrap());
    options.insert(keys::OPTION_ENABLE_TRUSTED_DEVICES.into(), "Y".into());
    config::set_options(options.clone()).unwrap();
    options
}

#[hbb_common::tokio::test]
async fn password_and_configuration_revocation_gate_real_persisted_grants() {
    let mut options = configure();
    assert!(host::begin(proof("wrong-password", &[])).await.is_err());
    let (enrollment, valid, _) = host::begin(proof(PASSWORD, &[])).await.unwrap().unwrap();
    let mut enrollment = enrollment.unwrap();
    assert!(!valid);
    enrollment.enroll = true;
    let token = enrollment.finish(DeviceInfo::default()).await.unwrap();
    let (trusted, valid, _) = host::begin(proof(PASSWORD, &token)).await.unwrap().unwrap();
    let trusted = trusted.unwrap();
    assert!(valid);
    trusted.check().await.unwrap();
    let (pending, _, _) = host::begin(proof(PASSWORD, &[])).await.unwrap().unwrap();
    let mut pending = pending.unwrap();
    pending.enroll = true;
    options.insert(keys::OPTION_ENABLE_TRUSTED_DEVICES.into(), "N".into());
    config::set_options(options.clone()).unwrap();
    assert!(trusted.check().await.is_err());
    assert!(pending.finish(DeviceInfo::default()).await.is_err());
    options.insert(keys::OPTION_ENABLE_TRUSTED_DEVICES.into(), "Y".into());
    config::set_options(options).unwrap();
    assert!(
        !host::begin(proof(PASSWORD, &token))
            .await
            .unwrap()
            .unwrap()
            .1
    );
    service_operations_share_and_revoke_persisted_grants().await;
    unavailable_grants_keep_password_totp_available().await;
}

async fn unavailable_grants_keep_password_totp_available() {
    let path = Config::path("trusted-devices-v2.json");
    let saved = fs::read(&path).unwrap();
    fs::write(&path, b"invalid grant store").unwrap();
    assert!(host::begin(proof("wrong-password", &[])).await.is_err());
    let login = host::begin(proof(PASSWORD, &[])).await;
    fs::write(&path, saved).unwrap();
    let (attempt, valid, _) = login.unwrap().unwrap();
    assert!(attempt.is_none());
    assert!(!valid);
}

async fn service_operations_share_and_revoke_persisted_grants() {
    let (enrollment, _, _) = host::begin(proof(PASSWORD, &[])).await.unwrap().unwrap();
    let mut enrollment = enrollment.unwrap();
    enrollment.enroll = true;
    let token = enrollment.finish(DeviceInfo::default()).await.unwrap();
    let (pending, valid, _) = host::begin(proof(PASSWORD, &token)).await.unwrap().unwrap();
    let mut pending = pending.unwrap();
    assert!(valid);
    pending.enroll = true;
    let devices: Vec<serde_json::Value> =
        serde_json::from_str(&authority::manage(false).await.unwrap()).unwrap();
    let record_id: [u8; TOKEN_BYTES] =
        serde_json::from_value(devices[0]["record_id"].clone()).unwrap();
    let login = host::begin(proof(PASSWORD, &record_id)).await.unwrap();
    assert!(!login.unwrap().1);
    let (enrollment, _, _) = host::begin(proof(PASSWORD, &[])).await.unwrap().unwrap();
    let mut enrollment = enrollment.unwrap();
    enrollment.enroll = true;
    let other_token = enrollment.finish(DeviceInfo::default()).await.unwrap();
    authority::remove(vec![record_id]).await.unwrap();
    assert!(pending.check().await.is_err());
    let login = host::begin(proof(PASSWORD, &token)).await.unwrap();
    assert!(!login.unwrap().1);
    let (remaining, valid, _) = host::begin(proof(PASSWORD, &other_token))
        .await
        .unwrap()
        .unwrap();
    assert!(valid);
    let remaining = remaining.unwrap();
    remaining.check().await.unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<DeviceInfo>>(&authority::manage(false).await.unwrap())
            .unwrap()
            .len(),
        1
    );
    authority::manage(true).await.unwrap();
    assert!(remaining.check().await.is_err());
    assert!(pending.finish(DeviceInfo::default()).await.is_err());
    let login = host::begin(proof(PASSWORD, &other_token)).await.unwrap();
    assert!(!login.unwrap().1);
    assert_eq!(authority::manage(false).await.unwrap(), "[]");
}

const NOW: i64 = 1_700_000_000_000;
const SCOPE: [u8; TOKEN_BYTES] = [7; TOKEN_BYTES];

fn test_path() -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "rustdesk-trust-v2-{}",
            hbb_common::uuid::Uuid::new_v4()
        ))
        .join("credentials.json")
}

#[test]
fn only_original_tokens_satisfy_trust_and_use_does_not_refresh_expiry() {
    let mut grants = Grants::default();
    grants.revoke(Some(SCOPE)).unwrap();
    let ticket = grants.begin(&SCOPE).unwrap();
    let token = random_token().unwrap();
    grants
        .enroll_digest(
            &ticket,
            token_verifier(&token).unwrap(),
            DeviceInfo {
                time: NOW,
                ..Default::default()
            },
        )
        .unwrap();
    for candidate in [
        &token[..],
        &sha256::hash(&token).0[..],
        &token[..TOKEN_BYTES - 1],
        &[],
    ] {
        let accepted = grants
            .verify_digest(&ticket, token_verifier(candidate).as_ref(), NOW)
            .unwrap();
        assert_eq!(accepted, candidate == token);
    }
    let verifier = token_verifier(&token);
    assert!(grants
        .verify_digest(&ticket, verifier.as_ref(), NOW + LIFETIME_MILLIS - 1)
        .unwrap());
    assert!(!grants
        .verify_digest(&ticket, verifier.as_ref(), NOW + LIFETIME_MILLIS)
        .unwrap());
    assert!(grants.begin(&[8; TOKEN_BYTES]).is_err());
}

#[test]
fn controller_credentials_are_bound_and_storage_errors_surface() {
    initialize_config();
    let path = test_path();
    let credentials = Credentials::new(path.clone(), "server-a".into(), &[1; TOKEN_BYTES]).unwrap();
    let token = random_token().unwrap();
    credentials.save(&token).unwrap();
    assert_eq!(credentials.load().unwrap(), Some(token.to_vec()));
    for (namespace, key) in [
        ("server-b", [1; TOKEN_BYTES]),
        ("server-a", [2; TOKEN_BYTES]),
    ] {
        assert!(Credentials::new(path.clone(), namespace.into(), &key)
            .unwrap()
            .load()
            .unwrap()
            .is_none());
    }
    fs::write(&path, b"invalid credential file").unwrap();
    assert!(credentials.load().is_err());
    let directory = test_path();
    fs::create_dir_all(&directory).unwrap();
    let invalid = Credentials::new(directory, "server-a".into(), &[1; TOKEN_BYTES]).unwrap();
    assert!(invalid.save(&token).is_err());
}
