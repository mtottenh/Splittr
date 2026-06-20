//! Splitting an amount across participants.

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
