//! Targeted, example-based tests for each ADR-0001 conflict rule, the
//! balance/settle-up behaviour, and the #38 authorization model. These pin the
//! exact semantics that the property tests exercise more broadly.
//!
//! Under entitlement (#38) an op only folds if its author's identity is
//! entitled, so a "user" here is a real identity with a key, and a group is
//! created by a founding member. `op(..)` is authored by [`test_key`], the
//! founder of `g0` (see [`g0`]); `op_by(name, ..)` is authored by `key(name)`.

use std::collections::BTreeMap;

use splittr_crdt::*;

fn test_key() -> SigningKey {
    SigningKey::from_seed([7u8; 32])
}

/// The founder/local author used by [`op`]; a member of `g0`.
fn me() -> UserId {
    user_id_for(&test_key().public())
}

/// A deterministic identity key per name — so each test "user" is a real
/// identity, as entitlement requires.
fn key(name: &str) -> SigningKey {
    let mut seed = [0u8; 32];
    for (i, b) in name.bytes().enumerate() {
        seed[i % 32] ^= b;
    }
    seed[31] = seed[31].wrapping_add(name.len() as u8 + 1);
    SigningKey::from_seed(seed)
}

fn uid(name: &str) -> UserId {
    user_id_for(&key(name).public())
}

fn hlc(n: u32) -> Hlc {
    Hlc {
        wall_ms: n as u64,
        counter: n,
        site: SiteId(0),
    }
}

fn op(counter: u32, kind: OpKind) -> Op {
    Op::signed(hlc(counter), &test_key(), kind)
}

fn op_by(name: &str, counter: u32, kind: OpKind) -> Op {
    Op::signed(hlc(counter), &key(name), kind)
}

/// The two mutual `DeclareFriend` ops that confirm a friendship between `a` and
/// `b` (uses counters `c` and `c + 1`).
fn friend(a: &str, b: &str, c: u32) -> [Op; 2] {
    [
        op_by(a, c, OpKind::DeclareFriend { other: uid(b) }),
        op_by(b, c + 1, OpKind::DeclareFriend { other: uid(a) }),
    ]
}

/// CreateGroup `g0` authored by [`test_key`], making it the founding member so
/// that `op(..)` is entitled to act on `g0`. Prepend to a group-scoped test.
fn g0() -> Op {
    op(
        100,
        OpKind::CreateGroup {
            group: GroupId::from("g0"),
            name: "Trip".into(),
        },
    )
}

fn group0() -> GroupId {
    GroupId::from("g0")
}

fn split(total: i64, users: &[&str]) -> Vec<Split> {
    let users: Vec<UserId> = users.iter().map(|u| UserId::from(*u)).collect();
    split_equal(Cents(total), &users)
}

fn single_payer(payer: &str, total: i64, users: &[&str]) -> ExpenseFields {
    ExpenseFields::single_payer(UserId::from(payer), Cents(total), split(total, users))
}

fn single_payer_ids(payer: &UserId, total: i64, users: &[UserId]) -> ExpenseFields {
    ExpenseFields::single_payer(
        payer.clone(),
        Cents(total),
        split_equal(Cents(total), users),
    )
}

#[test]
fn delete_wins_and_is_terminal_regardless_of_order_or_hlc() {
    let create = OpKind::CreateExpense {
        expense: ExpenseId::from("e0"),
        group: Some(group0()),
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

    let forward = project(&[g0(), create_op.clone(), void_op.clone()]);
    let backward = project(&[void_op, create_op, g0()]);

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
            group: Some(group0()),
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

    let with_higher = project(&[g0(), create.clone(), higher]);
    assert_eq!(
        with_higher.expenses[&ExpenseId::from("e0")].fields.total,
        Cents(2000),
        "highest-HLC version wins"
    );

    let with_lower = project(&[g0(), create, lower]);
    assert_eq!(
        with_lower.expenses[&ExpenseId::from("e0")].fields.total,
        Cents(1000),
        "a lower-HLC edit loses to the create"
    );
}

#[test]
fn membership_is_lww_and_order_independent() {
    let add = op(
        1,
        OpKind::SetMembership {
            group: group0(),
            user: UserId::from("u0"),
            member: true,
        },
    );
    let remove = op(
        2,
        OpKind::SetMembership {
            group: group0(),
            user: UserId::from("u0"),
            member: false,
        },
    );

    let forward = project(&[g0(), add.clone(), remove.clone()]);
    let backward = project(&[remove, g0(), add]);

    assert!(
        !forward.groups[&group0()]
            .members
            .contains(&UserId::from("u0")),
        "highest-HLC (remove) wins"
    );
    assert_eq!(forward, backward, "order must not matter");
}

#[test]
fn alias_merge_preserves_balances_with_zero_change() {
    // A real friend expense between confirmed friends a & b: a pays 10.00 split
    // equally with b, authored by a (a participant + friend, #31/#38).
    let (a, b) = (uid("a"), uid("b"));
    let [fa, fb] = friend("a", "b", 10);
    let expense = OpKind::CreateExpense {
        expense: ExpenseId::from("e0"),
        group: None,
        fields: single_payer_ids(&a, 1000, &[a.clone(), b.clone()]),
        draft: false,
    };

    let before = net_balances(&project(&[
        fa.clone(),
        fb.clone(),
        op_by("a", 0, expense.clone()),
    ]));
    assert_eq!(before.get(&a), Some(&Cents(500)));
    assert_eq!(before.get(&b), Some(&Cents(-500)));

    // b claims they are a (signed by b — a party to the merge). The two net out.
    let merge = op_by(
        "b",
        1,
        OpKind::AddAlias {
            alias: b.clone(),
            canonical: a.clone(),
        },
    );
    let after = net_balances(&project(&[fa, fb, op_by("a", 0, expense), merge]));
    assert!(after.is_empty(), "merged users net to zero");
}

#[test]
fn settlement_void_removes_it() {
    let record = op(
        0,
        OpKind::RecordSettlement {
            settlement: SettlementId::from("s0"),
            group: Some(group0()),
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
    let p = project(&[g0(), record, void]);
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

    let net = net_balances(&project(&[
        g0(),
        op(
            0,
            OpKind::CreateExpense {
                expense: ExpenseId::from("e0"),
                group: Some(group0()),
                fields,
                draft: false,
            },
        ),
    ]));

    assert_eq!(net.get(&UserId::from("a")), Some(&Cents(200))); // paid 700, owes 500
    assert_eq!(net.get(&UserId::from("b")), Some(&Cents(-200))); // paid 300, owes 500
}

#[test]
fn settle_up_suggests_minimal_payments() {
    // a pays 9.00 split equally three ways → a is owed 6.00; b and c owe 3.00 each.
    let p = project(&[
        g0(),
        op(
            0,
            OpKind::CreateExpense {
                expense: ExpenseId::from("e0"),
                group: Some(group0()),
                fields: single_payer("a", 900, &["a", "b", "c"]),
                draft: false,
            },
        ),
    ]);

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
            group: Some(group0()),
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
    let p = project(&[unlock.clone(), lock.clone(), create.clone(), g0()]);
    assert!(!p.expenses[&ExpenseId::from("e0")].locked);
    assert_eq!(project(&[g0(), create.clone(), lock.clone(), unlock]), p);

    // With only the lock, it reads as locked.
    let locked = project(&[g0(), create, lock]);
    assert!(locked.expenses[&ExpenseId::from("e0")].locked);
}

#[test]
fn profile_name_is_lww() {
    // A placeholder person's name — anyone may set it (#2/#38).
    let who = UserId::from("user:guest");
    let early = op(
        1,
        OpKind::UpsertProfile {
            user: who.clone(),
            name: "Al".into(),
        },
    );
    let late = op(
        2,
        OpKind::UpsertProfile {
            user: who.clone(),
            name: "Alice".into(),
        },
    );
    let p = project(&[late.clone(), early.clone()]); // delivered out of order
    assert_eq!(p.users[&who].name, "Alice", "highest HLC wins");
    assert_eq!(project(&[early, late]), p, "order independent");
}

#[test]
fn agreement_key_is_published_and_lww() {
    // Your agreement key is published by *you* (author == subject, #6/#38).
    let a = uid("a");
    let profile = op_by(
        "a",
        0,
        OpKind::UpsertProfile {
            user: a.clone(),
            name: "Al".into(),
        },
    );
    let k1 = op_by(
        "a",
        1,
        OpKind::SetAgreementKey {
            user: a.clone(),
            key: [1u8; 32],
        },
    );
    let k2 = op_by(
        "a",
        2,
        OpKind::SetAgreementKey {
            user: a.clone(),
            key: [2u8; 32],
        },
    );
    let p = project(&[k2.clone(), k1.clone(), profile.clone()]);
    assert_eq!(
        p.users[&a].agreement_pub,
        Some([2u8; 32]),
        "highest-HLC key wins"
    );
    assert_eq!(project(&[profile, k1, k2]), p, "order independent");
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
    // The forged cert is ignored, so `device` acts as its own identity — and
    // creating a group is free, making it the founder. So the group exists.
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
    assert!(valid.verify().is_ok());

    // Mutating the content (without re-signing) breaks the id hash first.
    let mut tampered = valid.clone();
    tampered.kind = OpKind::CreateGroup {
        group: GroupId::from("g0"),
        name: "Hacked".into(),
    };
    assert_eq!(tampered.verify(), Err(VerifyError::HashMismatch));

    // A self-consistent op (id matches its content) whose signature is for *other*
    // content isolates the forgery case, distinct from corruption. Borrow a second
    // op's signature: `valid`'s id/content are untouched, so only the signature
    // check fails.
    let other = op(
        1,
        OpKind::CreateGroup {
            group: GroupId::from("g1"),
            name: "Other".into(),
        },
    );
    let mut forged = valid.clone();
    forged.sig = other.sig;
    assert_eq!(forged.verify(), Err(VerifyError::BadSignature));
}

#[test]
fn draft_expense_is_excluded_until_published() {
    let draft = op(
        0,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: Some(group0()),
            fields: single_payer("a", 1000, &["a", "b"]),
            draft: true,
        },
    );
    let p = project(&[g0(), draft.clone()]);
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
    let published = project(&[g0(), publish.clone(), draft.clone()]);
    assert!(published.expenses[&ExpenseId::from("e0")].published);
    assert_eq!(
        net_balances(&published).get(&UserId::from("a")),
        Some(&Cents(500)),
        "a published expense counts toward balances"
    );
    assert_eq!(
        project(&[draft, publish, g0()]),
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
    // A non-group friend expense between confirmed friends: a pays 10.00 split
    // with b, authored by a.
    let (a, b) = (uid("a"), uid("b"));
    let [fa, fb] = friend("a", "b", 10);
    let p = project(&[
        fa,
        fb,
        op_by(
            "a",
            0,
            OpKind::CreateExpense {
                expense: ExpenseId::from("e0"),
                group: None,
                fields: single_payer_ids(&a, 1000, &[a.clone(), b.clone()]),
                draft: false,
            },
        ),
    ]);

    // It moves balances globally.
    let net = net_balances(&p);
    assert_eq!(net.get(&a), Some(&Cents(500)));
    assert_eq!(net.get(&b), Some(&Cents(-500)));

    // But it is not scoped into any group.
    assert!(
        p.for_group(&group0()).expenses.is_empty(),
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

// --- Authorization / entitlement (#38) -------------------------------------

#[test]
fn non_member_expense_is_dropped() {
    // `mallory` is not a member of g0 (founded by test_key), so her expense
    // posted to g0 does not fold.
    let forged = op_by(
        "mallory",
        1,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: Some(group0()),
            fields: single_payer("a", 1000, &["a", "b"]),
            draft: false,
        },
    );
    let p = project(&[g0(), forged.clone()]);
    assert!(
        p.expenses.is_empty(),
        "an expense by a non-member must not count"
    );
    // Order-independent.
    assert_eq!(project(&[forged, g0()]), p);
}

#[test]
fn a_member_added_by_the_founder_can_post() {
    // test_key founds g0 and admits `bob`; bob's expense then counts.
    let admit = op(
        1,
        OpKind::SetMembership {
            group: group0(),
            user: uid("bob"),
            member: true,
        },
    );
    let by_bob = op_by(
        "bob",
        2,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: Some(group0()),
            fields: single_payer("a", 1000, &["a", "b"]),
            draft: false,
        },
    );
    let p = project(&[g0(), admit, by_bob]);
    assert!(
        p.expenses.contains_key(&ExpenseId::from("e0")),
        "an admitted member may post"
    );
    assert!(p.groups[&group0()].members.contains(&uid("bob")));
}

#[test]
fn a_non_member_cannot_admit_themselves() {
    // mallory tries to add herself; she isn't a member, so it's ignored.
    let self_admit = op_by(
        "mallory",
        1,
        OpKind::SetMembership {
            group: group0(),
            user: uid("mallory"),
            member: true,
        },
    );
    let p = project(&[g0(), self_admit]);
    assert!(
        !p.groups[&group0()].members.contains(&uid("mallory")),
        "a non-member cannot grant themselves membership"
    );
}

#[test]
fn post_removal_ops_are_dropped_order_independently() {
    // Founder admits bob (t1); bob posts e0 (t2, counts); founder removes bob
    // (t3); bob posts e1 (t4, dropped — he's no longer a member).
    let admit = op(
        1,
        OpKind::SetMembership {
            group: group0(),
            user: uid("bob"),
            member: true,
        },
    );
    let e0 = op_by(
        "bob",
        2,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: Some(group0()),
            fields: single_payer("a", 1000, &["a", "b"]),
            draft: false,
        },
    );
    let remove = op(
        3,
        OpKind::SetMembership {
            group: group0(),
            user: uid("bob"),
            member: false,
        },
    );
    let e1 = op_by(
        "bob",
        4,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e1"),
            group: Some(group0()),
            fields: single_payer("a", 500, &["a", "b"]),
            draft: false,
        },
    );

    let p = project(&[g0(), admit.clone(), e0.clone(), remove.clone(), e1.clone()]);
    assert!(
        p.expenses.contains_key(&ExpenseId::from("e0")),
        "pre-removal op counts"
    );
    assert!(
        !p.expenses.contains_key(&ExpenseId::from("e1")),
        "post-removal op is dropped"
    );
    // Membership authorization is order-independent.
    assert_eq!(project(&[e1, remove, e0, admit, g0()]), p);
}

#[test]
fn dependent_ops_on_an_unauthorized_expense_are_dropped() {
    // mallory's create is dropped, so a (real) edit referencing that id has
    // nothing to act on and is also dropped.
    let forged_create = op_by(
        "mallory",
        1,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: Some(group0()),
            fields: single_payer("a", 1000, &["a", "b"]),
            draft: false,
        },
    );
    let edit = op(
        2,
        OpKind::EditExpense {
            expense: ExpenseId::from("e0"),
            fields: single_payer("a", 2000, &["a", "b"]),
        },
    );
    let p = project(&[g0(), forged_create, edit]);
    assert!(
        p.expenses.is_empty(),
        "edit of a never-authorized expense does nothing"
    );
}

#[test]
fn cannot_forge_another_identitys_profile_or_key() {
    // bob tries to set alice's display name and agreement key — both dropped.
    let alice = uid("alice");
    let forged_name = op_by(
        "bob",
        1,
        OpKind::UpsertProfile {
            user: alice.clone(),
            name: "Pwned".into(),
        },
    );
    let forged_key = op_by(
        "bob",
        2,
        OpKind::SetAgreementKey {
            user: alice.clone(),
            key: [9u8; 32],
        },
    );
    let p = project(&[forged_name, forged_key]);
    assert!(
        !p.users.contains_key(&alice),
        "you cannot write another identity's profile/key"
    );
}

#[test]
fn a_friend_expense_needs_a_participant_author() {
    // mallory creates a non-group expense she is not part of → dropped.
    let (a, b) = (uid("a"), uid("b"));
    let forged = op_by(
        "mallory",
        1,
        OpKind::CreateExpense {
            expense: ExpenseId::from("e0"),
            group: None,
            fields: single_payer_ids(&a, 1000, &[a.clone(), b]),
            draft: false,
        },
    );
    assert!(project(&[forged]).expenses.is_empty());
}

#[test]
fn only_a_party_can_claim_an_alias() {
    // A placeholder guest exists; carol (unrelated) cannot claim it, but the
    // real owner can.
    let guest = UserId::from("user:guest");
    let name = op(
        1,
        OpKind::UpsertProfile {
            user: guest.clone(),
            name: "Guest".into(),
        },
    );
    let forged_claim = op_by(
        "carol",
        2,
        OpKind::AddAlias {
            alias: guest.clone(),
            canonical: uid("dave"), // carol claims the guest is *dave* — not her business
        },
    );
    let p = project(&[name.clone(), forged_claim]);
    assert!(
        p.aliases.is_empty(),
        "only a party to the merge may assert it"
    );

    // dave claims the guest himself → authorized; guest resolves to dave.
    let real_claim = op_by(
        "dave",
        3,
        OpKind::AddAlias {
            alias: guest.clone(),
            canonical: uid("dave"),
        },
    );
    let p = project(&[name, real_claim]);
    assert_eq!(p.aliases.get(&guest), Some(&uid("dave")));
}

#[test]
fn concurrent_claims_of_a_placeholder_converge() {
    // Two real users concurrently claim the same placeholder as themselves.
    // The canonical id is deterministic (lowest), independent of order.
    let guest = UserId::from("user:guest");
    let (a, b) = (uid("a"), uid("b"));
    let claim_a = op_by(
        "a",
        1,
        OpKind::AddAlias {
            alias: guest.clone(),
            canonical: a.clone(),
        },
    );
    let claim_b = op_by(
        "b",
        2,
        OpKind::AddAlias {
            alias: guest.clone(),
            canonical: b.clone(),
        },
    );
    let forward = project(&[claim_a.clone(), claim_b.clone()]);
    let backward = project(&[claim_b, claim_a]);
    assert_eq!(forward.aliases, backward.aliases, "claims converge");
    // The guest resolves to the deterministic canonical of {a, b, guest}.
    let canon = [a.clone(), b.clone()].into_iter().min().unwrap();
    assert_eq!(forward.aliases.get(&guest), Some(&canon));
    // `me` (founder) is unaffected.
    assert_eq!(me().0.find("id:"), Some(0));
}

// --- Friendships & friend-expense entitlement (#7/#38) ---------------------

fn friend_expense(payer: &str, others: &[&UserId]) -> OpKind {
    let mut parties = vec![uid(payer)];
    parties.extend(others.iter().map(|u| (*u).clone()));
    OpKind::CreateExpense {
        expense: ExpenseId::from("e0"),
        group: None,
        fields: single_payer_ids(&uid(payer), 1000, &parties),
        draft: false,
    }
}

#[test]
fn a_friend_edge_needs_both_declarations() {
    let (a, b) = (uid("a"), uid("b"));
    // Only a declares → not yet friends → a's friend expense with b is dropped.
    let one_sided = project(&[
        op_by("a", 1, OpKind::DeclareFriend { other: b.clone() }),
        op_by("a", 2, friend_expense("a", &[&b])),
    ]);
    assert!(
        one_sided.expenses.is_empty(),
        "a one-sided declaration is not a friendship"
    );
    assert!(one_sided.friends.is_empty());

    // Both declare → confirmed edge → the expense counts.
    let [fa, fb] = friend("a", "b", 1);
    let mutual = project(&[fa, fb, op_by("a", 3, friend_expense("a", &[&b]))]);
    assert!(mutual.expenses.contains_key(&ExpenseId::from("e0")));
    assert!(mutual.friends[&a].contains(&b) && mutual.friends[&b].contains(&a));
}

#[test]
fn friend_expense_between_non_friends_is_dropped() {
    // a and b never declared friendship; a's "b owes me" must not count.
    let (a, b) = (uid("a"), uid("b"));
    let p = project(&[op_by("a", 1, friend_expense("a", &[&b]))]);
    assert!(
        p.expenses.is_empty(),
        "strangers can't post friend expenses"
    );
    let _ = a;
}

#[test]
fn friend_expense_with_a_placeholder_needs_no_reciprocation() {
    // Tracking a guest (placeholder, no key) is always allowed for a participant.
    let guest = UserId::from("user:guest");
    let p = project(&[op_by("a", 1, friend_expense("a", &[&guest]))]);
    assert!(
        p.expenses.contains_key(&ExpenseId::from("e0")),
        "a guest needs no friend edge"
    );
}

#[test]
fn friendship_is_order_independent_and_alias_aware() {
    let (a, b) = (uid("a"), uid("b"));
    let [fa, fb] = friend("a", "b", 1);
    let expense = op_by("a", 3, friend_expense("a", &[&b]));
    let forward = project(&[fa.clone(), fb.clone(), expense.clone()]);
    let backward = project(&[expense, fb, fa]);
    assert_eq!(forward, backward, "friendship folds order-independently");

    // A friend expense with a *claimed* guest: the guest is aliased to a real
    // friend, so the edge resolves through the alias.
    let guest = UserId::from("user:guest");
    let claim = op_by(
        "b",
        5,
        OpKind::AddAlias {
            alias: guest.clone(),
            canonical: b.clone(),
        },
    );
    let [ga, gb] = friend("a", "b", 6);
    let with_guest = op_by("a", 8, friend_expense("a", &[&guest]));
    let p = project(&[claim, ga, gb, with_guest]);
    assert!(
        p.expenses.contains_key(&ExpenseId::from("e0")),
        "an expense with a guest who is a claimed friend counts"
    );
    assert_eq!(net_balances(&p).get(&a), Some(&Cents(500)));
}
