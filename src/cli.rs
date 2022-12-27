use crate::client::*;
use hbb_common::{
    config::PeerConfig,
    log,
    message_proto::*,
    protobuf::Message as _,
    tokio::{self, sync::mpsc},
    Stream,
};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct Session {
    id: String,
    lc: Arc<RwLock<LoginConfigHandler>>,
    sender: mpsc::UnboundedSender<Data>,
    password: String,
}

impl Session {
    pub fn new(id: &str, sender: mpsc::UnboundedSender<Data>) -> Self {
        let mut password = "".to_owned();
        if PeerConfig::load(id).password.is_empty() {
            password = rpassword::prompt_password("Enter password: ").unwrap();
        }
        let session = Self {
            id: id.to_owned(),
            sender,
            password,
            lc: Default::default(),
        };
        session
            .lc
            .write()
            .unwrap()
            .initialize(id.to_owned(), false, true);
        session
    }
}

#[async_trait]
impl Interface for Session {
    fn msgbox(&self, msgtype: &str, title: &str, text: &str) {
        if msgtype == "input-password" {
            self.sender
                .send(Data::Login((self.password.clone(), true)))
                .ok();
        } else if msgtype == "re-input-password" {
            log::error!("{}: {}", title, text);
            let pass = rpassword::prompt_password("Enter password: ").unwrap();
            self.sender.send(Data::Login((pass, true))).ok();
        } else if msgtype.contains("error") {
            log::error!("{}: {}: {}", msgtype, title, text);
        } else {
            log::info!("{}: {}: {}", msgtype, title, text);
        }
    }

    fn handle_login_error(&mut self, err: &str) -> bool {
        handle_login_error(self.lc.clone(), err, self)
    }

    fn handle_peer_info(&mut self, pi: PeerInfo) {
        let username = self.lc.read().unwrap().get_username(&pi);
        self.lc.write().unwrap().handle_peer_info(username, pi);
    }

    async fn handle_hash(&mut self, hash: Hash, peer: &mut Stream) {
        handle_hash(self.lc.clone(), hash, self, peer).await;
    }

    async fn handle_login_from_ui(&mut self, password: String, remember: bool, peer: &mut Stream) {
        handle_login_from_ui(self.lc.clone(), password, remember, peer).await;
    }

    async fn handle_test_delay(&mut self, t: TestDelay, peer: &mut Stream) {
        handle_test_delay(t, peer).await;
    }

    fn send(&self, data: Data) {
        self.sender.send(data).ok();
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn start_one_port_forward(
    id: String,
    port: i32,
    remote_host: String,
    remote_port: i32,
    key: String,
    token: String,
) {
    crate::common::test_rendezvous_server();
    crate::common::test_nat_type();
    let (sender, mut receiver) = mpsc::unbounded_channel::<Data>();
    let handler = Session::new(&id, sender);
    handler.lc.write().unwrap().port_forward = (remote_host, remote_port);
    if let Err(err) =
        crate::port_forward::listen(handler.id.clone(), port, handler.clone(), receiver, &key, &token).await
    {
        log::error!("Failed to listen on {}: {}", port, err);
    }
    log::info!("port forward (:{}) exit", port);
}
