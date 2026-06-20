//! The local user's identity and device keys (#6/#16, ADR-0004/0005).
//!
//! - The **identity (root) key** (Ed25519) is the stable user id; it derives the
//!   X25519 agreement key and signs only device certificates / revocations.
//! - The **device key** (Ed25519) signs all normal ops; revoking a device blocks
//!   its future ops without touching the identity.
//!
//! Secure storage of the seeds is a platform concern (#22/#34).

use splittr_crdt::{AgreementKey, AgreementPublic, PublicKey, SigningKey, UserId};

pub struct Identity {
    root: SigningKey,
    agreement: AgreementKey,
    device: SigningKey,
    user_id: UserId,
}

impl Identity {
    /// Build from distinct identity and device seeds.
    pub fn from_seeds(identity_seed: [u8; 32], device_seed: [u8; 32]) -> Self {
        let root = SigningKey::from_seed(identity_seed);
        let user_id = user_id_for(&root.public());
        Self {
            root,
            agreement: AgreementKey::from_seed(identity_seed),
            device: SigningKey::from_seed(device_seed),
            user_id,
        }
    }

    /// Convenience for tests: derive a distinct device seed from the identity
    /// seed so the device key is never equal to the root key.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let mut device_seed = seed;
        device_seed[0] ^= 0xff;
        Self::from_seeds(seed, device_seed)
    }

    /// Generate a fresh identity + device from the OS CSPRNG.
    pub fn generate() -> Self {
        let root = SigningKey::generate();
        let user_id = user_id_for(&root.public());
        Self {
            root,
            agreement: AgreementKey::generate(),
            device: SigningKey::generate(),
            user_id,
        }
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    /// The identity (root) public key.
    pub fn public(&self) -> PublicKey {
        self.root.public()
    }

    /// This device's public key.
    pub fn device_public(&self) -> PublicKey {
        self.device.public()
    }

    /// The X25519 public key peers use to encrypt content to this user (#14).
    pub fn agreement_public(&self) -> AgreementPublic {
        self.agreement.public()
    }

    /// Signs normal ops.
    pub(crate) fn device_key(&self) -> &SigningKey {
        &self.device
    }

    /// Signs device certificates / revocations only (#16/ADR-0005).
    pub(crate) fn root_key(&self) -> &SigningKey {
        &self.root
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
