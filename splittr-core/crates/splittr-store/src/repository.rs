//! Wires an [`OpStore`] to the CRDT [`Materializer`]: the durable op-log plus the
//! read-model [`Projection`], recomputed on demand and **cached between appends**
//! (invalidated whenever a new op is stored). This is the object the
//! application/FFI layer (#23) drives.

use std::cell::RefCell;

use splittr_crdt::{Materializer, Op, Projection};

use crate::error::Result;
use crate::op_store::OpStore;

/// Outcome of appending an op.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Applied {
    /// Newly stored and folded into the projection.
    Stored,
    /// Already present (same content id) — a no-op.
    Duplicate,
    /// Failed verification (forged/tampered) — not stored, not folded.
    Rejected,
}

pub struct Repository<S: OpStore> {
    store: S,
    materializer: Materializer,
    /// Memoized fold, dropped on every successful append. Recomputed lazily on
    /// the next read so a burst of queries between commands folds at most once.
    cached: RefCell<Option<Projection>>,
}

impl<S: OpStore> Repository<S> {
    /// Open a repository over `store`, rebuilding the projection by replaying
    /// the persisted op-log through the materializer.
    pub fn open(store: S) -> Result<Self> {
        let mut materializer = Materializer::default();
        for op in store.ops()? {
            materializer.apply(&op);
        }
        Ok(Self {
            store,
            materializer,
            cached: RefCell::new(None),
        })
    }

    /// Verify, persist, and fold `op` into the projection. This is the trust
    /// boundary: a forged/tampered op is [`Applied::Rejected`] (never stored).
    /// A duplicate (same content id) is [`Applied::Duplicate`]. Idempotent.
    pub fn append(&mut self, op: &Op) -> Result<Applied> {
        if !op.verify() {
            return Ok(Applied::Rejected);
        }
        if !self.store.append(op)? {
            return Ok(Applied::Duplicate);
        }
        self.materializer.apply(op);
        *self.cached.borrow_mut() = None; // invalidate; recompute on next read
        Ok(Applied::Stored)
    }

    /// The current read model — served from the memoized fold, recomputing it
    /// once if the cache was invalidated by an append.
    pub fn projection(&self) -> Projection {
        let mut slot = self.cached.borrow_mut();
        if slot.is_none() {
            *slot = Some(self.materializer.projection());
        }
        slot.as_ref().expect("just populated").clone()
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }
}
