use super::{storage::Storage, TOKEN_BYTES};
use base::message_proto::{login_response, message, LoginResponse, Message};
use hbb_common::{
    anyhow::anyhow,
    bail,
    config::Config,
    password_security::symmetric_crypt,
    sodiumoxide::{crypto::hash::sha256, hex},
    tokio, ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::{convert::TryInto, path::PathBuf};

#[derive(Default)]
pub struct Session {
    credentials: Option<Credentials>,
    supported: bool,
    requested: bool,
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
        let binding = serde_json::to_vec(&(&namespace, key))?;
        let name = hex::encode(sha256::hash(&binding));
        let credentials = Credentials::new(
            Config::path(format!("trusted-device-v2/{name}.json")),
            namespace,
            key,
        )?;
        Ok(Self {
            credentials: Some(credentials),
            ..Default::default()
        })
    }

    pub async fn login(&mut self, mut message: Message) -> ResultType<Message> {
        self.supported = false;
        self.requested = false;
        if let (Some(credentials), Some(message::Union::LoginRequest(request))) =
            (self.credentials.clone(), message.union.as_mut())
        {
            if !request.password.is_empty() {
                request.trusted_device_v2_token.clear();
                let token = match tokio::task::spawn_blocking(move || credentials.load()).await? {
                    Ok(token) => token.unwrap_or_default(),
                    Err(error) => {
                        self.credentials = None;
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
        if matches!(response.union, Some(login_response::Union::PeerInfo(_))) {
            let token = response.trusted_device_v2_token.to_vec();
            if self.requested && token.is_empty() {
                bail!("Login succeeded, but the host did not remember this device");
            }
            if !token.is_empty() {
                if !self.requested {
                    bail!("Host returned an unsolicited trusted-device credential");
                }
                tokio::task::spawn_blocking(move || credentials.save(&token)).await??;
            }
        }
        Ok(self.supported)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Issuer {
    namespace: String,
    public_key: [u8; TOKEN_BYTES],
}

#[derive(Serialize, Deserialize)]
struct SavedToken {
    issuer: Issuer,
    ciphertext: Vec<u8>,
}

#[derive(Clone)]
pub(super) struct Credentials {
    path: PathBuf,
    issuer: Issuer,
}

impl Credentials {
    pub fn new(path: PathBuf, namespace: String, public_key: &[u8]) -> ResultType<Self> {
        Ok(Self {
            path,
            issuer: Issuer {
                namespace,
                public_key: public_key
                    .try_into()
                    .map_err(|_| anyhow!("Invalid trusted host public key"))?,
            },
        })
    }

    pub fn load(&self) -> ResultType<Option<Vec<u8>>> {
        let storage = Storage::lock(&self.path)?;
        let saved: Option<SavedToken> = storage.read()?;
        let Some(saved) = saved.filter(|saved| saved.issuer == self.issuer) else {
            return Ok(None);
        };
        let token = symmetric_crypt(&saved.ciphertext, false)
            .map_err(|_| anyhow!("Could not decrypt trusted-device credential"))?;
        if token.len() != TOKEN_BYTES {
            bail!("Invalid stored trusted-device credential");
        }
        Ok(Some(token))
    }

    pub fn save(&self, token: &[u8]) -> ResultType<()> {
        if token.len() != TOKEN_BYTES {
            bail!("Invalid issued trusted-device credential");
        }
        let ciphertext = symmetric_crypt(token, true)
            .map_err(|_| anyhow!("Could not encrypt trusted-device credential"))?;
        let saved = SavedToken {
            issuer: self.issuer.clone(),
            ciphertext,
        };
        Storage::lock(&self.path)?.write(&Some(saved))
    }
}
