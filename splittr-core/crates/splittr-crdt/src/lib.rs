//! `splittr-crdt` — the event-sourced core.
//!
//! The source of truth is an append-only log of immutable [`Op`]s. All readable
//! state is the [`Projection`] produced by [`project`] (or incrementally by a
//! [`Materializer`]) — a **pure function of the op set**: any delivery order or
//! duplication yields identical state and balances. Conflict resolution per
//! entity follows ADR-0001 §Conflict-resolution rules:
//!
//! 1. Immutable facts accumulate (grow-only set; dedup by content-addressed id).
//! 2. Mutable scalars (group name) → last-writer-wins by HLC.
//! 3. Membership → LWW per `(group, user)`.
//! 4. Expense edits → whole-version LWW (highest HLC wins; always a valid split).
//! 5. Deletes → monotonic, terminal tombstones (**delete wins**).
//! 6. Aliases (claim/merge) → union-find to a canonical id, resolved before
//!    balances are aggregated (so a claim never changes a balance).
//!
//! Module layout: [`clock`] (HLC), [`op`] (operations), [`projection`] (read
//! model), [`materialize`] (the fold), [`balance`] (derived balances).

mod balance;
mod clock;
mod materialize;
mod op;
mod projection;

pub use balance::{my_net_by_group, net_balances, pairwise_with, settle_up};
pub use clock::{Hlc, SiteId};
pub use materialize::{project, Materializer};
pub use op::{Op, OpId, OpKind};
pub use projection::{
    DeviceRecord, ExpenseRecord, GroupRecord, Projection, SettlementRecord, UserRecord,
};

// Re-export the crypto + domain types that appear in this crate's public API so
// callers (and tests) need only depend on `splittr-crdt`.
pub use splittr_crypto::{
    open_seed, recovery_phrase, seal_seed, seed_from_phrase, AgreementKey, AgreementPublic,
    PairingTranscript, PublicKey, Signature, SigningKey,
};
pub use splittr_domain::{
    convert, minor_units, simplify_debts, split_by_weights, split_equal, Cents, ExpenseFields,
    ExpenseId, GroupId, OriginalAmount, SettlementId, Split, SplitError, SplitPlan, Transfer,
    UserId,
};
