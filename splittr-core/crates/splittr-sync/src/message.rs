//! The sync wire protocol: the messages two replicas exchange to converge their
//! op-sets (#9/#20).
//!
//! Balances are never sent — only signed ops. Because the fold is
//! order-independent and ops are content-addressed (ADR-0001), reconciliation is
//! a set-union: each side learns the other's op-ids, then sends the ops the other
//! is missing. Two replicas with the same op-set fold to identical balances.

use serde::{Deserialize, Serialize};
use splittr_crdt::{Op, OpId};

/// A single sync message. Encoded with postcard for the wire (same canonical,
/// deterministic encoding the op-log uses).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SyncMessage {
    /// "I hold exactly these op-ids." Opens a round and answers the peer's open.
    Have(Vec<OpId>),
    /// "Here are ops you were missing." May be empty (a no-op terminator).
    Ops(Vec<Op>),
}

impl SyncMessage {
    /// Encode for the wire.
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("sync message encoding is infallible")
    }

    /// Decode a wire message; `None` if the bytes are malformed.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }
}
