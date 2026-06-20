//! Targeted, example-based tests for each ADR-0001 conflict rule. These pin the
//! exact semantics that the property tests exercise more broadly.

use splittr_crdt::*;
use splittr_domain::{split_equal, Cents, ExpenseId, GroupId, Split, UserId};

fn op(counter: u32, kind: OpKind) -> Op {
    Op::new(
        Hlc {
            wall_ms: counter as u64,
            counter,
            site: SiteId(0),
        },
        ActorId("test".into()),
        kind,
    )
}

fn split_two(total: i64, a: &str, b: &str) -> Vec<Split> {
    split_equal(Cents(total), &[UserId::from(a), UserId::from(b)])
}

#[test]
fn delete_wins_and_is_terminal_regardless_of_order_or_hlc() {
    let create = OpKind::CreateExpense {
        expense: ExpenseId::from("e0"),
        group: GroupId::from("g0"),
        payer: UserId::from("a"),
        total: Cents(1000),
        splits: split_two(1000, "a", "b"),
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
            group: GroupId::from("g0"),
            payer: UserId::from("a"),
            total: Cents(1000),
            splits: split_two(1000, "a", "b"),
        },
    );
    let higher = op(
        5,
        OpKind::EditExpense {
            expense: ExpenseId::from("e0"),
            payer: UserId::from("a"),
            total: Cents(2000),
            splits: split_two(2000, "a", "b"),
        },
    );
    let lower = op(
        0,
        OpKind::EditExpense {
            expense: ExpenseId::from("e0"),
            payer: UserId::from("a"),
            total: Cents(9999),
            splits: split_two(9999, "a", "b"),
        },
    );

    let with_higher = project(&[create.clone(), higher]);
    assert_eq!(
        with_higher.expenses[&ExpenseId::from("e0")].total,
        Cents(2000),
        "highest-HLC version wins"
    );

    let with_lower = project(&[create, lower]);
    assert_eq!(
        with_lower.expenses[&ExpenseId::from("e0")].total,
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
        group: GroupId::from("g0"),
        payer: UserId::from("a"),
        total: Cents(1000),
        splits: split_two(1000, "a", "b"),
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
            settlement: splittr_domain::SettlementId::from("s0"),
            group: GroupId::from("g0"),
            from: UserId::from("b"),
            to: UserId::from("a"),
            amount: Cents(500),
        },
    );
    let void = op(
        1,
        OpKind::VoidSettlement {
            settlement: splittr_domain::SettlementId::from("s0"),
        },
    );
    let p = project(&[record, void]);
    assert!(p.settlements.is_empty());
    assert!(net_balances(&p).is_empty());
}
