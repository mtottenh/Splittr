//! Multi-device/-user sync for Splittr (#9/#20).
//!
//! Sync is a **set-union of signed ops**: balances are never sent, only ops, and
//! every replica folds the same op-set to identical balances (ADR-0001).
//!
//! - [`message`] / [`reconcile`] — the wire protocol and the transport-agnostic
//!   reconciliation (a pure, property-tested [`SyncSession`]).
//! - [`transport`] — the [`Transport`] seam ADR-0002 keeps the iroh dependency
//!   behind, plus the bounded [`sync`] driver.
//! - [`framed`] — a [`FramedTransport`] over any async byte stream; an iroh QUIC
//!   bi-stream is exactly such a stream, so the iroh transport is a thin,
//!   documented wrapper with no extra protocol code.
//!
//! Everything is exercised over in-memory transports (a message duplex and a byte
//! duplex), so correctness is verified without a network; only the iroh
//! connection *setup* lives outside this crate.

mod framed;
mod message;
mod reconcile;
mod transport;

pub use framed::{FramedError, FramedTransport};
pub use message::SyncMessage;
pub use reconcile::{missing_ops, SyncSession, SyncStore};
pub use transport::{delta_for, sync, Role, Transport};
