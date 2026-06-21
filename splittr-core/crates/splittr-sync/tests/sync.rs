//! The async [`sync`] driver over the in-memory [`Transport`]: two replicas
//! converge in one bounded exchange, and edits made while partitioned propagate
//! on the next reconnect.

mod common;

use common::{duplex, key, op, replica, world};
use splittr_crdt::{OpKind, UserId};
use splittr_sync::{sync, Role, SyncStore};

#[tokio::test]
async fn two_replicas_converge_over_a_transport() {
    let (fa, fb) = (key(11), key(12));
    let mut a = replica();
    let mut b = replica();
    for o in world(&fa, "a") {
        a.append(&o).unwrap();
    }
    for o in world(&fb, "b") {
        b.append(&o).unwrap();
    }

    let (mut ta, mut tb) = duplex();
    let (ra, rb) = tokio::join!(
        sync(&mut a, &mut ta, Role::Initiator),
        sync(&mut b, &mut tb, Role::Responder),
    );
    ra.unwrap();
    rb.unwrap();

    assert_eq!(a.projection(), b.projection(), "converged");
    assert_eq!(a.op_ids().len(), b.op_ids().len());
    assert_eq!(a.projection().groups.len(), 2);
}

#[tokio::test]
async fn edits_while_partitioned_propagate_on_reconnect() {
    let f = key(13);
    let mut a = replica();
    let mut b = replica();
    for o in world(&f, "a") {
        a.append(&o).unwrap();
    }

    // First reconnect: B catches up to A.
    {
        let (mut ta, mut tb) = duplex();
        let (ra, rb) = tokio::join!(
            sync(&mut a, &mut ta, Role::Initiator),
            sync(&mut b, &mut tb, Role::Responder),
        );
        ra.unwrap();
        rb.unwrap();
    }
    assert_eq!(a.projection(), b.projection());

    // A edits while partitioned (a new profile op).
    let edit = op(
        &f,
        50,
        OpKind::UpsertProfile {
            user: UserId::from("user:guest-a"),
            name: "Renamed".into(),
        },
    );
    a.append(&edit).unwrap();
    assert_ne!(a.projection(), b.projection(), "diverged again");

    // Second reconnect: the edit reaches B.
    {
        let (mut ta, mut tb) = duplex();
        // Roles swapped — either side may initiate.
        let (rb, ra) = tokio::join!(
            sync(&mut b, &mut tb, Role::Initiator),
            sync(&mut a, &mut ta, Role::Responder),
        );
        ra.unwrap();
        rb.unwrap();
    }
    assert!(b.op_ids().contains(&edit.id), "the offline edit synced");
    assert_eq!(a.projection(), b.projection());
}
