//! The transport seam (#20): how sync messages travel between two replicas.
//!
//! [`Transport`] is the swappable boundary ADR-0002 keeps the iroh dependency
//! behind — an in-memory duplex for tests, an iroh QUIC stream in production. The
//! [`sync`] driver runs one bounded peer exchange over any transport, reusing the
//! pure [`SyncSession`] protocol so there is a single source of reconciliation
//! truth.

use splittr_crdt::Op;

use crate::message::SyncMessage;
use crate::reconcile::{SyncSession, SyncStore};

/// A bidirectional, ordered, reliable message channel to one peer (e.g. a single
/// QUIC stream). Implementations only move [`SyncMessage`]s; all reconciliation
/// semantics live in [`sync`]/[`SyncSession`].
pub trait Transport {
    /// Transport-level failure (connection dropped, peer closed, …).
    type Error;

    /// Send a message to the peer.
    fn send(
        &mut self,
        msg: SyncMessage,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;

    /// Receive the next message from the peer.
    fn recv(
        &mut self,
    ) -> impl std::future::Future<Output = Result<SyncMessage, Self::Error>> + Send;
}

/// Which side of a 1:1 exchange a replica plays. Exactly one peer initiates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Initiator,
    Responder,
}

/// Reconcile `store` with one peer over `transport` to a converged op-set.
///
/// The exchange is bounded and fixed-shape: the initiator announces its ids; each
/// side answers the other's `Have` with its own `Have` (once) plus the ops the
/// peer lacks; both ingest the ops they receive. Over an ordered, reliable
/// transport each side both sends and receives exactly two messages.
pub async fn sync<S, T>(store: &mut S, transport: &mut T, role: Role) -> Result<(), T::Error>
where
    S: SyncStore,
    T: Transport,
{
    let mut session = SyncSession::new();
    if role == Role::Initiator {
        transport.send(session.open(store)).await?;
    }
    // Peer sends its `Have` then its `Ops`; respond to each in turn.
    for _ in 0..2 {
        let msg = transport.recv().await?;
        for reply in session.handle(msg, store) {
            transport.send(reply).await?;
        }
    }
    Ok(())
}

/// The ops `store` would send a peer holding `theirs` — exposed for callers that
/// drive a custom exchange (e.g. gossip fan-out) rather than the 1:1 [`sync`].
pub fn delta_for<S: SyncStore>(store: &S, theirs: &[splittr_crdt::OpId]) -> Vec<Op> {
    crate::reconcile::missing_ops(store, theirs)
}
