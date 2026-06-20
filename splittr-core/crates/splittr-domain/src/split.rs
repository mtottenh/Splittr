//! Splitting an amount across participants.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::UserId;
use crate::money::Cents;

/// One participant's owed share of an expense, in cents.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Split {
    pub user: UserId,
    pub owed: Cents,
}

/// Splits `total` equally across `users`, distributing any remainder cents to
/// the first participants (largest-remainder method). The result always
/// satisfies `sum(result.owed) == total`.
pub fn split_equal(total: Cents, users: &[UserId]) -> Vec<Split> {
    let n = users.len() as i64;
    if n == 0 {
        return Vec::new();
    }
    let base = total.0.div_euclid(n);
    let remainder = total.0 - base * n; // 0..n for total >= 0
    users
        .iter()
        .enumerate()
        .map(|(i, u)| {
            let extra = if (i as i64) < remainder { 1 } else { 0 };
            Split {
                user: u.clone(),
                owed: Cents(base + extra),
            }
        })
        .collect()
}

/// How an expense total should be divided. The four product strategies map on:
/// "equally" → [`SplitPlan::Equal`], "exact amounts" → [`SplitPlan::Exact`],
/// and both "percentages" and "shares" → [`SplitPlan::Weighted`] (the UI
/// supplies the weights). Keeping one weighted strategy avoids duplicating the
/// proportional-distribution logic.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SplitPlan {
    Equal { participants: Vec<UserId> },
    Exact { amounts: BTreeMap<UserId, Cents> },
    Weighted { weights: BTreeMap<UserId, u64> },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SplitError {
    /// No participants were provided.
    Empty,
    /// All weights were zero.
    ZeroWeight,
    /// Exact amounts did not sum to the total.
    Unbalanced { expected: Cents, actual: Cents },
}

impl std::fmt::Display for SplitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SplitError::Empty => write!(f, "no participants"),
            SplitError::ZeroWeight => write!(f, "weights must not all be zero"),
            SplitError::Unbalanced { expected, actual } => write!(
                f,
                "amounts sum to {} but the total is {}",
                actual.0, expected.0
            ),
        }
    }
}

impl std::error::Error for SplitError {}

impl SplitPlan {
    /// Compute per-participant splits that always sum exactly to `total`.
    pub fn compute(&self, total: Cents) -> Result<Vec<Split>, SplitError> {
        match self {
            SplitPlan::Equal { participants } => {
                if participants.is_empty() {
                    return Err(SplitError::Empty);
                }
                Ok(split_equal(total, participants))
            }
            SplitPlan::Exact { amounts } => {
                if amounts.is_empty() {
                    return Err(SplitError::Empty);
                }
                let actual = amounts.values().fold(Cents::ZERO, |acc, c| acc + *c);
                if actual != total {
                    return Err(SplitError::Unbalanced {
                        expected: total,
                        actual,
                    });
                }
                Ok(amounts
                    .iter()
                    .map(|(user, owed)| Split {
                        user: user.clone(),
                        owed: *owed,
                    })
                    .collect())
            }
            SplitPlan::Weighted { weights } => split_by_weights(total, weights),
        }
    }
}

/// Distribute `total` proportionally to `weights`, assigning leftover cents by
/// the largest-remainder method (deterministic, ties broken by user id). The
/// result always sums to `total`. Used for both "percentages" and "shares".
pub fn split_by_weights(
    total: Cents,
    weights: &BTreeMap<UserId, u64>,
) -> Result<Vec<Split>, SplitError> {
    if weights.is_empty() {
        return Err(SplitError::Empty);
    }
    let total_weight: u128 = weights.values().map(|w| *w as u128).sum();
    if total_weight == 0 {
        return Err(SplitError::ZeroWeight);
    }

    // floored share + fractional remainder per user.
    let mut shares: Vec<(UserId, i64, u128)> = Vec::with_capacity(weights.len());
    let mut distributed: i64 = 0;
    for (user, weight) in weights {
        let numerator = total.0 as i128 * *weight as i128;
        let floored = (numerator / total_weight as i128) as i64;
        let remainder = (numerator % total_weight as i128) as u128;
        shares.push((user.clone(), floored, remainder));
        distributed += floored;
    }

    // Hand the leftover cents to the largest remainders first (tie: user id).
    let mut order: Vec<usize> = (0..shares.len()).collect();
    order.sort_by(|&a, &b| {
        shares[b]
            .2
            .cmp(&shares[a].2)
            .then_with(|| shares[a].0.cmp(&shares[b].0))
    });
    let mut leftover = total.0 - distributed;
    let mut i = 0;
    while leftover > 0 {
        shares[order[i % order.len()]].1 += 1;
        leftover -= 1;
        i += 1;
    }

    Ok(shares
        .into_iter()
        .map(|(user, owed, _)| Split {
            user,
            owed: Cents(owed),
        })
        .collect())
}

#[cfg(test)]
mod plan_tests {
    use super::*;

    fn user(n: &str) -> UserId {
        UserId::from(n)
    }

    fn sum(splits: &[Split]) -> i64 {
        splits.iter().map(|s| s.owed.0).sum()
    }

    #[test]
    fn exact_must_balance() {
        let amounts: BTreeMap<UserId, Cents> =
            [(user("a"), Cents(600)), (user("b"), Cents(400))].into();
        let splits = SplitPlan::Exact { amounts }.compute(Cents(1000)).unwrap();
        assert_eq!(sum(&splits), 1000);

        let bad: BTreeMap<UserId, Cents> = [(user("a"), Cents(600))].into();
        assert!(matches!(
            SplitPlan::Exact { amounts: bad }.compute(Cents(1000)),
            Err(SplitError::Unbalanced { .. })
        ));
    }

    #[test]
    fn weighted_distributes_proportionally_and_exactly() {
        // 3:1 over 10.00 → 7.50 / 2.50.
        let weights: BTreeMap<UserId, u64> = [(user("a"), 3), (user("b"), 1)].into();
        let splits = split_by_weights(Cents(1000), &weights).unwrap();
        assert_eq!(
            splits.iter().find(|s| s.user == user("a")).unwrap().owed,
            Cents(750)
        );
        assert_eq!(
            splits.iter().find(|s| s.user == user("b")).unwrap().owed,
            Cents(250)
        );
        assert_eq!(sum(&splits), 1000);
    }

    #[test]
    fn weighted_handles_indivisible_totals_exactly() {
        // 1:1:1 over 10.00 → 3.34 / 3.33 / 3.33.
        let weights: BTreeMap<UserId, u64> =
            [(user("a"), 1), (user("b"), 1), (user("c"), 1)].into();
        let splits = split_by_weights(Cents(1000), &weights).unwrap();
        assert_eq!(sum(&splits), 1000);
        let mut owed: Vec<i64> = splits.iter().map(|s| s.owed.0).collect();
        owed.sort_unstable();
        assert_eq!(owed, vec![333, 333, 334]);
    }

    #[test]
    fn zero_weights_rejected() {
        let weights: BTreeMap<UserId, u64> = [(user("a"), 0), (user("b"), 0)].into();
        assert_eq!(
            split_by_weights(Cents(1000), &weights),
            Err(SplitError::ZeroWeight)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn users(names: &[&str]) -> Vec<UserId> {
        names.iter().map(|n| UserId::from(*n)).collect()
    }

    #[test]
    fn distributes_remainder_to_first() {
        let s = split_equal(Cents(1000), &users(&["a", "b", "c"]));
        assert_eq!(
            s.iter().map(|x| x.owed.0).collect::<Vec<_>>(),
            vec![334, 333, 333]
        );
        assert_eq!(s.iter().map(|x| x.owed.0).sum::<i64>(), 1000);
    }

    #[test]
    fn divides_cleanly() {
        let s = split_equal(Cents(900), &users(&["a", "b"]));
        assert_eq!(
            s.iter().map(|x| x.owed.0).collect::<Vec<_>>(),
            vec![450, 450]
        );
    }

    #[test]
    fn no_users_is_empty() {
        assert!(split_equal(Cents(100), &[]).is_empty());
    }
}
