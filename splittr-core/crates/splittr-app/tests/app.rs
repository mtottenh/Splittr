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
    app.add_expense(
        &group,
        "Hotel",
        paid(&me, 3000),
        Cents(3000),
        SplitPlan::Equal {
            participants: vec![me.clone(), bob.clone()],
        },
        "travel",
        None,
        0,
    )
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
fn group_summary_shows_my_balance_and_names() {
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();
    let group = app
        .create_group("Trip", std::slice::from_ref(&bob))
        .unwrap();
    app.add_expense(
        &group,
        "Lunch",
        paid(&me, 1000),
        Cents(1000),
        SplitPlan::Equal {
            participants: vec![me.clone(), bob.clone()],
        },
        "food",
        None,
        0,
    )
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
    app.add_expense(
        &group,
        "Dinner",
        paid(&me, 1000),
        Cents(1000),
        SplitPlan::Weighted { weights },
        "food",
        None,
        0,
    )
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
        .add_expense(
            &group,
            "Oops",
            paid(&me, 999),
            Cents(1000),
            SplitPlan::Equal {
                participants: vec![me, bob],
            },
            "general",
            None,
            0,
        )
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
        .add_expense(
            &group,
            "x",
            paid(&me, 1000),
            Cents(1000),
            SplitPlan::Equal {
                participants: vec![me],
            },
            "general",
            None,
            0,
        )
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
        .add_expense(
            &group,
            "Hotel",
            paid(&me, 1000),
            Cents(1000),
            SplitPlan::Equal {
                participants: vec![me.clone(), bob.clone()],
            },
            "travel",
            None,
            0,
        )
        .unwrap();

    app.lock_expense(&expense).unwrap();
    assert!(app.group_detail(&group).unwrap().expenses[0].locked);

    let edit = app.edit_expense(
        &expense,
        "Hotel!",
        paid(&me, 1000),
        Cents(1000),
        SplitPlan::Equal {
            participants: vec![me.clone(), bob.clone()],
        },
        "travel",
        None,
        0,
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
        "Hotel!",
        paid(&me, 1000),
        Cents(1000),
        SplitPlan::Equal {
            participants: vec![me, bob],
        },
        "travel",
        None,
        0,
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
        .add_draft_expense(
            &group,
            "Maybe dinner",
            paid(&me, 4000),
            Cents(4000),
            SplitPlan::Equal {
                participants: vec![me.clone(), bob.clone()],
            },
            "food",
            None,
            0,
        )
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
fn non_group_expense_shows_in_friend_detail_and_overall_net() {
    let mut app = new_app();
    app.set_my_name("Me").unwrap();
    let me = app.me().clone();
    let bob = app.add_person("Bob").unwrap();

    // No group: I pay 20.00, split equally with Bob.
    app.add_non_group_expense(
        "Coffee runs",
        paid(&me, 2000),
        Cents(2000),
        SplitPlan::Equal {
            participants: vec![me.clone(), bob.clone()],
        },
        "food",
        None,
        0,
        false,
    )
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
    let expense = app
        .add_expense(
            &group,
            "Old dinner",
            paid(&me, 1000),
            Cents(1000),
            SplitPlan::Equal {
                participants: vec![me.clone(), bob.clone()],
            },
            "food",
            None,
            100,
        )
        .unwrap();

    // Close the period up to t=200: the expense is now frozen.
    app.set_closed_period(&group, 200).unwrap();
    let edit = app.edit_expense(
        &expense,
        "Renamed",
        paid(&me, 1000),
        Cents(1000),
        SplitPlan::Equal {
            participants: vec![me.clone(), bob.clone()],
        },
        "food",
        None,
        100,
    );
    assert!(matches!(edit, Err(AppError::Validation(_))));
    assert!(matches!(
        app.delete_expense(&expense),
        Err(AppError::Validation(_))
    ));

    // Reopening the period (lower watermark) re-enables edits.
    app.set_closed_period(&group, 0).unwrap();
    app.edit_expense(
        &expense,
        "Renamed",
        paid(&me, 1000),
        Cents(1000),
        SplitPlan::Equal {
            participants: vec![me, bob],
        },
        "food",
        None,
        100,
    )
    .unwrap();
    assert_eq!(
        app.group_detail(&group).unwrap().expenses[0].description,
        "Renamed"
    );
}
