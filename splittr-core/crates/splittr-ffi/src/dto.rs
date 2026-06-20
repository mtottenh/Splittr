//! Flutter-friendly data-transfer types for the FFI boundary.
//!
//! These mirror the `splittr-app` view-models and command inputs using only
//! types `flutter_rust_bridge` marshals cleanly: `String`, integers, `Option`,
//! `Vec`, and plain structs/enums. Ids are strings and money is `i64` cents.

/// A payer (or exact-amount) entry: who, and how many cents.
pub struct Payer {
    pub user_id: String,
    pub cents: i64,
}

/// A weight entry for proportional splits (percentages or shares).
pub struct Weight {
    pub user_id: String,
    pub weight: u64,
}

/// How to divide an expense — the FFI mirror of the domain `SplitPlan`.
pub enum SplitPlanDto {
    Equal { participants: Vec<String> },
    Exact { amounts: Vec<Payer> },
    Weighted { weights: Vec<Weight> },
}

/// Input for adding or editing an expense.
pub struct ExpenseInput {
    pub group_id: String,
    pub description: String,
    pub paid_by: Vec<Payer>,
    pub total_cents: i64,
    pub split: SplitPlanDto,
    pub category: String,
    pub notes: Option<String>,
    pub date_ms: i64,
}

pub struct GroupSummaryDto {
    pub id: String,
    pub name: String,
    pub member_count: u32,
    pub my_net_cents: i64,
}

pub struct MemberBalanceDto {
    pub user_id: String,
    pub name: String,
    pub net_cents: i64,
}

/// A `(user, name, amount)` triple — one payer or one split share.
pub struct MemberAmountDto {
    pub user_id: String,
    pub name: String,
    pub cents: i64,
}

pub struct ExpenseViewDto {
    pub id: String,
    pub group_id: String,
    pub group_name: String,
    pub description: String,
    pub total_cents: i64,
    pub category: String,
    pub date_ms: i64,
    pub notes: Option<String>,
    pub my_net_cents: i64,
    pub locked: bool,
    pub paid_by: Vec<MemberAmountDto>,
    pub splits: Vec<MemberAmountDto>,
}

pub struct SettlementViewDto {
    pub id: String,
    pub from: String,
    pub from_name: String,
    pub to: String,
    pub to_name: String,
    pub amount_cents: i64,
}

pub struct TransferDto {
    pub from: String,
    pub to: String,
    pub amount_cents: i64,
}

pub struct GroupDetailDto {
    pub id: String,
    pub name: String,
    pub members: Vec<MemberBalanceDto>,
    pub expenses: Vec<ExpenseViewDto>,
    pub settlements: Vec<SettlementViewDto>,
    pub settle_up: Vec<TransferDto>,
}

/// A friend with the running balance to the current user (positive = owes you).
pub struct FriendBalanceDto {
    pub user_id: String,
    pub name: String,
    pub net_cents: i64,
}

pub struct FriendDetailDto {
    pub user_id: String,
    pub name: String,
    pub net_cents: i64,
    pub shared: Vec<ExpenseViewDto>,
}

/// One entry in the activity feed. `kind` is a coarse tag for icon selection:
/// `group`, `expense`, `edit`, `delete`, `settlement`, `person`.
pub struct ActivityEntryDto {
    pub kind: String,
    pub summary: String,
    pub wall_ms: i64,
}
