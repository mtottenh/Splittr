//! Conversions between `splittr-app` types and the FFI DTOs. Kept in one place
//! so the marshalling logic isn't scattered.

use std::collections::BTreeMap;

use splittr_app::{
    Cents, ExpenseView, GroupDetail, GroupSummary, MemberBalance, SplitPlan, Transfer, UserId,
};

use crate::dto::{
    ExpenseViewDto, GroupDetailDto, GroupSummaryDto, MemberBalanceDto, Payer, SplitPlanDto,
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

impl From<ExpenseView> for ExpenseViewDto {
    fn from(e: ExpenseView) -> Self {
        ExpenseViewDto {
            id: e.id.0,
            description: e.description,
            total_cents: e.total.0,
            category: e.category,
            date_ms: e.date_ms,
            my_net_cents: e.my_net.0,
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
            settle_up: d.settle_up.into_iter().map(Into::into).collect(),
        }
    }
}
