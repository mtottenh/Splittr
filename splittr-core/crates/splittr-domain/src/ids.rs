//! Strongly-typed identifiers. Each id is its own newtype so a `UserId` can
//! never be passed where a `GroupId` is expected.

use serde::{Deserialize, Serialize};

/// Generates a `String`-backed newtype id with the usual conveniences.
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
