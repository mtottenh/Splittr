//! Pure (transport-free) reconciliation: the [`SyncSession`] protocol drives two
//! replicas to an identical op-set and therefore identical balances, transfers
//! only the delta, is idempotent, and rejects forged ops at ingest.

mod common;

use common::{converge, key, op, replica, world, Replica};
use splittr_crdt::{Op, OpKind, UserId};
use splittr_store::OpStore;
use splittr_sync::{missing_ops, SyncStore};

fn ids(r: &Replica) -> std::collections::BTreeSet<splittr_crdt::OpId> {
    r.op_ids().into_iter().collect()
}

#[test]
fn divergent_replicas_converge_to_identical_balances() {
    let (fa, fb) = (key(1), key(2));
    let mut a = replica();
    let mut b = replica();
    for o in world(&fa, "a") {
        a.append(&o).unwrap();
    }
    for o in world(&fb, "b") {
        b.append(&o).unwrap();
    }
    // A shared op both already hold — must not be re-sent or duplicated.
    let shared = op(
        &fa,
        9,
        OpKind::UpsertProfile {
            user: UserId::from("user:guest-a"),
            name: "Guest".into(),
        },
    );
    a.append(&shared).unwrap();
    b.append(&shared).unwrap();

    assert_ne!(a.projection(), b.projection(), "diverged before sync");
    converge(&mut a, &mut b);

    assert_eq!(ids(&a), ids(&b), "op-sets are unified");
    assert_eq!(
        a.projection(),
        b.projection(),
        "same op-set folds to identical state (ADR-0001)"
    );
    // Both founders' groups survive the union.
    assert_eq!(a.projection().groups.len(), 2);
}

#[test]
fn only_the_missing_ops_cross_the_wire() {
    let f = key(3);
    let mut a = replica();
    let mut b = replica();
    let ops = world(&f, "x");
    // A holds all; B holds only the first.
    for o in &ops {
        a.append(o).unwrap();
    }
    b.append(&ops[0]).unwrap();

    // A would send B exactly the two ops B lacks — not the shared one.
    let delta = missing_ops(&a, &b.op_ids());
    assert_eq!(delta.len(), ops.len() - 1);
    assert!(delta.iter().all(|o| o.id != ops[0].id));
    // B has nothing A lacks.
    assert!(missing_ops(&b, &a.op_ids()).is_empty());
}

#[test]
fn re_syncing_is_idempotent() {
    let (fa, fb) = (key(4), key(5));
    let mut a = replica();
    let mut b = replica();
    for o in world(&fa, "a") {
        a.append(&o).unwrap();
    }
    for o in world(&fb, "b") {
        b.append(&o).unwrap();
    }
    converge(&mut a, &mut b);
    let (pa, n) = (a.projection(), a.op_ids().len());
    // A second pass changes nothing and transfers nothing.
    converge(&mut a, &mut b);
    assert_eq!(a.projection(), pa);
    assert_eq!(a.op_ids().len(), n);
    assert!(missing_ops(&a, &b.op_ids()).is_empty());
    assert!(missing_ops(&b, &a.op_ids()).is_empty());
}

#[test]
fn forged_ops_are_rejected_at_the_trust_boundary() {
    let f = key(6);
    let mut a = replica();
    let mut b = replica();
    let ops = world(&f, "a");
    for o in &ops {
        a.append(o).unwrap();
    }
    // A malicious peer's log holds a tampered op: a validly-signed op whose
    // content was mutated after signing (so its id no longer matches its bytes),
    // under a novel id A doesn't already have.
    let mut forged: Op = op(
        &key(7),
        77,
        OpKind::CreateGroup {
            group: splittr_crdt::GroupId::from("evil"),
            name: "Legit".into(),
        },
    );
    forged.kind = OpKind::CreateGroup {
        group: splittr_crdt::GroupId::from("evil"),
        name: "Hacked".into(),
    };
    b.store_mut().append(&forged).unwrap(); // bypasses verification

    converge(&mut a, &mut b);
    // A ingests via the trust boundary, so the forged op (id no longer matches its
    // content) is rejected and never reaches A.
    assert!(!a.op_ids().contains(&forged.id), "forged op rejected");
}
