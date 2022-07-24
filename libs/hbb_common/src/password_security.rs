pub mod password {
    use crate::config::Config;
    use std::sync::{Arc, RwLock};

    lazy_static::lazy_static! {
        pub static ref TEMPORARY_PASSWORD:Arc<RwLock<String>> = Arc::new(RwLock::new(Config::get_auto_password(temporary_password_length())));
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum VerificationMethod {
        OnlyUseTemporaryPassword,
        OnlyUsePermanentPassword,
        UseBothPasswords,
    }

    // Should only be called in server
    pub fn update_temporary_password() {
        *TEMPORARY_PASSWORD.write().unwrap() =
            Config::get_auto_password(temporary_password_length());
    }

    // Should only be called in server
    pub fn temporary_password() -> String {
        TEMPORARY_PASSWORD.read().unwrap().clone()
    }

    fn verification_method() -> VerificationMethod {
        let method = Config::get_option("verification-method");
        if method == "use-temporary-password" {
            VerificationMethod::OnlyUseTemporaryPassword
        } else if method == "use-permanent-password" {
            VerificationMethod::OnlyUsePermanentPassword
        } else {
            VerificationMethod::UseBothPasswords // default
        }
    }

    pub fn temporary_password_length() -> usize {
        let length = Config::get_option("temporary-password-length");
        if length == "8" {
            8
        } else if length == "10" {
            10
        } else {
            6 // default
        }
    }

    pub fn temporary_enabled() -> bool {
        verification_method() != VerificationMethod::OnlyUsePermanentPassword
    }

    pub fn permanent_enabled() -> bool {
        verification_method() != VerificationMethod::OnlyUseTemporaryPassword
    }

    pub fn has_valid_password() -> bool {
        temporary_enabled() && !temporary_password().is_empty()
            || permanent_enabled() && !Config::get_permanent_password().is_empty()
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
