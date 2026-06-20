//! Property-based convergence tests — the headline guarantee of ADR-0001:
//! the projection is a pure function of the op **set**, so any delivery order,
//! reordering or duplication produces identical state and balances.

mod common;

use common::{op_log, shuffle};
use proptest::prelude::*;
use splittr_crdt::{net_balances, project};

proptest! {
    /// The projection (and the balances) are identical regardless of the order
    /// in which ops are applied.
    #[test]
    fn convergence_under_reordering(ops in op_log(), seed in any::<u64>()) {
        let base = project(&ops);

        let mut reversed = ops.clone();
        reversed.reverse();
        prop_assert_eq!(project(&reversed), base.clone());

        let mut by_id = ops.clone();
        by_id.sort_by_key(|a| a.id);
        prop_assert_eq!(project(&by_id), base.clone());

        let mut shuffled = ops.clone();
        shuffle(&mut shuffled, seed);
        let shuffled_proj = project(&shuffled);
        prop_assert_eq!(&shuffled_proj, &base);
        prop_assert_eq!(net_balances(&shuffled_proj), net_balances(&base));
    }

    /// Applying the same ops twice changes nothing (set semantics / dedup).
    #[test]
    fn idempotent_under_duplication(ops in op_log()) {
        let base = project(&ops);
        let mut doubled = ops.clone();
        doubled.extend(ops.clone());
        prop_assert_eq!(project(&doubled), base);
    }

    /// With valid expenses and two-party settlements, balances always net to
    /// zero — the money invariant.
    #[test]
    fn balances_always_sum_to_zero(ops in op_log()) {
        let balances = net_balances(&project(&ops));
        let sum: i64 = balances.values().map(|c| c.0).sum();
        prop_assert_eq!(sum, 0);
    }
}
