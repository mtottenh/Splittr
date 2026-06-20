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
        if !e.published {
            continue; // drafts don't affect balances (#15)
        }
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

/// What every other user owes the local user `me`, in cents (positive = the
/// other user owes `me`; negative = `me` owes them) — the per-friend balances.
///
/// Each expense is attributed to a minimal set of debtor→creditor transfers (a
/// per-expense settle-up), recorded settlements are folded in, and only the
/// edges that touch `me` are kept. This reconciles exactly with
/// [`net_balances`]: `sum(pairwise_with(p, me).values()) == net_balances(p)[me]`.
pub fn pairwise_with(p: &Projection, me: &UserId) -> BTreeMap<UserId, Cents> {
    let resolve = |u: &UserId| p.aliases.get(u).cloned().unwrap_or_else(|| u.clone());
    let me = resolve(me);
    let mut owed: BTreeMap<UserId, Cents> = BTreeMap::new();

    for e in p.expenses.values() {
        if !e.published {
            continue; // drafts don't affect balances (#15)
        }
        let mut net: BTreeMap<UserId, Cents> = BTreeMap::new();
        for (payer, paid) in &e.fields.paid_by {
            *net.entry(resolve(payer)).or_default() += *paid;
        }
        for s in &e.fields.splits {
            *net.entry(resolve(&s.user)).or_default() -= s.owed;
        }
        net.retain(|_, c| !c.is_zero());
        // A per-expense settle-up yields integer transfers that exactly
        // represent this expense's nets, so the pairwise totals stay exact.
        for t in simplify_debts(&net) {
            if t.to == me {
                *owed.entry(t.from).or_default() += t.amount;
            } else if t.from == me {
                *owed.entry(t.to).or_default() -= t.amount;
            }
        }
    }

    for s in p.settlements.values() {
        let (from, to) = (resolve(&s.from), resolve(&s.to));
        if to == me {
            // Someone paid `me`: they now owe `me` less.
            *owed.entry(from).or_default() -= s.amount;
        } else if from == me {
            // `me` paid someone: they owe `me` more.
            *owed.entry(to).or_default() += s.amount;
        }
    }

    owed.retain(|_, c| !c.is_zero());
    owed
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use splittr_domain::{ExpenseFields, ExpenseId, GroupId, Split, UserId};

    use super::*;
    use crate::projection::{ExpenseRecord, GroupRecord, Projection, SettlementRecord};

    fn uid(s: &str) -> UserId {
        UserId::new(s)
    }

    /// Build a projection with one group and the given expenses/settlements.
    fn projection(
        members: &[&str],
        expenses: Vec<ExpenseRecord>,
        settlements: Vec<SettlementRecord>,
    ) -> Projection {
        let group = GroupId::new("g");
        let mut p = Projection::default();
        p.groups.insert(
            group,
            GroupRecord {
                name: "g".into(),
                currency: "USD".into(),
                members: members.iter().map(|m| uid(m)).collect::<BTreeSet<_>>(),
                closed_until_ms: 0,
            },
        );
        for (i, e) in expenses.into_iter().enumerate() {
            p.expenses.insert(ExpenseId::new(format!("e{i}")), e);
        }
        for (i, s) in settlements.into_iter().enumerate() {
            p.settlements
                .insert(splittr_domain::SettlementId::new(format!("s{i}")), s);
        }
        p
    }

    fn expense(paid_by: &[(&str, i64)], splits: &[(&str, i64)]) -> ExpenseRecord {
        let paid: BTreeMap<UserId, Cents> =
            paid_by.iter().map(|(u, c)| (uid(u), Cents(*c))).collect();
        let total = Cents(paid.values().map(|c| c.0).sum());
        let splits = splits
            .iter()
            .map(|(u, c)| Split {
                user: uid(u),
                owed: Cents(*c),
            })
            .collect();
        ExpenseRecord {
            group: Some(GroupId::new("g")),
            fields: ExpenseFields::new(paid, total, splits),
            locked: false,
            published: true,
        }
    }

    fn settlement(from: &str, to: &str, amount: i64) -> SettlementRecord {
        SettlementRecord {
            group: Some(GroupId::new("g")),
            from: uid(from),
            to: uid(to),
            amount: Cents(amount),
        }
    }

    #[test]
    fn pairwise_tracks_a_simple_debt() {
        // Alice paid 30, split equally three ways: bob & carol each owe 10.
        let p = projection(
            &["a", "b", "c"],
            vec![expense(&[("a", 30)], &[("a", 10), ("b", 10), ("c", 10)])],
            vec![],
        );
        let owed = pairwise_with(&p, &uid("a"));
        assert_eq!(owed.get(&uid("b")), Some(&Cents(10)));
        assert_eq!(owed.get(&uid("c")), Some(&Cents(10)));
    }

    #[test]
    fn a_settlement_clears_the_pairwise_balance() {
        let p = projection(
            &["a", "b"],
            vec![expense(&[("a", 20)], &[("a", 10), ("b", 10)])],
            vec![settlement("b", "a", 10)],
        );
        assert!(pairwise_with(&p, &uid("a")).is_empty());
    }

    proptest::proptest! {
        /// The per-friend balances always reconcile with the overall net.
        #[test]
        fn pairwise_reconciles_with_net(
            // Each tuple is (payer_index, total_cents) for an equal 3-way split.
            specs in proptest::collection::vec((0usize..3, 1i64..10_000), 0..8),
        ) {
            let people = ["a", "b", "c"];
            let mut expenses = Vec::new();
            for (payer, total) in &specs {
                // Equal-ish split across all three, remainder to the first.
                let base = total / 3;
                let rem = total - base * 3;
                let splits = vec![
                    ("a", base + rem),
                    ("b", base),
                    ("c", base),
                ];
                expenses.push(expense(&[(people[*payer], *total)], &splits));
            }
            let p = projection(&people, expenses, vec![]);
            let net = net_balances(&p);
            for me in people {
                let owed = pairwise_with(&p, &uid(me));
                let sum: i64 = owed.values().map(|c| c.0).sum();
                let expected = net.get(&uid(me)).copied().unwrap_or(Cents::ZERO).0;
                proptest::prop_assert_eq!(sum, expected);
            }
        }
    }
}
