use super::{
    authority::{self, Request, Response},
    client::Credentials,
    config, host, random_token, token_verifier, DeviceInfo, Grants, TOKEN_BYTES,
};
use base::config::keys;
use hbb_common::{
    config::{Config, Config2, PeerConfig},
    sodiumoxide::crypto::hash::sha256,
};
use std::fs;

mod configuration;
mod recovery;

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
        supports_v2: true,
        ..Default::default()
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

async fn begin_enrollment() -> host::Attempt {
    let (attempt, valid, _) = host::begin(proof(PASSWORD, &[])).await.unwrap().unwrap();
    assert!(!valid);
    let mut attempt = attempt.unwrap();
    attempt.enroll = true;
    attempt
}

async fn is_trusted(token: &[u8]) -> bool {
    let (_, valid, _) = host::begin(proof(PASSWORD, token)).await.unwrap().unwrap();
    valid
}

fn legacy_proof(hwid: &[u8], info: &DeviceInfo, supports_v2: bool) -> host::LoginProof {
    host::LoginProof {
        legacy: super::legacy::verifier(hwid, info).unwrap(),
        supports_v2,
        ..proof(PASSWORD, &[])
    }
}

#[hbb_common::tokio::test]
async fn password_and_configuration_revocation_gate_real_persisted_grants() {
    let mut options = configure();
    configuration::malformed_host_field_preserves_authentication();
    configuration::legacy_settings_survive_saving().await;
    configuration::expiration_changes_preserve_grants().await;
    recovery::unknown_expiration_preserves_login_and_settings().await;
    recovery::unusable_grant_scope_keeps_current_password_totp_available().await;
    assert!(configuration::devices().await.is_empty());
    assert!(host::begin(proof("wrong-password", &[])).await.is_err());
    let enrollment = begin_enrollment().await;
    let token = enrollment.finish(DeviceInfo::default()).await.unwrap();
    let (trusted, valid, _) = host::begin(proof(PASSWORD, &token)).await.unwrap().unwrap();
    let trusted = trusted.unwrap();
    assert!(valid);
    trusted.check().await.unwrap();
    configuration::encrypted_store_preserves_grants(&token).await;
    configuration::renewal_preserves_authorization(&token).await;
    let pending = begin_enrollment().await;
    options.insert(keys::OPTION_ENABLE_TRUSTED_DEVICES.into(), "N".into());
    let disabled = options.clone();
    config::update_auth(true, move || config::set_options(disabled))
        .await
        .unwrap();
    assert!(trusted.check().await.is_err());
    assert!(pending.finish(DeviceInfo::default()).await.is_err());
    options.insert(keys::OPTION_ENABLE_TRUSTED_DEVICES.into(), "Y".into());
    config::update_auth(true, move || config::set_options(options))
        .await
        .unwrap();
    assert!(!is_trusted(&token).await);
    service_operations_share_and_revoke_persisted_grants().await;
    recovery::service_sync_preserves_grants_and_revocations().await;
    unavailable_grants_keep_password_totp_available().await;
    password_write_failure_is_reported().await;
}

async fn password_write_failure_is_reported() {
    let result = config::update_auth(true, || {
        let path = Config::file();
        fs::rename(&path, path.with_extension("backup"))?;
        fs::create_dir(&path)?;
        config::set_password("changed-test-password")
    })
    .await;
    let path = Config::file();
    fs::remove_dir(&path).unwrap();
    fs::rename(path.with_extension("backup"), &path).unwrap();
    config::set_password(PASSWORD).unwrap();
    assert!(
        result.is_err(),
        "Password persistence failure must be reported"
    );
}

async fn unavailable_grants_keep_password_totp_available() {
    Config2::set_trusted_devices_v2("invalid grant store".into()).unwrap();
    assert!(host::begin(proof("wrong-password", &[])).await.is_err());
    let login = host::begin(proof(PASSWORD, &[])).await;
    let (attempt, valid, _) = login.unwrap().unwrap();
    assert!(attempt.is_some());
    assert!(!valid);
    assert!(configuration::devices().await.is_empty());
    let enrollment = begin_enrollment().await;
    let token = enrollment.finish(DeviceInfo::default()).await.unwrap();
    let path = Config2::file();
    fs::write(&path, b"\xff").unwrap();
    Config::set_option("trusted-device-test-option".into(), "N".into());
    let after_save: Config2 =
        hbb_common::toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(after_save.options["trusted-device-test-option"], "N");
    assert!(Config2::get_trusted_devices_v2().unwrap().is_empty());
    assert!(!is_trusted(&token).await);
}

async fn service_operations_share_and_revoke_persisted_grants() {
    let enrollment = begin_enrollment().await;
    let token = enrollment.finish(DeviceInfo::default()).await.unwrap();
    let synced = serde_json::to_value(Config2::get()).unwrap();
    assert!(
        synced["trusted_devices_v2"].as_str()
            == Some(Config2::get_trusted_devices_v2().unwrap().as_str()),
        "The service configuration snapshot must include newly enrolled grants"
    );
    let (pending, valid, _) = host::begin(proof(PASSWORD, &token)).await.unwrap().unwrap();
    let mut pending = pending.unwrap();
    assert!(valid);
    pending.enroll = true;
    let devices = configuration::devices().await;
    let record_id: [u8; TOKEN_BYTES] =
        serde_json::from_value(devices[0]["record_id"].clone()).unwrap();
    assert!(!is_trusted(&record_id).await);
    let enrollment = begin_enrollment().await;
    let other_token = enrollment.finish(DeviceInfo::default()).await.unwrap();
    assert!(matches!(
        authority::request(Request::Remove(vec![record_id]))
            .await
            .unwrap(),
        Response::Done
    ));
    assert!(pending.check().await.is_err());
    assert!(!is_trusted(&token).await);
    let (remaining, valid, _) = host::begin(proof(PASSWORD, &other_token))
        .await
        .unwrap()
        .unwrap();
    assert!(valid);
    let remaining = remaining.unwrap();
    remaining.check().await.unwrap();
    let stale_config: Config2 = hbb_common::config::load_path(Config2::file());
    assert_eq!(configuration::devices().await.len(), 1);
    authority::request(Request::Clear).await.unwrap();
    // Ordinary settings saves must not restore grants from a stale snapshot.
    config::set_config2(stale_config).unwrap();
    Config::set_option("trusted-device-test-option".into(), "Y".into());
    assert!(remaining.check().await.is_err());
    assert!(pending.finish(DeviceInfo::default()).await.is_err());
    assert!(!is_trusted(&other_token).await);
    assert!(configuration::devices().await.is_empty());
}

const NOW: i64 = 1_700_000_000_000;
const SCOPE: [u8; TOKEN_BYTES] = [7; TOKEN_BYTES];

#[test]
fn only_original_tokens_satisfy_trust_and_use_does_not_refresh_expiry() {
    let mut grants = Grants::default();
    grants.revoke(Some(SCOPE)).unwrap();
    let ticket = grants.begin(&SCOPE).unwrap();
    let token = random_token().unwrap();
    let expires_at = super::policy::expires_at(NOW, 90);
    grants
        .enroll_digest(
            &ticket,
            token_verifier(&token).unwrap(),
            DeviceInfo {
                time: NOW,
                expires_at: Some(expires_at),
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
        .verify_digest(&ticket, verifier.as_ref(), expires_at - 1)
        .unwrap());
    assert!(!grants
        .verify_digest(&ticket, verifier.as_ref(), expires_at)
        .unwrap());
    assert!(grants.begin(&[8; TOKEN_BYTES]).is_err());
}

#[test]
fn controller_credentials_are_bound_and_storage_errors_surface() {
    initialize_config();
    let peer_id = uuid::Uuid::new_v4().to_string();
    let path = Config::path(format!("peers/{peer_id}.toml"));
    let mut settings = PeerConfig::default();
    settings.password = vec![5; TOKEN_BYTES];
    settings.info.platform = "Windows".into();
    settings
        .options
        .insert("alias".into(), "trusted-peer".into());
    settings.store(&peer_id);
    let credentials =
        Credentials::new(peer_id.clone(), "server-a".into(), &[1; TOKEN_BYTES]).unwrap();
    let token = random_token().unwrap();
    credentials.save(&token, settings.info.clone()).unwrap();
    assert_eq!(credentials.load().unwrap(), Some(token.to_vec()));
    let loaded = PeerConfig::load(&peer_id);
    assert_eq!(loaded.password, settings.password);
    assert_eq!(loaded.options, settings.options);
    assert!(serde_json::to_value(&loaded)
        .unwrap()
        .get("trusted_device_v2")
        .is_none());
    settings.store(&peer_id);
    assert_eq!(credentials.load().unwrap(), Some(token.to_vec()));
    for (namespace, key) in [
        ("server-b", [1; TOKEN_BYTES]),
        ("server-a", [2; TOKEN_BYTES]),
    ] {
        assert!(Credentials::new(peer_id.clone(), namespace.into(), &key)
            .unwrap()
            .load()
            .unwrap()
            .is_none());
    }
    recovery::corrupt_controller_credentials_allow_reenrollment(&credentials, &peer_id);
    let corrupt = format!("{}\ninvalid = [\n", fs::read_to_string(&path).unwrap());
    for corrupt in [corrupt.as_bytes(), b"\xff"] {
        fs::write(&path, corrupt).unwrap();
        assert!(credentials.load().is_err());
        settings.store(&peer_id);
        assert_eq!(PeerConfig::load(&peer_id).options, settings.options);
        assert!(credentials.load().unwrap().is_none());
    }
    let blocked_peer = uuid::Uuid::new_v4().to_string();
    fs::create_dir_all(Config::path(format!("peers/{blocked_peer}.toml"))).unwrap();
    let invalid = Credentials::new(blocked_peer, "server-a".into(), &[1; TOKEN_BYTES]).unwrap();
    assert!(invalid.save(&token, settings.info).is_err());
}
