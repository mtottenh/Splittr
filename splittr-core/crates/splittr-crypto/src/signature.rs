//! Detached Ed25519 signatures and their verification.

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

use crate::keys::{PublicKey, SigningKey};

/// An Ed25519 signature (64 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Signature(#[serde(with = "BigArray")] pub [u8; 64]);

/// Sign `msg` with `key`.
pub fn sign(key: &SigningKey, msg: &[u8]) -> Signature {
    Signature(key.sign_raw(msg))
}

/// Verify that `sig` is `pubkey`'s signature over `msg`. Returns `false` for any
/// malformed key/signature rather than erroring.
pub fn verify(pubkey: &PublicKey, msg: &[u8], sig: &Signature) -> bool {
    use ed25519_dalek::Verifier;
    match ed25519_dalek::VerifyingKey::from_bytes(&pubkey.0) {
        Ok(vk) => vk
            .verify(msg, &ed25519_dalek::Signature::from_bytes(&sig.0))
            .is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_then_verify_roundtrips() {
        let key = SigningKey::from_seed([1u8; 32]);
        let sig = sign(&key, b"hello");
        assert!(verify(&key.public(), b"hello", &sig));
    }

    #[test]
    fn tampered_message_fails() {
        let key = SigningKey::from_seed([1u8; 32]);
        let sig = sign(&key, b"hello");
        assert!(!verify(&key.public(), b"goodbye", &sig));
    }

    #[test]
    fn wrong_key_fails() {
        let signer = SigningKey::from_seed([1u8; 32]);
        let other = SigningKey::from_seed([2u8; 32]);
        let sig = sign(&signer, b"hi");
        assert!(!verify(&other.public(), b"hi", &sig));
    }

    #[test]
    fn signatures_are_deterministic() {
        // Ed25519 is deterministic — required for content-addressed op dedup.
        let key = SigningKey::from_seed([3u8; 32]);
        assert_eq!(sign(&key, b"x"), sign(&key, b"x"));
    }
}
