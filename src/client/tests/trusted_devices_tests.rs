use super::*;
use hbb_common::tcp::FramedStream;

#[tokio::test]
async fn existing_legacy_consent_is_preserved_across_upgrades() {
    crate::trusted_devices::tests::initialize_config();
    let mut login = LoginConfigHandler::default();
    login.id = uuid::Uuid::new_v4().to_string();
    login
        .config
        .options
        .insert("trust-this-device".into(), "Y".into());
    let request = login.create_login_msg(String::new(), String::new(), vec![1; 32]);
    let Some(message::Union::LoginRequest(request)) = request.union else {
        panic!("expected a login request");
    };
    assert!(
        request.hwid == crate::get_hwid(),
        "an existing legacy consent must remain usable with old hosts"
    );
    verified_channel_controls_tokens(&login).await;
    corrupt_credentials_do_not_block_password_login(&login).await;
}

async fn corrupt_credentials_do_not_block_password_login(login: &LoginConfigHandler) {
    use crate::trusted_devices::client::Session;
    let path = Config::path(format!("peers/{}.toml", login.id));
    let saved = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid cached credential").unwrap();
    let mut trust = Session::new(login, true, Some(&[1; 32])).unwrap();
    let request = login.create_login_msg(String::new(), String::new(), vec![1; 32]);
    let outgoing = trust.login(request.clone()).await;
    let stored = std::fs::read(&path).unwrap();
    std::fs::write(&path, saved).unwrap();
    assert_eq!(outgoing.unwrap(), request);
    assert_eq!(stored, b"invalid cached credential");
    let mut response = LoginResponse::new();
    response.set_error(REQUIRE_2FA.into());
    response.trusted_device_v2_supported = true;
    assert!(!trust.response(&response).await.unwrap());
    let mut auth = Message::new();
    auth.set_auth_2fa(Auth2FA {
        trust_this_device: true,
        ..Default::default()
    });
    assert!(!trust.outgoing(auth).auth_2fa().trust_this_device);
}

async fn verified_channel_controls_tokens(login: &LoginConfigHandler) {
    use crate::trusted_devices::client::Session;
    let token = vec![9; crate::trusted_devices::TOKEN_BYTES];
    let mut trust = Session::new(login, true, Some(&[1; 32])).unwrap();
    let mut response = LoginResponse::new();
    response.set_error(REQUIRE_2FA.into());
    response.trusted_device_v2_supported = true;
    assert!(trust.response(&response).await.unwrap());
    let mut auth = Message::new();
    auth.set_auth_2fa(Auth2FA {
        trust_this_device: true,
        ..Default::default()
    });
    assert!(trust.outgoing(auth).auth_2fa().trust_this_device);
    let mut success = LoginResponse::new();
    success.set_peer_info(PeerInfo {
        platform: "Windows".into(),
        ..Default::default()
    });
    success.trusted_device_v2_token = token.clone().into();
    trust.response(&success).await.unwrap();
    assert_eq!(PeerConfig::peers(Some(vec![login.id.clone()])).len(), 1);
    another_connection_cannot_supply_this_streams_token(&mut trust).await;
    let request = login.create_login_msg(String::new(), String::new(), vec![1; 32]);
    assert_eq!(sent_token(&mut trust, &request).await, token);
    for mut unverified in [
        Session::new(login, false, Some(&[1; 32])).unwrap(),
        Session::new(login, true, None).unwrap(),
    ] {
        assert!(sent_token(&mut unverified, &request).await.is_empty());
    }
    response.trusted_device_v2_supported = false;
    response.enable_trusted_devices = true;
    assert!(!trust.response(&response).await.unwrap());
    assert_eq!(sent_token(&mut trust, &request).await, token);
    response.trusted_device_v2_supported = true;
    trust.response(&response).await.unwrap();
    assert_eq!(sent_token(&mut trust, &request).await, token);
    let mut migration = Session::new(login, true, Some(&[1; 32])).unwrap();
    let outgoing = migration.login(request).await.unwrap();
    assert!(outgoing.login_request().trusted_device_v2_supported);
    success.trusted_device_v2_supported = true;
    migration.response(&success).await.unwrap();
    let mut no_consent = Session::new(login, true, Some(&[1; 32])).unwrap();
    assert!(no_consent.response(&success).await.is_err());
}

async fn another_connection_cannot_supply_this_streams_token(
    trust: &mut crate::trusted_devices::client::Session,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (remote, _) = listener.accept().await.unwrap();
    let mut peer = Stream::Tcp(FramedStream::from(socket, addr));
    let mut remote = Stream::Tcp(FramedStream::from(remote, addr));
    let login = Arc::new(RwLock::new(LoginConfigHandler::default()));
    let mut old_connection = crate::trusted_devices::client::Session::new(
        &login.read().unwrap(),
        peer.is_secured(),
        None,
    )
    .unwrap();
    let request = login
        .read()
        .unwrap()
        .create_login_msg(String::new(), String::new(), vec![1; 32]);
    assert!(!sent_token(trust, &request).await.is_empty());
    assert!(!peer.is_secured());
    send_login(
        login,
        String::new(),
        String::new(),
        vec![1; 32],
        &mut old_connection,
        &mut peer,
    )
    .await
    .unwrap();
    let bytes = remote.next().await.unwrap().unwrap();
    let message = Message::parse_from_bytes(&bytes).unwrap();
    assert!(
        message.login_request().trusted_device_v2_token.is_empty(),
        "another connection's credential was sent on this unsecured stream"
    );
}

async fn sent_token(trust: &mut crate::trusted_devices::client::Session, request: &Message) -> Vec<u8> {
    trust
        .login(request.clone())
        .await
        .unwrap()
        .login_request()
        .trusted_device_v2_token
        .to_vec()
}
