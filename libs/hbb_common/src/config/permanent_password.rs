use sha2::{Digest, Sha256};
use sodiumoxide::base64;

use crate::{
    log,
    password_security::{decrypt_str_or_original, symmetric_crypt},
};

pub(super) const PASSWORD_ENC_VERSION: &str = "00";
pub(super) const PERMANENT_PASSWORD_ENC_VERSION: &str = "01";
pub(super) const PERMANENT_PASSWORD_HASH_PREFIX: &str = "00";
const HBBS_PRESET_PASSWORD_HASH_PREFIX: &str = "00";
pub(super) const PERMANENT_PASSWORD_H1_LEN: usize = 32;
pub(super) const DEFAULT_SALT_LEN: usize = 32;
pub const ENCRYPT_MAX_LEN: usize = 128; // used for password, pin, etc, not for all
const VERSION_LEN: usize = 2;

#[cfg(test)]
pub(super) fn is_permanent_password_hashed_storage(v: &str) -> bool {
    decode_permanent_password_h1_from_hashed_storage(v).is_some()
}

pub fn compute_permanent_password_h1(
    password: &str,
    salt: &str,
) -> [u8; PERMANENT_PASSWORD_H1_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt.as_bytes());
    let out = hasher.finalize();
    let mut h1 = [0u8; PERMANENT_PASSWORD_H1_LEN];
    h1.copy_from_slice(&out[..PERMANENT_PASSWORD_H1_LEN]);
    h1
}

pub(super) fn constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    sodiumoxide::utils::memcmp(a, b)
}

pub(super) fn encode_permanent_password_storage_from_h1(
    h1: &[u8; PERMANENT_PASSWORD_H1_LEN],
) -> String {
    PERMANENT_PASSWORD_HASH_PREFIX.to_owned() + &base64::encode(h1, base64::Variant::Original)
}

pub(super) fn encode_permanent_password_encrypted_storage_from_h1(
    h1: &[u8; PERMANENT_PASSWORD_H1_LEN],
) -> Option<String> {
    let hashed_storage = encode_permanent_password_storage_from_h1(h1);
    encrypt_permanent_password_storage(&hashed_storage)
}

pub(super) fn decode_permanent_password_h1_from_hashed_storage(
    storage: &str,
) -> Option<[u8; PERMANENT_PASSWORD_H1_LEN]> {
    decode_password_h1_after_prefix(storage, PERMANENT_PASSWORD_HASH_PREFIX)
}

fn decode_password_h1_after_prefix(
    storage: &str,
    prefix: &str,
) -> Option<[u8; PERMANENT_PASSWORD_H1_LEN]> {
    let encoded = storage.strip_prefix(prefix)?;

    let v = base64::decode(encoded.as_bytes(), base64::Variant::Original).ok()?;
    if v.len() != PERMANENT_PASSWORD_H1_LEN {
        return None;
    }
    let mut h1 = [0u8; PERMANENT_PASSWORD_H1_LEN];
    h1.copy_from_slice(&v[..PERMANENT_PASSWORD_H1_LEN]);
    Some(h1)
}

fn encrypt_permanent_password_storage(storage: &str) -> Option<String> {
    if storage.chars().count() > ENCRYPT_MAX_LEN {
        return None;
    }
    let encrypted = symmetric_crypt(storage.as_bytes(), true).ok()?;
    Some(
        PERMANENT_PASSWORD_ENC_VERSION.to_owned()
            + &base64::encode(encrypted, base64::Variant::Original),
    )
}

pub(super) fn decrypt_permanent_password_str_or_original(storage: &str) -> (String, bool, bool) {
    if storage.len() > VERSION_LEN && storage.starts_with(PERMANENT_PASSWORD_ENC_VERSION) {
        if let Ok(decoded) = base64::decode(
            &storage.as_bytes()[VERSION_LEN..],
            base64::Variant::Original,
        ) {
            if let Ok(v) = symmetric_crypt(&decoded, false) {
                return (String::from_utf8_lossy(&v).to_string(), true, false);
            }
        }
    }
    (storage.to_owned(), false, !storage.is_empty())
}

pub fn local_permanent_password_storage_is_usable_for_auth(storage: &str, salt: &str) -> bool {
    if storage.is_empty() {
        return false;
    }

    if decode_permanent_password_h1_from_storage(storage).is_some() {
        return !salt.is_empty();
    }
    if storage.starts_with(PERMANENT_PASSWORD_ENC_VERSION) {
        let (_, decrypted, _) = decrypt_permanent_password_str_or_original(storage);
        if decrypted {
            log::error!("Permanent password storage looks current but cannot be decoded as a hash");
            return false;
        }
    }

    let (_, decrypted, looks_like_plaintext) =
        decrypt_str_or_original(storage, PASSWORD_ENC_VERSION);
    if storage.starts_with(PASSWORD_ENC_VERSION) && !decrypted && !looks_like_plaintext {
        log::error!("Permanent password storage looks encrypted but cannot be decrypted");
        return false;
    }
    true
}

pub fn preset_permanent_password_storage_is_usable_for_auth(storage: &str, salt: &str) -> bool {
    if storage.is_empty() {
        return false;
    }
    if salt.is_empty() {
        return true;
    }
    decode_preset_password_h1_from_storage(storage).is_some()
}

pub fn decode_preset_password_h1_from_storage(
    storage: &str,
) -> Option<[u8; PERMANENT_PASSWORD_H1_LEN]> {
    decode_password_h1_after_prefix(storage, HBBS_PRESET_PASSWORD_HASH_PREFIX)
}

#[cfg(test)]
fn local_permanent_password_storage_matches_plain(storage: &str, salt: &str, input: &str) -> bool {
    if storage.is_empty() || input.is_empty() {
        return false;
    }
    if !local_permanent_password_storage_is_usable_for_auth(storage, salt) {
        return false;
    }
    if let Some(stored_h1) = decode_permanent_password_h1_from_storage(storage) {
        if salt.is_empty() {
            log::error!("Salt is empty but permanent password storage is hashed");
            return false;
        }
        let h1 = compute_permanent_password_h1(input, salt);
        return constant_time_eq_32(&h1, &stored_h1);
    }
    storage == input
}

pub(super) fn preset_permanent_password_storage_matches_plain(
    storage: &str,
    salt: &str,
    input: &str,
) -> bool {
    if storage.is_empty() || input.is_empty() {
        return false;
    }
    if salt.is_empty() {
        return storage == input;
    }
    let Some(stored_h1) = decode_preset_password_h1_from_storage(storage) else {
        return false;
    };
    let h1 = compute_permanent_password_h1(input, salt);
    constant_time_eq_32(&h1, &stored_h1)
}

pub fn decode_permanent_password_h1_from_storage(
    storage: &str,
) -> Option<[u8; PERMANENT_PASSWORD_H1_LEN]> {
    if storage.starts_with(PERMANENT_PASSWORD_ENC_VERSION) {
        let (hashed_storage, decrypted, _) = decrypt_permanent_password_str_or_original(storage);
        if !decrypted {
            return None;
        }
        return decode_permanent_password_h1_from_hashed_storage(&hashed_storage);
    }
    None
}

// Salt can be updated only when the password is empty, plaintext, or decryptable
// legacy storage. Current-prefixed storage is treated as salt-bound.
pub(super) fn password_is_empty_or_not_hashed(permanent_password_storage: &str) -> bool {
    if permanent_password_storage.is_empty() {
        return true;
    }
    if decode_permanent_password_h1_from_storage(permanent_password_storage).is_some() {
        return false;
    }
    if permanent_password_storage.starts_with(PERMANENT_PASSWORD_ENC_VERSION) {
        return false;
    }
    let (_, decrypted, looks_like_plaintext) =
        decrypt_str_or_original(permanent_password_storage, PASSWORD_ENC_VERSION);
    decrypted || looks_like_plaintext
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::password_security::encrypt_str_or_original;

    fn encode_hbbs_preset_password_storage_from_h1(h1: &[u8; PERMANENT_PASSWORD_H1_LEN]) -> String {
        HBBS_PRESET_PASSWORD_HASH_PREFIX.to_owned() + &base64::encode(h1, base64::Variant::Original)
    }

    #[test]
    fn test_permanent_password_h1_storage_roundtrip() {
        let salt = "salt123";
        let password = "p@ssw0rd";
        let h1 = compute_permanent_password_h1(password, salt);
        let stored = encode_permanent_password_storage_from_h1(&h1);
        assert!(stored.starts_with(PERMANENT_PASSWORD_HASH_PREFIX));
        assert!(is_permanent_password_hashed_storage(&stored));
        let decoded = decode_permanent_password_h1_from_hashed_storage(&stored).unwrap();
        assert_eq!(&decoded[..], &h1[..]);
    }

    #[test]
    fn test_permanent_password_encrypted_storage_uses_01_outer_and_00_inner() {
        let h1 = compute_permanent_password_h1("p@ssw0rd", "salt123");
        let storage = encode_permanent_password_encrypted_storage_from_h1(&h1).unwrap();

        assert!(storage.starts_with(PERMANENT_PASSWORD_ENC_VERSION));
        assert!(!is_permanent_password_hashed_storage(&storage));

        let (inner, decrypted, should_store) = decrypt_permanent_password_str_or_original(&storage);
        assert!(decrypted);
        assert!(!should_store);
        assert!(inner.starts_with(PERMANENT_PASSWORD_HASH_PREFIX));
        assert_eq!(
            decode_permanent_password_h1_from_storage(&storage),
            Some(h1)
        );
    }

    #[test]
    fn test_encrypted_hashed_password_storage_matches_plain_with_salt() {
        let salt = "salt123";
        let h1 = compute_permanent_password_h1("p@ssw0rd", salt);
        let storage = encode_permanent_password_encrypted_storage_from_h1(&h1).unwrap();

        assert!(local_permanent_password_storage_is_usable_for_auth(
            &storage, salt
        ));
        assert!(local_permanent_password_storage_matches_plain(
            &storage, salt, "p@ssw0rd"
        ));
        assert!(!local_permanent_password_storage_matches_plain(
            &storage, salt, "wrong"
        ));
    }

    #[test]
    fn test_hbbs_00_hashed_preset_password_storage_is_decoded_for_preset_auth() {
        let h1 = compute_permanent_password_h1("p@ssw0rd", "salt123");
        let storage = encode_hbbs_preset_password_storage_from_h1(&h1);

        assert_eq!(decode_preset_password_h1_from_storage(&storage), Some(h1));
    }

    #[test]
    fn test_hbbs_00_hashed_preset_password_storage_matches_plain_with_salt() {
        let salt = "salt123";
        let h1 = compute_permanent_password_h1("p@ssw0rd", salt);
        let storage = encode_hbbs_preset_password_storage_from_h1(&h1);

        assert!(preset_permanent_password_storage_is_usable_for_auth(
            &storage, salt
        ));
        assert!(preset_permanent_password_storage_matches_plain(
            &storage, salt, "p@ssw0rd"
        ));
        assert!(!preset_permanent_password_storage_matches_plain(
            &storage, salt, "wrong"
        ));
    }

    #[test]
    fn test_encrypted_hash_storage_is_not_accepted_as_preset_storage() {
        let salt = "salt123";
        let h1 = compute_permanent_password_h1("p@ssw0rd", salt);
        let storage = encode_permanent_password_encrypted_storage_from_h1(&h1).unwrap();

        assert!(!preset_permanent_password_storage_is_usable_for_auth(
            &storage, salt
        ));
        assert!(!preset_permanent_password_storage_matches_plain(
            &storage, salt, "p@ssw0rd"
        ));
    }

    #[test]
    fn test_hbbs_00_shaped_preset_password_without_salt_stays_plaintext() {
        let h1 = compute_permanent_password_h1("p@ssw0rd", "salt123");
        let storage = encode_hbbs_preset_password_storage_from_h1(&h1);

        assert!(preset_permanent_password_storage_is_usable_for_auth(
            &storage, ""
        ));
        assert!(preset_permanent_password_storage_matches_plain(
            &storage, "", &storage
        ));
        assert!(!preset_permanent_password_storage_matches_plain(
            &storage, "", "p@ssw0rd"
        ));
    }

    #[test]
    fn test_hashed_preset_password_storage_without_salt_is_not_usable() {
        let h1 = compute_permanent_password_h1("p@ssw0rd", "salt123");
        let storage = encode_permanent_password_storage_from_h1(&h1);

        assert!(!local_permanent_password_storage_is_usable_for_auth(
            &storage, ""
        ));
        assert!(!local_permanent_password_storage_matches_plain(
            &storage, "", "p@ssw0rd"
        ));
    }

    #[test]
    fn test_legacy_plain_preset_password_without_salt_keeps_old_behavior() {
        let storage = "01not-a-valid-hash";

        assert!(preset_permanent_password_storage_is_usable_for_auth(
            storage, ""
        ));
        assert!(preset_permanent_password_storage_matches_plain(
            storage,
            "",
            "01not-a-valid-hash"
        ));
    }

    #[test]
    fn test_malformed_preset_password_with_salt_is_not_usable_for_auth() {
        for storage in ["01not-a-valid-hash", "00not-a-valid-hash"] {
            assert!(!preset_permanent_password_storage_is_usable_for_auth(
                storage,
                "preset-salt"
            ));
            assert!(!preset_permanent_password_storage_matches_plain(
                storage,
                "preset-salt",
                storage
            ));
        }
    }

    #[test]
    fn test_invalid_current_version_storage_is_not_usable_for_auth() {
        let encrypted = symmetric_crypt(b"not-a-hash", true).unwrap();
        let encrypted_non_hash = PERMANENT_PASSWORD_ENC_VERSION.to_owned()
            + &base64::encode(encrypted, base64::Variant::Original);

        assert!(!local_permanent_password_storage_is_usable_for_auth(
            &encrypted_non_hash,
            "salt123"
        ));
        assert!(!local_permanent_password_storage_matches_plain(
            &encrypted_non_hash,
            "salt123",
            &encrypted_non_hash
        ));
    }

    #[test]
    fn test_legacy_plain_preset_password_that_decodes_as_hash_requires_salt() {
        let h1 = compute_permanent_password_h1("plain-looking-hash", "salt123");
        let storage = encode_permanent_password_storage_from_h1(&h1);

        assert!(!local_permanent_password_storage_is_usable_for_auth(
            &storage, ""
        ));
        assert!(!local_permanent_password_storage_matches_plain(
            &storage, "", &storage
        ));
    }

    #[test]
    fn test_password_is_empty_or_not_hashed_accepts_plaintext_and_decryptable_legacy_plaintext() {
        let storage =
            encrypt_str_or_original("legacy-secret", PASSWORD_ENC_VERSION, ENCRYPT_MAX_LEN);

        assert!(password_is_empty_or_not_hashed("00secret"));
        assert!(password_is_empty_or_not_hashed(&storage));
    }

    #[test]
    fn test_password_is_empty_or_not_hashed_treats_locked_00_storage_as_hashed() {
        let invalid_payload = vec![42u8; sodiumoxide::crypto::secretbox::MACBYTES + 1];
        let locked_storage = PASSWORD_ENC_VERSION.to_owned()
            + &base64::encode(invalid_payload, base64::Variant::Original);

        assert!(!password_is_empty_or_not_hashed(&locked_storage));
    }

    #[test]
    fn test_password_is_empty_or_not_hashed_treats_invalid_01_storage_as_hashed() {
        assert!(!password_is_empty_or_not_hashed("01not-a-valid-hash"));
    }
}
