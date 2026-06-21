//! Conversions between `splittr-app` types and the FFI DTOs. Kept in one place
//! so the marshalling logic isn't scattered.

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use splittr_app::{
    ActivityEntry, Cents, DeviceView, ExpenseFieldsInput, ExpenseView, FriendBalance, FriendDetail,
    GroupDetail, GroupSummary, MemberAmount, MemberBalance, OriginalAmount, SettlementView,
    SplitPlan, Transfer, UserId,
};

use crate::dto::{
    ActivityEntryDto, DeviceViewDto, ExpenseInput, ExpenseViewDto, FriendBalanceDto,
    FriendDetailDto, GroupDetailDto, GroupSummaryDto, MemberAmountDto, MemberBalanceDto,
    OriginalAmountDto, Payer, SettlementViewDto, SplitPlanDto, TransferDto, Weight,
};

impl From<DeviceView> for DeviceViewDto {
    fn from(d: DeviceView) -> Self {
        DeviceViewDto {
            device: d.device,
            site: d.site,
            revoked: d.revoked,
            this_device: d.this_device,
        }
    }
}

/// FFI → domain conversion for the optional foreign-currency metadata (#3).
/// Rejects a negative `rate_micro` at the boundary instead of wrapping it to a
/// huge magnitude via `as u64`.
pub(crate) fn to_original(dto: Option<OriginalAmountDto>) -> Result<Option<OriginalAmount>> {
    dto.map(|o| {
        Ok(OriginalAmount {
            currency: o.currency,
            amount: Cents(o.amount_cents),
            rate_micro: rate_micro_u64(o.rate_micro)?,
        })
    })
    .transpose()
}

/// Validate a `rate_micro` crossing the FFI as `i64`: it must be non-negative.
pub(crate) fn rate_micro_u64(rate_micro: i64) -> Result<u64> {
    u64::try_from(rate_micro).map_err(|_| anyhow!("rate_micro must be non-negative"))
}

impl From<OriginalAmount> for OriginalAmountDto {
    fn from(o: OriginalAmount) -> Self {
        OriginalAmountDto {
            currency: o.currency,
            amount_cents: o.amount.0,
            rate_micro: o.rate_micro as i64,
        }
    }
}

/// FFI → domain conversion for the editable expense fields (#3/#15/#31). The
/// `group_id`/`draft` flags on [`ExpenseInput`] are handled by the caller.
pub(crate) fn to_fields(input: ExpenseInput) -> Result<ExpenseFieldsInput> {
    Ok(ExpenseFieldsInput {
        description: input.description,
        paid_by: to_paid_by(input.paid_by),
        total: Cents(input.total_cents),
        split: to_split_plan(input.split),
        category: input.category,
        notes: input.notes,
        date_ms: input.date_ms,
        original: to_original(input.original)?,
    })
}

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
            currency: g.currency,
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
            currency: e.currency,
            category: e.category,
            date_ms: e.date_ms,
            notes: e.notes,
            my_net_cents: e.my_net.0,
            locked: e.locked,
            published: e.published,
            original: e.original.map(Into::into),
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
            currency: d.currency,
            members: d.members.into_iter().map(Into::into).collect(),
            expenses: d.expenses.into_iter().map(Into::into).collect(),
            settlements: d.settlements.into_iter().map(Into::into).collect(),
            settle_up: d.settle_up.into_iter().map(Into::into).collect(),
        }
    }
}
