//! Read-side view-models — UI-friendly projections over the engine state.

use splittr_crdt::{
    net_balances, settle_up, Cents, ExpenseId, GroupId, Projection, Transfer, UserId,
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

/// An expense as shown in a group's ledger.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExpenseView {
    pub id: ExpenseId,
    pub description: String,
    pub total: Cents,
    pub category: String,
    pub date_ms: i64,
    /// The current user's involvement: paid minus owed (positive = lent).
    pub my_net: Cents,
}

/// Everything the group-detail screen needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GroupDetail {
    pub id: GroupId,
    pub name: String,
    pub members: Vec<MemberBalance>,
    pub expenses: Vec<ExpenseView>,
    pub settle_up: Vec<Transfer>,
}

fn display_name(p: &Projection, user: &UserId) -> String {
    p.users
        .get(user)
        .map(|r| r.name.clone())
        .unwrap_or_else(|| user.to_string())
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
        .map(|(id, e)| {
            let paid = e.fields.paid_by.get(me).copied().unwrap_or(Cents::ZERO);
            let owed = e
                .fields
                .splits
                .iter()
                .filter(|s| &s.user == me)
                .fold(Cents::ZERO, |acc, s| acc + s.owed);
            ExpenseView {
                id: id.clone(),
                description: e.fields.description.clone(),
                total: e.fields.total,
                category: e.fields.category.clone(),
                date_ms: e.fields.date_ms,
                my_net: paid - owed,
            }
        })
        .collect();
    // Most recent first; stable tie-break by id.
    expenses.sort_by(|a, b| b.date_ms.cmp(&a.date_ms).then_with(|| a.id.0.cmp(&b.id.0)));

    Some(GroupDetail {
        id: group.clone(),
        name: record.name.clone(),
        members,
        expenses,
        settle_up: settle_up(&scoped),
    })
}
