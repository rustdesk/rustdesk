pub mod password {
    use crate::config::Config;
    use std::{
        fmt::Display,
        str::FromStr,
        sync::{Arc, RwLock},
    };

    lazy_static::lazy_static! {
        pub static ref RANDOM_PASSWORD:Arc<RwLock<String>> = Arc::new(RwLock::new(Config::get_auto_password()));
    }

    const SECURITY_ENABLED: &'static str = "security-password-enabled";
    const RANDOM_ENABLED: &'static str = "random-password-enabled";
    const ONETIME_ENABLED: &'static str = "onetime-password-enabled";
    const ONETIME_ACTIVATED: &'static str = "onetime-password-activated";
    const UPDATE_METHOD: &'static str = "random-password-update-method";

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum UpdateMethod {
        KEEP,
        UPDATE,
        DISABLE,
    }

    impl FromStr for UpdateMethod {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            if s == "KEEP" {
                Ok(Self::KEEP)
            } else if s == "UPDATE" {
                Ok(Self::UPDATE)
            } else if s == "DISABLE" {
                Ok(Self::DISABLE)
            } else {
                Err(())
            }
        }
    }

    impl Display for UpdateMethod {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                UpdateMethod::KEEP => write!(f, "KEEP"),
                UpdateMethod::UPDATE => write!(f, "UPDATE"),
                UpdateMethod::DISABLE => write!(f, "DISABLE"),
            }
        }
    }

    pub fn set_random_password(password: &str) {
        *RANDOM_PASSWORD.write().unwrap() = password.to_owned();
    }

    pub fn random_password() -> String {
        let mut password = RANDOM_PASSWORD.read().unwrap().clone();
        if password.is_empty() {
            password = Config::get_auto_password();
            set_random_password(&password);
        }
        password
    }

    pub fn random_password_valid() -> bool {
        if random_enabled() {
            onetime_password_activated() || !onetime_password_enabled()
        } else {
            false
        }
    }

    pub fn passwords() -> Vec<String> {
        let mut v = vec![];
        if random_password_valid() {
            v.push(random_password());
        }
        if security_enabled() {
            v.push(Config::get_security_password());
        }
        v
    }

    pub fn after_session(authorized: bool) {
        if authorized && random_enabled() {
            UpdateMethod::from_str(&update_method())
                .map(|method| match method {
                    UpdateMethod::KEEP => {}
                    UpdateMethod::UPDATE => set_random_password(&Config::get_auto_password()),
                    UpdateMethod::DISABLE => set_random_enabled(false),
                })
                .ok();
        }
    }

    pub fn update_method() -> String {
        let mut method = Config::get_option(UPDATE_METHOD);
        if UpdateMethod::from_str(&method).is_err() {
            method = UpdateMethod::KEEP.to_string(); // default is keep
            set_update_method(&method);
        }
        method
    }

    pub fn set_update_method(method: &str) {
        Config::set_option(UPDATE_METHOD.to_owned(), method.to_owned());
    }

    pub fn random_enabled() -> bool {
        str2bool(RANDOM_ENABLED, true, || {
            set_onetime_password_activated(false);
            set_random_password(&Config::get_auto_password());
        })
    }

    pub fn set_random_enabled(enabled: bool) {
        if enabled != random_enabled() {
            Config::set_option(RANDOM_ENABLED.to_owned(), bool2str(enabled));
            set_onetime_password_activated(false);
            if enabled {
                set_random_password(&Config::get_auto_password());
            }
        }
    }

    pub fn security_enabled() -> bool {
        str2bool(SECURITY_ENABLED, true, || {})
    }

    pub fn set_security_enabled(enabled: bool) {
        if enabled != security_enabled() {
            Config::set_option(SECURITY_ENABLED.to_owned(), bool2str(enabled));
        }
    }

    pub fn onetime_password_enabled() -> bool {
        str2bool(ONETIME_ENABLED, false, || {
            set_onetime_password_activated(false);
            set_random_password(&Config::get_auto_password());
        })
    }

    pub fn set_onetime_password_enabled(enabled: bool) {
        if enabled != onetime_password_enabled() {
            Config::set_option(ONETIME_ENABLED.to_owned(), bool2str(enabled));
            set_onetime_password_activated(false);
            set_random_password(&Config::get_auto_password());
        }
    }

    pub fn onetime_password_activated() -> bool {
        str2bool(ONETIME_ACTIVATED, false, || {})
    }

    pub fn set_onetime_password_activated(activated: bool) {
        if activated != onetime_password_activated() {
            Config::set_option(ONETIME_ACTIVATED.to_owned(), bool2str(activated));
            if activated {
                set_random_password(&Config::get_auto_password());
            }
        }
    }

    // notice: Function nesting
    fn str2bool(key: &str, default: bool, default_set: impl Fn()) -> bool {
        let option = Config::get_option(key);
        if option == "Y" {
            true
        } else if option == "N" {
            false
        } else {
            Config::set_option(key.to_owned(), bool2str(default));
            default_set();
            default
        }
    }

    fn bool2str(option: bool) -> String {
        if option { "Y" } else { "N" }.to_owned()
    }
}

pub mod config {
    use super::base64::decrypt as decrypt00;
    use super::base64::encrypt as encrypt00;

    const VERSION_LEN: usize = 2;

    pub fn encrypt_str_or_original(s: &str, version: &str) -> String {
        if version.len() == VERSION_LEN {
            if version == "00" {
                if let Ok(s) = encrypt00(s.as_bytes()) {
                    return version.to_owned() + &s;
                }
            }
        }

        s.to_owned()
    }

    // bool: whether should store to re-encrypt when load
    pub fn decrypt_str_or_original(s: &str, current_version: &str) -> (String, bool) {
        if s.len() > VERSION_LEN {
            let version = &s[..VERSION_LEN];
            if version == "00" {
                if let Ok(v) = decrypt00(&s[VERSION_LEN..].as_bytes()) {
                    return (
                        String::from_utf8_lossy(&v).to_string(),
                        version != current_version,
                    );
                }
            }
        }

        (s.to_owned(), !s.is_empty())
    }

    pub fn encrypt_vec_or_original(v: &[u8], version: &str) -> Vec<u8> {
        if version.len() == VERSION_LEN {
            if version == "00" {
                if let Ok(s) = encrypt00(v) {
                    let mut version = version.to_owned().into_bytes();
                    version.append(&mut s.into_bytes());
                    return version;
                }
            }
        }

        v.to_owned()
    }

    // bool: whether should store to re-encrypt when load
    pub fn decrypt_vec_or_original(v: &[u8], current_version: &str) -> (Vec<u8>, bool) {
        if v.len() > VERSION_LEN {
            let version = String::from_utf8_lossy(&v[..VERSION_LEN]);
            if version == "00" {
                if let Ok(v) = decrypt00(&v[VERSION_LEN..]) {
                    return (v, version != current_version);
                }
            }
        }

        (v.to_owned(), !v.is_empty())
    }

    mod test {

        #[test]
        fn test() {
            use crate::password_security::config::*;

            println!("test str");
            let data = "Hello World";
            let encrypted = encrypt_str_or_original(data, "00");
            let (decrypted, store) = decrypt_str_or_original(&encrypted, "00");
            println!("data: {}", data);
            println!("encrypted: {}", encrypted);
            println!("decrypted: {}", decrypted);
            assert_eq!(data, decrypted);
            assert_eq!("00", &encrypted[..2]);
            assert_eq!(store, false);
            let (_, store2) = decrypt_str_or_original(&encrypted, "01");
            assert_eq!(store2, true);

            println!("test vec");
            let data: Vec<u8> = vec![1, 2, 3, 4];
            let encrypted = encrypt_vec_or_original(&data, "00");
            let (decrypted, store) = decrypt_vec_or_original(&encrypted, "00");
            println!("data: {:?}", data);
            println!("encrypted: {:?}", encrypted);
            println!("decrypted: {:?}", decrypted);
            assert_eq!(data, decrypted);
            assert_eq!("00".as_bytes(), &encrypted[..2]);
            assert_eq!(store, false);
            let (_, store2) = decrypt_vec_or_original(&encrypted, "01");
            assert_eq!(store2, true);

            println!("test old");
            let data = "00Hello World";
            let (decrypted, store) = decrypt_str_or_original(&data, "00");
            assert_eq!(data, decrypted);
            assert_eq!(store, true);
            let data: Vec<u8> = vec!['0' as u8, '0' as u8, 1, 2, 3, 4];
            let (decrypted, store) = decrypt_vec_or_original(&data, "00");
            assert_eq!(data, decrypted);
            assert_eq!(store, true);
            let (_, store) = decrypt_str_or_original("", "00");
            assert_eq!(store, false);
            let (_, store) = decrypt_vec_or_original(&vec![], "00");
            assert_eq!(store, false);
        }
    }
}

mod base64 {
    use super::symmetric_crypt;
    use sodiumoxide::base64;

    pub fn encrypt(v: &[u8]) -> Result<String, ()> {
        if v.len() > 0 {
            symmetric_crypt(v, true).map(|v| base64::encode(v, base64::Variant::Original))
        } else {
            Err(())
        }
    }

    pub fn decrypt(v: &[u8]) -> Result<Vec<u8>, ()> {
        if v.len() > 0 {
            base64::decode(v, base64::Variant::Original).and_then(|v| symmetric_crypt(&v, false))
        } else {
            Err(())
        }
    }
}

fn symmetric_crypt(data: &[u8], encrypt: bool) -> Result<Vec<u8>, ()> {
    use sodiumoxide::crypto::secretbox;
    use std::convert::TryInto;

    let mut keybuf = crate::get_uuid();
    keybuf.resize(secretbox::KEYBYTES, 0);
    let key = secretbox::Key(keybuf.try_into().map_err(|_| ())?);
    let nonce = secretbox::Nonce([0; secretbox::NONCEBYTES]);

    if encrypt {
        Ok(secretbox::seal(data, &nonce, &key))
    } else {
        secretbox::open(data, &nonce, &key)
    }
}
