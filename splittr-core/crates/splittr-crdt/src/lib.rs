//! `splittr-crdt` — the event-sourced core.
//!
//! The source of truth is an append-only log of immutable [`Op`]s. All readable
//! state is the [`Projection`] produced by [`project`], a **pure function of the
//! op set**: any delivery order or duplication yields identical state and
//! balances. Conflict resolution per entity follows ADR-0001 §Conflict-resolution
//! rules:
//!
//! 1. Immutable facts accumulate (grow-only set; dedup by content-addressed id).
//! 2. Mutable scalars (group name) → last-writer-wins by HLC.
//! 3. Membership → LWW per `(group, user)`.
//! 4. Expense edits → whole-version LWW (highest HLC wins; always a valid split).
//! 5. Deletes → monotonic, terminal tombstones (**delete wins**).
//! 6. Aliases (claim/merge) → union-find to a canonical id, resolved before
//!    balances are aggregated (so a claim never changes a balance).
//!
//! Signing/verification (ADR-0001 §operation model) lands with #6/#14/#16; here
//! [`Op::author`] is an abstract actor and signatures are out of scope.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use splittr_domain::{Cents, ExpenseId, GroupId, SettlementId, Split, UserId};

pub use splittr_domain::{split_equal, Cents as DomainCents};

// ---------------------------------------------------------------------------
// Clocks & identifiers
// ---------------------------------------------------------------------------

/// A replica/device identifier; the HLC tiebreaker and (later) the signing
/// device key (#16).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SiteId(pub u64);

/// The identity that authored an op. A placeholder until real keypairs (#6).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ActorId(pub String);

/// Hybrid Logical Clock. Field declaration order defines the derived total
/// order (compare `wall_ms`, then `counter`, then `site`) — ADR-0001 §HLC.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Hlc {
    pub wall_ms: u64,
    pub counter: u32,
    pub site: SiteId,
}

/// Content-addressed operation id: BLAKE3 over the canonical encoding of
/// `(hlc, author, kind)`. Equal content ⇒ equal id ⇒ free deduplication.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct OpId(pub [u8; 32]);

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// The operation vocabulary (ADR-0001 §Operation vocabulary). A focused subset
/// that exercises every conflict rule; identity/key ops join with #6/#14/#16.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum OpKind {
    CreateGroup {
        group: GroupId,
        name: String,
    },
    /// Mutable scalar — LWW (rule 2). Representative of the eventual `SetGroupMeta`.
    SetGroupName {
        group: GroupId,
        name: String,
    },
    /// Membership register — LWW per `(group, user)` (rule 3).
    SetMembership {
        group: GroupId,
        user: UserId,
        member: bool,
    },
    CreateExpense {
        expense: ExpenseId,
        group: GroupId,
        payer: UserId,
        total: Cents,
        splits: Vec<Split>,
    },
    /// A new whole version of an expense — whole-version LWW (rule 4).
    EditExpense {
        expense: ExpenseId,
        payer: UserId,
        total: Cents,
        splits: Vec<Split>,
    },
    /// Terminal tombstone — delete wins (rule 5).
    VoidExpense {
        expense: ExpenseId,
    },
    RecordSettlement {
        settlement: SettlementId,
        group: GroupId,
        from: UserId,
        to: UserId,
        amount: Cents,
    },
    VoidSettlement {
        settlement: SettlementId,
    },
    /// Claim/merge edge (#8) — union-find to a canonical id (rule 6).
    AddAlias {
        alias: UserId,
        canonical: UserId,
    },
}

/// An immutable, content-addressed operation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Op {
    pub id: OpId,
    pub hlc: Hlc,
    pub author: ActorId,
    pub kind: OpKind,
}

impl Op {
    /// Build an op, computing its content-addressed id from `(hlc, author, kind)`.
    pub fn new(hlc: Hlc, author: ActorId, kind: OpKind) -> Self {
        let id = content_id(&hlc, &author, &kind);
        Op {
            id,
            hlc,
            author,
            kind,
        }
    }
}

fn content_id(hlc: &Hlc, author: &ActorId, kind: &OpKind) -> OpId {
    let bytes = postcard::to_allocvec(&(hlc, author, kind))
        .expect("canonical encoding of an op is infallible");
    OpId(*blake3::hash(&bytes).as_bytes())
}

// ---------------------------------------------------------------------------
// Projection (read model)
// ---------------------------------------------------------------------------

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

/// Fold an op-log into the read model. Pure and order-independent.
pub fn project(ops: &[Op]) -> Projection {
    // Deduplicate by content id — gives set semantics and idempotency. BTreeMap
    // keeps iteration deterministic (though resolution does not depend on it).
    let mut unique: BTreeMap<OpId, &Op> = BTreeMap::new();
    for op in ops {
        unique.entry(op.id).or_insert(op);
    }

    let mut group_created: BTreeSet<GroupId> = BTreeSet::new();
    let mut group_name: BTreeMap<GroupId, (Hlc, String)> = BTreeMap::new();
    let mut membership: BTreeMap<(GroupId, UserId), (Hlc, bool)> = BTreeMap::new();
    let mut expense_group: BTreeMap<ExpenseId, GroupId> = BTreeMap::new();
    let mut expense_version: BTreeMap<ExpenseId, (Hlc, UserId, Cents, Vec<Split>)> =
        BTreeMap::new();
    let mut expense_voided: BTreeSet<ExpenseId> = BTreeSet::new();
    let mut settlement: BTreeMap<SettlementId, (GroupId, UserId, UserId, Cents)> = BTreeMap::new();
    let mut settlement_voided: BTreeSet<SettlementId> = BTreeSet::new();
    let mut alias_edges: Vec<(UserId, UserId)> = Vec::new();

    for op in unique.values() {
        let hlc = op.hlc;
        match &op.kind {
            OpKind::CreateGroup { group, name } => {
                group_created.insert(group.clone());
                lww(&mut group_name, group.clone(), hlc, name.clone());
            }
            OpKind::SetGroupName { group, name } => {
                lww(&mut group_name, group.clone(), hlc, name.clone());
            }
            OpKind::SetMembership {
                group,
                user,
                member,
            } => {
                lww(&mut membership, (group.clone(), user.clone()), hlc, *member);
            }
            OpKind::CreateExpense {
                expense,
                group,
                payer,
                total,
                splits,
            } => {
                expense_group
                    .entry(expense.clone())
                    .or_insert_with(|| group.clone());
                lww_version(
                    &mut expense_version,
                    expense.clone(),
                    hlc,
                    payer.clone(),
                    *total,
                    splits.clone(),
                );
            }
            OpKind::EditExpense {
                expense,
                payer,
                total,
                splits,
            } => {
                lww_version(
                    &mut expense_version,
                    expense.clone(),
                    hlc,
                    payer.clone(),
                    *total,
                    splits.clone(),
                );
            }
            OpKind::VoidExpense { expense } => {
                expense_voided.insert(expense.clone());
            }
            OpKind::RecordSettlement {
                settlement: sid,
                group,
                from,
                to,
                amount,
            } => {
                settlement
                    .entry(sid.clone())
                    .or_insert_with(|| (group.clone(), from.clone(), to.clone(), *amount));
            }
            OpKind::VoidSettlement { settlement: sid } => {
                settlement_voided.insert(sid.clone());
            }
            OpKind::AddAlias { alias, canonical } => {
                alias_edges.push((alias.clone(), canonical.clone()));
            }
        }
    }

    let aliases = resolve_aliases(&alias_edges);

    let mut groups = BTreeMap::new();
    for g in &group_created {
        let name = group_name
            .get(g)
            .map(|(_, n)| n.clone())
            .unwrap_or_default();
        let members = membership
            .iter()
            .filter(|((gg, _), (_, is_member))| gg == g && *is_member)
            .map(|((_, u), _)| u.clone())
            .collect::<BTreeSet<_>>();
        groups.insert(g.clone(), GroupRecord { name, members });
    }

    let mut expenses = BTreeMap::new();
    for (e, (_, payer, total, splits)) in &expense_version {
        if expense_voided.contains(e) {
            continue; // delete wins (rule 5)
        }
        if let Some(group) = expense_group.get(e) {
            expenses.insert(
                e.clone(),
                ExpenseRecord {
                    group: group.clone(),
                    payer: payer.clone(),
                    total: *total,
                    splits: splits.clone(),
                },
            );
        }
    }

    let mut settlements = BTreeMap::new();
    for (s, (group, from, to, amount)) in &settlement {
        if settlement_voided.contains(s) {
            continue;
        }
        settlements.insert(
            s.clone(),
            SettlementRecord {
                group: group.clone(),
                from: from.clone(),
                to: to.clone(),
                amount: *amount,
            },
        );
    }

    Projection {
        groups,
        expenses,
        settlements,
        aliases,
    }
}

/// Net balance per user, in cents (positive = is owed; negative = owes). User
/// ids are resolved through the alias map first, so a claim/merge never changes
/// the totals. The values always sum to zero for a valid ledger.
pub fn net_balances(p: &Projection) -> BTreeMap<UserId, Cents> {
    let resolve = |u: &UserId| p.aliases.get(u).cloned().unwrap_or_else(|| u.clone());
    let mut net: BTreeMap<UserId, Cents> = BTreeMap::new();

    for e in p.expenses.values() {
        *net.entry(resolve(&e.payer)).or_default() += e.total;
        for s in &e.splits {
            *net.entry(resolve(&s.user)).or_default() -= s.owed;
        }
    }
    for s in p.settlements.values() {
        *net.entry(resolve(&s.from)).or_default() += s.amount;
        *net.entry(resolve(&s.to)).or_default() -= s.amount;
    }

    net.retain(|_, c| !c.is_zero());
    net
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Last-writer-wins insert: keep the value with the strictly-greatest HLC.
fn lww<K: Ord, V>(map: &mut BTreeMap<K, (Hlc, V)>, key: K, hlc: Hlc, val: V) {
    if let Some((current, _)) = map.get(&key) {
        if *current >= hlc {
            return;
        }
    }
    map.insert(key, (hlc, val));
}

/// LWW for whole expense versions (rule 4).
fn lww_version(
    map: &mut BTreeMap<ExpenseId, (Hlc, UserId, Cents, Vec<Split>)>,
    expense: ExpenseId,
    hlc: Hlc,
    payer: UserId,
    total: Cents,
    splits: Vec<Split>,
) {
    if let Some((current, _, _, _)) = map.get(&expense) {
        if *current >= hlc {
            return;
        }
    }
    map.insert(expense, (hlc, payer, total, splits));
}

/// Resolve alias edges to a canonical id per connected component. The canonical
/// id is the lexicographically-smallest user id in the component, making the
/// result deterministic and independent of edge order (ADR-0001 rule 7).
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
