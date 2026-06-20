//! `splittr-crypto` — identity keys, signatures, AEAD and key agreement.
//!
//! A leaf crate (no dependency on the rest of the engine). Ops are signed by a
//! [`SigningKey`] and authored by its [`PublicKey`]; [`verify`] checks a
//! [`Signature`] against the author and content. [`seal`]/[`open`] provide
//! symmetric encryption at rest (#22); [`AgreementKey`] provides X25519 key
//! agreement (#6) that E2E content encryption (#14) builds on. Key storage and
//! device certificates remain platform concerns (#6/#16).

mod aead;
mod agreement;
mod keys;
mod recovery;
mod signature;

pub use aead::{open, seal, AeadKey};
pub use agreement::{AgreementKey, AgreementPublic};
pub use keys::{PublicKey, SigningKey};
pub use recovery::{recovery_phrase, seed_from_phrase};
pub use signature::{sign, verify, Signature};
