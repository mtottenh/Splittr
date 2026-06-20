//! Read-side view-models — UI-friendly projections over the engine state.

use splittr_crdt::{
    net_balances, pairwise_with, settle_up, Cents, ExpenseId, ExpenseRecord, GroupId, Op, OpKind,
    Projection, SettlementId, Transfer, UserId,
};

/// A group as shown in the groups list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GroupSummary {
    pub id: GroupId,
    pub name: String,
    pub member_count: usize,
    /// The current user's net within this group (positive = is owed).
    pub my_net: Cents,
}

/// A member with their group-scoped balance.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberBalance {
    pub user: UserId,
    pub name: String,
    pub net: Cents,
}

/// A `(user, name, amount)` triple — one payer or one split share.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberAmount {
    pub user: UserId,
    pub name: String,
    pub cents: Cents,
}

/// An expense as shown in a group's ledger (and the friend view). Carries enough
/// detail to render payer/split lines and to pre-fill the edit form.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExpenseView {
    pub id: ExpenseId,
    pub group: GroupId,
    pub group_name: String,
    pub description: String,
    pub total: Cents,
    pub category: String,
    pub date_ms: i64,
    pub notes: Option<String>,
    /// The current user's involvement: paid minus owed (positive = lent).
    pub my_net: Cents,
    pub locked: bool,
    /// `false` while the expense is a draft (not counted in balances).
    pub published: bool,
    pub paid_by: Vec<MemberAmount>,
    pub splits: Vec<MemberAmount>,
}

/// A recorded payment as shown in a group's ledger.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SettlementView {
    pub id: SettlementId,
    pub from: UserId,
    pub from_name: String,
    pub to: UserId,
    pub to_name: String,
    pub amount: Cents,
}

/// Everything the group-detail screen needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GroupDetail {
    pub id: GroupId,
    pub name: String,
    pub members: Vec<MemberBalance>,
    pub expenses: Vec<ExpenseView>,
    pub settlements: Vec<SettlementView>,
    pub settle_up: Vec<Transfer>,
}

/// A friend with the running balance between them and the current user.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FriendBalance {
    pub user: UserId,
    pub name: String,
    /// Positive = the friend owes the current user.
    pub net: Cents,
}

/// Everything the friend-detail screen needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FriendDetail {
    pub user: UserId,
    pub name: String,
    pub net: Cents,
    pub shared: Vec<ExpenseView>,
}

/// A single entry in the activity feed, derived from the op-log.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ActivityEntry {
    /// A coarse kind tag the UI maps to an icon: `group`, `expense`, `edit`,
    /// `delete`, `settlement`, `person`.
    pub kind: String,
    pub summary: String,
    pub wall_ms: i64,
}

fn display_name(p: &Projection, user: &UserId) -> String {
    p.users
        .get(user)
        .map(|r| r.name.clone())
        .unwrap_or_else(|| user.to_string())
}

fn member_amount(p: &Projection, user: &UserId, cents: Cents) -> MemberAmount {
    MemberAmount {
        user: user.clone(),
        name: display_name(p, user),
        cents,
    }
}

fn expense_view(p: &Projection, me: &UserId, id: &ExpenseId, e: &ExpenseRecord) -> ExpenseView {
    let paid = e.fields.paid_by.get(me).copied().unwrap_or(Cents::ZERO);
    let owed = e
        .fields
        .splits
        .iter()
        .filter(|s| &s.user == me)
        .fold(Cents::ZERO, |acc, s| acc + s.owed);
    ExpenseView {
        id: id.clone(),
        group: e.group.clone(),
        group_name: p
            .groups
            .get(&e.group)
            .map(|g| g.name.clone())
            .unwrap_or_default(),
        description: e.fields.description.clone(),
        total: e.fields.total,
        category: e.fields.category.clone(),
        date_ms: e.fields.date_ms,
        notes: e.fields.notes.clone(),
        my_net: paid - owed,
        locked: e.locked,
        published: e.published,
        paid_by: e
            .fields
            .paid_by
            .iter()
            .map(|(u, c)| member_amount(p, u, *c))
            .collect(),
        splits: e
            .fields
            .splits
            .iter()
            .map(|s| member_amount(p, &s.user, s.owed))
            .collect(),
    }
}

/// Most recent first; stable tie-break by id.
fn by_recency(a: &ExpenseView, b: &ExpenseView) -> std::cmp::Ordering {
    b.date_ms.cmp(&a.date_ms).then_with(|| a.id.0.cmp(&b.id.0))
}

pub fn groups(p: &Projection, me: &UserId) -> Vec<GroupSummary> {
    p.groups
        .iter()
        .map(|(id, group)| {
            let net = net_balances(&p.for_group(id));
            GroupSummary {
                id: id.clone(),
                name: group.name.clone(),
                member_count: group.members.len(),
                my_net: net.get(me).copied().unwrap_or(Cents::ZERO),
            }
        })
        .collect()
}

pub fn group_detail(p: &Projection, me: &UserId, group: &GroupId) -> Option<GroupDetail> {
    let record = p.groups.get(group)?;
    let scoped = p.for_group(group);
    let net = net_balances(&scoped);

    let members = record
        .members
        .iter()
        .map(|user| MemberBalance {
            user: user.clone(),
            name: display_name(p, user),
            net: net.get(user).copied().unwrap_or(Cents::ZERO),
        })
        .collect();

    let mut expenses: Vec<ExpenseView> = scoped
        .expenses
        .iter()
        .map(|(id, e)| expense_view(p, me, id, e))
        .collect();
    expenses.sort_by(by_recency);

    let mut settlements: Vec<SettlementView> = scoped
        .settlements
        .iter()
        .map(|(id, s)| SettlementView {
            id: id.clone(),
            from: s.from.clone(),
            from_name: display_name(p, &s.from),
            to: s.to.clone(),
            to_name: display_name(p, &s.to),
            amount: s.amount,
        })
        .collect();
    settlements.sort_by(|a, b| a.id.0.cmp(&b.id.0));

    Some(GroupDetail {
        id: group.clone(),
        name: record.name.clone(),
        members,
        expenses,
        settlements,
        settle_up: settle_up(&scoped),
    })
}

fn participates(e: &ExpenseRecord, user: &UserId) -> bool {
    e.fields.paid_by.contains_key(user) || e.fields.splits.iter().any(|s| &s.user == user)
}

/// Everyone the current user knows, with the running balance to each.
pub fn friends(p: &Projection, me: &UserId) -> Vec<FriendBalance> {
    let owed = pairwise_with(p, me);
    p.users
        .keys()
        .filter(|u| *u != me)
        .map(|u| FriendBalance {
            user: u.clone(),
            name: display_name(p, u),
            net: owed.get(u).copied().unwrap_or(Cents::ZERO),
        })
        .collect()
}

pub fn friend_detail(p: &Projection, me: &UserId, friend: &UserId) -> Option<FriendDetail> {
    p.users.get(friend)?;
    let owed = pairwise_with(p, me);
    let mut shared: Vec<ExpenseView> = p
        .expenses
        .iter()
        .filter(|(_, e)| participates(e, me) && participates(e, friend))
        .map(|(id, e)| expense_view(p, me, id, e))
        .collect();
    shared.sort_by(by_recency);
    Some(FriendDetail {
        user: friend.clone(),
        name: display_name(p, friend),
        net: owed.get(friend).copied().unwrap_or(Cents::ZERO),
        shared,
    })
}

fn money(amount: Cents) -> String {
    let cents = amount.0.abs();
    format!("{}.{:02}", cents / 100, cents % 100)
}

/// A reverse-chronological feed derived from the signed op-log. Names are
/// resolved against the current projection (best effort).
pub fn activity(ops: &[Op], p: &Projection, me: &UserId) -> Vec<ActivityEntry> {
    let mut entries: Vec<ActivityEntry> = ops
        .iter()
        .filter_map(|op| {
            let (kind, summary) = match &op.kind {
                OpKind::CreateGroup { name, .. } => {
                    ("group", format!("Created the group \"{name}\""))
                }
                OpKind::CreateExpense { fields, .. } => {
                    ("expense", format!("Added \"{}\"", fields.description))
                }
                OpKind::EditExpense { fields, .. } => {
                    ("edit", format!("Edited \"{}\"", fields.description))
                }
                OpKind::VoidExpense { .. } => ("delete", "Deleted an expense".to_string()),
                OpKind::RecordSettlement {
                    from, to, amount, ..
                } => (
                    "settlement",
                    format!(
                        "{} paid {} {}",
                        display_name(p, from),
                        display_name(p, to),
                        money(*amount)
                    ),
                ),
                OpKind::UpsertProfile { user, name } if user == me => {
                    ("person", format!("You set your name to {name}"))
                }
                OpKind::UpsertProfile { name, .. } => ("person", format!("Added {name}")),
                _ => return None,
            };
            Some(ActivityEntry {
                kind: kind.to_string(),
                summary,
                wall_ms: op.hlc.wall_ms as i64,
            })
        })
        .collect();
    entries.sort_by(|a, b| b.wall_ms.cmp(&a.wall_ms));
    entries
}
