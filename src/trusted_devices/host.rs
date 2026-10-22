use super::{
    authority::{self, Credential, Request, Response},
    random_token, token_verifier, DeviceInfo, Ticket, TOKEN_BYTES,
};
use base::message_proto::{Hash, LoginRequest};
use hbb_common::{
    bail,
    sodiumoxide::{crypto::hash::sha256, utils},
    ResultType,
};

#[derive(Default)]
pub struct LoginProof {
    pub password: Vec<u8>,
    pub challenge: String,
    pub salt: String,
    pub token: Vec<u8>,
    pub legacy: Option<[u8; TOKEN_BYTES]>,
    pub supports_v2: bool,
}

impl LoginProof {
    pub fn from_login(request: &mut LoginRequest, hash: &Hash) -> ResultType<Self> {
        let token = std::mem::take(&mut request.trusted_device_v2_token).to_vec();
        Ok(Self {
            password: request.password.to_vec(),
            challenge: hash.challenge.clone(),
            salt: hash.salt.clone(),
            token,
            legacy: super::legacy::verifier(
                &request.hwid,
                &DeviceInfo {
                    id: request.my_id.clone(),
                    name: request.my_name.clone(),
                    platform: request.my_platform.clone(),
                    ..Default::default()
                },
            )?,
            supports_v2: request.trusted_device_v2_supported,
        })
    }
}

pub struct Attempt {
    ticket: Ticket,
    credential: Option<Credential>,
    pub enroll: bool,
}

pub async fn begin(
    proof: LoginProof,
) -> ResultType<Option<(Option<Attempt>, bool, totp_rs::TOTP)>> {
    let verifier = token_verifier(&proof.token);
    let legacy = proof.legacy;
    let supports_v2 = proof.supports_v2;
    let settings = super::config::update(move || {
        let settings = super::config::snapshot()?;
        if !settings.enabled || settings.totp.is_empty() {
            return Ok(None);
        }
        settings.verify_password(&proof)?;
        let totp = crate::auth_2fa::TOTPInfo::from_str(&settings.totp)?;
        Ok(Some((settings.scope, totp)))
    })
    .await?;
    let Some((scope, totp)) = settings else {
        return Ok(None);
    };
    let (ticket, credential) = match authority::request(Request::Begin {
        scope,
        verifier,
        legacy,
    })
    .await
    {
        Ok(Response::Begun(ticket, credential)) => (ticket, credential),
        Err(error) if error.is::<authority::Unavailable>() => {
            log_unavailable(&error);
            return Ok(Some((None, false, totp)));
        }
        Err(error) => return Err(error),
        _ => bail!("Invalid trusted-device authorization response"),
    };
    let valid = credential.is_some();
    if !valid && !supports_v2 {
        return Ok(Some((None, false, totp)));
    }
    let enroll = supports_v2 && matches!(&credential, Some(Credential::Legacy(_)));
    Ok(Some((
        Some(Attempt {
            ticket,
            credential,
            enroll,
        }),
        valid,
        totp,
    )))
}

pub fn log_unavailable(error: &hbb_common::anyhow::Error) {
    const LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
    hbb_common::throttled_log!(LOG_INTERVAL, warn,
        "Trusted-device authority unavailable; continuing password/TOTP without trusted devices: {error:#}");
}

impl Attempt {
    pub async fn check(&self) -> ResultType<()> {
        self.check_settings().await?;
        match authority::request(Request::Check {
            ticket: self.ticket.clone(),
            credential: self.credential.clone(),
        })
        .await?
        {
            Response::Done => Ok(()),
            _ => bail!("Invalid trusted-device validation response"),
        }
    }

    pub async fn finish(self, info: DeviceInfo) -> ResultType<Vec<u8>> {
        if !self.enroll {
            return Ok(Vec::new());
        }
        self.check_settings().await?;
        let token = random_token()?;
        let legacy = match self.credential {
            Some(Credential::Legacy(verifier)) => Some(verifier),
            _ => None,
        };
        match authority::request(Request::Enroll {
            ticket: self.ticket,
            verifier: sha256::hash(&token).0,
            info,
            legacy,
        })
        .await?
        {
            Response::Done => Ok(token.to_vec()),
            _ => bail!("Invalid trusted-device enrollment response"),
        }
    }

    async fn check_settings(&self) -> ResultType<()> {
        let scope = self.ticket.scope;
        super::config::update(move || {
            let settings = super::config::snapshot()?;
            if !settings.enabled || settings.totp.is_empty() || !utils::memcmp(&settings.scope, &scope) {
                bail!("Trusted-device authentication settings changed; reconnect and authenticate again");
            }
            Ok(())
        }).await
    }
}
