//! The local user's identity: a signing key plus the stable user id derived
//! from its public key. Secure storage of the key is a platform concern
//! (#16/#22); here the key is held in memory and supplied at construction.

use splittr_crdt::{AgreementKey, AgreementPublic, PublicKey, SigningKey, UserId};

pub struct Identity {
    key: SigningKey,
    /// X25519 key for content encryption (#14), derived from the same seed.
    agreement: AgreementKey,
    user_id: UserId,
}

impl Identity {
    /// Build from a 32-byte seed (both the Ed25519 signing key and the X25519
    /// agreement key are derived from it).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let key = SigningKey::from_seed(seed);
        let user_id = user_id_for(&key.public());
        Self {
            key,
            agreement: AgreementKey::from_seed(seed),
            user_id,
        }
    }

    /// Generate a fresh identity from the OS CSPRNG.
    pub fn generate() -> Self {
        let key = SigningKey::generate();
        let user_id = user_id_for(&key.public());
        Self {
            key,
            agreement: AgreementKey::generate(),
            user_id,
        }
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn public(&self) -> PublicKey {
        self.key.public()
    }

    /// The X25519 public key peers use to encrypt content to this user (#14).
    pub fn agreement_public(&self) -> AgreementPublic {
        self.agreement.public()
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
