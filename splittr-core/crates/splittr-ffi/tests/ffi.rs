//! Tests for the FFI facade: the engine drives the full flow through the same
//! string/int DTOs Dart will use, and persists across reopen.

use splittr_ffi::*;

fn seed() -> Vec<u8> {
    vec![1u8; 32]
}

fn db_key() -> Vec<u8> {
    vec![2u8; 32]
}

fn device_seed() -> Vec<u8> {
    vec![3u8; 32]
}

fn db_path(dir: &tempfile::TempDir) -> String {
    dir.path().join("engine.redb").to_str().unwrap().to_string()
}

#[test]
fn engine_runs_the_core_flow() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(db_path(&dir), seed(), device_seed(), db_key(), 1).unwrap();

    engine.set_my_name("Me".into()).unwrap();
    let me = engine.my_user_id();
    let bob = engine.add_person("Bob".into()).unwrap();
    let group = engine
        .create_group("Trip".into(), vec![bob.clone()], "USD".into())
        .unwrap();

    engine
        .add_expense(ExpenseInput {
            group_id: Some(group.clone()),
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
            draft: false,
            original: None,
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
fn draft_publish_and_closed_period_through_the_ffi() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(db_path(&dir), seed(), device_seed(), db_key(), 1).unwrap();
    engine.set_my_name("Me".into()).unwrap();
    let me = engine.my_user_id();
    let bob = engine.add_person("Bob".into()).unwrap();
    let group = engine
        .create_group("Trip".into(), vec![bob.clone()], "USD".into())
        .unwrap();

    let input = |draft: bool, date_ms: i64| ExpenseInput {
        group_id: Some(group.clone()),
        description: "Dinner".into(),
        paid_by: vec![Payer {
            user_id: me.clone(),
            cents: 2000,
        }],
        total_cents: 2000,
        split: SplitPlanDto::Equal {
            participants: vec![me.clone(), bob.clone()],
        },
        category: "food".into(),
        notes: None,
        date_ms,
        draft,
        original: None,
    };

    // A draft is in the ledger but doesn't move balances.
    let expense = engine.add_expense(input(true, 100)).unwrap();
    let detail = engine.group_detail(group.clone()).unwrap();
    assert!(!detail.expenses[0].published);
    assert!(detail.settle_up.is_empty());

    // Publishing makes it count.
    engine.publish_expense(expense.clone()).unwrap();
    let detail = engine.group_detail(group.clone()).unwrap();
    assert!(detail.expenses[0].published);
    assert_eq!(detail.settle_up.len(), 1);

    // Closing the period freezes it against edits.
    engine.set_closed_period(group.clone(), 200).unwrap();
    assert!(engine
        .edit_expense(expense.clone(), input(false, 100))
        .is_err());

    // Reopening lets edits through again.
    engine.set_closed_period(group.clone(), 0).unwrap();
    assert!(engine.edit_expense(expense, input(false, 100)).is_ok());
}

#[test]
fn device_authorize_and_revoke_through_the_ffi() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(db_path(&dir), seed(), device_seed(), db_key(), 1).unwrap();
    engine.set_my_name("Me".into()).unwrap();

    // This (primary) device is self-authorized on open.
    let devices = engine.list_devices();
    assert_eq!(devices.len(), 1);
    assert!(devices[0].this_device);
    assert_eq!(devices[0].device, engine.my_device_public());
    assert!(!devices[0].revoked);

    // Enrol a second device (just its public key here; the handshake is #35).
    let second = "11".repeat(32);
    engine.authorize_device(second.clone(), 2).unwrap();
    let devices = engine.list_devices();
    assert_eq!(devices.len(), 2);
    assert!(devices.iter().any(|d| d.device == second && !d.revoked));

    // Revoke it.
    engine.revoke_device(second.clone()).unwrap();
    assert!(engine
        .list_devices()
        .iter()
        .any(|d| d.device == second && d.revoked));

    // A bad hex key is rejected.
    assert!(engine.authorize_device("nothex".into(), 3).is_err());
}

#[test]
fn device_only_reopen_locks_the_root_until_unlocked() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(&dir);

    // First (full) run: self-enrol this device and record the identity pubkey.
    let identity_public = {
        let engine = Engine::open(path.clone(), seed(), device_seed(), db_key(), 1).unwrap();
        engine.set_my_name("Me".into()).unwrap();
        engine.identity_public()
    };

    // Daily run: open device-only (root sealed). Device-signed ops still work.
    let engine = Engine::open_device_only(
        path,
        parse_hex(&identity_public),
        device_seed(),
        db_key(),
        1,
    )
    .unwrap();
    assert!(!engine.is_root_unlocked());
    assert_eq!(engine.identity_public(), identity_public);
    engine.set_my_name("Me Again".into()).unwrap();
    assert_eq!(engine.my_name().as_deref(), Some("Me Again"));
    engine.add_person("Bob".into()).unwrap();
    assert_eq!(engine.list_devices().len(), 1); // still enrolled

    // Privileged device certs need the root, which is locked.
    let second = "11".repeat(32);
    assert!(engine.authorize_device(second.clone(), 2).is_err());

    // A foreign seed is rejected; the real identity seed unlocks the root.
    assert!(engine.unlock_root(vec![9u8; 32]).is_err());
    assert!(!engine.is_root_unlocked());
    engine.unlock_root(seed()).unwrap();
    assert!(engine.is_root_unlocked());
    engine.authorize_device(second.clone(), 2).unwrap();
    assert!(engine.list_devices().iter().any(|d| d.device == second));

    // Re-seal: privileged actions are gated again.
    engine.lock_root();
    assert!(!engine.is_root_unlocked());
    assert!(engine.revoke_device(second).is_err());
}

#[test]
fn root_seed_seals_and_opens_under_a_passphrase() {
    let blob = seal_root_seed("1234".into(), seed()).unwrap();
    assert_ne!(blob, seed()); // not stored in the clear
    assert_eq!(open_root_seed("1234".into(), blob.clone()), Some(seed()));
    assert_eq!(open_root_seed("0000".into(), blob), None); // wrong PIN
}

#[test]
fn pairing_short_auth_string_matches_and_detects_tampering() {
    let identity = "aa".repeat(32);
    let primary = "bb".repeat(32);
    let new_device = "cc".repeat(32);
    let challenge = vec![7u8; 32];

    let sas = pairing_short_auth_string(
        identity.clone(),
        primary.clone(),
        new_device.clone(),
        challenge.clone(),
    )
    .unwrap();
    // Recomputing from the same transcript agrees (both devices compare this).
    assert_eq!(
        sas,
        pairing_short_auth_string(
            identity.clone(),
            primary.clone(),
            new_device,
            challenge.clone()
        )
        .unwrap()
    );
    // A MITM-substituted device key changes the string.
    let tampered =
        pairing_short_auth_string(identity, primary, "dd".repeat(32), challenge).unwrap();
    assert_ne!(sas, tampered);
}

fn parse_hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

#[test]
fn rejects_a_malformed_seed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Engine::open(db_path(&dir), vec![1, 2, 3], device_seed(), db_key(), 1).is_err());
}

#[test]
fn state_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(&dir);

    let group = {
        let engine = Engine::open(path.clone(), seed(), device_seed(), db_key(), 1).unwrap();
        engine
            .create_group("Trip".into(), vec![], "USD".into())
            .unwrap()
    };

    let reopened = Engine::open(path, seed(), device_seed(), db_key(), 1).unwrap();
    assert!(reopened.group_detail(group).is_some());
}
