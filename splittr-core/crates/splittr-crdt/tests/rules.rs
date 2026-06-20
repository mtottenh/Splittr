//! Targeted, example-based tests for each ADR-0001 conflict rule and the
//! balance/settle-up behaviour. These pin the exact semantics that the property
//! tests exercise more broadly.

use std::collections::BTreeMap;

use splittr_crdt::*;

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

fn split(total: i64, users: &[&str]) -> Vec<Split> {
    let users: Vec<UserId> = users.iter().map(|u| UserId::from(*u)).collect();
    split_equal(Cents(total), &users)
}

fn single_payer(payer: &str, total: i64, users: &[&str]) -> ExpenseFields {
    ExpenseFields::single_payer(UserId::from(payer), Cents(total), split(total, users))
}

#[test]
fn delete_wins_and_is_terminal_regardless_of_order_or_hlc() {
    let create = OpKind::CreateExpense {
        expense: ExpenseId::from("e0"),
        group: Some(GroupId::from("g0")),
        fields: single_payer("a", 1000, &["a", "b"]),
        draft: false,
    };
    // Void has a *lower* HLC than create, yet delete still wins (terminal).
    let create_op = op(5, create);
    let void_op = op(
        1,
        OpKind::VoidExpense {
            expense: ExpenseId::from("e0"),
        },
    );

    let forward = project(&[create_op.clone(), void_op.clone()]);
    let backward = project(&[void_op, create_op]);

    assert!(
        forward.expenses.is_empty(),
        "voided expense must not appear"
    );
    assert_eq!(forward, backward, "order must not matter");
}

#[test]
fn expense_edit_is_whole_version_lww() {
    let create = op(
        1,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: Some(GroupId::from("g0")),
            fields: single_payer("a", 1000, &["a", "b"]),
            draft: false,
        },
    );
    let higher = op(
        5,
        OpKind::EditExpense {
            expense: ExpenseId::from("e0"),
            fields: single_payer("a", 2000, &["a", "b"]),
        },
    );
    let lower = op(
        0,
        OpKind::EditExpense {
            expense: ExpenseId::from("e0"),
            fields: single_payer("a", 9999, &["a", "b"]),
        },
    );

    let with_higher = project(&[create.clone(), higher]);
    assert_eq!(
        with_higher.expenses[&ExpenseId::from("e0")].fields.total,
        Cents(2000),
        "highest-HLC version wins"
    );

    let with_lower = project(&[create, lower]);
    assert_eq!(
        with_lower.expenses[&ExpenseId::from("e0")].fields.total,
        Cents(1000),
        "a lower-HLC edit loses to the create"
    );
}

#[test]
fn membership_is_lww_and_order_independent() {
    let create_group = op(
        0,
        OpKind::CreateGroup {
            group: GroupId::from("g0"),
            name: "Trip".into(),
        },
    );
    let add = op(
        1,
        OpKind::SetMembership {
            group: GroupId::from("g0"),
            user: UserId::from("u0"),
            member: true,
        },
    );
    let remove = op(
        2,
        OpKind::SetMembership {
            group: GroupId::from("g0"),
            user: UserId::from("u0"),
            member: false,
        },
    );

    let forward = project(&[create_group.clone(), add.clone(), remove.clone()]);
    let backward = project(&[remove, create_group, add]);

    assert!(
        !forward.groups[&GroupId::from("g0")]
            .members
            .contains(&UserId::from("u0")),
        "highest-HLC (remove) wins"
    );
    assert_eq!(forward, backward, "order must not matter");
}

#[test]
fn alias_merge_preserves_balances_with_zero_change() {
    // a pays 10.00 split equally with b → a is owed 5.00, b owes 5.00.
    let expense = OpKind::CreateExpense {
        expense: ExpenseId::from("e0"),
        group: Some(GroupId::from("g0")),
        fields: single_payer("a", 1000, &["a", "b"]),
        draft: false,
    };

    let before = net_balances(&project(&[op(0, expense.clone())]));
    assert_eq!(before.get(&UserId::from("a")), Some(&Cents(500)));
    assert_eq!(before.get(&UserId::from("b")), Some(&Cents(-500)));

    // Merge b into a (canonical = min id = "a"); the two net out to settled.
    let after = net_balances(&project(&[
        op(0, expense),
        op(
            1,
            OpKind::AddAlias {
                alias: UserId::from("b"),
                canonical: UserId::from("a"),
            },
        ),
    ]));
    assert!(after.is_empty(), "merged users net to zero");
}

#[test]
fn settlement_void_removes_it() {
    let record = op(
        0,
        OpKind::RecordSettlement {
            settlement: SettlementId::from("s0"),
            group: Some(GroupId::from("g0")),
            from: UserId::from("b"),
            to: UserId::from("a"),
            amount: Cents(500),
        },
    );
    let void = op(
        1,
        OpKind::VoidSettlement {
            settlement: SettlementId::from("s0"),
        },
    );
    let p = project(&[record, void]);
    assert!(p.settlements.is_empty());
    assert!(net_balances(&p).is_empty());
}

#[test]
fn multiple_payers_are_credited_correctly() {
    // a and b both pay (700 / 300) for a 10.00 expense split equally between them.
    let mut paid_by = BTreeMap::new();
    paid_by.insert(UserId::from("a"), Cents(700));
    paid_by.insert(UserId::from("b"), Cents(300));
    let fields = ExpenseFields::new(paid_by, Cents(1000), split(1000, &["a", "b"]));

    let net = net_balances(&project(&[op(
        0,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: Some(GroupId::from("g0")),
            fields,
            draft: false,
        },
    )]));

    assert_eq!(net.get(&UserId::from("a")), Some(&Cents(200))); // paid 700, owes 500
    assert_eq!(net.get(&UserId::from("b")), Some(&Cents(-200))); // paid 300, owes 500
}

#[test]
fn settle_up_suggests_minimal_payments() {
    // a pays 9.00 split equally three ways → a is owed 6.00; b and c owe 3.00 each.
    let p = project(&[op(
        0,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: Some(GroupId::from("g0")),
            fields: single_payer("a", 900, &["a", "b", "c"]),
            draft: false,
        },
    )]);

    let transfers = settle_up(&p);
    assert_eq!(transfers.len(), 2, "both debtors pay the single creditor");
    assert!(transfers.iter().all(|t| t.to == UserId::from("a")));
    assert_eq!(transfers.iter().map(|t| t.amount.0).sum::<i64>(), 600);
}

#[test]
fn expense_lock_is_lww_and_does_not_affect_presence() {
    let create = op(
        0,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: Some(GroupId::from("g0")),
            fields: single_payer("a", 1000, &["a", "b"]),
            draft: false,
        },
    );
    let lock = op(
        1,
        OpKind::SetExpenseLock {
            expense: ExpenseId::from("e0"),
            locked: true,
        },
    );
    let unlock = op(
        2,
        OpKind::SetExpenseLock {
            expense: ExpenseId::from("e0"),
            locked: false,
        },
    );

    // Highest HLC (unlock) wins, regardless of delivery order; the expense is
    // still present either way.
    let p = project(&[unlock.clone(), lock.clone(), create.clone()]);
    assert!(!p.expenses[&ExpenseId::from("e0")].locked);
    assert_eq!(project(&[create.clone(), lock.clone(), unlock]), p);

    // With only the lock, it reads as locked.
    let locked = project(&[create, lock]);
    assert!(locked.expenses[&ExpenseId::from("e0")].locked);
}

#[test]
fn profile_name_is_lww() {
    let early = op(
        1,
        OpKind::UpsertProfile {
            user: UserId::from("a"),
            name: "Al".into(),
        },
    );
    let late = op(
        2,
        OpKind::UpsertProfile {
            user: UserId::from("a"),
            name: "Alice".into(),
        },
    );
    let p = project(&[late.clone(), early.clone()]); // delivered out of order
    assert_eq!(
        p.users[&UserId::from("a")].name,
        "Alice",
        "highest HLC wins"
    );
    assert_eq!(project(&[early, late]), p, "order independent");
}

#[test]
fn agreement_key_is_published_and_lww() {
    let profile = op(
        0,
        OpKind::UpsertProfile {
            user: UserId::from("a"),
            name: "Al".into(),
        },
    );
    let k1 = op(
        1,
        OpKind::SetAgreementKey {
            user: UserId::from("a"),
            key: [1u8; 32],
        },
    );
    let k2 = op(
        2,
        OpKind::SetAgreementKey {
            user: UserId::from("a"),
            key: [2u8; 32],
        },
    );
    let p = project(&[k2.clone(), k1.clone(), profile.clone()]);
    assert_eq!(
        p.users[&UserId::from("a")].agreement_pub,
        Some([2u8; 32]),
        "highest-HLC key wins"
    );
    assert_eq!(project(&[profile, k1, k2]), p, "order independent");
}

fn hlc(n: u32) -> Hlc {
    Hlc {
        wall_ms: n as u64,
        counter: n,
        site: SiteId(0),
    }
}

#[test]
fn device_revocation_excludes_only_post_revocation_ops() {
    let root = SigningKey::from_seed([10u8; 32]);
    let device = SigningKey::from_seed([11u8; 32]);
    let (id_pub, dev_pub) = (root.public(), device.public());

    let authz = Op::signed(
        hlc(1),
        &root,
        OpKind::AuthorizeDevice {
            identity: id_pub,
            device: dev_pub,
            site: 7,
        },
    );
    let before = Op::signed(
        hlc(3),
        &device,
        OpKind::CreateGroup {
            group: GroupId::from("g_before"),
            name: "Before".into(),
        },
    );
    let revoke = Op::signed(
        hlc(5),
        &root,
        OpKind::RevokeDevice {
            identity: id_pub,
            device: dev_pub,
        },
    );
    let after = Op::signed(
        hlc(7),
        &device,
        OpKind::CreateGroup {
            group: GroupId::from("g_after"),
            name: "After".into(),
        },
    );

    let p = project(&[authz.clone(), before.clone(), revoke.clone(), after.clone()]);
    assert!(
        p.groups.contains_key(&GroupId::from("g_before")),
        "a pre-revocation op by the device still counts"
    );
    assert!(
        !p.groups.contains_key(&GroupId::from("g_after")),
        "a post-revocation op by the device is excluded"
    );
    assert!(p.devices[&dev_pub].revoked);
    assert_eq!(p.devices[&dev_pub].identity, id_pub);
    assert_eq!(p.devices[&dev_pub].site, 7);

    // Order-independent.
    assert_eq!(project(&[after, revoke, before, authz]), p);
}

#[test]
fn foreign_device_authorization_is_ignored() {
    let alice = SigningKey::from_seed([20u8; 32]);
    let mallory = SigningKey::from_seed([21u8; 32]);
    let device = SigningKey::from_seed([22u8; 32]);

    // Mallory tries to authorize `device` for Alice's identity (not self-signed).
    let forged = Op::signed(
        hlc(1),
        &mallory,
        OpKind::AuthorizeDevice {
            identity: alice.public(),
            device: device.public(),
            site: 1,
        },
    );
    let by_device = Op::signed(
        hlc(2),
        &device,
        OpKind::CreateGroup {
            group: GroupId::from("g"),
            name: "G".into(),
        },
    );

    let p = project(&[forged, by_device]);
    assert!(
        !p.devices.contains_key(&device.public()),
        "an authorize not signed by the identity is ignored"
    );
    // Self-sovereign default (ADR-0005): with no valid cert the device acts as
    // its own identity, so its op counts. Entitlement gating (member-authorized
    // identity) is deferred to the pre-sync hardening tracked in #38.
    assert!(p.groups.contains_key(&GroupId::from("g")));
}

#[test]
fn signed_op_verifies_and_tampering_is_detected() {
    let valid = op(
        0,
        OpKind::CreateGroup {
            group: GroupId::from("g0"),
            name: "Trip".into(),
        },
    );
    assert!(valid.verify());

    // Mutating the content (without re-signing) breaks both the id hash and the
    // signature.
    let mut tampered = valid.clone();
    tampered.kind = OpKind::CreateGroup {
        group: GroupId::from("g0"),
        name: "Hacked".into(),
    };
    assert!(!tampered.verify());
}

#[test]
fn draft_expense_is_excluded_until_published() {
    let draft = op(
        0,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: Some(GroupId::from("g0")),
            fields: single_payer("a", 1000, &["a", "b"]),
            draft: true,
        },
    );
    let p = project(std::slice::from_ref(&draft));
    assert!(
        !p.expenses[&ExpenseId::from("e0")].published,
        "draft is not published"
    );
    assert!(
        net_balances(&p).is_empty(),
        "a draft must not affect balances"
    );

    // Publishing (any order) flips it on and it now counts.
    let publish = op(
        1,
        OpKind::PublishExpense {
            expense: ExpenseId::from("e0"),
        },
    );
    let published = project(&[publish.clone(), draft.clone()]);
    assert!(published.expenses[&ExpenseId::from("e0")].published);
    assert_eq!(
        net_balances(&published).get(&UserId::from("a")),
        Some(&Cents(500)),
        "a published expense counts toward balances"
    );
    assert_eq!(
        project(&[draft, publish]),
        published,
        "publish is order-independent"
    );
}

#[test]
fn group_currency_defaults_to_usd_and_is_lww() {
    let create = op(
        0,
        OpKind::CreateGroup {
            group: GroupId::from("g0"),
            name: "Trip".into(),
        },
    );
    // Default currency with no SetGroupCurrency op.
    assert_eq!(
        project(std::slice::from_ref(&create)).groups[&GroupId::from("g0")].currency,
        "USD"
    );

    let eur = op(
        1,
        OpKind::SetGroupCurrency {
            group: GroupId::from("g0"),
            currency: "EUR".into(),
        },
    );
    let gbp = op(
        2,
        OpKind::SetGroupCurrency {
            group: GroupId::from("g0"),
            currency: "GBP".into(),
        },
    );
    let p = project(&[gbp.clone(), eur.clone(), create.clone()]);
    assert_eq!(
        p.groups[&GroupId::from("g0")].currency,
        "GBP",
        "highest-HLC currency wins"
    );
    assert_eq!(project(&[create, eur, gbp]), p, "order independent");
}

#[test]
fn non_group_expense_counts_in_balances_but_not_in_any_group() {
    // a non-group expense: a pays 10.00 split with b, no group attached.
    let p = project(&[op(
        0,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: None,
            fields: single_payer("a", 1000, &["a", "b"]),
            draft: false,
        },
    )]);

    // It moves balances globally.
    let net = net_balances(&p);
    assert_eq!(net.get(&UserId::from("a")), Some(&Cents(500)));
    assert_eq!(net.get(&UserId::from("b")), Some(&Cents(-500)));

    // But it is not scoped into any group.
    assert!(
        p.for_group(&GroupId::from("g0")).expenses.is_empty(),
        "a non-group expense must not appear under any group"
    );
    assert_eq!(p.expenses[&ExpenseId::from("e0")].group, None);
}

#[test]
fn closed_period_watermark_is_lww() {
    let create_group = op(
        0,
        OpKind::CreateGroup {
            group: GroupId::from("g0"),
            name: "Trip".into(),
        },
    );
    let close_early = op(
        1,
        OpKind::SetClosedPeriod {
            group: GroupId::from("g0"),
            until_ms: 100,
        },
    );
    let close_late = op(
        2,
        OpKind::SetClosedPeriod {
            group: GroupId::from("g0"),
            until_ms: 500,
        },
    );

    let p = project(&[
        close_late.clone(),
        close_early.clone(),
        create_group.clone(),
    ]);
    assert_eq!(
        p.groups[&GroupId::from("g0")].closed_until_ms,
        500,
        "highest-HLC watermark wins"
    );
    assert_eq!(
        project(&[create_group, close_early, close_late]),
        p,
        "order independent"
    );
}
