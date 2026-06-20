//! Wires an [`OpStore`] to the CRDT [`Materializer`]: the durable op-log plus an
//! in-memory projection kept up to date as ops are appended. This is the object
//! the application/FFI layer (#23) will drive.

use splittr_crdt::{Materializer, Op, Projection};

use crate::error::Result;
use crate::op_store::OpStore;

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

    /// Persist `op` and fold it into the projection. Idempotent: a duplicate op
    /// (same content id) is neither re-stored nor re-applied. Returns whether
    /// the op was newly stored.
    pub fn append(&mut self, op: &Op) -> Result<bool> {
        let newly_stored = self.store.append(op)?;
        if newly_stored {
            self.materializer.apply(op);
        }
        Ok(newly_stored)
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
