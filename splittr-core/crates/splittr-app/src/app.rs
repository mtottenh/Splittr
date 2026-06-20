//! The application facade: turns user intents into validated, signed ops and
//! exposes read-side view-models. This is the API the FFI (#23) will surface.

use std::collections::BTreeMap;

use splittr_crdt::{
    Cents, ExpenseFields, ExpenseId, GroupId, Op, OpKind, OriginalAmount, Projection, SettlementId,
    SiteId, SplitPlan, UserId,
};
use splittr_store::{Applied, OpStore, Repository};
use uuid::Uuid;

use crate::clock::HlcGenerator;
use crate::error::{AppError, Result};
use crate::identity::Identity;
use crate::query::{
    self, ActivityEntry, DeviceView, FriendBalance, FriendDetail, GroupDetail, GroupSummary,
};

/// Drives the engine for the local user. All writes go through [`App::commit`],
/// which stamps an HLC, signs the op with the local identity, and appends it.
pub struct App<S: OpStore> {
    identity: Identity,
    repo: Repository<S>,
    clock: HlcGenerator,
}

impl<S: OpStore> App<S> {
    pub fn new(identity: Identity, store: S, site: SiteId) -> Result<Self> {
        let mut app = Self {
            identity,
            repo: Repository::open(store)?,
            clock: HlcGenerator::new(site),
        };
        app.ensure_device_authorized(site)?;
        Ok(app)
    }

    /// Self-issue this device's certificate (root-signed) the first time it runs,
    /// so its device-signed ops attribute to the identity (#16/ADR-0005).
    ///
    /// Already-enrolled devices open without the root (daily, locked operation,
    /// #34); bootstrapping a *new* device's first certificate needs the root, so
    /// it must be opened unlocked.
    fn ensure_device_authorized(&mut self, site: SiteId) -> Result<()> {
        let device = self.identity.device_public();
        if self.repo.projection().devices.contains_key(&device) {
            return Ok(());
        }
        if !self.identity.root_unlocked() {
            return Err(AppError::RootLocked);
        }
        let identity = self.identity.public();
        self.commit_as_root(OpKind::AuthorizeDevice {
            identity,
            device,
            site: site.0,
        })
    }

    /// Unlock the identity (root) key from its seed so privileged actions
    /// (enrol/revoke a device) can be signed. Returns an error if the seed does
    /// not match this identity. Pair with [`lock_root`](Self::lock_root) (#34).
    pub fn unlock_root(&mut self, identity_seed: [u8; 32]) -> Result<()> {
        if self.identity.unlock(identity_seed) {
            Ok(())
        } else {
            Err(AppError::NotAuthorized(
                "recovery seed does not match this identity".into(),
            ))
        }
    }

    /// Re-seal the root: drop the in-memory secret after a privileged action.
    pub fn lock_root(&mut self) {
        self.identity.lock();
    }

    /// Whether the root is currently unlocked.
    pub fn root_unlocked(&self) -> bool {
        self.identity.root_unlocked()
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
        })?;
        // Publish the X25519 agreement key so peers can encrypt to us (#6/#14).
        // Content-addressed, so re-emitting the same key is a deduped no-op.
        // Needs the root seed; in daily (locked) operation the key is already
        // published, so skipping it is correct.
        self.publish_agreement_key()
    }

    /// Publish the local user's X25519 agreement public key (#6). A no-op when
    /// the root is locked (the key is derived from the root seed and is already
    /// in the log from first run).
    pub fn publish_agreement_key(&mut self) -> Result<()> {
        let Some(agreement) = self.identity.agreement_public() else {
            return Ok(());
        };
        let user = self.me().clone();
        self.commit(OpKind::SetAgreementKey {
            user,
            key: agreement.0,
        })
    }

    /// The local user's X25519 agreement public key (#6/#14): derived from the
    /// root seed when unlocked, otherwise read from the published projection.
    pub fn my_agreement_public(&self) -> Option<[u8; 32]> {
        if let Some(agreement) = self.identity.agreement_public() {
            return Some(agreement.0);
        }
        self.repo
            .projection()
            .users
            .get(self.me())
            .and_then(|u| u.agreement_pub)
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

    /// Set the group's base/display currency (#3).
    pub fn set_group_currency(&mut self, group: &GroupId, currency: &str) -> Result<()> {
        self.require_member(group)?;
        self.commit(OpKind::SetGroupCurrency {
            group: group.clone(),
            currency: currency.to_string(),
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
        original: Option<OriginalAmount>,
    ) -> Result<ExpenseId> {
        self.create_expense(
            Some(group),
            description,
            paid_by,
            total,
            split,
            category,
            notes,
            date_ms,
            original,
            false,
        )
    }

    /// Add a private draft expense (excluded from balances until published, #15).
    #[allow(clippy::too_many_arguments)]
    pub fn add_draft_expense(
        &mut self,
        group: &GroupId,
        description: &str,
        paid_by: BTreeMap<UserId, Cents>,
        total: Cents,
        split: SplitPlan,
        category: &str,
        notes: Option<String>,
        date_ms: i64,
        original: Option<OriginalAmount>,
    ) -> Result<ExpenseId> {
        self.create_expense(
            Some(group),
            description,
            paid_by,
            total,
            split,
            category,
            notes,
            date_ms,
            original,
            true,
        )
    }

    /// Add a non-group (friend-to-friend) expense — no group membership is
    /// required; the participants are implied by `paid_by`/`split` (#31).
    #[allow(clippy::too_many_arguments)]
    pub fn add_non_group_expense(
        &mut self,
        description: &str,
        paid_by: BTreeMap<UserId, Cents>,
        total: Cents,
        split: SplitPlan,
        category: &str,
        notes: Option<String>,
        date_ms: i64,
        original: Option<OriginalAmount>,
        draft: bool,
    ) -> Result<ExpenseId> {
        self.create_expense(
            None,
            description,
            paid_by,
            total,
            split,
            category,
            notes,
            date_ms,
            original,
            draft,
        )
    }

    /// Shared create path for group/non-group and active/draft expenses
    /// (one implementation — DRY).
    #[allow(clippy::too_many_arguments)]
    fn create_expense(
        &mut self,
        group: Option<&GroupId>,
        description: &str,
        paid_by: BTreeMap<UserId, Cents>,
        total: Cents,
        split: SplitPlan,
        category: &str,
        notes: Option<String>,
        date_ms: i64,
        original: Option<OriginalAmount>,
        draft: bool,
    ) -> Result<ExpenseId> {
        if let Some(group) = group {
            self.require_member(group)?;
        }
        let fields = build_fields(
            description,
            paid_by,
            total,
            split,
            category,
            notes,
            date_ms,
            original,
        )?;
        let expense = ExpenseId::new(format!("expense:{}", Uuid::new_v4()));
        self.commit(OpKind::CreateExpense {
            expense: expense.clone(),
            group: group.cloned(),
            fields,
            draft,
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
        original: Option<OriginalAmount>,
    ) -> Result<()> {
        self.require_editable(expense)?;
        let fields = build_fields(
            description,
            paid_by,
            total,
            split,
            category,
            notes,
            date_ms,
            original,
        )?;
        self.commit(OpKind::EditExpense {
            expense: expense.clone(),
            fields,
        })
    }

    pub fn delete_expense(&mut self, expense: &ExpenseId) -> Result<()> {
        self.require_editable(expense)?;
        self.commit(OpKind::VoidExpense {
            expense: expense.clone(),
        })
    }

    /// Publish a draft expense so it counts toward balances (#15).
    pub fn publish_expense(&mut self, expense: &ExpenseId) -> Result<()> {
        if !self.repo.projection().expenses.contains_key(expense) {
            return Err(AppError::NotFound(format!("expense {expense}")));
        }
        self.commit(OpKind::PublishExpense {
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

    /// Close a group's accounting period: expenses dated at or before `until_ms`
    /// become uneditable (#15). Lowering it again reopens the period.
    pub fn set_closed_period(&mut self, group: &GroupId, until_ms: i64) -> Result<()> {
        self.require_member(group)?;
        self.commit(OpKind::SetClosedPeriod {
            group: group.clone(),
            until_ms,
        })
    }

    // --- settlements -------------------------------------------------------

    pub fn record_settlement(
        &mut self,
        group: &GroupId,
        from: &UserId,
        to: &UserId,
        amount: Cents,
    ) -> Result<SettlementId> {
        self.create_settlement(Some(group), from, to, amount)
    }

    /// Record a non-group (friend-to-friend) settlement (#31).
    pub fn record_non_group_settlement(
        &mut self,
        from: &UserId,
        to: &UserId,
        amount: Cents,
    ) -> Result<SettlementId> {
        self.create_settlement(None, from, to, amount)
    }

    /// Shared settlement path for group and non-group payments (one impl — DRY).
    fn create_settlement(
        &mut self,
        group: Option<&GroupId>,
        from: &UserId,
        to: &UserId,
        amount: Cents,
    ) -> Result<SettlementId> {
        if let Some(group) = group {
            self.require_member(group)?;
        }
        if from == to {
            return Err(AppError::Validation("payer and payee must differ".into()));
        }
        if amount.0 <= 0 {
            return Err(AppError::Validation("amount must be positive".into()));
        }
        let settlement = SettlementId::new(format!("settlement:{}", Uuid::new_v4()));
        self.commit(OpKind::RecordSettlement {
            settlement: settlement.clone(),
            group: group.cloned(),
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

    /// The local user's overall net across all expenses/settlements (#31).
    pub fn overall_net(&self) -> Cents {
        query::overall_net(&self.repo.projection(), self.me())
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

    /// This device's public key (hex). Used to enrol it from another device.
    pub fn my_device_public(&self) -> [u8; 32] {
        self.identity.device_public().0
    }

    /// The identity (root) public key — persisted by the shell so later launches
    /// can open device-only (root locked, #34).
    pub fn identity_public(&self) -> [u8; 32] {
        self.identity.public().0
    }

    /// Devices authorized for the local identity (#16).
    pub fn devices(&self) -> Vec<DeviceView> {
        query::devices(
            &self.repo.projection(),
            self.identity.public(),
            self.identity.device_public(),
        )
    }

    // --- devices (#16/ADR-0005) -------------------------------------------

    /// Authorize another device (its 32-byte public key) to act for this
    /// identity. Root-signed.
    pub fn authorize_device(&mut self, device: [u8; 32], site: u64) -> Result<()> {
        let identity = self.identity.public();
        self.commit_as_root(OpKind::AuthorizeDevice {
            identity,
            device: splittr_crdt::PublicKey(device),
            site,
        })
    }

    /// Revoke a device. Root-signed; its ops at/after this op stop counting.
    pub fn revoke_device(&mut self, device: [u8; 32]) -> Result<()> {
        let identity = self.identity.public();
        self.commit_as_root(OpKind::RevokeDevice {
            identity,
            device: splittr_crdt::PublicKey(device),
        })
    }

    // --- internals ---------------------------------------------------------

    fn set_membership(&mut self, group: &GroupId, user: &UserId, member: bool) -> Result<()> {
        self.commit(OpKind::SetMembership {
            group: group.clone(),
            user: user.clone(),
            member,
        })
    }

    /// Build, sign (with the **device** key) and append a normal op.
    fn commit(&mut self, kind: OpKind) -> Result<()> {
        self.commit_signed(kind, false)
    }

    /// Sign with the **identity (root)** key — only for device certs/revocations.
    fn commit_as_root(&mut self, kind: OpKind) -> Result<()> {
        self.commit_signed(kind, true)
    }

    fn commit_signed(&mut self, kind: OpKind, as_root: bool) -> Result<()> {
        let hlc = self.clock.now();
        let key = if as_root {
            self.identity.root_key().ok_or(AppError::RootLocked)?
        } else {
            self.identity.device_key()
        };
        let op = Op::signed(hlc, key, kind);
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

    /// The expense must exist, not be locked, and not fall in a closed period
    /// (#15). The single guard for edit/delete.
    fn require_editable(&self, expense: &ExpenseId) -> Result<()> {
        let projection = self.repo.projection();
        let rec = projection
            .expenses
            .get(expense)
            .ok_or_else(|| AppError::NotFound(format!("expense {expense}")))?;
        if rec.locked {
            return Err(AppError::Validation("expense is locked".into()));
        }
        let closed_until = rec
            .group
            .as_ref()
            .and_then(|g| projection.groups.get(g))
            .map(|g| g.closed_until_ms)
            .unwrap_or(0);
        if closed_until > 0 && rec.fields.date_ms <= closed_until {
            return Err(AppError::Validation("expense is in a closed period".into()));
        }
        Ok(())
    }

    fn set_expense_lock(&mut self, expense: &ExpenseId, locked: bool) -> Result<()> {
        let group = match self.repo.projection().expenses.get(expense) {
            None => return Err(AppError::NotFound(format!("expense {expense}"))),
            Some(rec) => rec.group.clone(),
        };
        // Group expenses require membership; non-group ones are personal.
        if let Some(group) = &group {
            self.require_member(group)?;
        }
        self.commit(OpKind::SetExpenseLock {
            expense: expense.clone(),
            locked,
        })
    }
}

/// Validate inputs and assemble a balanced [`ExpenseFields`]. Amounts are in the
/// expense's base currency; `original` carries the pre-conversion amount/rate for
/// a foreign-currency expense (#3).
#[allow(clippy::too_many_arguments)]
fn build_fields(
    description: &str,
    paid_by: BTreeMap<UserId, Cents>,
    total: Cents,
    split: SplitPlan,
    category: &str,
    notes: Option<String>,
    date_ms: i64,
    original: Option<OriginalAmount>,
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
    fields.original = original;

    if !fields.is_balanced() {
        return Err(AppError::Validation("splits must sum to the total".into()));
    }
    Ok(fields)
}
