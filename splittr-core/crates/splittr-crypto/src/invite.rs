//! Signed, expiring invite tokens for onboarding (#7).
//!
//! An invite is the out-of-band introduction the shell shares as a link/QR: it
//! proves *which identity* is inviting (so the recipient can preview and trust
//! the inviter's fingerprint) and carries an opaque `context` (e.g. `"friend"`
//! or a group id). It is **not** an on-log capability — accepting an invite is
//! recorded by ordinary signed ops (a friendship declaration #7, or a member
//! adding the joiner #38) — so the token never has to be verified inside the
//! convergent fold. Verification and expiry are pure functions, unit-testable
//! without any transport.

use serde::{Deserialize, Serialize};

use crate::keys::{PublicKey, SigningKey};
use crate::signature::{sign, verify, Signature};

/// A signed invitation from `inviter` for some `context`, valid until `expiry_ms`
/// (Unix ms). `nonce` makes each invite unique so a relay can single-use them.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Invite {
    pub inviter: PublicKey,
    pub context: String,
    pub nonce: [u8; 16],
    pub expiry_ms: u64,
    pub sig: Signature,
}

impl Invite {
    /// Create an invite signed by `key` (the inviter's identity).
    pub fn create(key: &SigningKey, context: String, nonce: [u8; 16], expiry_ms: u64) -> Invite {
        let inviter = key.public();
        let sig = sign(key, &canonical(&inviter, &context, &nonce, expiry_ms));
        Invite {
            inviter,
            context,
            nonce,
            expiry_ms,
            sig,
        }
    }

    /// Verify the inviter's signature over the token's content. Does **not**
    /// check expiry (no clock here) — use [`is_expired`](Invite::is_expired).
    pub fn verify(&self) -> bool {
        verify(
            &self.inviter,
            &canonical(&self.inviter, &self.context, &self.nonce, self.expiry_ms),
            &self.sig,
        )
    }

    /// Whether the invite has expired as of `now_ms` (Unix ms).
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.expiry_ms
    }

    /// Encode for sharing as a link/QR payload.
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("invite encoding is infallible")
    }

    /// Decode a shared invite; `None` if the bytes are malformed.
    pub fn from_bytes(bytes: &[u8]) -> Option<Invite> {
        postcard::from_bytes(bytes).ok()
    }
}

/// The canonical bytes signed by an invite — domain-separated so an invite
/// signature can never be confused with an op signature.
fn canonical(inviter: &PublicKey, context: &str, nonce: &[u8; 16], expiry_ms: u64) -> Vec<u8> {
    let mut v = Vec::with_capacity(64 + context.len());
    v.extend_from_slice(b"splittr-invite-v1");
    v.extend_from_slice(&inviter.0);
    v.extend_from_slice(&(context.len() as u64).to_le_bytes());
    v.extend_from_slice(context.as_bytes());
    v.extend_from_slice(nonce);
    v.extend_from_slice(&expiry_ms.to_le_bytes());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alice() -> SigningKey {
        SigningKey::from_seed([1u8; 32])
    }

    fn invite() -> Invite {
        Invite::create(&alice(), "friend".into(), [9u8; 16], 1_000)
    }

    #[test]
    fn create_then_verify() {
        let i = invite();
        assert!(i.verify());
        assert_eq!(i.inviter, alice().public());
    }

    #[test]
    fn tampering_any_field_breaks_verification() {
        for mutate in [
            (|i: &mut Invite| i.context = "group:x".into()) as fn(&mut Invite),
            |i| i.nonce = [0u8; 16],
            |i| i.expiry_ms = 2_000,
            |i| i.inviter = SigningKey::from_seed([2u8; 32]).public(),
        ] {
            let mut i = invite();
            mutate(&mut i);
            assert!(!i.verify(), "a tampered invite must not verify");
        }
    }

    #[test]
    fn expiry_is_checked_against_a_clock() {
        let i = invite(); // expires at 1_000
        assert!(!i.is_expired(999));
        assert!(i.is_expired(1_000));
        assert!(i.is_expired(5_000));
    }

    #[test]
    fn encodes_and_decodes_round_trip() {
        let i = invite();
        let bytes = i.to_bytes();
        assert_eq!(Invite::from_bytes(&bytes), Some(i));
        assert_eq!(Invite::from_bytes(b"not an invite"), None);
    }
}
