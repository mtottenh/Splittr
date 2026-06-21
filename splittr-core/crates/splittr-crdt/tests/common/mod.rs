//! Shared proptest generators for the integration tests (kept in one place so
//! the convergence and materializer suites don't duplicate them).
//!
//! Under entitlement (#38) an op only folds if its author is entitled, so the
//! generator models a real authorized world: a few **identities** (each a
//! signing key, its user id derived from its public key), a deterministic
//! **setup** that founds a group and admits them, then random member-authored
//! ops over a small fixed universe — plus some ops by an *outsider* identity and
//! some forged device certs, so both the authorized and the dropped paths are
//! exercised. Convergence (same op-set ⇒ same projection) must hold regardless.

#![allow(dead_code)] // not every test binary uses every helper

use std::collections::BTreeMap;

use proptest::prelude::*;
use splittr_crdt::*;

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

/// The user id for an identity site (its key's public id). Sites 0..=2 are set
/// up as group members; site 3 is an unauthorized outsider.
pub fn member(site: u64) -> UserId {
    user_id_for(&site_pub(site))
}

fn placeholder() -> UserId {
    UserId::new("user:guest".to_string())
}

/// The people referenced in expense/membership ops: three real members plus a
/// placeholder guest (#2).
fn person(i: u8) -> UserId {
    match i % 4 {
        0 => member(0),
        1 => member(1),
        2 => member(2),
        _ => placeholder(),
    }
}

fn participants(mask: &[bool]) -> Vec<UserId> {
    let mut users: Vec<UserId> = mask
        .iter()
        .enumerate()
        .filter(|(_, b)| **b)
        .map(|(i, _)| person(i as u8))
        .collect();
    if users.is_empty() {
        users.push(person(0));
    }
    users
}

fn valid_splits(total: i64, mask: &[bool]) -> Vec<Split> {
    split_equal(Cents(total), &participants(mask))
}

/// A balanced expense version: payments and splits both sum to `total`.
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
        (0u8..2, 0u8..3).prop_map(|(g, n)| OpKind::SetGroupName {
            group: group(g),
            name: format!("name{n}")
        }),
        (0u8..2, 0u8..4, any::<bool>()).prop_map(|(g, u, m)| OpKind::SetMembership {
            group: group(g),
            user: person(u),
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
                from: person(from),
                to: person(to),
                amount: Cents(amount),
            }
        }),
        (0u8..4).prop_map(|s| OpKind::VoidSettlement {
            settlement: settlement(s)
        }),
        // Claim the placeholder guest as a real member (canonical = a member).
        (0u8..3).prop_map(|m| OpKind::AddAlias {
            alias: placeholder(),
            canonical: member(m as u64),
        }),
        (0u8..4, 0u8..3).prop_map(|(u, n)| OpKind::UpsertProfile {
            user: person(u),
            name: format!("name{n}")
        }),
        (0u8..4, any::<u8>()).prop_map(|(u, k)| OpKind::SetAgreementKey {
            user: person(u),
            key: [k; 32],
        }),
        (0u8..6, any::<bool>()).prop_map(|(e, locked)| OpKind::SetExpenseLock {
            expense: expense(e),
            locked
        }),
        // Device authorize/revoke (#16): identity = site 0, so only the ops also
        // authored by site 0 are honoured and the rest exercise the ignore path.
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

/// The deterministic authorized setup prepended to every log: site 0 founds g0
/// and admits sites 1, 2 and the guest; site 1 founds g1 and admits sites 0, 2.
/// Pairs are `(kind, author_site)`.
fn setup() -> Vec<(OpKind, u64)> {
    vec![
        (
            OpKind::CreateGroup {
                group: group(0),
                name: "g0".into(),
            },
            0,
        ),
        (membership(0, member(1)), 0),
        (membership(0, member(2)), 0),
        (membership(0, placeholder()), 0),
        (
            OpKind::CreateGroup {
                group: group(1),
                name: "g1".into(),
            },
            1,
        ),
        (membership(1, member(0)), 1),
        (membership(1, member(2)), 1),
    ]
}

fn membership(g: u8, user: UserId) -> OpKind {
    OpKind::SetMembership {
        group: group(g),
        user,
        member: true,
    }
}

/// A log of ops: the authorized setup, then random ops authored by varying
/// identities (site 3 is an outsider whose ops drop). Each op gets a unique HLC
/// counter (so ids are unique) with a randomised wall time + HLC site, so HLC
/// order is deliberately *not* aligned with position.
pub fn op_log() -> impl Strategy<Value = Vec<Op>> {
    // (kind, author_site 0..4, wall, hlc_site)
    prop::collection::vec((op_kind(), 0u64..4, 0u64..5, 0u64..3), 0..40).prop_map(|specs| {
        let mut counter = 0u32;
        let mut sign = |kind: OpKind, author: u64, wall: u64, hlc_site: u64| {
            let hlc = Hlc {
                wall_ms: wall,
                counter,
                site: SiteId(hlc_site),
            };
            counter += 1;
            Op::signed(hlc, &site_key(author), kind)
        };

        let mut ops: Vec<Op> = setup()
            .into_iter()
            .map(|(kind, author)| sign(kind, author, 0, author))
            .collect();
        for (kind, author, wall, hlc_site) in specs {
            ops.push(sign(kind, author, wall, hlc_site));
        }
        ops
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
