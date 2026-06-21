//! Mapping a public key to its canonical [`UserId`] (#6/#38).
//!
//! A real user's id *is* their identity (root) public key: `id:<hex>`. This is
//! the single definition shared by the fold (which resolves an op's author to a
//! user id to check entitlement) and the app layer (which derives the local
//! user's id). Placeholder people (#2) get generated `user:<uuid>` ids instead.

use splittr_crypto::PublicKey;
use splittr_domain::UserId;

/// Lowercase hex of a 32-byte value, allocated once.
pub fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(64);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// The stable [`UserId`] for an identity (root) public key.
pub fn user_id_for(public_key: &PublicKey) -> UserId {
    UserId::new(format!("id:{}", hex32(&public_key.0)))
}

/// Whether a user id is a placeholder person (#2) rather than a real identity.
pub fn is_placeholder(user: &UserId) -> bool {
    user.0.starts_with("user:")
}
