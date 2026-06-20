//! `splittr-crypto` — identity keys and Ed25519 signatures for ops.
//!
//! A leaf crate (no dependency on the rest of the engine). Ops are signed by a
//! [`SigningKey`] and authored by its [`PublicKey`]; [`verify`] checks a
//! [`Signature`] against the author and content. Key storage, device
//! certificates and E2E content encryption build on these primitives (#6/#14/#16).

mod keys;
mod signature;

pub use keys::{PublicKey, SigningKey};
pub use signature::{sign, verify, Signature};
