//! The headline guarantee, property-tested: however a fixed op universe is
//! partitioned across two replicas (A-only / B-only / both), one sync session
//! leaves them holding the identical op-set and folding to identical balances.
//! This is the convergence proptest from #19, re-asserted across the sync layer.

mod common;

use std::collections::BTreeSet;

use common::{converge, op_pool, replica};
use proptest::prelude::*;
use splittr_sync::SyncStore;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn any_partition_converges(assign in prop::collection::vec(0u8..3, op_pool().len())) {
        let pool = op_pool();
        let mut a = replica();
        let mut b = replica();
        // 0 → A only, 1 → B only, 2 → both.
        for (op, who) in pool.iter().zip(&assign) {
            if *who != 1 {
                a.append(op).unwrap();
            }
            if *who != 0 {
                b.append(op).unwrap();
            }
        }

        converge(&mut a, &mut b);

        let ia: BTreeSet<_> = a.op_ids().into_iter().collect();
        let ib: BTreeSet<_> = b.op_ids().into_iter().collect();
        prop_assert_eq!(ia, ib, "op-sets unified for every partition");
        prop_assert_eq!(a.projection(), b.projection(), "identical fold");
    }
}
