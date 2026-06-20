//! The application facade: turns user intents into validated, signed ops and
//! exposes read-side view-models. This is the API the FFI (#23) will surface.

use std::collections::BTreeMap;

use splittr_crdt::{
    Cents, ExpenseFields, ExpenseId, GroupId, Op, OpKind, Projection, SettlementId, SiteId,
    SplitPlan, UserId,
};
use splittr_store::{Applied, OpStore, Repository};
use uuid::Uuid;

use crate::clock::HlcGenerator;
use crate::error::{AppError, Result};
use crate::identity::Identity;
use crate::query::{self, ActivityEntry, FriendBalance, FriendDetail, GroupDetail, GroupSummary};

/// Drives the engine for the local user. All writes go through [`App::commit`],
/// which stamps an HLC, signs the op with the local identity, and appends it.
pub struct App<S: OpStore> {
    identity: Identity,
    repo: Repository<S>,
    clock: HlcGenerator,
}

impl<S: OpStore> App<S> {
    pub fn new(identity: Identity, store: S, site: SiteId) -> Result<Self> {
        Ok(Self {
            identity,
            repo: Repository::open(store)?,
            clock: HlcGenerator::new(site),
        })
    }

    /// The local user's id.
    pub fn me(&self) -> &UserId {
        self.identity.user_id()
    }

    /// The current read model (escape hatch; prefer the query methods).
    pub fn projection(&self) -> Projection {
        self.repo.projection()
    }

    // --- profiles & people -------------------------------------------------

    pub fn set_my_name(&mut self, name: &str) -> Result<()> {
        let user = self.me().clone();
        self.commit(OpKind::UpsertProfile {
            user,
            name: name.to_string(),
        })
    }

    /// The local user's display name, if a profile has been set.
    pub fn my_name(&self) -> Option<String> {
        self.repo
            .projection()
            .users
            .get(self.me())
            .map(|r| r.name.clone())
    }

    /// Create a placeholder person (#2) and return their id.
    pub fn add_person(&mut self, name: &str) -> Result<UserId> {
        let user = UserId::new(format!("user:{}", Uuid::new_v4()));
        self.commit(OpKind::UpsertProfile {
            user: user.clone(),
            name: name.to_string(),
        })?;
        Ok(user)
    }

    // --- groups ------------------------------------------------------------

    /// Create a group (the local user is added as a member) and return its id.
    pub fn create_group(&mut self, name: &str, members: &[UserId]) -> Result<GroupId> {
        let group = GroupId::new(format!("group:{}", Uuid::new_v4()));
        self.commit(OpKind::CreateGroup {
            group: group.clone(),
            name: name.to_string(),
        })?;
        let me = self.me().clone();
        self.set_membership(&group, &me, true)?;
        for member in members {
            self.set_membership(&group, member, true)?;
        }
        Ok(group)
    }

    pub fn rename_group(&mut self, group: &GroupId, name: &str) -> Result<()> {
        self.require_member(group)?;
        self.commit(OpKind::SetGroupName {
            group: group.clone(),
            name: name.to_string(),
        })
    }

    pub fn add_member(&mut self, group: &GroupId, user: &UserId) -> Result<()> {
        self.require_member(group)?;
        self.set_membership(group, user, true)
    }

    pub fn remove_member(&mut self, group: &GroupId, user: &UserId) -> Result<()> {
        self.require_member(group)?;
        self.set_membership(group, user, false)
    }

    // --- expenses ----------------------------------------------------------

    /// Add an expense to a group and return its id.
    #[allow(clippy::too_many_arguments)]
    pub fn add_expense(
        &mut self,
        group: &GroupId,
        description: &str,
        paid_by: BTreeMap<UserId, Cents>,
        total: Cents,
        split: SplitPlan,
        category: &str,
        notes: Option<String>,
        date_ms: i64,
    ) -> Result<ExpenseId> {
        self.require_member(group)?;
        let fields = build_fields(description, paid_by, total, split, category, notes, date_ms)?;
        let expense = ExpenseId::new(format!("expense:{}", Uuid::new_v4()));
        self.commit(OpKind::CreateExpense {
            expense: expense.clone(),
            group: group.clone(),
            fields,
        })?;
        Ok(expense)
    }

    /// Replace an expense with a new version (whole-version LWW).
    #[allow(clippy::too_many_arguments)]
    pub fn edit_expense(
        &mut self,
        expense: &ExpenseId,
        description: &str,
        paid_by: BTreeMap<UserId, Cents>,
        total: Cents,
        split: SplitPlan,
        category: &str,
        notes: Option<String>,
        date_ms: i64,
    ) -> Result<()> {
        self.require_unlocked(expense)?;
        let fields = build_fields(description, paid_by, total, split, category, notes, date_ms)?;
        self.commit(OpKind::EditExpense {
            expense: expense.clone(),
            fields,
        })
    }

    pub fn delete_expense(&mut self, expense: &ExpenseId) -> Result<()> {
        self.require_unlocked(expense)?;
        self.commit(OpKind::VoidExpense {
            expense: expense.clone(),
        })
    }

    /// Lock an expense against further edits/deletes (#15).
    pub fn lock_expense(&mut self, expense: &ExpenseId) -> Result<()> {
        self.set_expense_lock(expense, true)
    }

    /// Unlock a previously locked expense.
    pub fn unlock_expense(&mut self, expense: &ExpenseId) -> Result<()> {
        self.set_expense_lock(expense, false)
    }

    // --- settlements -------------------------------------------------------

    pub fn record_settlement(
        &mut self,
        group: &GroupId,
        from: &UserId,
        to: &UserId,
        amount: Cents,
    ) -> Result<SettlementId> {
        self.require_member(group)?;
        if from == to {
            return Err(AppError::Validation("payer and payee must differ".into()));
        }
        if amount.0 <= 0 {
            return Err(AppError::Validation("amount must be positive".into()));
        }
        let settlement = SettlementId::new(format!("settlement:{}", Uuid::new_v4()));
        self.commit(OpKind::RecordSettlement {
            settlement: settlement.clone(),
            group: group.clone(),
            from: from.clone(),
            to: to.clone(),
            amount,
        })?;
        Ok(settlement)
    }

    pub fn delete_settlement(&mut self, settlement: &SettlementId) -> Result<()> {
        self.commit(OpKind::VoidSettlement {
            settlement: settlement.clone(),
        })
    }

    // --- queries -----------------------------------------------------------

    pub fn groups(&self) -> Vec<GroupSummary> {
        query::groups(&self.repo.projection(), self.me())
    }

    pub fn group_detail(&self, group: &GroupId) -> Option<GroupDetail> {
        query::group_detail(&self.repo.projection(), self.me(), group)
    }

    /// Everyone the local user knows, with the running balance to each.
    pub fn friends(&self) -> Vec<FriendBalance> {
        query::friends(&self.repo.projection(), self.me())
    }

    pub fn friend_detail(&self, friend: &UserId) -> Option<FriendDetail> {
        query::friend_detail(&self.repo.projection(), self.me(), friend)
    }

    /// A reverse-chronological feed derived from the signed op-log.
    pub fn activity(&self) -> Vec<ActivityEntry> {
        let ops = self.repo.store().ops().unwrap_or_default();
        query::activity(&ops, &self.repo.projection(), self.me())
    }

    // --- internals ---------------------------------------------------------

    fn set_membership(&mut self, group: &GroupId, user: &UserId, member: bool) -> Result<()> {
        self.commit(OpKind::SetMembership {
            group: group.clone(),
            user: user.clone(),
            member,
        })
    }

    /// Build, sign and append an op for the local identity.
    fn commit(&mut self, kind: OpKind) -> Result<()> {
        let hlc = self.clock.now();
        let op = Op::signed(hlc, self.identity.key(), kind);
        match self.repo.append(&op)? {
            Applied::Rejected => Err(AppError::Validation("op failed verification".into())),
            Applied::Stored | Applied::Duplicate => Ok(()),
        }
    }

    fn require_member(&self, group: &GroupId) -> Result<()> {
        match self.repo.projection().groups.get(group) {
            None => Err(AppError::NotFound(format!("group {group}"))),
            Some(g) if g.members.contains(self.me()) => Ok(()),
            Some(_) => Err(AppError::NotAuthorized(format!(
                "not a member of group {group}"
            ))),
        }
    }

    /// The expense must exist and not be locked.
    fn require_unlocked(&self, expense: &ExpenseId) -> Result<()> {
        match self.repo.projection().expenses.get(expense) {
            None => Err(AppError::NotFound(format!("expense {expense}"))),
            Some(rec) if rec.locked => Err(AppError::Validation("expense is locked".into())),
            Some(_) => Ok(()),
        }
    }

    fn set_expense_lock(&mut self, expense: &ExpenseId, locked: bool) -> Result<()> {
        let group = match self.repo.projection().expenses.get(expense) {
            None => return Err(AppError::NotFound(format!("expense {expense}"))),
            Some(rec) => rec.group.clone(),
        };
        self.require_member(&group)?;
        self.commit(OpKind::SetExpenseLock {
            expense: expense.clone(),
            locked,
        })
    }
}

/// Validate inputs and assemble a balanced [`ExpenseFields`].
#[allow(clippy::too_many_arguments)]
fn build_fields(
    description: &str,
    paid_by: BTreeMap<UserId, Cents>,
    total: Cents,
    split: SplitPlan,
    category: &str,
    notes: Option<String>,
    date_ms: i64,
) -> Result<ExpenseFields> {
    if total.0 <= 0 {
        return Err(AppError::Validation("total must be positive".into()));
    }
    if paid_by.is_empty() {
        return Err(AppError::Validation("an expense needs a payer".into()));
    }
    let paid: i64 = paid_by.values().map(|c| c.0).sum();
    if paid != total.0 {
        return Err(AppError::Validation(format!(
            "payments ({paid}) must equal the total ({})",
            total.0
        )));
    }
    let splits = split
        .compute(total)
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let mut fields = ExpenseFields::new(paid_by, total, splits);
    fields.description = description.to_string();
    fields.category = category.to_string();
    fields.notes = notes;
    fields.date_ms = date_ms;

    if !fields.is_balanced() {
        return Err(AppError::Validation("splits must sum to the total".into()));
    }
    Ok(fields)
}
