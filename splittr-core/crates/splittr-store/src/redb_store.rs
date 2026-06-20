//! A durable op store backed by [redb](https://www.redb.org/) — a pure-Rust
//! embedded key-value store (no C toolchain). Ops are keyed by their
//! content-addressed id and stored as canonical `postcard` bytes. The value is
//! opaque to the store, which is what lets it hold E2E-encrypted ops later (#14)
//! without changes here.

use std::path::Path;

use redb::{Database, ReadableTable, TableDefinition};
use splittr_crdt::{Op, OpId};

use crate::error::{backend, Result};
use crate::op_store::OpStore;

const OPS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("ops");

pub struct RedbOpStore {
    db: Database,
}

impl RedbOpStore {
    /// Open (or create) the database at `path`, ensuring the ops table exists.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = Database::create(path).map_err(backend)?;
        let write = db.begin_write().map_err(backend)?;
        write.open_table(OPS).map_err(backend)?;
        write.commit().map_err(backend)?;
        Ok(Self { db })
    }
}

impl OpStore for RedbOpStore {
    fn append(&mut self, op: &Op) -> Result<bool> {
        let bytes = postcard::to_allocvec(op)?;
        let write = self.db.begin_write().map_err(backend)?;
        let newly_stored;
        {
            let mut table = write.open_table(OPS).map_err(backend)?;
            let previous = table
                .insert(op.id.0.as_slice(), bytes.as_slice())
                .map_err(backend)?;
            newly_stored = previous.is_none();
        }
        write.commit().map_err(backend)?;
        Ok(newly_stored)
    }

    fn contains(&self, id: &OpId) -> Result<bool> {
        let read = self.db.begin_read().map_err(backend)?;
        let table = read.open_table(OPS).map_err(backend)?;
        Ok(table.get(id.0.as_slice()).map_err(backend)?.is_some())
    }

    fn ops(&self) -> Result<Vec<Op>> {
        let read = self.db.begin_read().map_err(backend)?;
        let table = read.open_table(OPS).map_err(backend)?;
        let mut ops = Vec::new();
        for entry in table.iter().map_err(backend)? {
            let (_id, value) = entry.map_err(backend)?;
            ops.push(postcard::from_bytes::<Op>(value.value())?);
        }
        Ok(ops)
    }

    fn len(&self) -> Result<usize> {
        // Derived from `ops()` to avoid depending on redb's metadata trait,
        // which has moved between versions.
        Ok(self.ops()?.len())
    }
}
