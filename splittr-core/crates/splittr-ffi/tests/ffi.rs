//! Tests for the FFI facade: the engine drives the full flow through the same
//! string/int DTOs Dart will use, and persists across reopen.

use splittr_ffi::*;

fn seed() -> Vec<u8> {
    vec![1u8; 32]
}

fn db_key() -> Vec<u8> {
    vec![2u8; 32]
}

fn db_path(dir: &tempfile::TempDir) -> String {
    dir.path().join("engine.redb").to_str().unwrap().to_string()
}

#[test]
fn engine_runs_the_core_flow() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(db_path(&dir), seed(), db_key(), 1).unwrap();

    engine.set_my_name("Me".into()).unwrap();
    let me = engine.my_user_id();
    let bob = engine.add_person("Bob".into()).unwrap();
    let group = engine
        .create_group("Trip".into(), vec![bob.clone()])
        .unwrap();

    engine
        .add_expense(ExpenseInput {
            group_id: group.clone(),
            description: "Hotel".into(),
            paid_by: vec![Payer {
                user_id: me.clone(),
                cents: 3000,
            }],
            total_cents: 3000,
            split: SplitPlanDto::Equal {
                participants: vec![me.clone(), bob.clone()],
            },
            category: "travel".into(),
            notes: None,
            date_ms: 0,
        })
        .unwrap();

    let detail = engine.group_detail(group.clone()).unwrap();
    let me_net = detail
        .members
        .iter()
        .find(|m| m.user_id == me)
        .unwrap()
        .net_cents;
    assert_eq!(me_net, 1500);
    assert_eq!(detail.expenses.len(), 1);
    assert_eq!(detail.settle_up.len(), 1);
    assert_eq!(detail.settle_up[0].amount_cents, 1500);

    engine
        .record_settlement(group.clone(), bob, me, 1500)
        .unwrap();
    assert!(engine.group_detail(group).unwrap().settle_up.is_empty());

    assert_eq!(engine.groups().len(), 1);
}

#[test]
fn rejects_a_malformed_seed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Engine::open(db_path(&dir), vec![1, 2, 3], db_key(), 1).is_err());
}

#[test]
fn state_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(&dir);

    let group = {
        let engine = Engine::open(path.clone(), seed(), db_key(), 1).unwrap();
        engine.create_group("Trip".into(), vec![]).unwrap()
    };

    let reopened = Engine::open(path, seed(), db_key(), 1).unwrap();
    assert!(reopened.group_detail(group).is_some());
}
