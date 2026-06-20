//! Minimal-cash-flow debt simplification (the "settle up" suggestion).

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::UserId;
use crate::money::Cents;

/// A suggested payment: `from` should pay `to` `amount`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Transfer {
    pub from: UserId,
    pub to: UserId,
    pub amount: Cents,
}

fn by_amount_desc(a: &(UserId, i64), b: &(UserId, i64)) -> Ordering {
    b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
}

/// Who should pay whom to settle `balances` (positive = is owed, negative =
/// owes) in as few transfers as possible.
///
/// Greedy largest-creditor / largest-debtor matching: each transfer zeroes at
/// least one party, so the number of transfers never exceeds `n - 1` for `n`
/// non-zero members. Deterministic — ties are broken by user id — so the
/// suggestion is stable between runs and across replicas.
pub fn simplify_debts(balances: &BTreeMap<UserId, Cents>) -> Vec<Transfer> {
    let mut creditors: Vec<(UserId, i64)> = balances
        .iter()
        .filter(|(_, c)| c.0 > 0)
        .map(|(u, c)| (u.clone(), c.0))
        .collect();
    let mut debtors: Vec<(UserId, i64)> = balances
        .iter()
        .filter(|(_, c)| c.0 < 0)
        .map(|(u, c)| (u.clone(), -c.0))
        .collect();

    creditors.sort_by(by_amount_desc);
    debtors.sort_by(by_amount_desc);

    let mut transfers = Vec::new();
    let (mut d, mut c) = (0usize, 0usize);
    while d < debtors.len() && c < creditors.len() {
        let pay = debtors[d].1.min(creditors[c].1);
        if pay > 0 {
            transfers.push(Transfer {
                from: debtors[d].0.clone(),
                to: creditors[c].0.clone(),
                amount: Cents(pay),
            });
        }
        debtors[d].1 -= pay;
        creditors[c].1 -= pay;
        if debtors[d].1 == 0 {
            d += 1;
        }
        if creditors[c].1 == 0 {
            c += 1;
        }
    }
    transfers
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn balances(pairs: &[(&str, i64)]) -> BTreeMap<UserId, Cents> {
        pairs
            .iter()
            .map(|(u, c)| (UserId::from(*u), Cents(*c)))
            .collect()
    }

    /// Apply transfers back onto the balances; a settled ledger is all-zero.
    fn apply(mut bal: BTreeMap<UserId, Cents>, transfers: &[Transfer]) -> BTreeMap<UserId, Cents> {
        for t in transfers {
            *bal.entry(t.from.clone()).or_default() += t.amount; // debtor pays → rises toward 0
            *bal.entry(t.to.clone()).or_default() -= t.amount; // creditor's due decreases
        }
        bal
    }

    #[test]
    fn settles_a_simple_chain() {
        let t = simplify_debts(&balances(&[("a", -500), ("c", 500)]));
        assert_eq!(
            t,
            vec![Transfer {
                from: UserId::from("a"),
                to: UserId::from("c"),
                amount: Cents(500)
            }]
        );
    }

    #[test]
    fn one_debtor_pays_two_creditors() {
        let bal = balances(&[("a", -1000), ("b", 500), ("c", 500)]);
        let t = simplify_debts(&bal);
        assert_eq!(t.len(), 2);
        assert!(apply(bal, &t).values().all(|c| c.is_zero()));
    }

    proptest! {
        /// For any zero-sum balances: transfers fully settle the ledger, use at
        /// most n-1 payments, and conserve the total amount owed.
        #[test]
        fn always_settles_to_zero(amounts in prop::collection::vec(-100_000i64..100_000, 0..12)) {
            let mut bal: BTreeMap<UserId, Cents> = BTreeMap::new();
            for (i, a) in amounts.iter().enumerate() {
                bal.insert(UserId::new(format!("u{i}")), Cents(*a));
            }
            // A balancing entry makes the ledger sum to zero.
            let sum: i64 = amounts.iter().sum();
            bal.insert(UserId::new("balancer"), Cents(-sum));
            bal.retain(|_, c| !c.is_zero());

            let nonzero = bal.len();
            let transfers = simplify_debts(&bal);

            prop_assert!(transfers.len() <= nonzero.saturating_sub(1));
            prop_assert!(apply(bal.clone(), &transfers).values().all(|c| c.is_zero()));

            let owed: i64 = bal.values().filter(|c| c.0 < 0).map(|c| -c.0).sum();
            let moved: i64 = transfers.iter().map(|t| t.amount.0).sum();
            prop_assert_eq!(moved, owed);
        }
    }
}
