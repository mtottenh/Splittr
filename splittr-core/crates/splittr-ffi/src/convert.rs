//! Conversions between `splittr-app` types and the FFI DTOs. Kept in one place
//! so the marshalling logic isn't scattered.

use std::collections::BTreeMap;

use splittr_app::{
    ActivityEntry, Cents, ExpenseView, FriendBalance, FriendDetail, GroupDetail, GroupSummary,
    MemberAmount, MemberBalance, SettlementView, SplitPlan, Transfer, UserId,
};

use crate::dto::{
    ActivityEntryDto, ExpenseViewDto, FriendBalanceDto, FriendDetailDto, GroupDetailDto,
    GroupSummaryDto, MemberAmountDto, MemberBalanceDto, Payer, SettlementViewDto, SplitPlanDto,
    TransferDto, Weight,
};

pub(crate) fn to_paid_by(payers: Vec<Payer>) -> BTreeMap<UserId, Cents> {
    payers
        .into_iter()
        .map(|p| (UserId::new(p.user_id), Cents(p.cents)))
        .collect()
}

pub(crate) fn to_split_plan(dto: SplitPlanDto) -> SplitPlan {
    match dto {
        SplitPlanDto::Equal { participants } => SplitPlan::Equal {
            participants: participants.into_iter().map(UserId::new).collect(),
        },
        SplitPlanDto::Exact { amounts } => SplitPlan::Exact {
            amounts: to_paid_by(amounts),
        },
        SplitPlanDto::Weighted { weights } => SplitPlan::Weighted {
            weights: weights
                .into_iter()
                .map(|Weight { user_id, weight }| (UserId::new(user_id), weight))
                .collect(),
        },
    }
}

impl From<GroupSummary> for GroupSummaryDto {
    fn from(g: GroupSummary) -> Self {
        GroupSummaryDto {
            id: g.id.0,
            name: g.name,
            member_count: g.member_count as u32,
            my_net_cents: g.my_net.0,
        }
    }
}

impl From<MemberBalance> for MemberBalanceDto {
    fn from(m: MemberBalance) -> Self {
        MemberBalanceDto {
            user_id: m.user.0,
            name: m.name,
            net_cents: m.net.0,
        }
    }
}

impl From<MemberAmount> for MemberAmountDto {
    fn from(m: MemberAmount) -> Self {
        MemberAmountDto {
            user_id: m.user.0,
            name: m.name,
            cents: m.cents.0,
        }
    }
}

impl From<ExpenseView> for ExpenseViewDto {
    fn from(e: ExpenseView) -> Self {
        ExpenseViewDto {
            id: e.id.0,
            group_id: e.group.map(|g| g.0),
            group_name: e.group_name,
            description: e.description,
            total_cents: e.total.0,
            category: e.category,
            date_ms: e.date_ms,
            notes: e.notes,
            my_net_cents: e.my_net.0,
            locked: e.locked,
            published: e.published,
            paid_by: e.paid_by.into_iter().map(Into::into).collect(),
            splits: e.splits.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<SettlementView> for SettlementViewDto {
    fn from(s: SettlementView) -> Self {
        SettlementViewDto {
            id: s.id.0,
            from: s.from.0,
            from_name: s.from_name,
            to: s.to.0,
            to_name: s.to_name,
            amount_cents: s.amount.0,
        }
    }
}

impl From<FriendBalance> for FriendBalanceDto {
    fn from(f: FriendBalance) -> Self {
        FriendBalanceDto {
            user_id: f.user.0,
            name: f.name,
            net_cents: f.net.0,
        }
    }
}

impl From<FriendDetail> for FriendDetailDto {
    fn from(f: FriendDetail) -> Self {
        FriendDetailDto {
            user_id: f.user.0,
            name: f.name,
            net_cents: f.net.0,
            shared: f.shared.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ActivityEntry> for ActivityEntryDto {
    fn from(a: ActivityEntry) -> Self {
        ActivityEntryDto {
            kind: a.kind,
            summary: a.summary,
            wall_ms: a.wall_ms,
        }
    }
}

impl From<Transfer> for TransferDto {
    fn from(t: Transfer) -> Self {
        TransferDto {
            from: t.from.0,
            to: t.to.0,
            amount_cents: t.amount.0,
        }
    }
}

impl From<GroupDetail> for GroupDetailDto {
    fn from(d: GroupDetail) -> Self {
        GroupDetailDto {
            id: d.id.0,
            name: d.name,
            members: d.members.into_iter().map(Into::into).collect(),
            expenses: d.expenses.into_iter().map(Into::into).collect(),
            settlements: d.settlements.into_iter().map(Into::into).collect(),
            settle_up: d.settle_up.into_iter().map(Into::into).collect(),
        }
    }
}
