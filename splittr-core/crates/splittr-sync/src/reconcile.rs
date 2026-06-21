//! Transport-agnostic op-set reconciliation — the semantics-bearing core of sync
//! (#9/#20), independent of how messages travel (in-memory, iroh, …).
//!
//! [`SyncStore`] is the seam onto a replica's op-log; [`SyncSession`] is a pure
//! state machine that drives a replica to convergence by exchanging
//! [`SyncMessage`]s. Keeping it pure (no I/O, no async) makes convergence
//! example- and property-testable without a network.

use std::collections::BTreeSet;

use splittr_crdt::{Op, OpId};
use splittr_store::{Applied, OpStore, Repository};

use crate::message::SyncMessage;

/// A replica's op-log, as far as sync is concerned: enumerate held ops, fetch one
/// by id, and ingest an inbound one.
pub trait SyncStore {
    /// The ids of every op this replica holds.
    fn op_ids(&self) -> Vec<OpId>;

    /// The op with this id, if held.
    fn get_op(&self, id: &OpId) -> Option<Op>;

    /// Verify and apply an inbound op, returning `true` if it was newly applied.
    ///
    /// Implementations **must** authenticate the op (`Op::verify`) before storing
    /// — this is the trust boundary for ops arriving from the network. A
    /// forged/duplicate op returns `false`. Entitlement (ADR-0006) is enforced by
    /// the fold, so storing a signed-but-unauthorized op is harmless: it simply
    /// doesn't count toward balances.
    fn ingest(&mut self, op: &Op) -> bool;
}

/// The ops in `store` whose id is not in `theirs` — i.e. the ops the peer is
/// missing. This is the only op data that crosses the wire (the "delta").
pub fn missing_ops<S: SyncStore + ?Sized>(store: &S, theirs: &[OpId]) -> Vec<Op> {
    let known: BTreeSet<&OpId> = theirs.iter().collect();
    store
        .op_ids()
        .into_iter()
        .filter(|id| !known.contains(id))
        .filter_map(|id| store.get_op(&id))
        .collect()
}

/// A bounded, order-independent reconciliation session for one side of a peer
/// exchange. The initiator [`open`](Self::open)s with its ids; each side answers
/// a peer's `Have` with its own `Have` (once) plus the ops the peer lacks, and
/// ingests any `Ops` it receives. The exchange quiesces after both `Have`s and
/// both `Ops` have crossed.
#[derive(Default)]
pub struct SyncSession {
    sent_have: bool,
}

impl SyncSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a sync as the initiator: announce our ids.
    pub fn open<S: SyncStore + ?Sized>(&mut self, store: &S) -> SyncMessage {
        self.sent_have = true;
        SyncMessage::Have(store.op_ids())
    }

    /// Handle an inbound message against `store`, returning the messages to send
    /// back (possibly none). Applying ops is idempotent and dedups by id.
    pub fn handle<S: SyncStore + ?Sized>(
        &mut self,
        msg: SyncMessage,
        store: &mut S,
    ) -> Vec<SyncMessage> {
        match msg {
            SyncMessage::Have(theirs) => {
                let mut out = Vec::new();
                // Reciprocate our ids exactly once, so an opening `Have` gets a
                // `Have` back without an endless ping-pong.
                if !self.sent_have {
                    self.sent_have = true;
                    out.push(SyncMessage::Have(store.op_ids()));
                }
                // Always answer with the peer's delta (even empty) so the exchange
                // has a fixed shape: each `Have` yields exactly one `Ops` back.
                out.push(SyncMessage::Ops(missing_ops(store, &theirs)));
                out
            }
            SyncMessage::Ops(ops) => {
                for op in &ops {
                    store.ingest(op);
                }
                Vec::new()
            }
        }
    }
}

/// A live replica (durable or in-memory op-log + the conflict-resolving fold) is
/// a [`SyncStore`]: `ingest` runs the full trust boundary — `Op::verify` then
/// fold — via [`Repository::append`].
impl<S: OpStore> SyncStore for Repository<S> {
    fn op_ids(&self) -> Vec<OpId> {
        self.store()
            .ops()
            .map(|ops| ops.into_iter().map(|o| o.id).collect())
            .unwrap_or_default()
    }

    fn get_op(&self, id: &OpId) -> Option<Op> {
        self.store().get(id).ok().flatten()
    }

    fn ingest(&mut self, op: &Op) -> bool {
        matches!(self.append(op), Ok(Applied::Stored))
    }
}
