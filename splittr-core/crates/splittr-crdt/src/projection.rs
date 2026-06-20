//! The read model produced by folding the op-log. A disposable cache — the
//! op-log is canonical and the projection is fully rebuildable from it.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use splittr_domain::{Cents, ExpenseId, GroupId, SettlementId, Split, UserId};

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Projection {
    pub groups: BTreeMap<GroupId, GroupRecord>,
    /// Present (non-voided) expenses only.
    pub expenses: BTreeMap<ExpenseId, ExpenseRecord>,
    /// Present (non-voided) settlements only.
    pub settlements: BTreeMap<SettlementId, SettlementRecord>,
    /// Resolution map: user id → canonical id (after alias merges).
    pub aliases: BTreeMap<UserId, UserId>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct GroupRecord {
    pub name: String,
    pub members: BTreeSet<UserId>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ExpenseRecord {
    pub group: GroupId,
    pub payer: UserId,
    pub total: Cents,
    pub splits: Vec<Split>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SettlementRecord {
    pub group: GroupId,
    pub from: UserId,
    pub to: UserId,
    pub amount: Cents,
}
