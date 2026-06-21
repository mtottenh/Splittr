//! Passphrase-sealed secret storage for the identity (root) seed (#34).
//!
//! The root key is kept encrypted on the primary device (ADR-0005). Daily ops
//! use the device key, so the root only needs unsealing for privileged actions
//! (enrol/revoke a device). We seal it under a key derived from the app-lock
//! passphrase with **Argon2id** — a deliberately slow, memory-hard KDF — so a
//! low-entropy PIN still resists offline brute force if the blob leaks. The
//! sealed blob then rides on the same XChaCha20-Poly1305 AEAD used at rest.
//!
//! Blob layout: `version(1) || salt(16) || seal(aead_key, seed)`, where
//! `aead_key = Argon2id(passphrase, salt)` with **explicitly pinned** parameters
//! (see [`derive_key`]). Pinning matters: if the `argon2` crate ever changed its
//! defaults, `Argon2::default()` would derive a *different* key from the same
//! passphrase+salt and silently make every sealed seed undecryptable. The
//! version byte lets the params/format be migrated deliberately instead.

use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use crate::aead::{open, seal, AeadKey};

const SALT_LEN: usize = 16;

/// Vault blob format version. Bump (and branch in [`open_seed`]) on any change to
/// the Argon2 params, salt length, or layout — changing the KDF params is a
/// versioned migration, never a silent default shift.
const VAULT_V1: u8 = 1;

/// Seal a 32-byte `seed` under `passphrase`, returning `version || salt || ciphertext`.
pub fn seal_seed(passphrase: &str, seed: &[u8; 32]) -> Vec<u8> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::getrandom(&mut salt).expect("OS RNG is available");
    let key = derive_key(passphrase, &salt);
    let sealed = seal(&key, seed);
    let mut out = Vec::with_capacity(1 + SALT_LEN + sealed.len());
    out.push(VAULT_V1);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&sealed);
    out
}

/// Open a [`seal_seed`] blob. Returns `None` if the version is unknown, the
/// passphrase is wrong, the blob was tampered with, or it is truncated/malformed.
pub fn open_seed(passphrase: &str, blob: &[u8]) -> Option<[u8; 32]> {
    let (&version, rest) = blob.split_first()?;
    if version != VAULT_V1 || rest.len() < SALT_LEN {
        return None;
    }
    let (salt, sealed) = rest.split_at(SALT_LEN);
    let key = derive_key(passphrase, salt);
    let plaintext = Zeroizing::new(open(&key, sealed)?);
    let seed: [u8; 32] = plaintext.as_slice().try_into().ok()?;
    Some(seed)
}

/// Argon2id with pinned parameters (m=19456 KiB, t=2, p=1, 32-byte output).
/// These match the `argon2 0.5` defaults today, but are stated explicitly so a
/// future crate bump cannot change them out from under existing vaults.
fn derive_key(passphrase: &str, salt: &[u8]) -> AeadKey {
    let params = Params::new(19_456, 2, 1, Some(32)).expect("valid argon2 params");
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut_slice())
        .expect("Argon2id derivation does not fail for valid params");
    AeadKey::new(*key)
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
        assert!(open_seed("pw", &[]).is_none());
    }

    #[test]
    fn blob_is_version_tagged_and_unknown_versions_are_rejected() {
        let mut blob = seal_seed("pw", &[4u8; 32]);
        assert_eq!(blob[0], VAULT_V1, "first byte is the vault format version");
        blob[0] = 0x7f; // an unknown future version
        assert!(open_seed("pw", &blob).is_none());
    }
}
