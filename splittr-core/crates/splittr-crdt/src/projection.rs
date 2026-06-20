//! The read model produced by folding the op-log. A disposable cache — the
//! op-log is canonical and the projection is fully rebuildable from it.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use splittr_domain::{Cents, ExpenseFields, ExpenseId, GroupId, SettlementId, UserId};

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Projection {
    pub groups: BTreeMap<GroupId, GroupRecord>,
    /// User display profiles (name).
    pub users: BTreeMap<UserId, UserRecord>,
    /// Present (non-voided) expenses only.
    pub expenses: BTreeMap<ExpenseId, ExpenseRecord>,
    /// Present (non-voided) settlements only.
    pub settlements: BTreeMap<SettlementId, SettlementRecord>,
    /// Resolution map: user id → canonical id (after alias merges).
    pub aliases: BTreeMap<UserId, UserId>,
}

impl Projection {
    /// A sub-projection containing only `group`'s expenses and settlements
    /// (users/aliases retained). Lets balance functions be reused for
    /// group-scoped totals without duplicating their logic.
    pub fn for_group(&self, group: &GroupId) -> Projection {
        Projection {
            groups: self.groups.clone(),
            users: self.users.clone(),
            expenses: self
                .expenses
                .iter()
                .filter(|(_, e)| &e.group == group)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            settlements: self
                .settlements
                .iter()
                .filter(|(_, s)| &s.group == group)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            aliases: self.aliases.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct GroupRecord {
    pub name: String,
    pub members: BTreeSet<UserId>,
    /// Expenses dated at or before this are in a closed period and cannot be
    /// edited (#15). `0` means no period is closed.
    pub closed_until_ms: i64,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UserRecord {
    pub name: String,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ExpenseRecord {
    pub group: GroupId,
    pub fields: ExpenseFields,
    /// Whether the expense is locked against further edits (#15).
    pub locked: bool,
    /// `false` while the expense is a private draft — drafts are excluded from
    /// balances and settle-up but kept in the ledger (#15).
    pub published: bool,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SettlementRecord {
    pub group: GroupId,
    pub from: UserId,
    pub to: UserId,
    pub amount: Cents,
}
