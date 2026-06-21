//! Multi-device/-user sync for Splittr (#9/#20).
//!
//! Sync is a **set-union of signed ops**: balances are never sent, only ops, and
//! every replica folds the same op-set to identical balances (ADR-0001). This
//! crate is the transport-*agnostic* core — the wire [`message`]s, the
//! [`reconcile`]iation protocol, and the [`transport`] seam ADR-0002 keeps the
//! iroh dependency behind. The iroh implementation of [`Transport`] lands behind
//! a feature flag; everything here is exercised with an in-memory transport.

mod message;
mod reconcile;
mod transport;

pub use message::SyncMessage;
pub use reconcile::{missing_ops, SyncSession, SyncStore};
pub use transport::{delta_for, sync, Role, Transport};
