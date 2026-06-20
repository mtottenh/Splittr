//! `splittr-app` — the command/query use-case layer over the engine.
//!
//! [`App`] is the facade the FFI (#23) will expose: each command validates and
//! authorizes inputs, then builds a **signed** op (with the local [`Identity`])
//! and appends it via the store's `Repository`; queries return UI-friendly
//! view-models. Pure Rust and `cargo test`-able over an in-memory store.
//!
//! Module layout: [`app`] (the facade), [`identity`] (local user + signing key),
//! [`clock`] (HLC generator), [`query`] (view-models), [`error`].

mod app;
mod clock;
mod error;
mod identity;
mod query;

pub use app::App;
pub use clock::HlcGenerator;
pub use error::{AppError, Result};
pub use identity::{user_id_for, Identity};
pub use query::{
    ActivityEntry, ExpenseView, FriendBalance, FriendDetail, GroupDetail, GroupSummary,
    MemberAmount, MemberBalance, SettlementView,
};

// Convenience re-exports so callers/tests depend only on `splittr-app`.
pub use splittr_crdt::{
    convert, minor_units, Cents, ExpenseId, GroupId, OriginalAmount, SettlementId, SigningKey,
    SiteId, SplitPlan, Transfer, UserId,
};
pub use splittr_store::{MemoryOpStore, OpStore, RedbOpStore, Repository};
