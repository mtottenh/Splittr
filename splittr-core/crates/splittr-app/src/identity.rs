//! The local user's identity: a signing key plus the stable user id derived
//! from its public key. Secure storage of the key is a platform concern
//! (#16/#22); here the key is held in memory and supplied at construction.

use splittr_crdt::{PublicKey, SigningKey, UserId};

pub struct Identity {
    key: SigningKey,
    user_id: UserId,
}

impl Identity {
    pub fn new(key: SigningKey) -> Self {
        let user_id = user_id_for(&key.public());
        Self { key, user_id }
    }

    /// Deterministically derive an identity from a 32-byte seed (tests / key
    /// derivation).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self::new(SigningKey::from_seed(seed))
    }

    /// Generate a fresh identity from the OS CSPRNG.
    pub fn generate() -> Self {
        Self::new(SigningKey::generate())
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn public(&self) -> PublicKey {
        self.key.public()
    }

    pub(crate) fn key(&self) -> &SigningKey {
        &self.key
    }
}

/// Derive a stable [`UserId`] from a public key — a real user's id *is* their
/// key. Placeholder people (#2) get generated ids instead.
pub fn user_id_for(public_key: &PublicKey) -> UserId {
    let mut hex = String::with_capacity(64);
    for byte in public_key.0 {
        hex.push_str(&format!("{byte:02x}"));
    }
    UserId::new(format!("id:{hex}"))
}
