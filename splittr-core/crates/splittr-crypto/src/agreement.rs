//! X25519 key agreement primitives (#6) — the basis for end-to-end encryption
//! of synced ops (#14). Identity-adjacent: a peer publishes its agreement public
//! key so others can derive a shared secret without a prior channel.
//!
//! Not yet wired into the engine; provided as a tested primitive that #14 builds
//! group/recipient keys on top of.

use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

/// An X25519 public key, shareable for key agreement.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AgreementPublic(pub [u8; 32]);

/// An X25519 secret key. Holds secret material, so it is intentionally **not**
/// serializable — secure storage is a platform concern (#6/#16/#22).
pub struct AgreementKey {
    secret: StaticSecret,
}

impl AgreementKey {
    /// Deterministically derive a key from a 32-byte seed.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            secret: StaticSecret::from(seed),
        }
    }

    /// Generate a fresh key from the OS CSPRNG.
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).expect("OS RNG is available");
        Self::from_seed(seed)
    }

    pub fn public(&self) -> AgreementPublic {
        AgreementPublic(XPublicKey::from(&self.secret).to_bytes())
    }

    /// The raw shared secret with `peer`. Run it through a KDF before using it as
    /// an encryption key (that wrapping is #14's job).
    pub fn diffie_hellman(&self, peer: &AgreementPublic) -> [u8; 32] {
        self.secret
            .diffie_hellman(&XPublicKey::from(peer.0))
            .to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_parties_derive_the_same_secret() {
        let alice = AgreementKey::from_seed([1u8; 32]);
        let bob = AgreementKey::from_seed([2u8; 32]);
        let a = alice.diffie_hellman(&bob.public());
        let b = bob.diffie_hellman(&alice.public());
        assert_eq!(a, b);
    }

    #[test]
    fn distinct_keys_derive_distinct_secrets() {
        let alice = AgreementKey::from_seed([1u8; 32]);
        let bob = AgreementKey::from_seed([2u8; 32]);
        let carol = AgreementKey::from_seed([3u8; 32]);
        assert_ne!(
            alice.diffie_hellman(&bob.public()),
            alice.diffie_hellman(&carol.public())
        );
    }

    #[test]
    fn from_seed_is_deterministic() {
        assert_eq!(
            AgreementKey::from_seed([9u8; 32]).public(),
            AgreementKey::from_seed([9u8; 32]).public()
        );
    }
}
