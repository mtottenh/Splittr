//! The local user's identity and device keys (#6/#16, ADR-0004/0005).
//!
//! - The **identity (root) key** (Ed25519) is the stable user id; it derives the
//!   X25519 agreement key and signs only device certificates / revocations.
//! - The **device key** (Ed25519) signs all normal ops; revoking a device blocks
//!   its future ops without touching the identity.
//!
//! The root key is held **encrypted at rest** on the primary device (#34): for
//! daily use the [`Identity`] runs with the root **locked** — the device key
//! still signs ops and the public identity (user id) is known — and only
//! privileged actions (enrol/revoke a device) need it [`unlock`](Identity::unlock)ed.
//! Secure storage of the seeds is a platform concern (#22/#34).

use splittr_crdt::{user_id_for, AgreementKey, AgreementPublic, PublicKey, SigningKey, UserId};

/// The secret half of the identity, present only while the root is unlocked.
struct RootSecret {
    root: SigningKey,
    agreement: AgreementKey,
}

pub struct Identity {
    identity_pub: PublicKey,
    user_id: UserId,
    device: SigningKey,
    /// `None` when the root is locked (sealed at rest); daily ops don't need it.
    root: Option<RootSecret>,
}

impl Identity {
    /// Build from distinct identity and device seeds, with the root **unlocked**.
    pub fn from_seeds(identity_seed: [u8; 32], device_seed: [u8; 32]) -> Self {
        let root = SigningKey::from_seed(identity_seed);
        let identity_pub = root.public();
        Self {
            user_id: user_id_for(&identity_pub),
            identity_pub,
            device: SigningKey::from_seed(device_seed),
            root: Some(RootSecret {
                root,
                agreement: AgreementKey::from_seed(identity_seed),
            }),
        }
    }

    /// Build for daily operation with the root **locked**: the device key signs
    /// ops and the public identity is known, but privileged actions need an
    /// [`unlock`](Identity::unlock) (#34). `identity_pub` is the (public)
    /// identity key the device is already enrolled under.
    pub fn device_only(identity_pub: PublicKey, device_seed: [u8; 32]) -> Self {
        Self {
            user_id: user_id_for(&identity_pub),
            identity_pub,
            device: SigningKey::from_seed(device_seed),
            root: None,
        }
    }

    /// Convenience for tests: derive a distinct device seed from the identity
    /// seed so the device key is never equal to the root key.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self::from_seeds(seed, device_seed_from(seed))
    }

    /// Generate a fresh identity + device from the OS CSPRNG (root unlocked).
    pub fn generate() -> Self {
        let root = SigningKey::generate();
        let identity_pub = root.public();
        Self {
            user_id: user_id_for(&identity_pub),
            identity_pub,
            device: SigningKey::generate(),
            root: Some(RootSecret {
                root,
                agreement: AgreementKey::generate(),
            }),
        }
    }

    /// Unlock the root from its seed (e.g. after the app lock decrypts the
    /// vault). Returns `false` if the seed does not match this identity.
    #[must_use]
    pub fn unlock(&mut self, identity_seed: [u8; 32]) -> bool {
        let root = SigningKey::from_seed(identity_seed);
        if root.public() != self.identity_pub {
            return false;
        }
        self.root = Some(RootSecret {
            root,
            agreement: AgreementKey::from_seed(identity_seed),
        });
        true
    }

    /// Drop the root secret, returning to daily (locked) operation.
    pub fn lock(&mut self) {
        self.root = None;
    }

    /// Whether the root is currently available for privileged actions.
    pub fn root_unlocked(&self) -> bool {
        self.root.is_some()
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    /// The identity (root) public key.
    pub fn public(&self) -> PublicKey {
        self.identity_pub
    }

    /// This device's public key.
    pub fn device_public(&self) -> PublicKey {
        self.device.public()
    }

    /// The X25519 public key peers use to encrypt content to this user (#14).
    /// Only derivable while the root is unlocked (it comes from the root seed);
    /// otherwise read it from the published projection.
    pub fn agreement_public(&self) -> Option<AgreementPublic> {
        self.root.as_ref().map(|r| r.agreement.public())
    }

    /// Signs normal ops.
    pub(crate) fn device_key(&self) -> &SigningKey {
        &self.device
    }

    /// Signs device certificates / revocations only — `None` if locked.
    pub(crate) fn root_key(&self) -> Option<&SigningKey> {
        self.root.as_ref().map(|r| &r.root)
    }
}

/// Derive a distinct device seed from an identity seed (test/dev convenience).
fn device_seed_from(mut seed: [u8; 32]) -> [u8; 32] {
    seed[0] ^= 0xff;
    seed
}
