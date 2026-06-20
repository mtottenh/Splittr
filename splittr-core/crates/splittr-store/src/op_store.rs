//! The op-log storage abstraction and an in-memory implementation.

use std::collections::BTreeMap;

use splittr_crdt::{Op, OpId};

use crate::error::Result;

/// Append-only storage for the operation log (the source of truth). Backends
/// deduplicate by content-addressed [`OpId`], so appending the same op twice is
/// a no-op. Ordering of [`OpStore::ops`] is unspecified — the fold is
/// order-independent (ADR-0001).
pub trait OpStore {
    /// Store `op`. Returns `true` if it was newly stored, `false` if already present.
    fn append(&mut self, op: &Op) -> Result<bool>;

    /// Whether an op with this id is stored.
    fn contains(&self, id: &OpId) -> Result<bool>;

    /// All stored ops.
    fn ops(&self) -> Result<Vec<Op>>;

    /// Number of stored ops.
    fn len(&self) -> Result<usize>;

    fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
}

/// Volatile, in-memory op store — for tests and the web/no-disk fallback.
#[derive(Default)]
pub struct MemoryOpStore {
    ops: BTreeMap<OpId, Op>,
}

impl MemoryOpStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl OpStore for MemoryOpStore {
    fn append(&mut self, op: &Op) -> Result<bool> {
        Ok(self.ops.insert(op.id, op.clone()).is_none())
    }

    fn contains(&self, id: &OpId) -> Result<bool> {
        Ok(self.ops.contains_key(id))
    }

    fn ops(&self) -> Result<Vec<Op>> {
        Ok(self.ops.values().cloned().collect())
    }

    fn len(&self) -> Result<usize> {
        Ok(self.ops.len())
    }
}
