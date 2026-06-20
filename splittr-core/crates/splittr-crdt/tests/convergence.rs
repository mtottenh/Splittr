//! Property-based convergence tests — the headline guarantee of ADR-0001:
//! the projection is a pure function of the op **set**, so any delivery order,
//! reordering or duplication produces identical state and balances.

use proptest::prelude::*;
use splittr_crdt::*;
use splittr_domain::{split_equal, Cents, ExpenseId, GroupId, SettlementId, Split, UserId};

// A small fixed universe keeps op ids colliding on the same entities, which is
// what actually exercises the conflict rules.
fn user(i: u8) -> UserId {
    UserId::new(format!("u{i}"))
}
fn group(i: u8) -> GroupId {
    GroupId::new(format!("g{i}"))
}
fn expense(i: u8) -> ExpenseId {
    ExpenseId::new(format!("e{i}"))
}
fn settlement(i: u8) -> SettlementId {
    SettlementId::new(format!("s{i}"))
}

fn participants(mask: &[bool]) -> Vec<UserId> {
    let mut users: Vec<UserId> = mask
        .iter()
        .enumerate()
        .filter(|(_, b)| **b)
        .map(|(i, _)| user(i as u8))
        .collect();
    if users.is_empty() {
        users.push(user(0));
    }
    users
}

fn valid_splits(total: i64, mask: &[bool]) -> Vec<Split> {
    split_equal(Cents(total), &participants(mask))
}

fn op_kind() -> impl Strategy<Value = OpKind> {
    prop_oneof![
        (0u8..2).prop_map(|g| OpKind::CreateGroup {
            group: group(g),
            name: format!("group{g}")
        }),
        (0u8..2, 0u8..3).prop_map(|(g, n)| OpKind::SetGroupName {
            group: group(g),
            name: format!("name{n}")
        }),
        (0u8..2, 0u8..4, any::<bool>()).prop_map(|(g, u, m)| OpKind::SetMembership {
            group: group(g),
            user: user(u),
            member: m
        }),
        (
            0u8..6,
            0u8..2,
            0u8..4,
            prop::collection::vec(any::<bool>(), 4..=4),
            1i64..100_000
        )
            .prop_map(|(e, g, payer, mask, total)| OpKind::CreateExpense {
                expense: expense(e),
                group: group(g),
                payer: user(payer),
                total: Cents(total),
                splits: valid_splits(total, &mask),
            }),
        (
            0u8..6,
            0u8..4,
            prop::collection::vec(any::<bool>(), 4..=4),
            1i64..100_000
        )
            .prop_map(|(e, payer, mask, total)| OpKind::EditExpense {
                expense: expense(e),
                payer: user(payer),
                total: Cents(total),
                splits: valid_splits(total, &mask),
            }),
        (0u8..6).prop_map(|e| OpKind::VoidExpense {
            expense: expense(e)
        }),
        (0u8..4, 0u8..2, 0u8..4, 0u8..4, 1i64..100_000).prop_map(|(s, g, from, to, amount)| {
            OpKind::RecordSettlement {
                settlement: settlement(s),
                group: group(g),
                from: user(from),
                to: user(to),
                amount: Cents(amount),
            }
        }),
        (0u8..4).prop_map(|s| OpKind::VoidSettlement {
            settlement: settlement(s)
        }),
        (0u8..4, 0u8..4).prop_map(|(a, b)| OpKind::AddAlias {
            alias: user(a),
            canonical: user(b)
        }),
    ]
}

/// A log of ops. Each gets a unique HLC counter (so all op ids are unique), with
/// a randomised wall time + site so HLC order is deliberately *not* aligned with
/// position — proving the fold uses the HLC, not arrival order.
fn op_log() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec((op_kind(), 0u64..5, 0u64..3), 0..40).prop_map(|specs| {
        specs
            .into_iter()
            .enumerate()
            .map(|(i, (kind, wall, site))| {
                let hlc = Hlc {
                    wall_ms: wall,
                    counter: i as u32,
                    site: SiteId(site),
                };
                Op::new(hlc, ActorId(format!("site{site}")), kind)
            })
            .collect()
    })
}

fn shuffle(ops: &mut [Op], seed: u64) {
    // Deterministic LCG-based Fisher–Yates so the permutation is reproducible.
    let mut s = seed ^ 0x9E37_79B9_7F4A_7C15;
    for i in (1..ops.len()).rev() {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (s >> 33) as usize % (i + 1);
        ops.swap(i, j);
    }
}

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
        by_id.sort_by(|a, b| a.id.cmp(&b.id));
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
