//! Folding the op-log into a [`Projection`].
//!
//! [`Materializer`] is the single source of truth for *how* ops become state:
//! it applies ops incrementally (idempotently, deduped by id) and produces a
//! projection on demand. [`project`] is a thin convenience wrapper over it, so
//! batch and incremental paths share one implementation (DRY) and cannot drift.
//!
//! Every "last write wins" decision goes through one [`lww`] over a [`Stamped`]
//! value, so the conflict policy lives in exactly one place.

use std::collections::{BTreeMap, BTreeSet};

use splittr_domain::{Cents, ExpenseFields, ExpenseId, GroupId, SettlementId, UserId};

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

struct SettlementData {
    group: GroupId,
    from: UserId,
    to: UserId,
    amount: Cents,
}

/// Accumulates ops into the state needed to produce a [`Projection`]. Applying
/// ops in any order, with any duplication, yields the same result (ADR-0001).
#[derive(Default)]
pub struct Materializer {
    /// Op ids already applied — makes [`Materializer::apply`] idempotent.
    seen: BTreeSet<OpId>,
    group_created: BTreeSet<GroupId>,
    group_name: BTreeMap<GroupId, Stamped<String>>,
    group_closed_until: BTreeMap<GroupId, Stamped<i64>>,
    profiles: BTreeMap<UserId, Stamped<String>>,
    membership: BTreeMap<(GroupId, UserId), Stamped<bool>>,
    expense_group: BTreeMap<ExpenseId, Stamped<GroupId>>,
    expense_version: BTreeMap<ExpenseId, Stamped<ExpenseFields>>,
    expense_locked: BTreeMap<ExpenseId, Stamped<bool>>,
    /// Grow-only set: an expense is published once any publish signal arrives
    /// (an active create or a `PublishExpense`). Order-independent.
    expense_published: BTreeSet<ExpenseId>,
    expense_voided: BTreeSet<ExpenseId>,
    settlements: BTreeMap<SettlementId, Stamped<SettlementData>>,
    settlement_voided: BTreeSet<SettlementId>,
    alias_edges: Vec<(UserId, UserId)>,
}

impl Materializer {
    /// Apply a single op. Idempotent: re-applying an already-seen op is a no-op.
    pub fn apply(&mut self, op: &Op) {
        if !self.seen.insert(op.id) {
            return;
        }
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
            OpKind::SetMembership {
                group,
                user,
                member,
            } => {
                lww(
                    &mut self.membership,
                    (group.clone(), user.clone()),
                    stamp(hlc, *member),
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
            OpKind::AddAlias { alias, canonical } => {
                self.alias_edges.push((alias.clone(), canonical.clone()));
            }
            OpKind::UpsertProfile { user, name } => {
                lww(&mut self.profiles, user.clone(), stamp(hlc, name.clone()));
            }
        }
    }

    /// Apply many ops.
    pub fn apply_all<'a>(&mut self, ops: impl IntoIterator<Item = &'a Op>) {
        for op in ops {
            self.apply(op);
        }
    }

    /// Produce the read model from the accumulated state.
    pub fn projection(&self) -> Projection {
        let aliases = resolve_aliases(&self.alias_edges);

        let mut groups = BTreeMap::new();
        for g in &self.group_created {
            let name = self
                .group_name
                .get(g)
                .map(|s| s.value.clone())
                .unwrap_or_default();
            let mut members = BTreeSet::new();
            for ((gid, uid), stamped) in &self.membership {
                if gid == g && stamped.value {
                    members.insert(uid.clone());
                }
            }
            let closed_until_ms = self.group_closed_until.get(g).map(|s| s.value).unwrap_or(0);
            groups.insert(
                g.clone(),
                GroupRecord {
                    name,
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
                },
            );
        }

        Projection {
            groups,
            users,
            expenses,
            settlements,
            aliases,
        }
    }
}

/// Fold an op-log into the read model. Order-independent and idempotent.
pub fn project(ops: &[Op]) -> Projection {
    let mut materializer = Materializer::default();
    materializer.apply_all(ops);
    materializer.projection()
}

fn stamp<V>(hlc: Hlc, value: V) -> Stamped<V> {
    Stamped { hlc, value }
}

/// Resolve alias edges to a canonical id per connected component. The canonical
/// id is the lexicographically-smallest user id in the component, making the
/// result deterministic and independent of edge order (ADR-0001 rule 6/7).
fn resolve_aliases(edges: &[(UserId, UserId)]) -> BTreeMap<UserId, UserId> {
    let mut adjacency: BTreeMap<UserId, BTreeSet<UserId>> = BTreeMap::new();
    let mut nodes: BTreeSet<UserId> = BTreeSet::new();
    for (a, b) in edges {
        nodes.insert(a.clone());
        nodes.insert(b.clone());
        adjacency.entry(a.clone()).or_default().insert(b.clone());
        adjacency.entry(b.clone()).or_default().insert(a.clone());
    }

    let mut canonical: BTreeMap<UserId, UserId> = BTreeMap::new();
    let mut visited: BTreeSet<UserId> = BTreeSet::new();
    for start in &nodes {
        if visited.contains(start) {
            continue;
        }
        let mut component: Vec<UserId> = Vec::new();
        let mut stack = vec![start.clone()];
        visited.insert(start.clone());
        while let Some(u) = stack.pop() {
            component.push(u.clone());
            if let Some(neighbours) = adjacency.get(&u) {
                for v in neighbours {
                    if visited.insert(v.clone()) {
                        stack.push(v.clone());
                    }
                }
            }
        }
        let canon = component
            .iter()
            .min()
            .cloned()
            .expect("non-empty component");
        for u in component {
            canonical.insert(u, canon.clone());
        }
    }
    canonical
}
