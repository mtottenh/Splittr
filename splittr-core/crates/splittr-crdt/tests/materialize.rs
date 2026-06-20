//! The incremental [`Materializer`] must agree with the batch [`project`], and
//! re-applying ops must be a no-op. This is the rebuild-equivalence guarantee
//! (ADR-0001 §Testing) that the store/sync layers will rely on.

mod common;

use common::op_log;
use proptest::prelude::*;
use splittr_crdt::{project, Materializer};

proptest! {
    /// Feeding ops one at a time yields the same projection as a batch fold.
    #[test]
    fn incremental_apply_matches_batch(ops in op_log()) {
        let mut materializer = Materializer::default();
        for op in &ops {
            materializer.apply(op);
        }
        prop_assert_eq!(materializer.projection(), project(&ops));
    }

    /// Applying the whole log a second time changes nothing (internal dedup).
    #[test]
    fn reapplying_ops_is_idempotent(ops in op_log()) {
        let mut materializer = Materializer::default();
        materializer.apply_all(ops.iter());
        let once = materializer.projection();
        materializer.apply_all(ops.iter());
        prop_assert_eq!(materializer.projection(), once);
    }
}
