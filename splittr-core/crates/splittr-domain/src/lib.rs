//! `splittr-domain` — pure value types and money/split math.
//!
//! No I/O, no async, no Flutter. This is the deterministic foundation the CRDT
//! fold builds on (see `../../docs/ARCHITECTURE.md` and ADR-0001). Money is
//! always integer cents so splits reconcile to the penny.

mod ids;
mod money;
mod split;

pub use ids::{ExpenseId, GroupId, SettlementId, UserId};
pub use money::Cents;
pub use split::{split_equal, Split};
