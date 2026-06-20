//! `splittr-store` — persistence for the event-sourced core.
//!
//! The op-log is the source of truth (ADR-0001 §Storage, #1). [`OpStore`] is the
//! storage abstraction (deduplicating by content-addressed id); [`MemoryOpStore`]
//! and [`RedbOpStore`] implement it. [`Repository`] wires a store to the CRDT
//! [`Materializer`](splittr_crdt::Materializer), keeping the projection rebuilt
//! from — and in sync with — the persisted log.
//!
//! Module layout: [`error`] (errors), [`op_store`] (trait + in-memory),
//! [`redb_store`] (durable backend), [`repository`] (store ⇄ fold).

mod error;
mod op_store;
mod redb_store;
mod repository;

pub use error::{Result, StoreError};
pub use op_store::{MemoryOpStore, OpStore};
pub use redb_store::RedbOpStore;
pub use repository::{Applied, Repository};
