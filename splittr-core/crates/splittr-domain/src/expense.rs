//! The mutable fields of an expense — its "version". Carried by create/edit ops
//! and stored as the single winning version under whole-version LWW
//! (ADR-0001 rule 4). Supports multiple payers.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::UserId;
use crate::money::Cents;
use crate::split::Split;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ExpenseFields {
    /// Who paid, and how much. Supports the common single-payer case and the
    /// occasional "we both chipped in" case.
    pub paid_by: BTreeMap<UserId, Cents>,
    pub total: Cents,
    pub splits: Vec<Split>,
    /// Unix epoch milliseconds.
    pub date_ms: i64,
    pub category: String,
    pub notes: Option<String>,
}

impl ExpenseFields {
    /// Build with the given payers/splits and default metadata.
    pub fn new(paid_by: BTreeMap<UserId, Cents>, total: Cents, splits: Vec<Split>) -> Self {
        Self {
            paid_by,
            total,
            splits,
            date_ms: 0,
            category: "general".to_string(),
            notes: None,
        }
    }

    /// Convenience for the common single-payer expense.
    pub fn single_payer(payer: UserId, total: Cents, splits: Vec<Split>) -> Self {
        let mut paid_by = BTreeMap::new();
        paid_by.insert(payer, total);
        Self::new(paid_by, total, splits)
    }

    pub fn total_paid(&self) -> Cents {
        self.paid_by
            .values()
            .copied()
            .fold(Cents::ZERO, |acc, c| acc + c)
    }

    pub fn total_owed(&self) -> Cents {
        self.splits.iter().fold(Cents::ZERO, |acc, s| acc + s.owed)
    }

    /// Both the payments and the splits reconcile to the total. Validated by the
    /// application layer before an op is created.
    pub fn is_balanced(&self) -> bool {
        self.total_paid() == self.total && self.total_owed() == self.total
    }
}
