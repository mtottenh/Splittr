//! Behavioural tests for the application facade over an in-memory (and redb)
//! store: the core flows, input validation, and identity-based authorization.

use std::collections::BTreeMap;

use splittr_app::*;

fn new_app() -> App<MemoryOpStore> {
    App::new(
        Identity::from_seed([1u8; 32]),
        MemoryOpStore::new(),
        SiteId(1),
    )
    .unwrap()
}

fn paid(user: &UserId, cents: i64) -> BTreeMap<UserId, Cents> {
    let mut m = BTreeMap::new();
    m.insert(user.clone(), Cents(cents));
    m
}

fn equal(users: &[&UserId]) -> SplitPlan {
    SplitPlan::Equal {
        participants: users.iter().map(|u| (*u).clone()).collect(),
    }
}

/// Editable fields with the common defaults (category, t=0, no notes/original).
fn fields(
    description: &str,
    paid_by: BTreeMap<UserId, Cents>,
    total: Cents,
    split: SplitPlan,
) -> ExpenseFieldsInput {
    ExpenseFieldsInput {
        description: description.into(),
        paid_by,
        total,
        split,
        category: "general".into(),
        notes: None,
        date_ms: 0,
        original: None,
    }
}

/// An active group expense from the given fields.
fn group_expense(group: &GroupId, fields: ExpenseFieldsInput) -> ExpenseDraft {
    ExpenseDraft {
        fields,
        group: Some(group.clone()),
        draft: false,
    }
}

#[test]
fn create_group_add_expense_then_settle_up() {
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Trip", std::slice::from_ref(&bob))
        .unwrap();

    // I pay 30.00, split equally between me and Bob.
    app.add_expense(group_expense(
        &group,
        fields("Hotel", paid(&me, 3000), Cents(3000), equal(&[&me, &bob])),
    ))
    .unwrap();

    let detail = app.group_detail(&group).unwrap();
    let bal = |u: &UserId| detail.members.iter().find(|m| &m.user == u).unwrap().net;
    assert_eq!(bal(&me), Cents(1500));
    assert_eq!(bal(&bob), Cents(-1500));
    assert_eq!(detail.expenses.len(), 1);
    assert_eq!(detail.expenses[0].description, "Hotel");

    // Settle-up suggests Bob pays me 15.00.
    assert_eq!(detail.settle_up.len(), 1);
    assert_eq!(detail.settle_up[0].from, bob);
    assert_eq!(detail.settle_up[0].to, me);
    assert_eq!(detail.settle_up[0].amount, Cents(1500));

    app.record_settlement(&group, &bob, &me, Cents(1500))
        .unwrap();
    let settled = app.group_detail(&group).unwrap();
    assert!(settled.settle_up.is_empty());
    assert!(settled.members.iter().all(|m| m.net == Cents(0)));
}

#[test]
fn activity_renders_settlement_in_the_group_currency() {
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Tokyo", std::slice::from_ref(&bob))
        .unwrap();
    app.set_group_currency(&group, "JPY").unwrap(); // 0 minor units

    // ¥1000 paid: must render as "1000", not "10.00".
    app.record_settlement(&group, &bob, &me, Cents(1000))
        .unwrap();
    let feed = app.activity();
    let settlement = feed
        .iter()
        .find(|e| e.kind == "settlement")
        .expect("a settlement entry");
    assert!(
        settlement.summary.ends_with("1000"),
        "expected yen with no decimals, got {:?}",
        settlement.summary
    );
}

#[test]
fn group_summary_shows_my_balance_and_names() {
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Trip", std::slice::from_ref(&bob))
        .unwrap();
    app.add_expense(group_expense(
        &group,
        fields("Lunch", paid(&me, 1000), Cents(1000), equal(&[&me, &bob])),
    ))
    .unwrap();

    let summaries = app.groups();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].name, "Trip");
    assert_eq!(summaries[0].member_count, 2);
    assert_eq!(summaries[0].my_net, Cents(500));

    let detail = app.group_detail(&group).unwrap();
    let bob_name = &detail.members.iter().find(|m| m.user == bob).unwrap().name;
    assert_eq!(bob_name, "Bob");
}

#[test]
fn weighted_split_through_the_app() {
    let mut app = new_app();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Trip", std::slice::from_ref(&bob))
        .unwrap();

    let mut weights = BTreeMap::new();
    weights.insert(me.clone(), 3u64);
    weights.insert(bob.clone(), 1u64);
    app.add_expense(group_expense(
        &group,
        fields(
            "Dinner",
            paid(&me, 1000),
            Cents(1000),
            SplitPlan::Weighted { weights },
        ),
    ))
    .unwrap();

    // I owe 3/4 (750), paid 1000 → net +250.
    let detail = app.group_detail(&group).unwrap();
    let my_net = detail.members.iter().find(|m| m.user == me).unwrap().net;
    assert_eq!(my_net, Cents(250));
}

#[test]
fn unbalanced_expense_is_rejected() {
    let mut app = new_app();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Trip", std::slice::from_ref(&bob))
        .unwrap();

    // Payments (999) don't equal the total (1000).
    let err = app
        .add_expense(group_expense(
            &group,
            fields("Oops", paid(&me, 999), Cents(1000), equal(&[&me, &bob])),
        ))
        .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
    // Nothing was recorded.
    assert!(app.group_detail(&group).unwrap().expenses.is_empty());
}

#[test]
fn non_member_cannot_add_expense_and_state_persists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ops.redb");

    // First identity creates a group, then we close the app.
    let group = {
        let mut owner = App::new(
            Identity::from_seed([1u8; 32]),
            RedbOpStore::open(&path).unwrap(),
            SiteId(1),
        )
        .unwrap();
        owner.create_group("Trip", &[]).unwrap()
    };

    // A different identity reopens the same store: it can *see* the group
    // (persistence works) but is not a member (authorization works).
    let mut outsider = App::new(
        Identity::from_seed([2u8; 32]),
        RedbOpStore::open(&path).unwrap(),
        SiteId(2),
    )
    .unwrap();
    assert!(outsider.group_detail(&group).is_some(), "state persisted");

    let me = outsider.me().clone();
    let err = outsider
        .add_expense(group_expense(
            &group,
            fields("x", paid(&me, 1000), Cents(1000), equal(&[&me])),
        ))
        .unwrap_err();
    assert!(matches!(err, AppError::NotAuthorized(_)));
}

#[test]
fn locked_expense_cannot_be_edited_or_deleted_until_unlocked() {
    let mut app = new_app();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Trip", std::slice::from_ref(&bob))
        .unwrap();
    let expense = app
        .add_expense(group_expense(
            &group,
            fields("Hotel", paid(&me, 1000), Cents(1000), equal(&[&me, &bob])),
        ))
        .unwrap();

    app.lock_expense(&expense).unwrap();
    assert!(app.group_detail(&group).unwrap().expenses[0].locked);

    let edit = app.edit_expense(
        &expense,
        fields("Hotel!", paid(&me, 1000), Cents(1000), equal(&[&me, &bob])),
    );
    assert!(matches!(edit, Err(AppError::Validation(_))));
    assert!(matches!(
        app.delete_expense(&expense),
        Err(AppError::Validation(_))
    ));

    // Unlock → edit succeeds.
    app.unlock_expense(&expense).unwrap();
    app.edit_expense(
        &expense,
        fields("Hotel!", paid(&me, 1000), Cents(1000), equal(&[&me, &bob])),
    )
    .unwrap();
    assert_eq!(
        app.group_detail(&group).unwrap().expenses[0].description,
        "Hotel!"
    );
}

#[test]
fn draft_expense_is_excluded_then_counts_after_publish() {
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Trip", std::slice::from_ref(&bob))
        .unwrap();

    let expense = app
        .add_expense(ExpenseDraft {
            fields: fields(
                "Maybe dinner",
                paid(&me, 4000),
                Cents(4000),
                equal(&[&me, &bob]),
            ),
            group: Some(group.clone()),
            draft: true,
        })
        .unwrap();

    // The draft shows in the ledger but doesn't move balances.
    let detail = app.group_detail(&group).unwrap();
    assert_eq!(detail.expenses.len(), 1);
    assert!(!detail.expenses[0].published);
    assert!(detail.members.iter().all(|m| m.net == Cents(0)));
    assert!(detail.settle_up.is_empty());

    // Publishing makes it count.
    app.publish_expense(&expense).unwrap();
    let detail = app.group_detail(&group).unwrap();
    assert!(detail.expenses[0].published);
    let my_net = detail.members.iter().find(|m| m.user == me).unwrap().net;
    assert_eq!(my_net, Cents(2000));
}

#[test]
fn set_my_name_publishes_my_agreement_key() {
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let published = app.projection().users[&me].agreement_pub;
    assert_eq!(
        published,
        app.my_agreement_public(),
        "the local user's X25519 key is published and converges (#6)"
    );
}

#[test]
fn group_currency_and_foreign_expense_metadata() {
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Iceland", std::slice::from_ref(&bob))
        .unwrap();
    app.set_group_currency(&group, "EUR").unwrap();
    assert_eq!(app.group_detail(&group).unwrap().currency, "EUR");

    // A €30.00 expense entered originally as $32.40 (1 USD = 0.925926 EUR).
    // The UI converts; here we store the base (EUR) amounts + the original.
    let original = OriginalAmount {
        currency: "USD".into(),
        amount: Cents(3240),
        rate_micro: 925_926,
    };
    let mut f = fields("Hotel", paid(&me, 3000), Cents(3000), equal(&[&me, &bob]));
    f.original = Some(original.clone());
    app.add_expense(group_expense(&group, f)).unwrap();

    let view = app.group_detail(&group).unwrap().expenses.remove(0);
    assert_eq!(view.currency, "EUR");
    assert_eq!(view.total, Cents(3000));
    assert_eq!(view.original, Some(original));
}

#[test]
fn non_group_expense_shows_in_friend_detail_and_overall_net() {
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();

    // No group: I pay 20.00, split equally with Bob.
    app.add_expense(ExpenseDraft {
        fields: fields(
            "Coffee runs",
            paid(&me, 2000),
            Cents(2000),
            equal(&[&me, &bob]),
        ),
        group: None,
        draft: false,
    })
    .unwrap();

    // Overall net + pairwise friend balance both reflect it.
    assert_eq!(app.overall_net(), Cents(1000));
    let bob_friend = app.friends().into_iter().find(|f| f.user == bob).unwrap();
    assert_eq!(bob_friend.net, Cents(1000));

    // It surfaces in the friend detail with no group name, and there are no groups.
    let detail = app.friend_detail(&bob).unwrap();
    assert_eq!(detail.shared.len(), 1);
    assert_eq!(detail.shared[0].group, None);
    assert!(app.groups().is_empty());

    // A non-group settlement clears the balance.
    app.record_non_group_settlement(&bob, &me, Cents(1000))
        .unwrap();
    assert_eq!(app.overall_net(), Cents(0));
}

#[test]
fn expense_in_a_closed_period_cannot_be_edited_or_deleted() {
    let mut app = new_app();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Trip", std::slice::from_ref(&bob))
        .unwrap();

    // An expense dated at t=100.
    let dated = |description: &str| {
        let mut f = fields(
            description,
            paid(&me, 1000),
            Cents(1000),
            equal(&[&me, &bob]),
        );
        f.date_ms = 100;
        f
    };
    let expense = app
        .add_expense(group_expense(&group, dated("Old dinner")))
        .unwrap();

    // Close the period up to t=200: the expense is now frozen.
    app.set_closed_period(&group, 200).unwrap();
    let edit = app.edit_expense(&expense, dated("Renamed"));
    assert!(matches!(edit, Err(AppError::Validation(_))));
    assert!(matches!(
        app.delete_expense(&expense),
        Err(AppError::Validation(_))
    ));

    // Reopening the period (lower watermark) re-enables edits.
    app.set_closed_period(&group, 0).unwrap();
    app.edit_expense(&expense, dated("Renamed")).unwrap();
    assert_eq!(
        app.group_detail(&group).unwrap().expenses[0].description,
        "Renamed"
    );
}

#[test]
fn root_lock_gates_privileged_actions_but_not_daily_ops() {
    // A full open starts unlocked and self-enrols this device.
    let mut app = new_app();
    assert!(app.root_unlocked());

    // Daily, device-signed ops keep working once the root is sealed (#34).
    app.lock_root();
    assert!(!app.root_unlocked());
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    app.add_person("Bob").unwrap();
    assert_eq!(app.my_name().as_deref(), Some("Me"));

    // Privileged device certs need the root: locked -> RootLocked.
    assert!(matches!(
        app.authorize_device([9u8; 32], 2),
        Err(AppError::RootLocked)
    ));
    assert!(matches!(
        app.revoke_device([9u8; 32]),
        Err(AppError::RootLocked)
    ));

    // A wrong seed is rejected; the matching seed re-enables privileged ops.
    assert!(app.unlock_root([2u8; 32]).is_err());
    assert!(!app.root_unlocked());
    app.unlock_root([1u8; 32]).unwrap();
    assert!(app.root_unlocked());
    app.authorize_device([9u8; 32], 2).unwrap();

    // The local user id is unchanged across lock/unlock.
    assert_eq!(app.me(), &me);
}

#[test]
fn claiming_a_placeholder_preserves_balances_with_zero_change() {
    // I create a group and a guest "Bob", and an expense I paid split with Bob.
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Trip", std::slice::from_ref(&bob))
        .unwrap();
    app.add_expense(group_expense(
        &group,
        fields("Hotel", paid(&me, 3000), Cents(3000), equal(&[&me, &bob])),
    ))
    .unwrap();

    // Bob owes me 15.00; my overall net is +15.00.
    assert_eq!(
        app.friends()
            .into_iter()
            .find(|f| f.user == bob)
            .map(|f| f.net),
        Some(Cents(1500))
    );
    let net_before = app.overall_net();
    assert_eq!(net_before, Cents(1500));

    // A second placeholder for the same person, merged into Bob. The merge only
    // redirects ids, so my overall balance is unchanged and the two placeholder
    // ids collapse to a single friend balance (which id survives is the
    // deterministic canonical, #8).
    let bob_dup = app.add_person("Bobby").unwrap();
    app.merge_people(&bob_dup, &bob).unwrap();
    assert_eq!(app.overall_net(), net_before, "merge preserves my balance");
    let with_balance: Vec<_> = app
        .friends()
        .into_iter()
        .filter(|f| (f.user == bob || f.user == bob_dup) && f.net != Cents(0))
        .collect();
    assert_eq!(with_balance.len(), 1, "the two ids merge into one balance");
    assert_eq!(with_balance[0].net, Cents(1500));
}

#[test]
fn settlement_between_two_aliased_ids_is_rejected() {
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let bob_dup = app.add_person("Bobby").unwrap();
    let group = app
        .create_group("Trip", &[bob.clone(), bob_dup.clone()])
        .unwrap();

    // Before merging, a settlement between the two distinct ids is allowed.
    app.record_settlement(&group, &bob, &bob_dup, Cents(100))
        .unwrap();

    // After merging them into one person, a payment between the two ids is a
    // self-settlement and must be rejected even though the raw ids differ.
    app.merge_people(&bob_dup, &bob).unwrap();
    let err = app.record_settlement(&group, &bob, &bob_dup, Cents(100));
    assert!(
        err.is_err(),
        "a self-settlement across aliases must be rejected"
    );
    // A genuine cross-person settlement still works.
    app.record_settlement(&group, &me, &bob, Cents(100))
        .unwrap();
}

#[test]
fn cannot_alias_yourself() {
    let mut app = new_app();
    let me = app.me().clone();
    assert!(matches!(
        app.claim_person(&me),
        Err(AppError::Validation(_))
    ));
}

#[test]
fn mutual_friendship_enables_a_cross_identity_friend_expense() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ops.redb");
    let (alice_seed, bob_seed) = ([1u8; 32], [2u8; 32]);
    let alice_id = user_id_for(&SigningKey::from_seed(alice_seed).public());
    let bob_id = user_id_for(&SigningKey::from_seed(bob_seed).public());

    // Alice declares friendship toward Bob — one-sided, not yet confirmed.
    {
        let mut alice = App::new(
            Identity::from_seed(alice_seed),
            RedbOpStore::open(&path).unwrap(),
            SiteId(1),
        )
        .unwrap();
        alice.add_friend(&bob_id).unwrap();
        assert!(
            alice.confirmed_friends().is_empty(),
            "one-sided is not confirmed"
        );
    }

    // Bob reciprocates and records a friend expense he paid, split with Alice.
    {
        let mut bob = App::new(
            Identity::from_seed(bob_seed),
            RedbOpStore::open(&path).unwrap(),
            SiteId(2),
        )
        .unwrap();
        bob.add_friend(&alice_id).unwrap();
        assert_eq!(
            bob.confirmed_friends(),
            vec![alice_id.clone()],
            "now mutual"
        );

        let mut pay = BTreeMap::new();
        pay.insert(bob_id.clone(), Cents(1000));
        bob.add_expense(ExpenseDraft {
            fields: ExpenseFieldsInput {
                description: "Dinner".into(),
                paid_by: pay,
                total: Cents(1000),
                split: equal(&[&bob_id, &alice_id]),
                category: "food".into(),
                notes: None,
                date_ms: 0,
                original: None,
            },
            group: None,
            draft: false,
        })
        .unwrap();
        assert_eq!(bob.overall_net(), Cents(500), "Bob is owed 5.00");
    }

    // Reopening as Alice: the friendship + the friend expense both persisted.
    let alice = App::new(
        Identity::from_seed(alice_seed),
        RedbOpStore::open(&path).unwrap(),
        SiteId(1),
    )
    .unwrap();
    assert_eq!(alice.confirmed_friends(), vec![bob_id.clone()]);
    assert_eq!(alice.overall_net(), Cents(-500), "Alice owes 5.00");
}

#[test]
fn cannot_befriend_yourself() {
    let mut app = new_app();
    let me = app.me().clone();
    assert!(matches!(app.add_friend(&me), Err(AppError::Validation(_))));
}

#[test]
fn create_invite_needs_the_root_unlocked() {
    let mut app = new_app();
    // Unlocked at open: a friend invite is signed and verifies.
    let invite = app.create_invite("friend".into(), 1_000).unwrap();
    assert!(invite.verify());

    // Sealed root: creating an invite is gated.
    app.lock_root();
    assert!(matches!(
        app.create_invite("friend".into(), 1_000),
        Err(AppError::RootLocked)
    ));
}
