use hbb_common::{
    bail,
    config::Config,
    get_time,
    password_security::{decrypt_vec_or_original, encrypt_vec_or_original},
    ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::sync::Mutex;
use totp_rs::{Algorithm, Secret, TOTP};

lazy_static::lazy_static! {
    static ref CURRENT_2FA: Mutex<Option<(TOTPInfo, TOTP)>> = Mutex::new(None);
}

const ISSUER: &str = "RustDesk";
const TAG_LOGIN: &str = "Connection";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TOTPInfo {
    pub name: String,
    pub secret: Vec<u8>,
    pub digits: usize,
    pub created_at: i64,
}

impl TOTPInfo {
    fn new_totp(&self) -> ResultType<TOTP> {
        let totp = TOTP::new(
            Algorithm::SHA1,
            self.digits,
            1,
            30,
            self.secret.clone(),
            Some(format!("{} {}", ISSUER, TAG_LOGIN)),
            self.name.clone(),
        )?;
        Ok(totp)
    }

    fn gen_totp_info(name: String, digits: usize) -> ResultType<TOTPInfo> {
        let secret = Secret::generate_secret();
        let totp = TOTPInfo {
            secret: secret.to_bytes()?,
            name,
            digits,
            created_at: get_time(),
            ..Default::default()
        };
        Ok(totp)
    }

    pub fn into_string(&self) -> ResultType<String> {
        let secret = encrypt_vec_or_original(self.secret.as_slice(), "00", 1024);
        let totp_info = TOTPInfo {
            secret,
            ..self.clone()
        };
        let s = serde_json::to_string(&totp_info)?;
        Ok(s)
    }

    pub fn from_str(data: &str) -> ResultType<TOTP> {
        let mut totp_info = serde_json::from_str::<TOTPInfo>(data)?;
        let (secret, success, _) = decrypt_vec_or_original(&totp_info.secret, "00");
        if success {
            totp_info.secret = secret;
            return Ok(totp_info.new_totp()?);
        } else {
            bail!("decrypt_vec_or_original 2fa secret failed")
        }
    }
}

pub fn generate2fa() -> String {
    if let Ok(info) = TOTPInfo::gen_totp_info(crate::ipc::get_id(), 6) {
        if let Ok(totp) = info.new_totp() {
            let code = totp.get_url();
            *CURRENT_2FA.lock().unwrap() = Some((info, totp));
            return code;
        }
    }
    "".to_owned()
}

pub fn verify2fa(code: String) -> bool {
    if let Some((info, totp)) = CURRENT_2FA.lock().unwrap().as_ref() {
        if let Ok(cur) = totp.generate_current() {
            let res = code == cur;
            if res {
                if let Ok(v) = info.into_string() {
                    crate::ipc::set_option("2fa", &v);
                    return res;
                }
            }
        }
    }
    false
}

pub fn get_2fa(raw: Option<String>) -> Option<TOTP> {
    TOTPInfo::from_str(&raw.unwrap_or(Config::get_option("2fa")))
        .map(|x| Some(x))
        .unwrap_or_default()
}
