//! Wires an [`OpStore`] to the CRDT [`Materializer`]: the durable op-log plus an
//! in-memory projection kept up to date as ops are appended. This is the object
//! the application/FFI layer (#23) will drive.

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
        Ok(Applied::Stored)
    }

    /// The current read model.
    pub fn projection(&self) -> Projection {
        self.materializer.projection()
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }
}
