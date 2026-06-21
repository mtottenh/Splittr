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

pub use app::{App, ExpenseDraft, ExpenseFieldsInput};
pub use clock::HlcGenerator;
pub use error::{AppError, Result};
pub use identity::Identity;
pub use query::{
    ActivityEntry, DeviceView, ExpenseView, FriendBalance, FriendDetail, GroupDetail, GroupSummary,
    MemberAmount, MemberBalance, SettlementView,
};

// Convenience re-exports so callers/tests depend only on `splittr-app`.
pub use splittr_crdt::{
    convert, is_placeholder, minor_units, open_seed, recovery_phrase, seal_seed, seed_from_phrase,
    user_id_for, Cents, ExpenseId, GroupId, Invite, OriginalAmount, PairingTranscript, PublicKey,
    SettlementId, SigningKey, SiteId, SplitPlan, Transfer, UserId,
};
pub use splittr_store::{MemoryOpStore, OpStore, RedbOpStore, Repository};
