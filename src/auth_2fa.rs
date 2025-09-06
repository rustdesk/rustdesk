use hbb_common::{
    anyhow::anyhow,
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
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let id = crate::ipc::get_id();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let id = Config::get_id();
    if let Ok(info) = TOTPInfo::gen_totp_info(id, 6) {
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
        if let Ok(res) = totp.check_current(&code) {
            if res {
                if let Ok(v) = info.into_string() {
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    crate::ipc::set_option("2fa", &v);
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    Config::set_option("2fa".to_owned(), v);
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelegramBot {
    #[serde(skip)]
    pub token_str: String,
    pub token: Vec<u8>,
    pub chat_id: String,
}

impl TelegramBot {
    fn into_string(&self) -> ResultType<String> {
        let token = encrypt_vec_or_original(self.token_str.as_bytes(), "00", 1024);
        let bot = TelegramBot {
            token,
            ..self.clone()
        };
        let s = serde_json::to_string(&bot)?;
        Ok(s)
    }

    fn save(&self) -> ResultType<()> {
        let s = self.into_string()?;
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        crate::ipc::set_option("bot", &s);
        #[cfg(any(target_os = "android", target_os = "ios"))]
        Config::set_option("bot".to_owned(), s);
        Ok(())
    }

    pub fn get() -> ResultType<Option<TelegramBot>> {
        let data = Config::get_option("bot");
        if data.is_empty() {
            return Ok(None);
        }
        let mut bot = serde_json::from_str::<TelegramBot>(&data)?;
        let (token, success, _) = decrypt_vec_or_original(&bot.token, "00");
        if success {
            bot.token_str = String::from_utf8(token)?;
            return Ok(Some(bot));
        }
        bail!("decrypt_vec_or_original telegram bot token failed")
    }
}

// https://gist.github.com/dideler/85de4d64f66c1966788c1b2304b9caf1
pub async fn send_2fa_code_to_telegram(text: &str, bot: TelegramBot) -> ResultType<()> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot.token_str);
    let params = serde_json::json!({"chat_id": bot.chat_id, "text": text});
    crate::post_request(url, params.to_string(), "").await?;
    Ok(())
}

pub fn get_chatid_telegram(bot_token: &str) -> ResultType<Option<String>> {
    let url = format!("https://api.telegram.org/bot{}/getUpdates", bot_token);
    // because caller is in tokio runtime, so we must call post_request_sync in new thread.
    let handle = std::thread::spawn(move || crate::post_request_sync(url, "".to_owned(), ""));
    let resp = handle.join().map_err(|_| anyhow!("Thread panicked"))??;
    let value = serde_json::from_str::<serde_json::Value>(&resp).map_err(|e| anyhow!(e))?;

    // Check for an error_code in the response
    if let Some(error_code) = value.get("error_code").and_then(|code| code.as_i64()) {
        // If there's an error_code, try to use the description for the error message
        let description = value["description"]
            .as_str()
            .unwrap_or("Unknown error occurred");
        return Err(anyhow!(
            "Telegram API error: {} (error_code: {})",
            description,
            error_code
        ));
    }

    let chat_id = &value["result"][0]["message"]["chat"]["id"];
    let chat_id = if let Some(id) = chat_id.as_i64() {
        Some(id.to_string())
    } else if let Some(id) = chat_id.as_str() {
        Some(id.to_owned())
    } else {
        None
    };

    if let Some(chat_id) = chat_id.as_ref() {
        let bot = TelegramBot {
            token_str: bot_token.to_owned(),
            chat_id: chat_id.to_owned(),
            ..Default::default()
        };
        bot.save()?;
    }

    Ok(chat_id)
}
