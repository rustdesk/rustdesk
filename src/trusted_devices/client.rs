use super::TOKEN_BYTES;
use base::message_proto::{login_response, message, LoginResponse, Message};
use hbb_common::{
    anyhow::anyhow,
    bail,
    config::{Config, PeerConfig, PeerInfoSerde},
    password_security::symmetric_crypt,
    tokio, ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::convert::TryInto;

#[derive(Default)]
pub struct Session {
    credentials: Option<Credentials>,
    supported: bool,
    requested: bool,
    legacy: bool,
}

impl Session {
    pub async fn respond(
        &mut self,
        response: LoginResponse,
        interface: &impl crate::client::Interface,
    ) -> LoginResponse {
        match self.response(&response).await {
            Ok(supported) => {
                interface.get_lch().write().unwrap().enable_trusted_devices = supported
            }
            Err(error) => {
                interface.get_lch().write().unwrap().enable_trusted_devices = false;
                const ERROR_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
                hbb_common::throttled_log!(
                    ERROR_LOG_INTERVAL,
                    error,
                    "Trusted-device storage: {error}"
                );
                interface.msgbox(
                    "custom-error-trusted-device",
                    "Error",
                    &format!("Trusted-device storage: {error}"),
                    "",
                );
            }
        }
        response
    }

    pub fn new(
        login: &crate::client::LoginConfigHandler,
        secured: bool,
        key: Option<&[u8]>,
    ) -> ResultType<Self> {
        if crate::common::is_direct_ip_access(login.get_id()) {
            return Ok(Self {
                credentials: Some(Credentials::direct_ip(login.get_id())),
                ..Default::default()
            });
        }
        let Some(key) = key.filter(|_| secured) else {
            return Ok(Self::default());
        };
        let mut servers = login
            .other_server
            .as_ref()
            .map(|(_, server, _)| vec![server.clone()])
            .unwrap_or_else(Config::get_rendezvous_servers);
        servers.sort();
        let namespace = serde_json::to_string(&servers)?;
        let credentials = Credentials::new(login.get_id().to_owned(), namespace, key)?;
        Ok(Self {
            credentials: Some(credentials),
            ..Default::default()
        })
    }

    pub async fn login(&mut self, mut message: Message) -> ResultType<Message> {
        self.supported = false;
        self.requested = false;
        self.legacy = false;
        if let (Some(credentials), Some(message::Union::LoginRequest(request))) =
            (self.credentials.clone(), message.union.as_mut())
        {
            request.trusted_device_v2_supported = true;
            self.legacy = !request.hwid.is_empty();
            if !request.password.is_empty() {
                request.trusted_device_v2_token.clear();
                let token = match tokio::task::spawn_blocking(move || credentials.load()).await? {
                    Ok(token) => token.unwrap_or_default(),
                    Err(error) => {
                        self.credentials = None;
                        self.legacy = false;
                        request.trusted_device_v2_supported = false;
                        const LOG_INTERVAL: std::time::Duration =
                            std::time::Duration::from_secs(30);
                        hbb_common::throttled_log!(LOG_INTERVAL, warn,
                            "Trusted-device credential unavailable; using normal authentication for this connection: {error}");
                        return Ok(message);
                    }
                };
                request.trusted_device_v2_token = token.into();
            }
        }
        Ok(message)
    }

    pub fn outgoing(&mut self, mut message: Message) -> Message {
        if let Some(message::Union::Auth2fa(auth)) = message.union.as_mut() {
            self.legacy = false;
            auth.trust_this_device &= self.supported && self.credentials.is_some();
            self.requested = auth.trust_this_device;
        }
        message
    }

    pub async fn response(&mut self, response: &LoginResponse) -> ResultType<bool> {
        self.supported = response.trusted_device_v2_supported && self.credentials.is_some();
        let Some(credentials) = self.credentials.clone() else {
            return Ok(false);
        };
        if let Some(login_response::Union::PeerInfo(info)) = response.union.as_ref() {
            let token = response.trusted_device_v2_token.to_vec();
            if self.requested && token.is_empty() {
                bail!("Login succeeded, but the host did not remember this device");
            }
            if !token.is_empty() {
                if !self.requested && !(self.legacy && response.trusted_device_v2_supported) {
                    bail!("Host returned an unsolicited trusted-device credential");
                }
                let info = PeerInfoSerde {
                    username: info.username.clone(),
                    hostname: info.hostname.clone(),
                    platform: info.platform.clone(),
                };
                tokio::task::spawn_blocking(move || credentials.save(&token, info)).await??;
            }
        }
        Ok(self.supported)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Issuer {
    namespace: String,
    public_key: Option<[u8; TOKEN_BYTES]>,
}

#[derive(Serialize, Deserialize)]
struct SavedToken {
    issuer: Issuer,
    ciphertext: Vec<u8>,
}

#[derive(Clone)]
pub(super) struct Credentials {
    peer_id: String,
    issuer: Issuer,
}

impl Credentials {
    fn direct_ip(peer_id: &str) -> Self {
        Self {
            peer_id: peer_id.to_owned(),
            issuer: Issuer {
                namespace: format!("direct-ip:{peer_id}"),
                public_key: None,
            },
        }
    }

    pub fn new(peer_id: String, namespace: String, public_key: &[u8]) -> ResultType<Self> {
        Ok(Self {
            peer_id,
            issuer: Issuer {
                namespace,
                public_key: Some(
                    public_key
                        .try_into()
                        .map_err(|_| anyhow!("Invalid trusted host public key"))?,
                ),
            },
        })
    }

    pub fn load(&self) -> ResultType<Option<Vec<u8>>> {
        let value = PeerConfig::get_trusted_device_v2(&self.peer_id)?;
        match self.decode(&value) {
            Ok(token) => Ok(token),
            Err(error) => {
                const LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
                hbb_common::throttled_log!(LOG_INTERVAL, warn,
                    "Ignoring invalid trusted-device credential; re-enrollment is available: {error}");
                Ok(None)
            }
        }
    }

    fn decode(&self, value: &str) -> ResultType<Option<Vec<u8>>> {
        if value.is_empty() {
            return Ok(None);
        }
        let saved: SavedToken = serde_json::from_str(value)
            .map_err(|_| anyhow!("Invalid stored trusted-device credential"))?;
        if saved.issuer != self.issuer {
            return Ok(None);
        }
        let token = symmetric_crypt(&saved.ciphertext, false)
            .map_err(|_| anyhow!("Could not decrypt trusted-device credential"))?;
        if token.len() != TOKEN_BYTES {
            bail!("Invalid stored trusted-device credential");
        }
        Ok(Some(token))
    }

    pub fn save(&self, token: &[u8], info: PeerInfoSerde) -> ResultType<()> {
        if token.len() != TOKEN_BYTES {
            bail!("Invalid issued trusted-device credential");
        }
        let ciphertext = symmetric_crypt(token, true)
            .map_err(|_| anyhow!("Could not encrypt trusted-device credential"))?;
        let saved = SavedToken {
            issuer: self.issuer.clone(),
            ciphertext,
        };
        PeerConfig::set_trusted_device_v2(&self.peer_id, serde_json::to_string(&saved)?, info)
    }
}
