//! Deriving balances and settle-up suggestions from a [`Projection`].

use std::collections::BTreeMap;

use splittr_domain::{simplify_debts, Cents, Transfer, UserId};

use crate::projection::Projection;

/// Net balance per user, in cents (positive = is owed; negative = owes).
///
/// Handles multiple payers per expense. User ids are resolved through the alias
/// map first, so a claim/merge (#8) never changes the totals. The values always
/// sum to zero for a valid ledger.
pub fn net_balances(p: &Projection) -> BTreeMap<UserId, Cents> {
    let resolve = |u: &UserId| p.aliases.get(u).cloned().unwrap_or_else(|| u.clone());
    let mut net: BTreeMap<UserId, Cents> = BTreeMap::new();

    for e in p.expenses.values() {
        for (payer, paid) in &e.fields.paid_by {
            *net.entry(resolve(payer)).or_default() += *paid;
        }
        for s in &e.fields.splits {
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

/// Suggested payments to settle everyone up, minimising the number of transfers
/// (the "settle up" feature). A deterministic projection over [`net_balances`].
pub fn settle_up(p: &Projection) -> Vec<Transfer> {
    simplify_debts(&net_balances(p))
}
