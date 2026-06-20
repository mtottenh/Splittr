//! Integration tests for the storage layer: round-trip + dedup, the repository
//! projection matching the batch fold, and durability across reopen.

use proptest::prelude::*;
use splittr_crdt::*;
use splittr_store::*;

fn test_key() -> SigningKey {
    SigningKey::from_seed([7u8; 32])
}

fn op(counter: u32, kind: OpKind) -> Op {
    Op::signed(
        Hlc {
            wall_ms: counter as u64,
            counter,
            site: SiteId(0),
        },
        &test_key(),
        kind,
    )
}

fn sample_ops() -> Vec<Op> {
    let g = GroupId::from("g0");
    let (a, b) = (UserId::from("a"), UserId::from("b"));
    vec![
        op(
            0,
            OpKind::CreateGroup {
                group: g.clone(),
                name: "Trip".into(),
            },
        ),
        op(
            1,
            OpKind::SetMembership {
                group: g.clone(),
                user: a.clone(),
                member: true,
            },
        ),
        op(
            2,
            OpKind::SetMembership {
                group: g.clone(),
                user: b.clone(),
                member: true,
            },
        ),
        op(
            3,
            OpKind::CreateExpense {
                expense: ExpenseId::from("e0"),
                group: Some(g.clone()),
                fields: ExpenseFields::single_payer(
                    a.clone(),
                    Cents(1000),
                    split_equal(Cents(1000), &[a.clone(), b.clone()]),
                ),
                draft: false,
            },
        ),
        op(
            4,
            OpKind::RecordSettlement {
                settlement: SettlementId::from("s0"),
                group: Some(g),
                from: b,
                to: a,
                amount: Cents(200),
            },
        ),
    ]
}

#[test]
fn memory_roundtrip_and_dedup() {
    let mut store = MemoryOpStore::new();
    let ops = sample_ops();

    for o in &ops {
        assert!(store.append(o).unwrap(), "first append stores the op");
    }
    assert!(!store.append(&ops[0]).unwrap(), "re-append is deduped");
    assert_eq!(store.len().unwrap(), ops.len());
    assert!(store.contains(&ops[0].id).unwrap());
    assert_eq!(store.ops().unwrap().len(), ops.len());
}

#[test]
fn repository_projection_matches_fold() {
    let ops = sample_ops();
    let mut repo = Repository::open(MemoryOpStore::new()).unwrap();
    for o in &ops {
        assert_eq!(repo.append(o).unwrap(), Applied::Stored);
    }
    assert_eq!(repo.projection(), project(&ops));
}

#[test]
fn cached_projection_is_invalidated_by_append() {
    // A read between appends must not serve a stale (cached) projection (#39).
    let mut repo = Repository::open(MemoryOpStore::new()).unwrap();
    repo.append(&op(
        0,
        OpKind::CreateGroup {
            group: GroupId::from("g0"),
            name: "Trip".into(),
        },
    ))
    .unwrap();
    assert!(repo.projection().groups.contains_key(&GroupId::from("g0")));

    // After a prior read populated the cache, a new op must still show up.
    repo.append(&op(
        1,
        OpKind::CreateGroup {
            group: GroupId::from("g1"),
            name: "Cabin".into(),
        },
    ))
    .unwrap();
    assert!(repo.projection().groups.contains_key(&GroupId::from("g1")));

    // A duplicate (no state change) must not invalidate incorrectly either.
    let dup = op(
        2,
        OpKind::CreateGroup {
            group: GroupId::from("g2"),
            name: "Lake".into(),
        },
    );
    assert_eq!(repo.append(&dup).unwrap(), Applied::Stored);
    assert_eq!(repo.append(&dup).unwrap(), Applied::Duplicate);
    assert_eq!(repo.projection().groups.len(), 3);
}

#[test]
fn repository_rejects_tampered_op_and_dedups() {
    let mut repo = Repository::open(MemoryOpStore::new()).unwrap();
    let valid = op(
        0,
        OpKind::CreateGroup {
            group: GroupId::from("g0"),
            name: "Trip".into(),
        },
    );
    assert_eq!(repo.append(&valid).unwrap(), Applied::Stored);
    assert_eq!(repo.append(&valid).unwrap(), Applied::Duplicate);

    // Same id/signature, mutated content → must be rejected, leaving state intact.
    let mut tampered = valid.clone();
    tampered.kind = OpKind::CreateGroup {
        group: GroupId::from("g0"),
        name: "Hacked".into(),
    };
    assert_eq!(repo.append(&tampered).unwrap(), Applied::Rejected);
    assert_eq!(repo.projection().groups[&GroupId::from("g0")].name, "Trip");
}

#[test]
fn redb_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ops.redb");
    let ops = sample_ops();

    {
        let mut repo = Repository::open(RedbOpStore::open(&path).unwrap()).unwrap();
        for o in &ops {
            repo.append(o).unwrap();
        }
        assert_eq!(repo.projection(), project(&ops));
    }

    // Reopen from disk in a fresh process-like instance.
    let reopened = Repository::open(RedbOpStore::open(&path).unwrap()).unwrap();
    assert_eq!(reopened.projection(), project(&ops));
}

#[test]
fn encrypted_redb_persists_and_hides_plaintext() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ops.redb");
    let key = [42u8; 32];
    let ops = sample_ops();

    {
        let mut repo = Repository::open(RedbOpStore::open_encrypted(&path, key).unwrap()).unwrap();
        for o in &ops {
            repo.append(o).unwrap();
        }
        assert_eq!(repo.projection(), project(&ops));
    }

    // Reopen with the right key → state is intact.
    let reopened = Repository::open(RedbOpStore::open_encrypted(&path, key).unwrap()).unwrap();
    assert_eq!(reopened.projection(), project(&ops));

    // The sensitive plaintext ("Trip") must not appear on disk.
    let raw = std::fs::read(&path).unwrap();
    assert!(
        raw.windows(4).all(|w| w != b"Trip"),
        "expense/group text leaked into the at-rest database"
    );
}

#[test]
fn encrypted_redb_rejects_the_wrong_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ops.redb");
    let ops = sample_ops();

    {
        let mut store = RedbOpStore::open_encrypted(&path, [1u8; 32]).unwrap();
        for o in &ops {
            store.append(o).unwrap();
        }
    }

    let wrong = RedbOpStore::open_encrypted(&path, [2u8; 32]).unwrap();
    assert!(
        wrong.ops().is_err(),
        "decryption must fail under a wrong key"
    );
}

// A compact local generator (the rich one lives in splittr-crdt's tests; this
// only needs enough variety to exercise persistence + replay).
fn arb_ops() -> impl Strategy<Value = Vec<Op>> {
    let kind = prop_oneof![
        (0u8..3).prop_map(|g| OpKind::CreateGroup {
            group: GroupId::new(format!("g{g}")),
            name: format!("group{g}")
        }),
        (0u8..3, 0u8..4, any::<bool>()).prop_map(|(g, u, m)| OpKind::SetMembership {
            group: GroupId::new(format!("g{g}")),
            user: UserId::new(format!("u{u}")),
            member: m
        }),
        (0u8..5, 0u8..3, 0u8..4, 1i64..10_000).prop_map(|(e, g, p, t)| {
            let payer = UserId::new(format!("u{p}"));
            OpKind::CreateExpense {
                expense: ExpenseId::new(format!("e{e}")),
                group: Some(GroupId::new(format!("g{g}"))),
                fields: ExpenseFields::single_payer(
                    payer.clone(),
                    Cents(t),
                    split_equal(Cents(t), &[payer]),
                ),
                draft: false,
            }
        }),
        (0u8..5).prop_map(|e| OpKind::VoidExpense {
            expense: ExpenseId::new(format!("e{e}"))
        }),
    ];
    prop::collection::vec((kind, 0u64..4), 0..25).prop_map(|specs| {
        specs
            .into_iter()
            .enumerate()
            .map(|(i, (k, wall))| op_with(wall, i as u32, k))
            .collect()
    })
}

fn op_with(wall: u64, counter: u32, kind: OpKind) -> Op {
    Op::signed(
        Hlc {
            wall_ms: wall,
            counter,
            site: SiteId(0),
        },
        &test_key(),
        kind,
    )
}

proptest! {
    #[test]
    fn memory_repo_matches_fold(ops in arb_ops()) {
        let mut repo = Repository::open(MemoryOpStore::new()).unwrap();
        for o in &ops {
            repo.append(o).unwrap();
        }
        prop_assert_eq!(repo.projection(), project(&ops));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Random ops survive the postcard → redb → reload → fold cycle intact.
    #[test]
    fn redb_reload_matches_fold(ops in arb_ops()) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ops.redb");
        {
            let mut repo = Repository::open(RedbOpStore::open(&path).unwrap()).unwrap();
            for o in &ops {
                repo.append(o).unwrap();
            }
        }
        let reopened = Repository::open(RedbOpStore::open(&path).unwrap()).unwrap();
        prop_assert_eq!(reopened.projection(), project(&ops));
    }
}
