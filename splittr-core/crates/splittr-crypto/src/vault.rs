//! Passphrase-sealed secret storage for the identity (root) seed (#34).
//!
//! The root key is kept encrypted on the primary device (ADR-0005). Daily ops
//! use the device key, so the root only needs unsealing for privileged actions
//! (enrol/revoke a device). We seal it under a key derived from the app-lock
//! passphrase with **Argon2id** — a deliberately slow, memory-hard KDF — so a
//! low-entropy PIN still resists offline brute force if the blob leaks. The
//! sealed blob then rides on the same XChaCha20-Poly1305 AEAD used at rest.
//!
//! Blob layout: `salt(16) || seal(aead_key, seed)`, where
//! `aead_key = Argon2id(passphrase, salt)`.

use argon2::Argon2;

use crate::aead::{open, seal, AeadKey};

const SALT_LEN: usize = 16;

/// Seal a 32-byte `seed` under `passphrase`, returning `salt || ciphertext`.
pub fn seal_seed(passphrase: &str, seed: &[u8; 32]) -> Vec<u8> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::getrandom(&mut salt).expect("OS RNG is available");
    let key = derive_key(passphrase, &salt);
    let sealed = seal(&key, seed);
    let mut out = Vec::with_capacity(SALT_LEN + sealed.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&sealed);
    out
}

/// Open a [`seal_seed`] blob. Returns `None` if the passphrase is wrong, the
/// blob was tampered with, or it is truncated/malformed.
pub fn open_seed(passphrase: &str, blob: &[u8]) -> Option<[u8; 32]> {
    if blob.len() < SALT_LEN {
        return None;
    }
    let (salt, sealed) = blob.split_at(SALT_LEN);
    let key = derive_key(passphrase, salt);
    let plaintext = open(&key, sealed)?;
    let seed: [u8; 32] = plaintext.try_into().ok()?;
    Some(seed)
}

fn derive_key(passphrase: &str, salt: &[u8]) -> AeadKey {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .expect("Argon2id derivation does not fail for valid params");
    AeadKey::new(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let seed = [9u8; 32];
        let blob = seal_seed("1234", &seed);
        assert_eq!(open_seed("1234", &blob), Some(seed));
    }

    #[test]
    fn wrong_passphrase_fails() {
        let blob = seal_seed("1234", &[1u8; 32]);
        assert!(open_seed("0000", &blob).is_none());
    }

    #[test]
    fn tampering_fails() {
        let mut blob = seal_seed("1234", &[2u8; 32]);
        *blob.last_mut().unwrap() ^= 0xff;
        assert!(open_seed("1234", &blob).is_none());
    }

    #[test]
    fn distinct_salts_per_call() {
        // Same passphrase + seed seals to different blobs (random salt), so the
        // ciphertext never reveals that two vaults hold the same secret.
        assert_ne!(seal_seed("pw", &[3u8; 32]), seal_seed("pw", &[3u8; 32]));
    }

    #[test]
    fn rejects_truncated_blob() {
        assert!(open_seed("pw", &[0u8; 4]).is_none());
    }
}
