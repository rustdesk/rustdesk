use super::LoginConfigHandler;

#[derive(Default, PartialEq, Eq)]
pub(super) enum LoginMethod {
    #[default]
    Automatic,
    Consent,
}

impl LoginConfigHandler {
    pub(crate) fn set_login_method(&mut self, method: &str) -> bool {
        self.login_method = match method {
            "consent" => LoginMethod::Consent,
            _ => return false,
        };
        true
    }

    pub(crate) fn skips_saved_password(&self) -> bool {
        self.login_method != LoginMethod::Automatic
    }

    pub(crate) fn get_login_method(&self) -> &'static str {
        match self.login_method {
            LoginMethod::Automatic => "",
            LoginMethod::Consent => "consent",
        }
    }
}

#[cfg(all(test, feature = "flutter"))]
mod tests {
    use crate::{client, flutter::FlutterHandler, ui_session_interface::Session};
    use base::message_proto::{Hash, Message, PeerInfo};
    use hbb_common::{
        config::PeerConfig, protobuf::Message as _, rendezvous_proto::ConnType, tcp::FramedStream,
        tokio, Stream,
    };

    async fn connection() -> (Stream, Stream) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (socket, addr) = listener.accept().await.unwrap();
        (
            Stream::Tcp(FramedStream::from(client, listener.local_addr().unwrap())),
            Stream::Tcp(FramedStream::from(socket, addr)),
        )
    }

    async fn received_password(peer: &mut Stream) -> Vec<u8> {
        let bytes = peer.next_timeout(3000).await.unwrap().unwrap();
        Message::parse_from_bytes(&bytes)
            .unwrap()
            .login_request()
            .password
            .to_vec()
    }

    #[test]
    fn session_creation_initializes_login_method() {
        let remote_id = hbb_common::config::LocalConfig::get_remote_id();
        for method in ["", "consent"] {
            let session_id = uuid::Uuid::new_v4();
            let result = crate::flutter_ffi::session_add_sync(
                session_id,
                format!("login-method-init-{}", session_id),
                false,
                false,
                false,
                false,
                false,
                String::new(),
                false,
                String::new(),
                false,
                None,
                method.to_owned(),
            );
            let monitor_session_id = uuid::Uuid::new_v4();
            let monitor_result = crate::flutter_ffi::session_add_existed_sync(
                format!("login-method-init-{}", session_id),
                monitor_session_id,
                vec![0],
                false,
            );
            let methods = [session_id, monitor_session_id].map(|id| {
                crate::flutter_ffi::session_get_common_sync(
                    id,
                    "login_method".to_owned(),
                    String::new(),
                )
                .0
            });
            crate::flutter::sessions::remove_session_by_session_id(&monitor_session_id);
            let session = crate::flutter::sessions::remove_session_by_session_id(&session_id);
            hbb_common::config::LocalConfig::set_remote_id(&remote_id);
            assert!(result.0.is_empty(), "{}", result.0);
            assert!(monitor_result.0.is_empty(), "{}", monitor_result.0);
            assert_eq!(methods, [Some(method.to_owned()), Some(method.to_owned())]);
            let session = session.unwrap();
            let request = session
                .lc
                .read()
                .unwrap()
                .create_login_msg(String::new(), String::new(), vec![1; 32]);
            assert_eq!(
                request.login_request().password.is_empty(),
                method == "consent"
            );
        }
    }

    #[tokio::test]
    async fn configured_passwords_are_skipped_only_for_the_selected_connection() {
        for conn_type in [
            ConnType::DEFAULT_CONN,
            ConnType::FILE_TRANSFER,
            ConnType::VIEW_CAMERA,
            ConnType::PORT_FORWARD,
            ConnType::RDP,
            ConnType::TERMINAL,
        ] {
            for skip in [false, true] {
                let session = Session::<FlutterHandler>::default();
                {
                    let mut lc = session.lc.write().unwrap();
                    lc.id = format!("skip-saved-password-test-{}", uuid::Uuid::new_v4());
                    lc.conn_type = conn_type;
                    lc.config.password = vec![1; 32];
                    lc.password = vec![2; 32];
                    lc.shared_password = Some("shared password".into());
                    let token = lc.get_conn_token();
                    if skip {
                        assert!(lc.set_login_method("consent"));
                    }
                    assert_eq!(lc.get_conn_token(), token);
                    let request = lc.create_login_msg(String::new(), String::new(), vec![4; 32]);
                    assert_eq!(request.login_request().password.is_empty(), skip);
                }
                let (mut stream, mut peer) = connection().await;
                assert!(
                    client::handle_hash(
                        session.lc.clone(),
                        "preset password",
                        Hash {
                            salt: "salt".into(),
                            challenge: "challenge".into(),
                            ..Default::default()
                        },
                        &session,
                        &mut Default::default(),
                        &mut stream,
                    )
                    .await
                );
                let password = received_password(&mut peer).await;
                assert_eq!(password.is_empty(), skip);
            }
        }
    }

    #[tokio::test]
    async fn admin_terminal_waits_for_os_credentials_before_requesting_consent() {
        let session = Session::<FlutterHandler>::default();
        {
            let mut lc = session.lc.write().unwrap();
            lc.conn_type = ConnType::TERMINAL;
            lc.is_terminal_admin = true;
            assert!(lc.set_login_method("consent"));
        }
        let (mut stream, mut peer) = connection().await;
        let mut trust = Default::default();
        assert!(
            client::handle_hash(
                session.lc.clone(),
                "preset password",
                Hash::default(),
                &session,
                &mut trust,
                &mut stream,
            )
            .await
        );
        assert!(peer.next_timeout(50).await.is_none());
        client::handle_login_from_ui(
            session.lc.clone(),
            "administrator".into(),
            "os password".into(),
            String::new(),
            false,
            &mut trust,
            &mut stream,
        )
        .await
        .unwrap();
        let bytes = peer.next_timeout(3000).await.unwrap().unwrap();
        let message = Message::parse_from_bytes(&bytes).unwrap();
        let login = message.login_request();
        assert!(login.password.is_empty());
        assert_eq!(login.os_login.username, "administrator");
        assert_eq!(login.os_login.password, "os password");
    }

    #[tokio::test]
    async fn consent_preserves_saved_password() {
        let (mut stream, mut peer) = connection().await;
        let id = format!("skip-saved-password-test-{}", uuid::Uuid::new_v4());
        let mut config = PeerConfig::default();
        config.password = vec![3; 32];
        config.store(&id);
        let session = Session::<FlutterHandler>::default();
        {
            let mut lc = session.lc.write().unwrap();
            lc.initialize(
                id.clone(),
                ConnType::DEFAULT_CONN,
                None,
                false,
                None,
                None,
                None,
                "",
            );
            assert!(!lc.set_login_method("password"));
            assert!(lc.set_login_method("consent"));
        }
        assert!(
            client::handle_hash(
                session.lc.clone(),
                "",
                Hash::default(),
                &session,
                &mut Default::default(),
                &mut stream,
            )
            .await
        );
        let first = received_password(&mut peer).await;
        assert!(first.is_empty());
        session
            .lc
            .write()
            .unwrap()
            .handle_peer_info(&PeerInfo::default());
        let saved = PeerConfig::load(&id).password;
        PeerConfig::remove(&id);
        assert_eq!(saved, config.password);
    }
}
