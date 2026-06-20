//! `splittr-domain` — pure value types and money/split math.
//!
//! No I/O, no async, no Flutter. This is the deterministic foundation the CRDT
//! fold builds on (see `../../docs/ARCHITECTURE.md` and ADR-0001). Money is
//! always integer cents so splits reconcile to the penny.

use serde::{Deserialize, Serialize};

/// A monetary amount in integer minor units (cents).
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct Cents(pub i64);

impl Cents {
    pub const ZERO: Cents = Cents(0);
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl core::ops::Add for Cents {
    type Output = Cents;
    fn add(self, o: Cents) -> Cents {
        Cents(self.0 + o.0)
    }
}
impl core::ops::Sub for Cents {
    type Output = Cents;
    fn sub(self, o: Cents) -> Cents {
        Cents(self.0 - o.0)
    }
}
impl core::ops::Neg for Cents {
    type Output = Cents;
    fn neg(self) -> Cents {
        Cents(-self.0)
    }
}
impl core::ops::AddAssign for Cents {
    fn add_assign(&mut self, o: Cents) {
        self.0 += o.0;
    }
}
impl core::ops::SubAssign for Cents {
    fn sub_assign(&mut self, o: Cents) {
        self.0 -= o.0;
    }
}

/// Generates a `String`-backed newtype id with the usual conveniences. Using
/// distinct types for each id prevents mixing a `UserId` with a `GroupId`.
macro_rules! string_id {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self { $name(s.into()) }
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl From<&str> for $name {
            fn from(s: &str) -> Self { $name(s.to_string()) }
        }
    };
}

string_id!(
    /// A person (real account or placeholder — see #2).
    UserId
);
string_id!(
    /// A shared ledger / group.
    GroupId
);
string_id!(
    /// An expense.
    ExpenseId
);
string_id!(
    /// A settlement (cash payment that pays down a debt).
    SettlementId
);

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
    fn equal_split_distributes_remainder_to_first() {
        let s = split_equal(Cents(1000), &users(&["a", "b", "c"]));
        assert_eq!(
            s.iter().map(|x| x.owed.0).collect::<Vec<_>>(),
            vec![334, 333, 333]
        );
        assert_eq!(s.iter().map(|x| x.owed.0).sum::<i64>(), 1000);
    }

    #[test]
    fn equal_split_divides_cleanly() {
        let s = split_equal(Cents(900), &users(&["a", "b"]));
        assert_eq!(
            s.iter().map(|x| x.owed.0).collect::<Vec<_>>(),
            vec![450, 450]
        );
    }

    #[test]
    fn equal_split_no_users_is_empty() {
        assert!(split_equal(Cents(100), &[]).is_empty());
    }
}
