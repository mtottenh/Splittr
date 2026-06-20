//! A durable op store backed by [redb](https://www.redb.org/) — a pure-Rust
//! embedded key-value store (no C toolchain). Ops are keyed by their
//! content-addressed id; the stored value is the canonical `postcard` encoding,
//! optionally **encrypted at rest** (#22) under a platform-supplied key.
//!
//! Encryption protects the sensitive op contents (amounts, names, group
//! structure) on disk. The key (the content-addressed `OpId`) stays in the
//! clear — it is a hash, but note it enables a confirmation attack against a
//! fully-guessed op, which is acceptable for the device-theft threat model.

use std::path::Path;

use redb::{Database, ReadableTable, TableDefinition};
use splittr_crdt::{Op, OpId};
use splittr_crypto::{open, seal, AeadKey};

use crate::error::{backend, Result, StoreError};
use crate::op_store::OpStore;

const OPS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("ops");

pub struct RedbOpStore {
    db: Database,
    /// When set, stored op values are encrypted at rest with this key.
    cipher: Option<AeadKey>,
}

impl RedbOpStore {
    /// Open (or create) an unencrypted database at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_inner(path, None)
    }

    /// Open (or create) a database whose op values are encrypted at rest with
    /// the 32-byte `key` (#22). The key comes from the platform keystore or a
    /// passphrase — never stored alongside the database.
    pub fn open_encrypted(path: impl AsRef<Path>, key: [u8; 32]) -> Result<Self> {
        Self::open_inner(path, Some(AeadKey::new(key)))
    }

    fn open_inner(path: impl AsRef<Path>, cipher: Option<AeadKey>) -> Result<Self> {
        let db = Database::create(path).map_err(backend)?;
        let write = db.begin_write().map_err(backend)?;
        write.open_table(OPS).map_err(backend)?;
        write.commit().map_err(backend)?;
        Ok(Self { db, cipher })
    }

    /// Serialize (and optionally encrypt) an op for storage.
    fn encode(&self, op: &Op) -> Result<Vec<u8>> {
        let bytes = postcard::to_allocvec(op)?;
        Ok(match &self.cipher {
            Some(key) => seal(key, &bytes),
            None => bytes,
        })
    }

    /// Decrypt (if needed) and deserialize a stored op.
    fn decode(&self, stored: &[u8]) -> Result<Op> {
        let bytes = match &self.cipher {
            Some(key) => open(key, stored).ok_or(StoreError::Decrypt)?,
            None => stored.to_vec(),
        };
        Ok(postcard::from_bytes::<Op>(&bytes)?)
    }
}

impl OpStore for RedbOpStore {
    fn append(&mut self, op: &Op) -> Result<bool> {
        let bytes = self.encode(op)?;
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
            ops.push(self.decode(value.value())?);
        }
        Ok(ops)
    }

    fn len(&self) -> Result<usize> {
        // Derived from `ops()` to avoid depending on redb's metadata trait,
        // which has moved between versions.
        Ok(self.ops()?.len())
    }
}
