//! `splittr-crypto` — identity keys, signatures, AEAD and key agreement.
//!
//! A leaf crate (no dependency on the rest of the engine). Ops are signed by a
//! [`SigningKey`] and authored by its [`PublicKey`]; [`verify`] checks a
//! [`Signature`] against the author and content. [`seal`]/[`open`] provide
//! symmetric encryption at rest (#22); [`AgreementKey`] provides X25519 key
//! agreement (#6) that E2E content encryption (#14) builds on. [`seal_seed`]/
//! [`open_seed`] keep the root seed encrypted under the app-lock passphrase
//! (#34); [`PairingTranscript`] derives the device-enrolment short
//! authentication string (#35). Key storage and device certificates remain
//! platform concerns (#6/#16).

mod aead;
mod agreement;
mod keys;
mod pairing;
mod recovery;
mod signature;
mod vault;

pub use aead::{open, seal, AeadKey};
pub use agreement::{AgreementKey, AgreementPublic};
pub use keys::{PublicKey, SigningKey};
pub use pairing::PairingTranscript;
pub use recovery::{recovery_phrase, seed_from_phrase};
pub use signature::{sign, verify, Signature};
pub use vault::{open_seed, seal_seed};
