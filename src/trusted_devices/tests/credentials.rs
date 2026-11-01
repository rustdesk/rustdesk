use super::*;
use hbb_common::password_security::symmetric_crypt;

#[test]
fn id_credentials_saved_before_direct_ip_support_remain_readable() {
    const NAMESPACE: &str = "[\"rs.example.com\"]";
    const PUBLIC_KEY: [u8; TOKEN_BYTES] = [1; TOKEN_BYTES];

    initialize_config();
    let peer_id = uuid::Uuid::new_v4().to_string();
    let token = random_token().unwrap();
    let saved = serde_json::json!({
        "issuer": {
            "namespace": NAMESPACE,
            "public_key": PUBLIC_KEY,
        },
        "ciphertext": symmetric_crypt(&token, true).unwrap(),
    });
    PeerConfig::set_trusted_device_v2(&peer_id, saved.to_string(), Default::default()).unwrap();

    let credentials = Credentials::new(peer_id, NAMESPACE.into(), &PUBLIC_KEY).unwrap();
    assert_eq!(credentials.load().unwrap(), Some(token.to_vec()));
}
