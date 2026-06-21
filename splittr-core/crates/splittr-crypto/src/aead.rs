//! Symmetric authenticated encryption for data at rest (#22).
//!
//! XChaCha20-Poly1305 with a random 192-bit nonce, so callers never have to
//! manage nonce uniqueness. The key is supplied by the platform (a keystore-held
//! secret or a passphrase-derived key); this crate is key-source agnostic.
//!
//! Every blob is **version-tagged** so the scheme can evolve without a flag day:
//! layout is `version(1) || nonce(24) || ciphertext+tag`. `open` rejects an
//! unknown version, and a future format adds a new tag rather than reinterpreting
//! old bytes.

use chacha20poly1305::aead::Aead;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};
use zeroize::Zeroize;

const NONCE_LEN: usize = 24;

/// At-rest AEAD format version. Bump (and branch in [`open`]) on any change to
/// the nonce length, cipher, or layout.
pub(crate) const AEAD_V1: u8 = 1;

/// A 256-bit symmetric key for at-rest encryption. Wiped from memory on drop;
/// not `Clone`, so copies of key material don't proliferate silently.
pub struct AeadKey([u8; 32]);

impl AeadKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl Drop for AeadKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Encrypt `plaintext`, returning `version || nonce || ciphertext`.
pub fn seal(key: &AeadKey, plaintext: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key.0));
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce).expect("OS RNG is available");
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .expect("XChaCha20-Poly1305 encryption does not fail for a valid key");
    let mut out = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
    out.push(AEAD_V1);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    out
}

/// Decrypt a [`seal`]ed blob. Returns `None` if the version is unknown, the key
/// is wrong, the data was tampered with (authentication failure), or the blob is
/// truncated.
pub fn open(key: &AeadKey, blob: &[u8]) -> Option<Vec<u8>> {
    let (&version, rest) = blob.split_first()?;
    if version != AEAD_V1 || rest.len() < NONCE_LEN {
        return None;
    }
    let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key.0));
    cipher.decrypt(XNonce::from_slice(nonce), ciphertext).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let key = AeadKey::new([7u8; 32]);
        let blob = seal(&key, b"amounts and names");
        assert_eq!(
            open(&key, &blob).as_deref(),
            Some(&b"amounts and names"[..])
        );
    }

    #[test]
    fn distinct_nonces_per_call() {
        let key = AeadKey::new([1u8; 32]);
        // Same plaintext encrypts to different blobs (random nonce).
        assert_ne!(seal(&key, b"x"), seal(&key, b"x"));
    }

    #[test]
    fn wrong_key_fails() {
        let blob = seal(&AeadKey::new([1u8; 32]), b"secret");
        assert!(open(&AeadKey::new([2u8; 32]), &blob).is_none());
    }

    #[test]
    fn tampering_fails() {
        let key = AeadKey::new([3u8; 32]);
        let mut blob = seal(&key, b"secret");
        *blob.last_mut().unwrap() ^= 0xff;
        assert!(open(&key, &blob).is_none());
    }

    #[test]
    fn blob_is_version_tagged_and_unknown_versions_are_rejected() {
        let key = AeadKey::new([5u8; 32]);
        let mut blob = seal(&key, b"secret");
        assert_eq!(blob[0], AEAD_V1, "first byte is the format version");
        blob[0] = 0xfe; // an unknown future version
        assert!(open(&key, &blob).is_none());
    }

    #[test]
    fn rejects_empty_blob() {
        assert!(open(&AeadKey::new([0u8; 32]), &[]).is_none());
    }
}
