//! Identity / device keys (Ed25519). A public key is a stable, portable
//! identifier (it will become the user/device id in #6/#16).

use serde::{Deserialize, Serialize};

/// An Ed25519 public key — the verifiable identity of an op's author.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PublicKey(pub [u8; 32]);

/// An Ed25519 signing key. Holds secret material, so it is intentionally **not**
/// serializable — secure storage of keys is a platform concern (#16/#22).
pub struct SigningKey {
    inner: ed25519_dalek::SigningKey,
}

impl SigningKey {
    /// Deterministically derive a key from a 32-byte seed (used in tests and for
    /// reproducible key derivation).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            inner: ed25519_dalek::SigningKey::from_bytes(&seed),
        }
    }

    /// Generate a fresh key from the OS CSPRNG.
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).expect("OS RNG is available");
        Self::from_seed(seed)
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.inner.verifying_key().to_bytes())
    }

    /// Raw signature bytes over `msg`. Used by [`crate::sign`].
    pub(crate) fn sign_raw(&self, msg: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        self.inner.sign(msg).to_bytes()
    }
}
