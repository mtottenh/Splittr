//! Folding the op-log into a [`Projection`].
//!
//! The fold is a **pure function of the op-set**: [`Materializer`] retains the
//! ops it is given (deduped by id) and [`Materializer::projection`] folds them on
//! demand, so the batch ([`project`]) and incremental paths run identical logic
//! and cannot drift (DRY).
//!
//! Authorization lives in [`crate::authorize`]: before the value fold, an
//! [`Authority`] pass decides — purely from the whole op-set, so order-
//! independently — which ops are *entitled* to fold (#16/#38). This module owns
//! only the **value fold**: every "last write wins" decision goes through one
//! [`lww`], so the conflict policy lives in exactly one place.

use std::collections::{BTreeMap, BTreeSet};

use splittr_domain::{Cents, ExpenseFields, ExpenseId, GroupId, SettlementId, UserId};

use crate::authorize::Authority;
use crate::clock::Hlc;
use crate::op::{Op, OpId, OpKind};
use crate::projection::{ExpenseRecord, GroupRecord, Projection, SettlementRecord, UserRecord};

/// A value tagged with the HLC at which it was written.
struct Stamped<V> {
    hlc: Hlc,
    value: V,
}

/// Last-writer-wins merge: keep the value with the strictly-greatest HLC.
/// The one and only place LWW is decided.
fn lww<K: Ord, V>(map: &mut BTreeMap<K, Stamped<V>>, key: K, incoming: Stamped<V>) {
    match map.get(&key) {
        Some(existing) if existing.hlc >= incoming.hlc => {}
        _ => {
            map.insert(key, incoming);
        }
    }
}

fn stamp<V>(hlc: Hlc, value: V) -> Stamped<V> {
    Stamped { hlc, value }
}

/// Retains the op-set (deduped by content id) and folds it on demand.
#[derive(Default)]
pub struct Materializer {
    ops: BTreeMap<OpId, Op>,
}

impl Materializer {
    /// Record an op. Idempotent: re-applying an already-seen op is a no-op.
    pub fn apply(&mut self, op: &Op) {
        self.ops.entry(op.id).or_insert_with(|| op.clone());
    }

    /// Record many ops.
    pub fn apply_all<'a>(&mut self, ops: impl IntoIterator<Item = &'a Op>) {
        for op in ops {
            self.apply(op);
        }
    }

    /// Fold the retained ops into the read model.
    pub fn projection(&self) -> Projection {
        fold(self.ops.values())
    }
}

/// Fold an op-log into the read model. Order-independent and idempotent.
pub fn project(ops: &[Op]) -> Projection {
    fold(ops.iter())
}

// ---------------------------------------------------------------------------
// Value fold: the read-model facts, applied only for entitled ops.
// ---------------------------------------------------------------------------

/// The accumulator for the value fold over the authorized op-set. Membership and
/// aliases are owned by [`Authority`]; everything else accumulates here.
#[derive(Default)]
struct Folder {
    group_created: BTreeSet<GroupId>,
    group_name: BTreeMap<GroupId, Stamped<String>>,
    group_currency: BTreeMap<GroupId, Stamped<String>>,
    group_closed_until: BTreeMap<GroupId, Stamped<i64>>,
    profiles: BTreeMap<UserId, Stamped<String>>,
    agreement_keys: BTreeMap<UserId, Stamped<[u8; 32]>>,
    expense_group: BTreeMap<ExpenseId, Stamped<Option<GroupId>>>,
    expense_version: BTreeMap<ExpenseId, Stamped<ExpenseFields>>,
    expense_locked: BTreeMap<ExpenseId, Stamped<bool>>,
    expense_published: BTreeSet<ExpenseId>,
    expense_voided: BTreeSet<ExpenseId>,
    settlements: BTreeMap<SettlementId, Stamped<SettlementData>>,
    settlement_voided: BTreeSet<SettlementId>,
}

struct SettlementData {
    group: Option<GroupId>,
    from: UserId,
    to: UserId,
    amount: Cents,
}

impl Folder {
    fn apply(&mut self, op: &Op) {
        let hlc = op.hlc;
        match &op.kind {
            OpKind::CreateGroup { group, name } => {
                self.group_created.insert(group.clone());
                lww(
                    &mut self.group_name,
                    group.clone(),
                    stamp(hlc, name.clone()),
                );
            }
            OpKind::SetGroupName { group, name } => {
                lww(
                    &mut self.group_name,
                    group.clone(),
                    stamp(hlc, name.clone()),
                );
            }
            OpKind::SetGroupCurrency { group, currency } => {
                lww(
                    &mut self.group_currency,
                    group.clone(),
                    stamp(hlc, currency.clone()),
                );
            }
            OpKind::CreateExpense {
                expense,
                group,
                fields,
                draft,
            } => {
                lww(
                    &mut self.expense_group,
                    expense.clone(),
                    stamp(hlc, group.clone()),
                );
                lww(
                    &mut self.expense_version,
                    expense.clone(),
                    stamp(hlc, fields.clone()),
                );
                if !draft {
                    self.expense_published.insert(expense.clone());
                }
            }
            OpKind::EditExpense { expense, fields } => {
                lww(
                    &mut self.expense_version,
                    expense.clone(),
                    stamp(hlc, fields.clone()),
                );
            }
            OpKind::PublishExpense { expense } => {
                self.expense_published.insert(expense.clone());
            }
            OpKind::VoidExpense { expense } => {
                self.expense_voided.insert(expense.clone());
            }
            OpKind::SetClosedPeriod { group, until_ms } => {
                lww(
                    &mut self.group_closed_until,
                    group.clone(),
                    stamp(hlc, *until_ms),
                );
            }
            OpKind::SetExpenseLock { expense, locked } => {
                lww(
                    &mut self.expense_locked,
                    expense.clone(),
                    stamp(hlc, *locked),
                );
            }
            OpKind::RecordSettlement {
                settlement,
                group,
                from,
                to,
                amount,
            } => {
                lww(
                    &mut self.settlements,
                    settlement.clone(),
                    stamp(
                        hlc,
                        SettlementData {
                            group: group.clone(),
                            from: from.clone(),
                            to: to.clone(),
                            amount: *amount,
                        },
                    ),
                );
            }
            OpKind::VoidSettlement { settlement } => {
                self.settlement_voided.insert(settlement.clone());
            }
            // Owned by Authority (membership, aliases, friendships) or consumed
            // in collect_devices (certs): never value-folded.
            OpKind::SetMembership { .. }
            | OpKind::AddAlias { .. }
            | OpKind::DeclareFriend { .. }
            | OpKind::AuthorizeDevice { .. }
            | OpKind::RevokeDevice { .. } => {}
            OpKind::UpsertProfile { user, name } => {
                lww(&mut self.profiles, user.clone(), stamp(hlc, name.clone()));
            }
            OpKind::SetAgreementKey { user, key } => {
                lww(&mut self.agreement_keys, user.clone(), stamp(hlc, *key));
            }
        }
    }

    fn assemble(self, authority: Authority) -> Projection {
        let mut groups = BTreeMap::new();
        for g in &self.group_created {
            let name = self
                .group_name
                .get(g)
                .map(|s| s.value.clone())
                .unwrap_or_default();
            let members = authority.current_members(g);
            let closed_until_ms = self.group_closed_until.get(g).map(|s| s.value).unwrap_or(0);
            let currency = self
                .group_currency
                .get(g)
                .map(|s| s.value.clone())
                .unwrap_or_else(|| "USD".to_string());
            groups.insert(
                g.clone(),
                GroupRecord {
                    name,
                    currency,
                    members,
                    closed_until_ms,
                },
            );
        }

        let mut expenses = BTreeMap::new();
        for (id, version) in &self.expense_version {
            if self.expense_voided.contains(id) {
                continue; // delete wins (rule 5)
            }
            if let Some(group) = self.expense_group.get(id) {
                expenses.insert(
                    id.clone(),
                    ExpenseRecord {
                        group: group.value.clone(),
                        fields: version.value.clone(),
                        locked: self
                            .expense_locked
                            .get(id)
                            .map(|s| s.value)
                            .unwrap_or(false),
                        published: self.expense_published.contains(id),
                    },
                );
            }
        }

        let mut settlements = BTreeMap::new();
        for (id, data) in &self.settlements {
            if self.settlement_voided.contains(id) {
                continue;
            }
            settlements.insert(
                id.clone(),
                SettlementRecord {
                    group: data.value.group.clone(),
                    from: data.value.from.clone(),
                    to: data.value.to.clone(),
                    amount: data.value.amount,
                },
            );
        }

        let mut users = BTreeMap::new();
        for (id, stamped) in &self.profiles {
            users.insert(
                id.clone(),
                UserRecord {
                    name: stamped.value.clone(),
                    agreement_pub: self.agreement_keys.get(id).map(|s| s.value),
                },
            );
        }

        let devices = authority.device_records();
        let friends = authority.confirmed_friends();
        Projection {
            groups,
            users,
            expenses,
            settlements,
            aliases: authority.aliases,
            devices,
            friends,
        }
    }
}

/// The shared fold: derive authorization from the whole op-set, then apply only
/// the entitled value ops.
fn fold<'a>(ops: impl Iterator<Item = &'a Op> + Clone) -> Projection {
    let authority = Authority::build(ops.clone());
    let mut folder = Folder::default();
    for op in ops {
        if authority.entitled(op) {
            folder.apply(op);
        }
    }
    folder.assemble(authority)
}
