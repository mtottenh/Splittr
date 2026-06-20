//! Shared proptest generators for the integration tests (kept in one place so
//! the convergence and materializer suites don't duplicate them).

#![allow(dead_code)] // not every test binary uses every helper

use std::collections::BTreeMap;

use proptest::prelude::*;
use splittr_crdt::*;

// A small fixed universe keeps op ids colliding on the same entities, which is
// what actually exercises the conflict rules.
pub fn user(i: u8) -> UserId {
    UserId::new(format!("u{i}"))
}
pub fn group(i: u8) -> GroupId {
    GroupId::new(format!("g{i}"))
}
pub fn expense(i: u8) -> ExpenseId {
    ExpenseId::new(format!("e{i}"))
}
pub fn settlement(i: u8) -> SettlementId {
    SettlementId::new(format!("s{i}"))
}

/// A deterministic signing key per site, so generated ops are validly signed.
fn site_key(site: u64) -> SigningKey {
    SigningKey::from_seed([site as u8; 32])
}

fn site_pub(site: u64) -> PublicKey {
    site_key(site).public()
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

/// A balanced expense version: payments and splits both sum to `total`, with
/// payments distributed across the masked payers (so multi-payer is exercised).
fn fields(total: i64, payer_mask: &[bool], split_mask: &[bool]) -> ExpenseFields {
    let payers = participants(payer_mask);
    let paid_by: BTreeMap<UserId, Cents> = split_equal(Cents(total), &payers)
        .into_iter()
        .map(|s| (s.user, s.owed))
        .collect();
    ExpenseFields::new(paid_by, Cents(total), valid_splits(total, split_mask))
}

pub fn op_kind() -> impl Strategy<Value = OpKind> {
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
            prop::collection::vec(any::<bool>(), 4..=4),
            prop::collection::vec(any::<bool>(), 4..=4),
            1i64..100_000,
            any::<bool>(),
        )
            .prop_map(|(e, g, payer_mask, split_mask, total, draft)| {
                OpKind::CreateExpense {
                    expense: expense(e),
                    group: Some(group(g)),
                    fields: fields(total, &payer_mask, &split_mask),
                    draft,
                }
            }),
        (
            0u8..6,
            prop::collection::vec(any::<bool>(), 4..=4),
            prop::collection::vec(any::<bool>(), 4..=4),
            1i64..100_000
        )
            .prop_map(|(e, payer_mask, split_mask, total)| OpKind::EditExpense {
                expense: expense(e),
                fields: fields(total, &payer_mask, &split_mask),
            }),
        (0u8..6).prop_map(|e| OpKind::PublishExpense {
            expense: expense(e)
        }),
        (0u8..6).prop_map(|e| OpKind::VoidExpense {
            expense: expense(e)
        }),
        (0u8..2, 0i64..100).prop_map(|(g, until)| OpKind::SetClosedPeriod {
            group: group(g),
            until_ms: until,
        }),
        (0u8..4, 0u8..2, 0u8..4, 0u8..4, 1i64..100_000).prop_map(|(s, g, from, to, amount)| {
            OpKind::RecordSettlement {
                settlement: settlement(s),
                group: Some(group(g)),
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
        (0u8..4, 0u8..3).prop_map(|(u, n)| OpKind::UpsertProfile {
            user: user(u),
            name: format!("name{n}")
        }),
        (0u8..4, any::<u8>()).prop_map(|(u, k)| OpKind::SetAgreementKey {
            user: user(u),
            key: [k; 32],
        }),
        (0u8..6, any::<bool>()).prop_map(|(e, locked)| OpKind::SetExpenseLock {
            expense: expense(e),
            locked
        }),
        // Device authorize/revoke (#16). identity = site 0 so some of these are
        // self-signed (honoured) and the rest exercise the ignore path — both
        // must fold order-independently.
        (1u64..3, 0u64..3).prop_map(|(dev, site)| OpKind::AuthorizeDevice {
            identity: site_pub(0),
            device: site_pub(dev),
            site,
        }),
        (1u64..3).prop_map(|dev| OpKind::RevokeDevice {
            identity: site_pub(0),
            device: site_pub(dev),
        }),
    ]
}

/// A log of ops. Each gets a unique HLC counter (so all op ids are unique), with
/// a randomised wall time + site so HLC order is deliberately *not* aligned with
/// position — proving the fold uses the HLC, not arrival order.
pub fn op_log() -> impl Strategy<Value = Vec<Op>> {
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
                Op::signed(hlc, &site_key(site), kind)
            })
            .collect()
    })
}

/// Deterministic LCG-based Fisher–Yates shuffle so a permutation is reproducible
/// from a seed.
pub fn shuffle(ops: &mut [Op], seed: u64) {
    let mut s = seed ^ 0x9E37_79B9_7F4A_7C15;
    for i in (1..ops.len()).rev() {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (s >> 33) as usize % (i + 1);
        ops.swap(i, j);
    }
}
